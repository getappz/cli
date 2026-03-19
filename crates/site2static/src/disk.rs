use std::fs;
use std::io::Write;
use std::path::Path;
use filetime::{set_file_times, FileTime};

/// Preserve modification time from source to destination.
fn preserve_mtime(source: &Path, dest: &Path) {
    if let Ok(meta) = fs::metadata(source) {
        if let Ok(mtime) = meta.modified() {
            let ft = FileTime::from_system_time(mtime);
            let _ = set_file_times(dest, ft, ft);
        }
    }
}

/// Save content to a file, creating parent dirs. Optionally preserve mtime from source.
pub fn save_file(file_name: &str, content: &[u8], output_dir: &Path, source_path: Option<&Path>) {
    let path = output_dir.join(file_name);
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                tracing::warn!("Couldn't create dir {}: {}", parent.display(), e);
                return;
            }
        }
    }
    let mut file = match fs::File::create(&path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("Couldn't create {}: {}", path.display(), e);
            return;
        }
    };
    if let Err(e) = file.write_all(content) {
        tracing::warn!("Couldn't write {}: {}", path.display(), e);
        return;
    }
    if let Some(source) = source_path {
        preserve_mtime(source, &path);
    }
}

/// Fast file comparison using size and mtime (no hashing).
/// Returns `Some(true)` if different, `Some(false)` if unchanged, `None` if uncertain.
pub fn files_differ_fast(source: &Path, dest: &Path) -> Result<Option<bool>, std::io::Error> {
    if !dest.exists() {
        return Ok(Some(true));
    }
    let src_meta = source.symlink_metadata()?;
    let dst_meta = dest.symlink_metadata()?;
    if src_meta.len() != dst_meta.len() {
        return Ok(Some(true));
    }
    let src_mtime = src_meta.modified().unwrap_or(std::time::SystemTime::now());
    let dst_mtime = dst_meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    if src_mtime == dst_mtime {
        return Ok(Some(false));
    }
    Ok(None) // Same size, different mtime — uncertain
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_save_file_creates_dirs() {
        let dir = TempDir::new().unwrap();
        save_file("sub/dir/file.txt", b"hello", dir.path(), None);
        let content = fs::read_to_string(dir.path().join("sub/dir/file.txt")).unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn test_files_differ_fast_dest_missing() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");
        fs::write(&src, "hello").unwrap();
        assert_eq!(files_differ_fast(&src, &dst).unwrap(), Some(true));
    }

    #[test]
    fn test_files_differ_fast_same_size_same_mtime() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src.txt");
        fs::write(&src, "hello").unwrap();
        // Copy preserving mtime
        save_file("dst.txt", b"hello", dir.path(), Some(src.as_path()));
        let dst = dir.path().join("dst.txt");
        assert_eq!(files_differ_fast(&src, &dst).unwrap(), Some(false));
    }
}
