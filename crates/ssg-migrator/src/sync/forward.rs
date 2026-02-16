//! One-way sync: source → output (re-migrate).

use crate::analyzer::analyze_project;
use crate::generator::generate_astro_project;
use crate::nextjs::generate_nextjs_project;
use crate::sync::SyncManifest;
use crate::types::{MigrationConfig, SsgWarning};
use crate::vfs::Vfs;
use camino::Utf8PathBuf;
use miette::{miette, Result};

/// Forward sync: re-run migration for changed source files.
/// v1: full re-migration when any source file changes.
pub fn sync_forward(
    vfs: &dyn Vfs,
    manifest: &SyncManifest,
    _changed_source_files: &[String],
) -> Result<Vec<SsgWarning>> {
    let source_dir = Utf8PathBuf::from(manifest.source_dir.as_str());
    let output_dir = Utf8PathBuf::from(manifest.output_dir.as_str());

    if !vfs.exists(source_dir.as_str()) {
        return Err(miette!("Source directory does not exist: {}", source_dir));
    }

    let analysis = analyze_project(vfs, &source_dir)?;

    let project_name = output_dir
        .file_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            if manifest.target == "nextjs" {
                "nextjs".to_string()
            } else {
                "migrated-astro-app".to_string()
            }
        });

    let config = MigrationConfig {
        source_dir: source_dir.clone(),
        output_dir: output_dir.clone(),
        project_name: project_name.clone(),
        force: true,
        static_export: false,
        transforms: None,
    };

    match manifest.target.as_str() {
        "nextjs" => {
            let warnings =
                generate_nextjs_project(vfs, &config, &analysis, &output_dir)
                    .map_err(|e| miette!("Failed to generate Next.js project: {}", e))?;
            Ok(warnings)
        }
        _ => {
            generate_astro_project(vfs, &config, &analysis)
                .map_err(|e| miette!("Failed to generate Astro project: {}", e))?;
            Ok(Vec::new())
        }
    }
}
