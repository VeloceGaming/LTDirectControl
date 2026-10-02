//! Per-user diagnostic files; never depend on the compiler's project folder.
use std::{
    fs::{self, File},
    io,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);

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
