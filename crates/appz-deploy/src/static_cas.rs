//! Content-Addressable Storage (CAS) scanner & Merkle tree builder.
//!
//! Adapted from WSL earlier appz-client with SHA-256 (Cloudflare R2 alignment).

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

pub const MAX_FILE_SIZE: usize = 25 * 1024 * 1024; // 25 MB edge upload limit

const DEFAULT_IGNORES: &[&str] = &[
    ".git",
    ".gitmodules",
    ".gitignore",
    ".gitattributes",
    ".cache",
    ".next",
    ".vercel",
    ".appz",
    "node_modules",
    ".DS_Store",
    "Thumbs.db",
];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScannedFileEntry {
    pub path: String,
    pub hash: String,
    pub size: usize,
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub immutable: Option<bool>,
}

/// A unique file content, located on disk. Its bytes are read again only if
/// the server asks for the blob, so a large site is never held in memory.
#[derive(Debug, Clone)]
pub struct LocalBlob {
    pub hash: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ScannedTree {
    pub files: Vec<ScannedFileEntry>,
    pub blobs_by_hash: HashMap<String, LocalBlob>,
    pub total_bytes: usize,
}

pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[inline]
pub fn guess_mime(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path);
    let ext = name.rsplit_once('.').map_or("", |(_, ext)| ext);
    if ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm") {
        "text/html; charset=utf-8"
    } else if ext.eq_ignore_ascii_case("css") {
        "text/css; charset=utf-8"
    } else if ext.eq_ignore_ascii_case("js") || ext.eq_ignore_ascii_case("mjs") {
        "application/javascript; charset=utf-8"
    } else if ext.eq_ignore_ascii_case("json") || ext.eq_ignore_ascii_case("map") {
        "application/json; charset=utf-8"
    } else if ext.eq_ignore_ascii_case("svg") {
        "image/svg+xml"
    } else if ext.eq_ignore_ascii_case("png") {
        "image/png"
    } else if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") {
        "image/jpeg"
    } else if ext.eq_ignore_ascii_case("webp") {
        "image/webp"
    } else if ext.eq_ignore_ascii_case("gif") {
        "image/gif"
    } else if ext.eq_ignore_ascii_case("ico") {
        "image/x-icon"
    } else if ext.eq_ignore_ascii_case("woff") {
        "font/woff"
    } else if ext.eq_ignore_ascii_case("woff2") {
        "font/woff2"
    } else if ext.eq_ignore_ascii_case("ttf") {
        "font/ttf"
    } else if ext.eq_ignore_ascii_case("txt") {
        "text/plain; charset=utf-8"
    } else if ext.eq_ignore_ascii_case("xml") {
        "application/xml; charset=utf-8"
    } else if ext.eq_ignore_ascii_case("wasm") {
        "application/wasm"
    } else if ext.eq_ignore_ascii_case("avif") {
        "image/avif"
    } else if ext.eq_ignore_ascii_case("otf") {
        "font/otf"
    } else if ext.eq_ignore_ascii_case("pdf") {
        "application/pdf"
    } else if ext.eq_ignore_ascii_case("webmanifest") {
        "application/manifest+json"
    } else if ext.eq_ignore_ascii_case("mp4") {
        "video/mp4"
    } else if ext.eq_ignore_ascii_case("webm") {
        "video/webm"
    } else if ext.eq_ignore_ascii_case("mp3") {
        "audio/mpeg"
    } else {
        "application/octet-stream"
    }
}

/// Whether a file can be cached forever: its URL changes whenever its content
/// does. Bundler-only folders (`/_astro/`, `/_next/static/`) always qualify;
/// in the shared `/assets/` and `/static/` folders only a file whose name
/// carries a content hash does — a hand-written `/assets/style.css` is edited
/// in place, and marking it immutable would serve stale copies after a deploy.
pub fn is_immutable_asset(path: &str) -> bool {
    if path.starts_with("/_astro/") || path.starts_with("/_next/static/") || path.contains(".hash.")
    {
        return true;
    }
    (path.starts_with("/assets/") || path.starts_with("/static/")) && has_content_hash(path)
}

