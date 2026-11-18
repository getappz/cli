//! Small-file read cache (cpeak-style): per-thread memory, version checks, global invalidation on hot reload.

use hyper::body::Bytes;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

const MAX_CACHED_FILE_SIZE: u64 = 1024 * 1024;
const MAX_CACHED_BYTES: usize = 32 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;
const MAX_AGE: Duration = Duration::from_secs(2);
const CHECK_INTERVAL: Duration = Duration::from_millis(100);
const SETTLE_TIME: Duration = Duration::from_secs(2);

static CACHE_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(PartialEq, Eq, Clone, Copy)]
struct Version {
    len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl Version {
    fn of(metadata: &std::fs::Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
        }
    }

    fn settled(&self) -> bool {
        self.modified
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age >= SETTLE_TIME)
    }
}

struct Cached {
    content: Bytes,
    version: Version,
    loaded_at: Instant,
    checked_at: Instant,
}

struct FileCache {
    generation: u64,
    files: HashMap<PathBuf, Cached>,
    bytes: usize,
}

thread_local! {
    static CACHE: RefCell<FileCache> = RefCell::new(FileCache {
        generation: 0,
        files: HashMap::new(),
        bytes: 0,
    });
}

/// Drop cached file contents on all worker threads (call on file watcher events).
pub fn invalidate_all() {
    CACHE_GENERATION.fetch_add(1, Ordering::Release);
}

/// Read a file into memory, using a per-thread cache for small, stable files.
pub fn read_bytes(path: &Path) -> io::Result<Bytes> {
    let generation = CACHE_GENERATION.load(Ordering::Acquire);

    let recently_checked = CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.generation != generation {
            cache.generation = generation;
            cache.files.clear();
            cache.bytes = 0;
        }
        cache
            .files
            .get(path)
            .filter(|cached| {
                cached.checked_at.elapsed() < CHECK_INTERVAL && cached.loaded_at.elapsed() < MAX_AGE
            })
            .map(|cached| cached.content.clone())
    });
    if let Some(content) = recently_checked {
        return Ok(content);
    }

    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    if metadata.len() > MAX_CACHED_FILE_SIZE {
        return Ok(Bytes::from(std::fs::read(path)?));
    }

    let version = Version::of(&metadata);
    let cached = CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let cached = cache
            .files
            .get_mut(path)
            .filter(|cached| cached.version == version && cached.loaded_at.elapsed() < MAX_AGE)?;
        cached.checked_at = Instant::now();
        Some(cached.content.clone())
    });
    if let Some(content) = cached {
        return Ok(content);
    }

    let content = Bytes::from(std::fs::read(path)?);
    let unchanged = std::fs::metadata(path).is_ok_and(|after| Version::of(&after) == version);
    if unchanged && version.settled() && content.len() as u64 == version.len {
        CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.files.len() >= MAX_ENTRIES || cache.bytes + content.len() > MAX_CACHED_BYTES {
                cache.files.clear();
                cache.bytes = 0;
            }
            cache.bytes += content.len();
            if let Some(previous) = cache.files.insert(
                path.to_path_buf(),
                Cached {
                    content: content.clone(),
                    version,
                    loaded_at: Instant::now(),
                    checked_at: Instant::now(),
                },
            ) {
                cache.bytes -= previous.content.len();
            }
        });
    }
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    #[test]
    fn same_metadata_rewrite_expires_even_after_a_cache_hit() {
        let path = std::env::temp_dir().join(format!("appz-cache-{}", std::process::id()));
        let modified = SystemTime::now() - Duration::from_secs(30);
        fs::write(&path, b"old").unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        assert_eq!(read_bytes(&path).unwrap().as_ref(), b"old");
        fs::write(&path, b"new").unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        CACHE.with(|cache| cache.borrow_mut().files.get_mut(&path).unwrap().loaded_at -= MAX_AGE);
        assert_eq!(read_bytes(&path).unwrap().as_ref(), b"new");
        invalidate_all();
        fs::remove_file(&path).unwrap();
        assert!(read_bytes(&path).is_err());
    }
}
