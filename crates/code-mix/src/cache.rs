//! Content-addressable pack cache with SQLite index.
//!
//! input_key = SHA256(version + options + sorted(path:content_hash))
//! content_hash = SHA256(output_content)
//! Store: ~/.appz/store/objects/{hash[0..2]}/{hash}, index.db

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tokio::fs;

use crate::repomix::RepomixError;
use crate::store;
use crate::types::PackOptions;

const CACHE_VERSION: u32 = 2;

/// Compute input_key from options + paths (using file content hashes).
pub async fn compute_input_key(
    workdir: &Path,
    options: &PackOptions,
    paths: &[String],
) -> Result<String, RepomixError> {
    let mut hasher = Sha256::new();
    hasher.update(CACHE_VERSION.to_le_bytes());
    hasher.update(options_cache_repr(options).as_bytes());

    let mut path_hashes: Vec<(String, String)> = Vec::with_capacity(paths.len());
    for path in paths.iter() {
        let full = workdir.join(path);
        let content = fs::read(&full).await.map_err(|e| {
            RepomixError(format!("Failed to read file for cache key {}: {}", path, e))
        })?;
        let file_hash = hex::encode(Sha256::digest(&content));
        path_hashes.push((path.clone(), file_hash));
    }
    path_hashes.sort_by(|a, b| a.0.cmp(&b.0));

    for (path, file_hash) in path_hashes {
        let line = format!("{}:{}\n", path, file_hash);
        hasher.update(line.as_bytes());
    }

    Ok(hex::encode(hasher.finalize()))
}

/// Compute content hash of output bytes.
pub fn compute_content_hash(content: &[u8]) -> String {
    hex::encode(Sha256::digest(content))
}

/// Check cache and return path to cached output if valid.
pub async fn get_cached_output(
    workdir: &Path,
    options: &PackOptions,
    paths: &[String],
) -> Result<Option<PathBuf>, RepomixError> {
    if options.no_cache || options.split_output.is_some() {
        return Ok(None);
    }

    let input_key = compute_input_key(workdir, options, paths).await?;
    let root = store::store_root()?;
    let conn = store::open_index(&root)?;

    let Some(content_hash) = store::get_content_hash(&conn, &input_key)? else {
        return Ok(None);
    };

    let obj_path = store::object_path(&root, &content_hash);
    if store::object_exists(&root, &content_hash) {
        Ok(Some(obj_path))
    } else {
        Ok(None)
    }
}

/// Write output to store and insert index entry with metadata.
pub async fn put_cached_output(
    input_key: &str,
    content: &[u8],
    meta: &store::PackMetadata,
) -> Result<(), RepomixError> {
    let content_hash = compute_content_hash(content);
    let root = store::store_root()?;
    store::ensure_store_dirs(&root)?;

    let obj_path = store::object_path(&root, &content_hash);
    if let Some(parent) = obj_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| RepomixError(format!("Failed to create object dir: {}", e)))?;
    }
    let temp_path = obj_path.with_extension("tmp");
    tokio::fs::write(&temp_path, content)
        .await
        .map_err(|e| RepomixError(format!("Failed to write object: {}", e)))?;
    std::fs::rename(&temp_path, &obj_path)
        .map_err(|e| RepomixError(format!("Failed to finalize object: {}", e)))?;

    let conn = store::open_index(&root)?;
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    store::insert_index(&conn, input_key, &content_hash, created_at, meta)?;

    Ok(())
}

/// Apply cached output to user's desired destination (file, stdout, clipboard).
pub async fn apply_cached_output(
    options: &PackOptions,
    cached_path: &Path,
) -> Result<(), RepomixError> {
    apply_output(options, cached_path).await
}

async fn apply_output(options: &PackOptions, from_path: &Path) -> Result<(), RepomixError> {
    let content = fs::read_to_string(from_path)
        .await
        .map_err(|e| RepomixError(format!("Failed to read output: {}", e)))?;

    if options.stdout {
        print!("{}", content);
        return Ok(());
    }

    if options.copy {
        copy_to_clipboard(&content).await?;
        return Ok(());
    }

    if let Some(ref out) = options.output {
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| RepomixError(format!("Failed to create output dir: {}", e)))?;
        }
        fs::write(out, content)
            .await
            .map_err(|e| RepomixError(format!("Failed to write output: {}", e)))?;
    }
    // Default: no file written; output is in cache for appz code search

    Ok(())
}

