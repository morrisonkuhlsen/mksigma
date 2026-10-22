//! Writing a file so a failure part-way never leaves a truncated one in its place.

use std::io::Write as _;
use std::path::{Path, PathBuf};

/// Writes `bytes` to `path` all or nothing: into a temporary file in the same folder, flushed to
/// the disk, then renamed over `path`. If anything fails, `path` keeps what it had and the
/// temporary file is removed.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temporary = temporary(path);
    let written = (|| {
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
        return written;
    }
    // The rename itself reaches the disk with its folder (where the system allows opening one).
    #[cfg(unix)]
    if let Some(folder) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        let _ = std::fs::File::open(folder).and_then(|f| f.sync_all());
    }
    Ok(())
}

/// `.name.<process>.saving` beside `path`, hidden on Unix.
fn temporary(path: &Path) -> PathBuf {
    let name = path.file_name().map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
    path.with_file_name(format!(".{name}.{}.saving", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mksigma-write-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn replaces_the_file_and_leaves_nothing_behind() {
        let dir = folder("replace");
        let path = dir.join("data.sav");
        std::fs::write(&path, b"old").unwrap();
        write_atomic(&path, b"new contents").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new contents");
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left, ["data.sav"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_failed_write_keeps_the_old_file() {
        let dir = folder("fail");
        // The target is a folder: the rename fails after the bytes were written.
        let path = dir.join("taken");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("inside"), b"kept").unwrap();
        assert!(write_atomic(&path, b"new").is_err());
        assert_eq!(std::fs::read(path.join("inside")).unwrap(), b"kept");
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left, ["taken"], "no temporary file left");
        // A folder that doesn't exist is an error, not a crash.
        assert!(write_atomic(&dir.join("missing/x.sav"), b"x").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
