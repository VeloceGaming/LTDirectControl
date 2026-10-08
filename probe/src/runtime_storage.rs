//! Per-user diagnostic files; never depend on the compiler's project folder.
use std::{
    fs::{self, File},
    io::{self, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

fn candidates(local_app_data: Option<&Path>, temporary: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(root) = local_app_data.filter(|root| root.is_absolute()) {
        roots.push(root.join("LTDirectControl"));
    }
    let fallback = temporary.join("LTDirectControl");
    if !roots.contains(&fallback) {
        roots.push(fallback);
    }
    roots
}

fn open_in(roots: &[PathBuf]) -> io::Result<(File, PathBuf)> {
    let mut error = io::Error::other("No diagnostic folder available");
    for root in roots {
        let opened = (|| {
            fs::create_dir_all(root)?;
            let path = root.join("probe.log");
            if path.is_file() {
                let _ = fs::copy(&path, root.join("probe.previous.log"));
            }
            File::create(path)
        })();
        match opened {
            Ok(file) => return Ok((file, root.clone())),
            Err(reason) => error = reason,
        }
    }
    Err(error)
}

pub fn open_log() -> io::Result<(File, PathBuf)> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    open_in(&candidates(local.as_deref(), &std::env::temp_dir()))
}
/// Keep two bounded earlier segments, then continue on the same open handle.
pub fn rotate_log(file: &mut File, directory: &Path) -> io::Result<()> {
    file.flush()?;
    let segment = directory.join("probe.segment.log");
    if segment.is_file() {
        fs::copy(&segment, directory.join("probe.segment.previous.log"))?;
    }
    fs::copy(directory.join("probe.log"), segment)?;
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    Ok(())
}
/// Optional preference; an absent or malformed file preserves quickcast.
pub fn cast_on_release(root: Option<&Path>) -> bool {
    root.and_then(|root| fs::read(root.join("controls.json")).ok())
        .filter(|bytes| bytes.len() <= 4096)
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| value.get("cast_on_release").and_then(|v| v.as_bool()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    #[test]
    fn rolling_log_keeps_late_match_records_and_two_prior_segments() {
        let root = scratch();
        let (file, _) = open_in(std::slice::from_ref(&root)).unwrap();
        let logger = crate::Logger(std::sync::Mutex::new(crate::LogState {
            file: Some(file),
            directory: Some(root.clone()),
            lines: 12_001,
            background_lines: 0,
            bytes: 0,
            rotating: true,
        }));
        logger.write("MATCH 06:48 goal accepted");
        logger.0.lock().unwrap().bytes = crate::LOG_BYTES_LIMIT;
        logger.write("MATCH 07:00 skill committed");
        assert!(fs::read_to_string(root.join("probe.segment.log"))
            .unwrap()
            .contains("06:48"));
        logger.0.lock().unwrap().bytes = crate::LOG_BYTES_LIMIT;
        logger.write("MATCH 10:00 result");
        drop(logger);
        assert!(fs::read_to_string(root.join("probe.segment.previous.log"))
            .unwrap()
            .contains("06:48"));
        assert!(fs::read_to_string(root.join("probe.segment.log"))
            .unwrap()
            .contains("07:00"));
        assert!(fs::read_to_string(root.join("probe.log"))
            .unwrap()
            .contains("10:00"));
        for name in [
            "probe.log",
            "probe.segment.log",
            "probe.segment.previous.log",
        ] {
            fs::remove_file(root.join(name)).unwrap();
        }
        fs::remove_dir(root).unwrap();
    }

    fn scratch() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "lt-storage-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    #[cfg(windows)]
    fn uses_windows_user_folder_without_a_d_drive() {
        let roots = candidates(
            Some(Path::new(r"C:\Users\Player\AppData\Local")),
            Path::new(r"C:\Temp"),
        );
        assert_eq!(
            roots[0],
            Path::new(r"C:\Users\Player\AppData\Local\LTDirectControl")
        );
        assert!(roots
            .iter()
            .all(|root| !root.to_string_lossy().starts_with("D:")));
    }

    #[test]
    fn absent_or_relative_user_folder_uses_temporary_storage() {
        let temp = std::env::temp_dir();
        assert_eq!(candidates(None, &temp), vec![temp.join("LTDirectControl")]);
        assert_eq!(
            candidates(Some(Path::new("relative")), &temp),
            candidates(None, &temp)
        );
    }

    #[test]
    fn inaccessible_primary_folder_falls_back_and_rotates_previous_log() {
        let root = scratch();
        let blocked = root.join("blocked");
        fs::write(&blocked, "not a directory").unwrap();
        let fallback = root.join("fallback");
        let (mut file, selected) = open_in(&[blocked, fallback.clone()]).unwrap();
        assert_eq!(selected, fallback);
        file.write_all(b"prior session").unwrap();
        drop(file);
        let (file, _) = open_in(std::slice::from_ref(&fallback)).unwrap();
        drop(file);
        assert_eq!(
            fs::read(fallback.join("probe.previous.log")).unwrap(),
            b"prior session"
        );
        assert!(fs::read(fallback.join("probe.log")).unwrap().is_empty());
        fs::remove_file(root.join("blocked")).unwrap();
        fs::remove_file(fallback.join("probe.log")).unwrap();
        fs::remove_file(fallback.join("probe.previous.log")).unwrap();
        fs::remove_dir(fallback).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
