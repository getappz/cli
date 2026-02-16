//! Astro project generator for React SPA migration.
//!
//! Modular structure:
//! - classifier: static safety analysis for React components
//! - config: astro.config, package.json, tsconfig
//! - files: tailwind, public, CSS, copy helpers
//! - layout: Layout.astro generation
//! - pages: page generation and client-only templates
//! - components: component copying and Astro conversion
//! - transform: React-to-Astro JSX transformation
//! - fix_imports: router hook shims, import path fixes
//! - readme: README generation

mod classifier;
mod components;
mod config;
mod files;
mod fix_imports;
mod layout;
mod pages;
mod readme;
mod regex;
mod templates;
mod transform;

#[allow(unused_imports)]
pub use classifier::{StaticSafety, StaticSafetyClassifier, StaticSafetyResult, UnsafeReason};

use crate::types::{MigrationConfig, ProjectAnalysis};
use crate::vfs::Vfs;
use miette::{miette, Result};

/// Generate an Astro project from a React SPA.
pub fn generate_astro_project(
    vfs: &dyn Vfs,
    config: &MigrationConfig,
    analysis: &ProjectAnalysis,
) -> Result<()> {
    let out = &config.output_dir;

    if vfs.exists(out.as_str()) && !config.force {
        return Err(miette!(
            "Output directory already exists. Use --force to overwrite."
        ));
    }
    if vfs.exists(out.as_str()) && config.force {
        vfs.remove_dir_all(out.as_str())
            .map_err(|e| miette!("Failed to remove existing directory: {}", e))?;
    }
    vfs.create_dir_all(out.as_str())
        .map_err(|e| miette!("Failed to create output directory: {}", e))?;

    let src_dir = out.join("src");
    let pages_dir = src_dir.join("pages");
    let components_dir = src_dir.join("components");
    let layouts_dir = src_dir.join("layouts");
    let public_dir = out.join("public");

    vfs.create_dir_all(pages_dir.as_str())?;
    vfs.create_dir_all(components_dir.as_str())?;
    vfs.create_dir_all(layouts_dir.as_str())?;
    vfs.create_dir_all(public_dir.as_str())?;

    config::generate_astro_config(vfs, out, analysis)?;
    config::generate_package_json(vfs, out, &config.project_name, analysis)?;
    config::generate_tsconfig(vfs, out)?;

    if analysis.has_tailwind {
        files::copy_tailwind_config(vfs, &analysis.source_dir, out)?;
    }

    files::copy_public_assets(vfs, &analysis.source_dir, &public_dir)?;

    let lib_dir = src_dir.join("lib");
    let source_lib_dir = analysis.source_dir.join("src/lib");
    if vfs.exists(source_lib_dir.as_str()) {
        vfs.create_dir_all(lib_dir.as_str())?;
        files::copy_dir_all(vfs, &source_lib_dir, &lib_dir)?;
    }

    let source_assets_dir = analysis.source_dir.join("src/assets");
    if vfs.exists(source_assets_dir.as_str()) {
        let assets_dir = src_dir.join("assets");
        files::copy_dir_all(vfs, &source_assets_dir, &assets_dir)?;
    }

    files::copy_css_files(vfs, &analysis.source_dir.join("src"), &src_dir)?;

    layout::generate_layout(vfs, &layouts_dir, &src_dir)?;
    components::generate_components(vfs, &components_dir, analysis)?;
    pages::generate_pages(vfs, &pages_dir, analysis)?;
    readme::generate_readme(vfs, out, &config.project_name)?;

    Ok(())
}

/// Simple regex-based Astro transform (used by transformer.rs).
#[allow(dead_code)]
pub fn transform_to_astro_simple(content: &str) -> String {
    let mut result = String::from("---\n");
    if content.contains("interface") && content.contains("Props") {
        result.push_str("// Props interface migrated\n");
    }
    result.push_str("---\n\n");
    let jsx = transform::extract_return_body(&content.replace("className", "class"));
    result.push_str(&jsx);
    result
}
