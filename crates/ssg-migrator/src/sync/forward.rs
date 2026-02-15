//! One-way sync: source → output (re-migrate).

use crate::analyzer::analyze_project;
use crate::generator::generate_astro_project;
use crate::nextjs::generate_nextjs_project;
use crate::sync::SyncManifest;
use crate::types::{MigrationConfig, SsgWarning};
use miette::{miette, Result};
use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};

/// Forward sync: re-run migration for changed source files.
/// v1: full re-migration when any source file changes.
pub async fn sync_forward(
    manifest: &SyncManifest,
    _changed_source_files: &[String],
) -> Result<Vec<SsgWarning>> {
    let source_dir = camino::Utf8PathBuf::from(manifest.source_dir.as_str());
    let output_dir = camino::Utf8PathBuf::from(manifest.output_dir.as_str());

    if !source_dir.exists() {
        return Err(miette!("Source directory does not exist: {}", source_dir));
    }

    let analysis = analyze_project(&source_dir)?;

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
        static_export: false, // sync manifest doesn't store this; default
    };

    match manifest.target.as_str() {
        "nextjs" => {
            let sandbox_config = SandboxConfig::new(output_dir.as_path())
                .with_settings(SandboxSettings::default().with_tool("bun", Some("latest")));

            let sandbox = create_sandbox(sandbox_config)
                .await
                .map_err(|e| miette!("Failed to create sandbox: {}", e))?;

            let fs = sandbox.fs();
            let warnings =
                generate_nextjs_project(&config, &analysis, fs)
                    .map_err(|e| miette!("Failed to generate Next.js project: {}", e))?;

            Ok(warnings)
        }
        _ => {
            generate_astro_project(&config, &analysis)
                .map_err(|e| miette!("Failed to generate Astro project: {}", e))?;
            Ok(Vec::new())
        }
    }
}