/// A name like `index-B1x2c3D4.js` or `main.073c9b0a.chunk.css`: some segment
/// after the first is a hash (8+ characters: mixed-case or hex with digits,
/// so ordinary words or version labels like `version2024` don't count).
fn has_content_hash(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    stem.split(['.', '-']).skip(1).any(|seg| {
        if seg.len() < 8 {
            return false;
        }
        let mut has_digit = false;
        let mut has_upper = false;
        let mut has_lower = false;
        let mut all_hex = true;
        for &b in seg.as_bytes() {
            if b.is_ascii_digit() {
                has_digit = true;
            } else if b.is_ascii_uppercase() {
                has_upper = true;
            } else if b.is_ascii_lowercase() {
                has_lower = true;
            } else if b != b'_' {
                return false;
            }
            if !b.is_ascii_hexdigit() {
                all_hex = false;
            }
        }
        (has_upper && has_lower) || (all_hex && has_digit)
    })
}

/// Whether any path component is one of the ignored names. Whole components
/// only: a substring match would also drop `.github/`, `.gitbook/` or
/// `app.cache.js` from the deploy.
fn is_ignored(rel: &str) -> bool {
    rel.split(['/', '\\'])
        .any(|part| DEFAULT_IGNORES.contains(&part))
}

/// SHA-256 of a file, streamed with a 64KB buffer to minimize syscall overhead.
fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path)?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn normalize_rel_path(rel: &Path) -> String {
    let rel_str = rel.to_string_lossy();
    let mut norm = String::with_capacity(rel_str.len() + 1);
    if !rel_str.starts_with('/') && !rel_str.starts_with('\\') {
        norm.push('/');
    }
    for c in rel_str.chars() {
        if c == '\\' {
            norm.push('/');
        } else {
            norm.push(c);
        }
    }
    norm
}

struct Candidate {
    abs_path: PathBuf,
    norm_path: String,
    size: usize,
    mime: &'static str,
    immutable: bool,
}

