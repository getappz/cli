//! Two-way sync for SSG migrator.
//!
//! - manifest: SyncManifest at .ssg-migrator/sync.json
//! - git: change detection via Vfs trait
//! - forward: source → output (one-way)
//! - backward: output → source (copy-only paths)

mod backward;
mod forward;

use crate::vfs::Vfs;
use camino::Utf8PathBuf;
use miette::{miette, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Manifest stored at `{output_dir}/.ssg-migrator/sync.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncManifest {
    pub source_dir: String,
    pub output_dir: String,
    pub target: String,
    /// Copy-only path mappings: "source/relative/path" -> "output/relative/path"
    pub file_mappings: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_forward: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_backward: Option<String>,
}

/// Path to the manifest file inside output_dir.
pub fn manifest_path(output_dir: &Utf8PathBuf) -> Utf8PathBuf {
    output_dir.join(".ssg-migrator/sync.json")
}

/// Write the sync manifest after migration.
pub fn write_manifest(
    vfs: &dyn Vfs,
    source_dir: &Utf8PathBuf,
    output_dir: &Utf8PathBuf,
    target: &str,
    mappings: &[(String, String)],
) -> Result<()> {
    let dir = output_dir.join(".ssg-migrator");
    vfs.create_dir_all(dir.as_str())
        .map_err(|e| miette!("Failed to create .ssg-migrator dir: {}", e))?;

    let manifest = SyncManifest {
        source_dir: source_dir.to_string(),
        output_dir: output_dir.to_string(),
        target: target.to_string(),
        file_mappings: mappings.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        last_sync_forward: None,
        last_sync_backward: None,
    };

    let path = manifest_path(output_dir);
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| miette!("Failed to serialize manifest: {}", e))?;
    vfs.write_string(path.as_str(), &json)
        .map_err(|e| miette!("Failed to write manifest: {}", e))?;
    Ok(())
}

/// Read the sync manifest from output_dir. Returns None if it doesn't exist.
pub fn read_manifest(vfs: &dyn Vfs, output_dir: &Utf8PathBuf) -> Result<Option<SyncManifest>> {
    let path = manifest_path(output_dir);
    if !vfs.exists(path.as_str()) {
        return Ok(None);
    }
    let contents = vfs
        .read_to_string(path.as_str())
        .map_err(|e| miette!("Failed to read manifest: {}", e))?;
    let manifest: SyncManifest =
        serde_json::from_str(&contents).map_err(|e| miette!("Invalid manifest: {}", e))?;
    Ok(Some(manifest))
}

/// Collect copy-only path mappings for a migration.
pub fn collect_copy_only_mappings(
    vfs: &dyn Vfs,
    source_dir: &Utf8PathBuf,
    _output_dir: &Utf8PathBuf,
    target: &str,
) -> Vec<(String, String)> {
    let mut mappings = Vec::new();

    // public/* -> public/*
    let public_src = source_dir.join("public");
    if vfs.exists(public_src.as_str()) {
        if let Ok(entries) = vfs.walk_dir(public_src.as_str()) {
            for entry in entries {
                if entry.is_file {
                    if let Some(rel) = entry.path.strip_prefix(source_dir.as_str()) {
                        let rel_str = rel.trim_start_matches('/').replace('\\', "/");
                        if !rel_str.is_empty() {
                            mappings.push((rel_str.clone(), rel_str));
                        }
                    }
                }
            }
        }
    }

    // src/lib/* -> src/lib/* (Astro) or src/client/lib/* (Next.js)
    let lib_src = source_dir.join("src/lib");
    if vfs.exists(lib_src.as_str()) {
        let out_prefix = match target {
            "nextjs" => "src/client/lib",
            _ => "src/lib",
        };
        if let Ok(entries) = vfs.walk_dir(lib_src.as_str()) {
            for entry in entries {
                if entry.is_file {
                    if let Some(rel) = entry.path.strip_prefix(source_dir.as_str()) {
                        let rel_str = rel.trim_start_matches('/').replace('\\', "/");
                        let suffix = rel_str.strip_prefix("src/lib/").unwrap_or(&rel_str);
                        let out_rel = format!("{}/{}", out_prefix, suffix);
                        mappings.push((rel_str, out_rel));
                    }
                }
            }
        }
    }

    // src/assets/* -> src/assets/* (Astro) or src/client/assets/* (Next.js)
    let assets_src = source_dir.join("src/assets");
    if vfs.exists(assets_src.as_str()) {
        let out_prefix = match target {
            "nextjs" => "src/client/assets",
            _ => "src/assets",
        };
        if let Ok(entries) = vfs.walk_dir(assets_src.as_str()) {
            for entry in entries {
                if entry.is_file {
                    if let Some(rel) = entry.path.strip_prefix(source_dir.as_str()) {
                        let rel_str = rel.trim_start_matches('/').replace('\\', "/");
                        let suffix = rel_str.strip_prefix("src/assets/").unwrap_or(&rel_str);
                        let out_rel = format!("{}/{}", out_prefix, suffix);
                        mappings.push((rel_str, out_rel));
                    }
                }
            }
        }
    }

    // tailwind.config.*
    for ext in ["ts", "js"] {
        let src = format!("tailwind.config.{}", ext);
        if vfs.exists(source_dir.join(&src).as_str()) {
            mappings.push((src.clone(), src));
        }
    }

    // postcss.config.*
    for ext in ["ts", "js"] {
        let src = format!("postcss.config.{}", ext);
        if vfs.exists(source_dir.join(&src).as_str()) {
            mappings.push((src.clone(), src));
        }
    }

    // *.css in src/ (immediate children only)
    let src_src = source_dir.join("src");
    if vfs.exists(src_src.as_str()) {
        if let Ok(entries) = vfs.list_dir(src_src.as_str()) {
            for entry in entries {
                if entry.is_file && entry.path.ends_with(".css") {
                    if let Some(rel) = entry.path.strip_prefix(source_dir.as_str()) {
                        let rel_str = rel.trim_start_matches('/').replace('\\', "/");
                        let out_rel = match target {
                            "nextjs" => format!(
                                "src/client/{}",
                                rel_str.strip_prefix("src/").unwrap_or(&rel_str)
                            ),
                            _ => rel_str.clone(),
                        };
                        mappings.push((rel_str, out_rel));
                    }
                }
            }
        }
    }

    mappings
}

/// Result of a backward sync.
#[derive(Debug, Default)]
pub struct SyncResult {
    pub synced: Vec<String>,
    pub skipped_unsafe: Vec<String>,
}

/// Convenience wrappers delegating to Vfs git methods.
pub fn changed_files(vfs: &dyn Vfs, repo_path: &str) -> Result<Vec<String>> {
    vfs.git_changed_files(repo_path)
}

pub fn staged_files(vfs: &dyn Vfs, repo_path: &str) -> Result<Vec<String>> {
    vfs.git_staged_files(repo_path)
}

pub fn is_git_repo(vfs: &dyn Vfs, path: &str) -> bool {
    vfs.git_is_repo(path)
}

pub use backward::sync_backward;
pub use forward::sync_forward;
