//! Content-Addressable Storage (CAS) scanner & Merkle tree builder.
//!
//! Adapted from WSL earlier appz-client with SHA-256 (Cloudflare R2 alignment).

use std::collections::HashMap;
use std::path::Path;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

const DEFAULT_IGNORES: &[&str] = &[
    ".git",
    ".gitmodules",
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

#[derive(Debug, Clone)]
pub struct LocalBlob {
    pub hash: String,
    pub data: Vec<u8>,
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
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
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
        _ => "application/octet-stream",
    }
}

pub fn is_immutable_asset(path: &str) -> bool {
    // Standard bundler immutable asset markers (e.g. /_astro/..., /assets/main.1a2b3c.js)
    path.starts_with("/_astro/")
        || path.starts_with("/assets/")
        || path.starts_with("/static/")
        || path.contains(".hash.")
}

fn is_ignored(rel: &str) -> bool {
    let norm = rel.replace('\\', "/");
    for &pattern in DEFAULT_IGNORES {
        if norm.contains(pattern) || norm.starts_with(pattern) {
            return true;
        }
    }
    false
}

pub fn scan_output_dir(output_dir: &Path) -> Result<ScannedTree, String> {
    if !output_dir.is_dir() {
        return Err(format!("Output directory not found: {}", output_dir.display()));
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

        let data = std::fs::read(abs_path)
            .map_err(|e| format!("Failed to read {}: {e}", abs_path.display()))?;
        let hash = sha256_hex(&data);
        let size = data.len();
        total_bytes += size;

        let norm_path = if rel_str.starts_with('/') {
            rel_str.replace('\\', "/")
        } else {
            format!("/{}", rel_str.replace('\\', "/"))
        };

        let mime = guess_mime(&norm_path);
        let immutable = is_immutable_asset(&norm_path);

        files.push(ScannedFileEntry {
            path: norm_path,
            hash: hash.clone(),
            size,
            r#type: mime.to_string(),
            immutable: if immutable { Some(true) } else { None },
        });

        blobs_by_hash.entry(hash.clone()).or_insert_with(|| LocalBlob {
            hash,
            data,
        });
    }

    Ok(ScannedTree {
        files,
        blobs_by_hash,
        total_bytes,
    })
}