/// Try to copy text to clipboard via system commands (pbcopy, wl-copy, xclip, clip).
async fn copy_to_clipboard(content: &str) -> Result<(), RepomixError> {
    let clip_cmds: &[&[&str]] = if cfg!(target_os = "macos") {
        &[&["pbcopy" as &str][..]]
    } else if cfg!(target_os = "windows") {
        &[&["cmd", "/c", "clip"][..]]
    } else if std::env::var("WAYLAND_DISPLAY").is_ok() {
        &[&["wl-copy"][..]]
    } else {
        &[&["xclip", "-selection", "clipboard"][..], &["xsel", "--clipboard", "--input"][..]]
    };

    for args in clip_cmds {
        let mut cmd = tokio::process::Command::new(args[0]);
        cmd.stdin(std::process::Stdio::piped()).args(&args[1..]);
        if let Ok(mut child) = cmd.spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                if stdin.write_all(content.as_bytes()).await.is_ok()
                    && stdin.shutdown().await.is_ok()
                {
                    if child.wait().await.map(|s| s.success()).unwrap_or(false) {
                        return Ok(());
                    }
                }
            }
        }
    }
    Err(RepomixError(
        "Clipboard not available. Install pbcopy (macOS), wl-copy (Wayland), or xclip/xsel (X11).".into(),
    ))
}

fn options_cache_repr(opts: &PackOptions) -> String {
    let mut parts = Vec::new();
    parts.push(opts.style.as_deref().unwrap_or("xml").to_string());
    parts.push(opts.compress.to_string());
    parts.push(opts.remove_comments.to_string());
    parts.push(opts.remove_empty_lines.to_string());
    parts.push(opts.include.join(","));
    parts.push(opts.ignore.join(","));
    if let Some(ref h) = opts.header {
        parts.push(h.clone());
    }
    if let Some(ref i) = opts.instruction {
        parts.push(i.display().to_string());
    }
    parts.join("\0")
}

/// Entry for list_cached (user-friendly metadata).
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub input_key: String,
    pub content_hash: String,
    pub created_at: i64,
    pub size_bytes: Option<u64>,
    pub workdir: Option<String>,
    pub style: Option<String>,
    pub file_count: Option<i64>,
    pub workspace: Option<String>,
}

/// List cached pack outputs with metadata.
pub fn list_cached(verbose: bool) -> Result<Vec<CacheEntry>, RepomixError> {
    let root = store::store_root()?;
    let conn = store::open_index(&root)?;
    let rows = store::list_entries(&conn)?;
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        let size_bytes = if verbose {
            store::object_size(&root, &row.content_hash)
        } else {
            None
        };
        entries.push(CacheEntry {
            input_key: row.input_key,
            content_hash: row.content_hash,
            created_at: row.created_at,
            size_bytes,
            workdir: row.workdir,
            style: row.style,
            file_count: row.file_count,
            workspace: row.workspace,
        });
    }
    Ok(entries)
}

/// Get packs for a workdir, with object paths. Returns entries with paths, most recent first.
pub fn get_packs_for_workdir(
    workdir: &Path,
) -> Result<Vec<(CacheEntry, PathBuf)>, RepomixError> {
    let canonical = workdir
        .canonicalize()
        .map_err(|e| RepomixError(format!("Invalid workdir: {}", e)))?;
    let workdir_str = canonical.to_string_lossy().into_owned();

    let root = store::store_root()?;
    let conn = store::open_index(&root)?;
    let rows = store::get_entries_for_workdir(&conn, &workdir_str)?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        if !store::object_exists(&root, &row.content_hash) {
            continue;
        }
        let path = store::object_path(&root, &row.content_hash);
        let size_bytes = store::object_size(&root, &row.content_hash);
        out.push((
            CacheEntry {
                input_key: row.input_key,
                content_hash: row.content_hash,
                created_at: row.created_at,
                size_bytes,
                workdir: row.workdir,
                style: row.style,
                file_count: row.file_count,
                workspace: row.workspace,
            },
            path,
        ));
    }
    Ok(out)
}

/// Validate content hash format (64 hex chars).
pub fn is_valid_content_hash(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Remove cached entries by content hash or all.
pub fn remove_cached(hashes: &[String], all: bool) -> Result<(), RepomixError> {
    let root = store::store_root()?;
    let conn = store::open_index(&root)?;

    if all {
        let rows = store::list_entries(&conn)?;
        for row in rows {
            store::delete_object(&root, &row.content_hash)?;
        }
        store::remove_all(&conn)?;
    } else {
        for h in hashes {
            if !is_valid_content_hash(h) {
                return Err(RepomixError(format!("Invalid content hash: '{}' (expected 64 hex chars)", h)));
            }
            store::delete_object(&root, h)?;
        }
        store::remove_by_hash(&conn, hashes)?;
    }
    Ok(())
}
