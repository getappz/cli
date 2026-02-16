//! Next.js App Router generator for React SPA migration.

mod config;
mod convert;
mod pages;
mod providers;
mod regex;
mod templates;
mod transform;
mod verify;

pub use convert::{convert_to_nextjs, parse_transforms, NextJsTransform};

use crate::common::copy_public_assets;
use crate::types::{MigrationConfig, ProjectAnalysis, SsgWarning};
use crate::vfs::Vfs;
use camino::Utf8PathBuf;
use miette::{miette, Result};

use self::config::copy_config_files;
use self::config::write_package_json;
use self::pages::{cleanup_client_files, create_app_router_pages};
use self::providers::create_providers;
use self::templates::{LAYOUT_TEMPLATE, USE_ROUTER_TEMPLATE};
use self::transform::transform_client_files;
use self::verify::verify_static_export;

/// Generate a Next.js App Router project from a React SPA.
pub fn generate_nextjs_project(
    vfs: &dyn Vfs,
    config: &MigrationConfig,
    analysis: &ProjectAnalysis,
    output_dir: &Utf8PathBuf,
) -> Result<Vec<SsgWarning>> {
    let source_dir = &config.source_dir;

    vfs.create_dir_all(output_dir.join("src/app").as_str())?;
    vfs.create_dir_all(output_dir.join("src/client").as_str())?;

    let source_src = source_dir.join("src");
    if vfs.exists(source_src.as_str()) {
        vfs.copy_dir(source_src.as_str(), output_dir.join("src/client").as_str())
            .map_err(|e| miette!("Failed to copy src: {}", e))?;
    }

    transform_client_files(vfs, output_dir, "src/client", config.transforms.as_deref())?;
    create_providers(vfs, output_dir, source_dir)?;

    vfs.write_string(
        output_dir.join("src/app/layout.tsx").as_str(),
        LAYOUT_TEMPLATE,
    )
    .map_err(|e| miette!("Failed to write layout.tsx: {}", e))?;

    vfs.write_string(
        output_dir.join("src/app/useRouter.tsx").as_str(),
        USE_ROUTER_TEMPLATE,
    )
    .map_err(|e| miette!("Failed to write useRouter.tsx: {}", e))?;

    create_app_router_pages(vfs, output_dir, analysis, config.static_export)?;
    cleanup_client_files(vfs, output_dir)?;

    write_package_json(vfs, output_dir, analysis, &config.project_name, config.static_export)?;
    copy_config_files(vfs, output_dir, source_dir, config.static_export)?;

    copy_public_assets(vfs, source_dir, output_dir.join("public").as_str())?;

    let warnings = if config.static_export {
        verify_static_export(vfs, output_dir)?
    } else {
        Vec::new()
    };

    Ok(warnings)
}
