//! Read files from the game's asset bundle, `bundle.game_data` beside the
//! executable. Layout (read from the file; `tools/read_preview_assets.py`
//! reads it the same way): an entry count, then per entry the extension,
//! the asset path and the size, each length a little-endian u32 before its
//! bytes, then the file's bytes, uncompressed. Only wanted entries are read;
//! the rest are skipped with a seek.
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Entries the bundle may hold, and the largest file read from it.
const MAX_ENTRIES: u32 = 100_000;
const MAX_FILE: u32 = 4_000_000;

/// The installed game's bundle, beside the running executable.
pub fn installed() -> Option<PathBuf> {
    Some(
        std::env::current_exe()
            .ok()?
            .parent()?
            .join("bundle.game_data"),
    )
}

/// Every entry for which `wanted(extension, path)` holds, as
/// (path, bytes), in bundle order. An unreadable or malformed bundle ends
/// the listing early; files over `MAX_FILE` are skipped.
pub fn files(bundle: &Path, mut wanted: impl FnMut(&str, &str) -> bool) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let Ok(mut file) = std::fs::File::open(bundle) else {
        return out;
    };
    let Some(count) = word(&mut file).filter(|c| *c <= MAX_ENTRIES) else {
        return out;
    };
    for _ in 0..count {
        let Some((extension, path, size)) = header(&mut file) else {
            break;
        };
        if size <= MAX_FILE && wanted(&extension, &path) {
            let mut bytes = vec![0u8; size as usize];
            if file.read_exact(&mut bytes).is_err() {
                break;
            }
            out.push((path, bytes));
        } else if file.seek(SeekFrom::Current(i64::from(size))).is_err() {
            break;
        }
    }
    out
}
/// One entry's extension, path and size.
fn header(file: &mut std::fs::File) -> Option<(String, String, u32)> {
    let extension = word(file).filter(|n| *n < 64).and_then(|n| text(file, n))?;
    let path = word(file)
        .filter(|n| *n < 1024)
        .and_then(|n| text(file, n))?;
    Some((extension, path, word(file)?))
}
fn word(file: &mut std::fs::File) -> Option<u32> {
    let mut b = [0u8; 4];
    file.read_exact(&mut b).ok()?;
    Some(u32::from_le_bytes(b))
}
fn text(file: &mut std::fs::File, len: u32) -> Option<String> {
    let mut b = vec![0u8; len as usize];
    file.read_exact(&mut b).ok()?;
    String::from_utf8(b).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(extension: &str, path: &str, bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for part in [extension.as_bytes(), path.as_bytes()] {
            out.extend((part.len() as u32).to_le_bytes());
            out.extend(part);
        }
        out.extend((bytes.len() as u32).to_le_bytes());
        out.extend(bytes);
        out
    }
    #[test]
    fn wanted_entries_are_read_and_the_rest_skipped() {
        let mut bundle = 3u32.to_le_bytes().to_vec();
        bundle.extend(entry("png", "asset/base/a", b"pixels"));
        bundle.extend(entry(
            "data_champion",
            "asset/base/setting/x",
            b"{\"id\":\"x\"}",
        ));
        bundle.extend(entry("data_champion", "asset/base/setting/y", b"{}"));
        let path = std::env::temp_dir().join(format!("lt-bundle-{}.game_data", std::process::id()));
        std::fs::write(&path, &bundle).unwrap();
        let found = files(&path, |extension, path| {
            extension == "data_champion" && path.ends_with("/x")
        });
        assert_eq!(
            found,
            vec![(
                "asset/base/setting/x".to_owned(),
                b"{\"id\":\"x\"}".to_vec()
            )]
        );
        // A truncated bundle ends the listing without a panic.
        std::fs::write(&path, &bundle[..bundle.len() - 1]).unwrap();
        assert_eq!(files(&path, |e, _| e == "data_champion").len(), 1);
        let _ = std::fs::remove_file(&path);
        assert!(files(Path::new("missing.game_data"), |_, _| true).is_empty());
    }
}