pub fn scan_output_dir(output_dir: &Path) -> Result<ScannedTree, String> {
    if !output_dir.is_dir() {
        return Err(format!(
            "Output directory not found: {}",
            output_dir.display()
        ));
    }

    let mut candidates = Vec::new();

    for entry in WalkDir::new(output_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let abs_path = entry.path();
        let rel = match abs_path.strip_prefix(output_dir) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let rel_lossy = rel.to_string_lossy();
        if is_ignored(&rel_lossy) {
            continue;
        }

        let norm_path = normalize_rel_path(rel);

        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to read metadata for {}: {e}", abs_path.display()))?;
        let size = metadata.len() as usize;
        if size > MAX_FILE_SIZE {
            return Err(format!(
                "File '{}' exceeds maximum allowed size of 25 MB (size: {} bytes)",
                norm_path, size
            ));
        }

        let mime = guess_mime(&norm_path);
        let immutable = is_immutable_asset(&norm_path);

        candidates.push(Candidate {
            abs_path: abs_path.to_path_buf(),
            norm_path,
            size,
            mime,
            immutable,
        });
    }

    if candidates.is_empty() {
        return Ok(ScannedTree {
            files: Vec::new(),
            blobs_by_hash: HashMap::new(),
            total_bytes: 0,
        });
    }

    let count = candidates.len();
    let mut files = Vec::with_capacity(count);
    let mut blobs_by_hash = HashMap::with_capacity(count);
    let mut total_bytes = 0;

    if count == 1 {
        let c = &candidates[0];
        let hash = sha256_file(&c.abs_path)
            .map_err(|e| format!("Failed to read {}: {e}", c.abs_path.display()))?;
        total_bytes += c.size;
        files.push(ScannedFileEntry {
            path: c.norm_path.clone(),
            hash: hash.clone(),
            size: c.size,
            r#type: c.mime.to_string(),
            immutable: if c.immutable { Some(true) } else { None },
        });
        blobs_by_hash.insert(
            hash.clone(),
            LocalBlob {
                hash,
                path: c.abs_path.clone(),
            },
        );
        return Ok(ScannedTree {
            files,
            blobs_by_hash,
            total_bytes,
        });
    }

    // Multi-threaded Merkle tree calculation across available cores
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 8)
        .min(count);

    let next_idx = AtomicUsize::new(0);
    let error_slot = Mutex::new(None::<String>);
    let hashes: Vec<Mutex<Option<String>>> = (0..count).map(|_| Mutex::new(None)).collect();

    std::thread::scope(|s| {
        for _ in 0..num_threads {
            s.spawn(|| {
                loop {
                    let idx = next_idx.fetch_add(1, Ordering::Relaxed);
                    if idx >= count {
                        break;
                    }
                    if error_slot.lock().unwrap().is_some() {
                        break;
                    }

                    let c = &candidates[idx];
                    match sha256_file(&c.abs_path) {
                        Ok(hash) => {
                            *hashes[idx].lock().unwrap() = Some(hash);
                        }
                        Err(e) => {
                            let mut lock = error_slot.lock().unwrap();
                            if lock.is_none() {
                                *lock =
                                    Some(format!("Failed to read {}: {e}", c.abs_path.display()));
                            }
                            break;
                        }
                    }
                }
            });
        }
    });

    if let Some(err) = error_slot.into_inner().unwrap() {
        return Err(err);
    }

    for (c, hash_mutex) in candidates.into_iter().zip(hashes) {
        let hash = hash_mutex
            .into_inner()
            .unwrap()
            .ok_or_else(|| "Missing computed hash".to_string())?;
        total_bytes += c.size;

        files.push(ScannedFileEntry {
            path: c.norm_path,
            hash: hash.clone(),
            size: c.size,
            r#type: c.mime.to_string(),
            immutable: if c.immutable { Some(true) } else { None },
        });

        blobs_by_hash
            .entry(hash.clone())
            .or_insert_with(|| LocalBlob {
                hash,
                path: c.abs_path,
            });
    }

    Ok(ScannedTree {
        files,
        blobs_by_hash,
        total_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_scan_output_dir_basic() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<h1>Hello</h1>").unwrap();
        let sub = dir.path().join("assets");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("style.css"), "body { color: red; }").unwrap();

        let scanned = scan_output_dir(dir.path()).unwrap();
        assert_eq!(scanned.files.len(), 2);
        assert_eq!(scanned.blobs_by_hash.len(), 2);

        let index_entry = scanned
            .files
            .iter()
            .find(|f| f.path == "/index.html")
            .unwrap();
        assert_eq!(index_entry.r#type, "text/html; charset=utf-8");
        assert_eq!(index_entry.immutable, None);

        let css_entry = scanned
            .files
            .iter()
            .find(|f| f.path == "/assets/style.css")
            .unwrap();
        assert_eq!(css_entry.r#type, "text/css; charset=utf-8");
        // Not content-hashed: edited in place, so never cached forever.
        assert_eq!(css_entry.immutable, None);
        assert_eq!(css_entry.hash, sha256_hex(b"body { color: red; }"));
        let blob = &scanned.blobs_by_hash[&css_entry.hash];
        assert_eq!(std::fs::read(&blob.path).unwrap(), b"body { color: red; }");
    }

    #[test]
    fn only_content_hashed_files_are_immutable() {
        for path in [
            "/assets/index-B1x2c3D4.js",
            "/assets/index-DkIpYxHs.css",
            "/static/js/main.1a2b3c4d.js",
            "/static/css/787.073c9b0a.chunk.css",
            "/_astro/page.B1x2c3D4.css",
            "/_next/static/chunks/app.js",
        ] {
            assert!(is_immutable_asset(path), "{path}");
        }
        for path in [
            "/assets/style.css",
            "/assets/main-navigation.js",
            "/assets/jquery-3.6.0.min.js",
            "/assets/app-version2024.js",
            "/static/logo.png",
            "/index.html",
        ] {
            assert!(!is_immutable_asset(path), "{path}");
        }
    }

    #[test]
    fn ignores_whole_path_components_only() {
        assert!(is_ignored(".git/config"));
        assert!(is_ignored("sub\\node_modules\\x.js"));
        assert!(is_ignored(".DS_Store"));
        assert!(!is_ignored(".gitbook/assets/a.png"));
        assert!(!is_ignored(".github/workflows/x.yml"));
        assert!(!is_ignored("js/app.cache.js"));
        assert!(!is_ignored("blog/next-steps.html"));
    }

    #[test]
    fn mime_types_ignore_extension_case() {
        assert_eq!(guess_mime("/IMG.PNG"), "image/png");
        assert_eq!(guess_mime("/photo.avif"), "image/avif");
        assert_eq!(guess_mime("/v1.2/LICENSE"), "application/octet-stream");
    }

    #[test]
    fn test_scan_output_dir_rejects_oversized_file() {
        let dir = tempdir().unwrap();
        let large_file = dir.path().join("large.bin");
        // Create sparse or truncated file simulating > 25 MB
        let f = std::fs::File::create(&large_file).unwrap();
        f.set_len((MAX_FILE_SIZE + 1024) as u64).unwrap();

        let err = scan_output_dir(dir.path()).unwrap_err();
        assert!(err.contains("exceeds maximum allowed size of 25 MB"));
        assert!(err.contains("/large.bin"));
    }
}
