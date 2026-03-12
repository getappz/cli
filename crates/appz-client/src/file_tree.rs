//! File tree building — scan output directory with ignore rules.
//!
//! Ported from Vercel `@vercel/client` buildFileTree. Uses content-addressed
//! structure (path -> content) for dedup.

use miette::{miette, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn file_mode(meta: &std::fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        meta.permissions().mode()
    }
    #[cfg(not(unix))]
    {
        0o644
    }
}

/// Default ignore patterns (aligned with Vercel .vercelignore).
const DEFAULT_IGNORES: &[&str] = &[
    ".hg",
    ".git",
    ".gitmodules",
    ".svn",
    ".cache",
    ".next",
    ".now",
    ".vercel",
    ".appz",
    ".npmignore",
    ".dockerignore",
    ".gitignore",
    ".*.swp",
    ".DS_Store",
    "node_modules",
    "__pycache__",
    "venv",
    "CVS",
];

/// Prebuilt output directory (Appz uses .appz/output; Vercel uses .vercel/output).
pub const OUTPUT_DIR: &str = ".appz/output";

/// Result of scanning a directory.
#[derive(Debug, Clone)]
pub struct FileTree {
    /// Absolute paths of files
    pub files: Vec<PathBuf>,
}

fn is_ignored(rel: &str, ignores: &[&str]) -> bool {
    let rel = rel.replace('\\', "/");
    for pattern in ignores {
        if rel.contains(pattern) || rel.starts_with(pattern.trim_start_matches(|c| c == '.' || c == '/')) {
            return true;
        }
        if pattern.starts_with('*') && rel.ends_with(pattern.trim_start_matches('*')) {
            return true;
        }
    }
    false
}

/// Build file tree from an output directory.
///
/// Walks the directory, applies ignore rules, returns absolute paths of files.
pub fn build_file_tree(output_dir: &Path, extra_ignores: &[&str]) -> Result<FileTree> {
    if !output_dir.is_dir() {
        return Err(miette!(
            "Output directory does not exist: {}",
            output_dir.display()
        ));
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(output_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let path = entry.path();
            if let Ok(rel) = path.strip_prefix(output_dir) {
                let rel_str = rel.to_string_lossy();
                if !is_ignored(&rel_str, DEFAULT_IGNORES)
                    && !is_ignored(&rel_str, extra_ignores)
                {
                    files.push(path.to_path_buf());
                }
            }
        }
    }

    Ok(FileTree { files })
}

/// Compute SHA1 hash of file contents.
pub fn sha1_hex(data: &[u8]) -> String {
    sha1_smol::Sha1::from(data).digest().to_string()
}

/// File reference with content hash and metadata (for deployment).
#[derive(Debug, Clone)]
pub struct FileRef {
    pub path: PathBuf,
    pub sha: String,
    pub data: Vec<u8>,
    pub mode: u32,
}

/// Build content-addressed file map from directory.
///
/// Reads each file, computes SHA1, returns Map<sha, FileRef>.
pub fn build_hashed_files(output_dir: &Path, tree: &FileTree) -> Result<HashMap<String, FileRef>> {
    let mut map: HashMap<String, FileRef> = HashMap::new();
    for abs_path in &tree.files {
        let data = std::fs::read(abs_path)
            .map_err(|e| miette!("Failed to read {}: {}", abs_path.display(), e))?;
        let meta = std::fs::metadata(abs_path)
            .map_err(|e| miette!("Failed to stat {}: {}", abs_path.display(), e))?;
        let mode = file_mode(&meta);
        let sha = sha1_hex(&data);

        let rel = abs_path
            .strip_prefix(output_dir)
            .map_err(|e| miette!("Path strip prefix: {}", e))?;

        // Dedup: same content can appear at multiple paths (Vercel uses names: string[])
        // We store one FileRef per sha; caller tracks which paths reference it
        if !map.contains_key(&sha) {
            map.insert(
                sha.clone(),
                FileRef {
                    path: rel.to_path_buf(),
                    sha: sha.clone(),
                    data,
                    mode,
                },
            );
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_sha1_hex() {
        assert_eq!(
            sha1_hex(b"hello"),
            "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"
        );
    }

    #[test]
    fn test_build_file_tree_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let tree = build_file_tree(tmp.path(), &[]).unwrap();
        assert!(tree.files.is_empty());
    }

    #[test]
    fn test_build_file_tree_with_file() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("index.html"), "<html></html>").unwrap();
        let tree = build_file_tree(tmp.path(), &[]).unwrap();
        assert_eq!(tree.files.len(), 1);
        assert!(tree.files[0].ends_with("index.html"));
    }
}
