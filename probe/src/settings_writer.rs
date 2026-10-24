//! Writing controls.json off the drawing thread (1.6.0-perf).
//!
//! `Settings::flush` takes the values on the client thread, as before, and
//! hands them to one writer thread, which merges them into the file and
//! replaces it (`save_to`, the same code a synchronous save runs). Only the
//! newest queued values are written (last write wins), and `WRITE` keeps a
//! synchronous save (the forced flush when the Settings window applies or
//! the mod shuts down) and the thread from writing at the same time; a
//! forced flush first waits for the thread to finish, so an older write can
//! never land after it.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Condvar, Mutex, OnceLock};
use std::time::Duration;

/// One save: the values to merge into `root/controls.json`.
pub struct Job {
    pub root: PathBuf,
    pub values: Value,
    pub cursor_size: u32,
    pub low_health: bool,
}
#[derive(Default)]
struct Progress {
    /// Jobs queued and jobs finished (written or skipped).
    queued: u64,
    finished: u64,
    /// Results the client thread has not logged yet.
    results: Vec<Result<(), String>>,
}
static PROGRESS: Mutex<Progress> = Mutex::new(Progress {
    queued: 0,
    finished: 0,
    results: Vec::new(),
});
static IDLE: Condvar = Condvar::new();
/// Held while controls.json is written, by the thread or a forced flush.
static WRITE: Mutex<()> = Mutex::new(());

/// Merge `job` into its controls.json and replace the file (a temporary
/// file, then a rename). Keys other mods or older versions wrote survive.
pub fn save_to(job: &Job) -> std::io::Result<()> {
    let _write = WRITE.lock().unwrap_or_else(|e| e.into_inner());
    merge_and_write(&job.root, &job.values, job.cursor_size, job.low_health)
}
fn merge_and_write(
    root: &Path,
    current: &Value,
    size: u32,
    low_health: bool,
) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    let path = root.join("controls.json");
    let mut old = match std::fs::read(&path) {
        Ok(b) if b.len() <= 1_048_576 => {
            serde_json::from_slice::<Value>(&b).map_err(std::io::Error::other)?
        }
        Ok(_) => return Err(std::io::Error::other("Existing controls.json too large")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e),
    };
    let object = old
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("Existing controls.json is not an object"))?;
    if let Some(current) = current.as_object() {
        for (k, v) in current {
            object.insert(k.clone(), v.clone());
        }
    }
    object.insert("version".into(), json!(1));
    object.insert("cursor_size".into(), json!(size));
    object.insert("low_health_effect".into(), json!(low_health));
    let tmp = root.join("controls.settings.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&old)?)?;
    std::fs::rename(tmp, path)
}

fn sender() -> Option<&'static Mutex<mpsc::Sender<Job>>> {
    static SENDER: OnceLock<Option<Mutex<mpsc::Sender<Job>>>> = OnceLock::new();
    SENDER
        .get_or_init(|| {
            let (tx, rx) = mpsc::channel::<Job>();
            std::thread::Builder::new()
                .name("lt settings writer".into())
                .spawn(move || run(rx))
                .ok()
                .map(|_| Mutex::new(tx))
        })
        .as_ref()
}
fn run(rx: mpsc::Receiver<Job>) {
    while let Ok(mut job) = rx.recv() {
        let mut taken = 1;
        // Last write wins: only the newest of the queued values is written.
        while let Ok(newer) = rx.try_recv() {
            job = newer;
            taken += 1;
        }
        let result = std::panic::catch_unwind(|| save_to(&job))
            .unwrap_or_else(|_| Err(std::io::Error::other("settings writer panicked")))
            .map_err(|e| e.to_string());
        let mut p = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
        p.finished += taken;
        p.results.push(result);
        IDLE.notify_all();
    }
}
/// Queue a save on the writer thread; the job back if the thread is
/// unavailable (the caller then saves synchronously).
pub fn queue(job: Job) -> Result<(), Job> {
    let Some(sender) = sender() else {
        return Err(job);
    };
    let mut p = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(tx) = sender.lock() else {
        return Err(job);
    };
    tx.send(job).map_err(|e| e.0)?;
    p.queued += 1;
    Ok(())
}
/// Wait (at most `limit`) until every queued save has been written.
pub fn settle(limit: Duration) {
    let p = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
    let _ = IDLE.wait_timeout_while(p, limit, |p| p.finished < p.queued);
}
/// Results of finished background saves since the last call, oldest first.
pub fn take_results() -> Vec<Result<(), String>> {
    let mut p = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
    if p.results.is_empty() {
        return Vec::new();
    }
    std::mem::take(&mut p.results)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_saves_land_in_order_and_the_last_one_wins() {
        let root = std::env::temp_dir().join(format!(
            "lt-settings-writer-{}-{}",
            std::process::id(),
            crate::platform_input::thread_id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("controls.json"), br#"{"other_mod":{"keep":1}}"#).unwrap();
        for n in 0..20 {
            assert!(queue(Job {
                root: root.clone(),
                values: json!({ "n": n }),
                cursor_size: 40,
                low_health: n % 2 == 0,
            })
            .is_ok());
        }
        settle(Duration::from_secs(10));
        let saved: Value =
            serde_json::from_slice(&std::fs::read(root.join("controls.json")).unwrap()).unwrap();
        assert_eq!(saved["n"], 19);
        assert_eq!(saved["other_mod"]["keep"], 1);
        assert_eq!(saved["low_health_effect"], false);
        // A synchronous save after the thread finished.
        save_to(&Job {
            root: root.clone(),
            values: json!({ "n": 20 }),
            cursor_size: 41,
            low_health: true,
        })
        .unwrap();
        let saved: Value =
            serde_json::from_slice(&std::fs::read(root.join("controls.json")).unwrap()).unwrap();
        assert_eq!(
            (saved["n"].as_i64(), saved["cursor_size"].as_i64()),
            (Some(20), Some(41))
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
