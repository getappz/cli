//! Next.js App Router generator for React SPA migration.
//!
//! Modular structure:
//! - regex: pre-compiled regexes
//! - templates: USE_ROUTER_TEMPLATE, LAYOUT_TEMPLATE, PAGE_TEMPLATE
//! - transform: CSS, TS/TSX transforms, image imports
//! - providers: create_providers from App.tsx
//! - pages: create_app_router_pages, cleanup_client_files
//! - config: write_package_json, copy_config_files
//! - verify: verify_static_export

mod config;
mod pages;
mod providers;
mod regex;
mod templates;
mod transform;
mod verify;

use crate::common::copy_public_assets;
use crate::types::{MigrationConfig, ProjectAnalysis, SsgWarning};
use miette::{miette, Result};
use sandbox::ScopedFs;

use self::config::copy_config_files;
use self::config::write_package_json;
use self::pages::{cleanup_client_files, create_app_router_pages};
use self::providers::create_providers;
use self::templates::{LAYOUT_TEMPLATE, USE_ROUTER_TEMPLATE};
use self::transform::transform_client_files;
use self::verify::verify_static_export;

/// Generate a Next.js App Router project from a React SPA.
pub fn generate_nextjs_project(
    config: &MigrationConfig,
    analysis: &ProjectAnalysis,
    fs: &ScopedFs,
) -> Result<Vec<SsgWarning>> {
    let source_dir = &config.source_dir;

    fs.create_dir_all("src/app")?;
    fs.create_dir_all("src/client")?;

    let source_src = source_dir.join("src");
    if source_src.exists() {
        fs.copy_from_external(source_src.as_path(), "src/client")
            .map_err(|e| miette!("Failed to copy src: {}", e))?;
    }

    transform_client_files(fs, "src/client")?;
    create_providers(fs, source_dir)?;

    fs.write_string("src/app/layout.tsx", LAYOUT_TEMPLATE)
        .map_err(|e| miette!("Failed to write layout.tsx: {}", e))?;

    fs.write_string("src/app/useRouter.tsx", USE_ROUTER_TEMPLATE)
        .map_err(|e| miette!("Failed to write useRouter.tsx: {}", e))?;

    create_app_router_pages(fs, analysis, config.static_export)?;
    cleanup_client_files(fs)?;

    write_package_json(fs, analysis, &config.project_name, config.static_export)?;
    copy_config_files(fs, source_dir, config.static_export)?;

    copy_public_assets(source_dir, fs, "public")?;

    let warnings = if config.static_export {
        verify_static_export(fs)?
    } else {
        Vec::new()
    };

    Ok(warnings)
}
