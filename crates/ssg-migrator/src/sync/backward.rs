//! Backward sync: output → source (copy-only paths only).

use crate::sync::{SyncManifest, SyncResult};
use crate::vfs::Vfs;
use miette::{miette, Result};

/// Backward sync: copy changed output files (that are copy-only) back to source.
pub fn sync_backward(
    vfs: &dyn Vfs,
    manifest: &SyncManifest,
    changed_output_files: &[String],
) -> Result<SyncResult> {
    if !vfs.exists(&manifest.output_dir) {
        return Err(miette!(
            "Output directory does not exist: {}",
            manifest.output_dir
        ));
    }
    if !vfs.exists(&manifest.source_dir) {
        return Err(miette!(
            "Source directory does not exist: {}",
            manifest.source_dir
        ));
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
            let src_path = format!("{}/{}", manifest.source_dir, src_rel);
            let out_path = format!("{}/{}", manifest.output_dir, out_rel);

            if vfs.exists(&out_path) && vfs.is_file(&out_path) {
                vfs.copy_file(&out_path, &src_path)
                    .map_err(|e| miette!("Failed to copy {} -> {}: {}", out_rel, src_rel, e))?;
                result.synced.push(out_rel.clone());
            }
        } else {
            result.skipped_unsafe.push(out_rel.clone());
        }
    }

    Ok(result)
}
