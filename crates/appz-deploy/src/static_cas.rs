//! Content-Addressable Storage (CAS) scanner & Merkle tree builder.
//!
//! Adapted from WSL earlier appz-client with SHA-256 (Cloudflare R2 alignment).

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

pub fn guess_mime(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path);
    let ext = name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "wasm" => "application/wasm",
        "avif" => "image/avif",
        "otf" => "font/otf",
        "pdf" => "application/pdf",
        "map" => "application/json; charset=utf-8",
        "webmanifest" => "application/manifest+json",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        _ => "application/octet-stream",
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
/// after the first is a hash (8+ word characters, with a digit or mixed case,
/// so ordinary words like `navigation` don't count).
fn has_content_hash(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    stem.split(['.', '-']).skip(1).any(|seg| {
        seg.len() >= 8
            && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && (seg.chars().any(|c| c.is_ascii_digit())
                || (seg.chars().any(|c| c.is_ascii_uppercase())
                    && seg.chars().any(|c| c.is_ascii_lowercase())))
    })
}

/// Whether any path component is one of the ignored names. Whole components
/// only: a substring match would also drop `.github/`, `.gitbook/` or
/// `app.cache.js` from the deploy.
fn is_ignored(rel: &str) -> bool {
    rel.split(['/', '\\'])
        .any(|part| DEFAULT_IGNORES.contains(&part))
}

/// SHA-256 of a file, streamed so only a small buffer is in memory.
fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut hasher = Sha256::new();
    std::io::copy(&mut std::fs::File::open(path)?, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn scan_output_dir(output_dir: &Path) -> Result<ScannedTree, String> {
    if !output_dir.is_dir() {
        return Err(format!(
            "Output directory not found: {}",
            output_dir.display()
        ));
    }

    let mut files = Vec::new();
    let mut blobs_by_hash = HashMap::new();
    let mut total_bytes = 0;

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

        let rel_str = rel.to_string_lossy().to_string();
        if is_ignored(&rel_str) {
            continue;
        }

        let norm_path = if rel_str.starts_with('/') {
            rel_str.replace('\\', "/")
        } else {
            format!("/{}", rel_str.replace('\\', "/"))
        };

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

        let hash = sha256_file(abs_path)
            .map_err(|e| format!("Failed to read {}: {e}", abs_path.display()))?;
        total_bytes += size;

        let mime = guess_mime(&norm_path);
        let immutable = is_immutable_asset(&norm_path);

        files.push(ScannedFileEntry {
            path: norm_path,
            hash: hash.clone(),
            size,
            r#type: mime.to_string(),
            immutable: if immutable { Some(true) } else { None },
        });

        blobs_by_hash
            .entry(hash.clone())
            .or_insert_with(|| LocalBlob {
                hash,
                path: abs_path.to_path_buf(),
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
