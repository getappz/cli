//! Backward sync: output → source (copy-only paths only).

use crate::sync::SyncManifest;
use miette::{miette, Result};
use std::fs;
use std::path::Path;

/// Result of a backward sync.
#[derive(Debug, Default)]
pub struct SyncResult {
    pub synced: Vec<String>,
    pub skipped_unsafe: Vec<String>,
}

/// Backward sync: copy changed output files (that are copy-only) back to source.
pub fn sync_backward(
    manifest: &SyncManifest,
    changed_output_files: &[String],
) -> Result<SyncResult> {
    let output_dir = Path::new(&manifest.output_dir);
    let source_dir = Path::new(&manifest.source_dir);

    if !output_dir.exists() {
        return Err(miette!("Output directory does not exist: {}", manifest.output_dir));
    }
    if !source_dir.exists() {
        return Err(miette!("Source directory does not exist: {}", manifest.source_dir));
    }

    // Reverse mapping: output path -> source path (for copy-only entries)
    let output_to_source: std::collections::HashMap<String, String> = manifest
        .file_mappings
        .iter()
        .map(|(src, out)| (out.clone(), src.clone()))
        .collect();

    let mut result = SyncResult::default();

    for out_rel in changed_output_files {
        if let Some(src_rel) = output_to_source.get(out_rel) {
            let src_path = source_dir.join(src_rel);
            let out_path = output_dir.join(out_rel);

            if out_path.exists() && out_path.is_file() {
                if let Some(parent) = src_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| miette!("Failed to create dir for {}: {}", src_rel, e))?;
                }
                fs::copy(&out_path, &src_path)
                    .map_err(|e| miette!("Failed to copy {} -> {}: {}", out_rel, src_rel, e))?;
                result.synced.push(out_rel.clone());
            }
        } else {
            result.skipped_unsafe.push(out_rel.clone());
        }
    }

    Ok(result)
}
