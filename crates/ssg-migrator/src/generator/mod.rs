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
use miette::{miette, Result};
use std::fs;

/// Generate an Astro project from a React SPA.
pub fn generate_astro_project(config: &MigrationConfig, analysis: &ProjectAnalysis) -> Result<()> {
    if config.output_dir.exists() && !config.force {
        return Err(miette!(
            "Output directory already exists. Use --force to overwrite."
        ));
    }
    if config.output_dir.exists() && config.force {
        fs::remove_dir_all(&config.output_dir)
            .map_err(|e| miette!("Failed to remove existing directory: {}", e))?;
    }
    fs::create_dir_all(&config.output_dir)
        .map_err(|e| miette!("Failed to create output directory: {}", e))?;

    let src_dir = config.output_dir.join("src");
    let pages_dir = src_dir.join("pages");
    let components_dir = src_dir.join("components");
    let layouts_dir = src_dir.join("layouts");
    let public_dir = config.output_dir.join("public");

    fs::create_dir_all(&pages_dir)
        .map_err(|e| miette!("Failed to create pages directory: {}", e))?;
    fs::create_dir_all(&components_dir)
        .map_err(|e| miette!("Failed to create components directory: {}", e))?;
    fs::create_dir_all(&layouts_dir)
        .map_err(|e| miette!("Failed to create layouts directory: {}", e))?;
    fs::create_dir_all(&public_dir)
        .map_err(|e| miette!("Failed to create public directory: {}", e))?;

    config::generate_astro_config(&config.output_dir, analysis)?;
    config::generate_package_json(&config.output_dir, &config.project_name, analysis)?;
    config::generate_tsconfig(&config.output_dir)?;

    if analysis.has_tailwind {
        files::copy_tailwind_config(&analysis.source_dir, &config.output_dir)?;
    }

    files::copy_public_assets(&analysis.source_dir, &public_dir)?;

    let lib_dir = src_dir.join("lib");
    let source_lib_dir = analysis.source_dir.join("src/lib");
    if source_lib_dir.exists() {
        fs::create_dir_all(&lib_dir)
            .map_err(|e| miette!("Failed to create lib directory: {}", e))?;
        files::copy_dir_all(&source_lib_dir, &lib_dir)?;
    }

    let source_assets_dir = analysis.source_dir.join("src/assets");
    if source_assets_dir.exists() {
        let assets_dir = src_dir.join("assets");
        files::copy_dir_all(&source_assets_dir, &assets_dir)?;
    }

    files::copy_css_files(&analysis.source_dir.join("src"), &src_dir)?;

    layout::generate_layout(&layouts_dir)?;
    components::generate_components(&components_dir, analysis)?;
    pages::generate_pages(&pages_dir, analysis)?;
    readme::generate_readme(&config.output_dir, &config.project_name)?;

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
