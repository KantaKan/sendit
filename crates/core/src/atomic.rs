//! Replace a file so a crash leaves either the old or the new contents, never half.
use std::{io::Write, path::Path};

pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        // Replaces an existing file on Windows too (MoveFileEx REPLACE_EXISTING).
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_contents_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.json");
        std::fs::write(&p, "old").unwrap();
        write_atomic(&p, b"new").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "new");
        assert!(!dir.path().join("x.json.tmp").exists());
    }

    #[test]
    fn failed_replace_keeps_the_old_target() {
        let dir = tempfile::tempdir().unwrap();
        // A directory can't be replaced by a file, so the rename fails.
        let p = dir.path().join("x.json");
        std::fs::create_dir(&p).unwrap();
        std::fs::write(p.join("keep"), "k").unwrap();
        assert!(write_atomic(&p, b"new").is_err());
        assert!(p.join("keep").exists());
        assert!(!dir.path().join("x.json.tmp").exists());
    }
}
