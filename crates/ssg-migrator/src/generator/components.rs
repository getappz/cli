//! Component generation for Astro projects.

use super::fix_imports::{
    fix_all_component_imports, fix_react_imports, replace_router_hooks,
};
use super::transform::transform_component_to_astro;
use crate::types::ProjectAnalysis;
use crate::vfs::Vfs;
use camino::Utf8PathBuf;
use miette::{miette, Result};

pub(super) fn generate_components(
    vfs: &dyn Vfs,
    components_dir: &Utf8PathBuf,
    analysis: &ProjectAnalysis,
) -> Result<()> {
    let ui_dir = components_dir.join("ui");
    vfs.create_dir_all(ui_dir.as_str())
        .map_err(|e| miette!("Failed to create ui directory: {}", e))?;

    let source_ui_dir = analysis.source_dir.join("src/components/ui");
    if vfs.exists(source_ui_dir.as_str()) {
        copy_ui_components(vfs, &source_ui_dir, &ui_dir)?;
    }

    let source_components_dir = analysis.source_dir.join("src/components");
    if vfs.exists(source_components_dir.as_str()) {
        copy_client_components(vfs, &source_components_dir, &ui_dir, analysis)?;
    }

    for component in &analysis.components {
        let is_page = component.file_path.to_string().contains("/pages/");
        let is_ui = component.file_path.to_string().contains("/components/ui/");
        if is_ui {
            continue;
        }

        if component.is_client_side {
            let dest_path = ui_dir.join(format!("{}.tsx", component.name));
            let content = vfs
                .read_to_string(component.file_path.as_str())
                .map_err(|e| miette!("Failed to read component: {}", e))?;
            let fixed = fix_react_imports(&content);
            let fixed = fix_all_component_imports(&fixed, analysis);
            vfs.write_string(dest_path.as_str(), &fixed)
                .map_err(|e| miette!("Failed to write component: {}", e))?;
        } else {
            let is_imported_by_client = analysis
                .components
                .iter()
                .any(|c| c.is_client_side && c.imports.contains(&component.name));

            if is_imported_by_client {
                let dest_path = ui_dir.join(format!("{}.tsx", component.name));
                let content = vfs
                    .read_to_string(component.file_path.as_str())
                    .map_err(|e| miette!("Failed to read component: {}", e))?;
                let fixed = fix_react_imports(&content);
                let fixed = fix_all_component_imports(&fixed, analysis);
                vfs.write_string(dest_path.as_str(), &fixed)
                    .map_err(|e| miette!("Failed to write component: {}", e))?;
            }

            if !is_page {
                let dest_path = components_dir.join(format!("{}.astro", component.name));
                let content = vfs
                    .read_to_string(component.file_path.as_str())
                    .map_err(|e| miette!("Failed to read component: {}", e))?;
                let astro_content =
                    transform_component_to_astro(&content, &component.name, analysis);
                vfs.write_string(dest_path.as_str(), &astro_content)
                    .map_err(|e| miette!("Failed to write Astro component: {}", e))?;
            }
        }
    }

    Ok(())
}

fn copy_ui_components(
    vfs: &dyn Vfs,
    source_ui_dir: &Utf8PathBuf,
    dest_ui_dir: &Utf8PathBuf,
) -> Result<()> {
    for entry in vfs.walk_dir(source_ui_dir.as_str())? {
        if !entry.is_file {
            continue;
        }
        let path = Utf8PathBuf::from(&entry.path);
        if path.extension() == Some("tsx") || path.extension() == Some("ts") {
            let file_name = path
                .file_name()
                .ok_or_else(|| miette!("Invalid file name"))?;
            let dest_path = dest_ui_dir.join(file_name);
            let content = vfs
                .read_to_string(&entry.path)
                .map_err(|e| miette!("Failed to read ui component: {}", e))?;
            let fixed = fix_react_imports(&content);
            vfs.write_string(dest_path.as_str(), &fixed)
                .map_err(|e| miette!("Failed to write ui component: {}", e))?;
        }
    }
    Ok(())
}

fn copy_client_components(
    vfs: &dyn Vfs,
    source_components_dir: &Utf8PathBuf,
    dest_ui_dir: &Utf8PathBuf,
    analysis: &ProjectAnalysis,
) -> Result<()> {
    for entry in vfs.walk_dir(source_components_dir.as_str())? {
        if !entry.is_file {
            continue;
        }
        let path = Utf8PathBuf::from(&entry.path);

        if path.to_string().contains("/components/ui/") {
            continue;
        }

        if path.extension() != Some("tsx") && path.extension() != Some("ts") {
            continue;
        }

        let file_stem = path
            .file_stem()
            .ok_or_else(|| miette!("Invalid file name"))?
            .to_string();

        let is_client = analysis
            .components
            .iter()
            .any(|c| c.name == file_stem && c.is_client_side);

        if !is_client {
            continue;
        }

        let file_name = path
            .file_name()
            .ok_or_else(|| miette!("Invalid file name"))?;
        let dest_path = dest_ui_dir.join(file_name);

        let content = vfs
            .read_to_string(&entry.path)
            .map_err(|e| miette!("Failed to read component: {}", e))?;

        let fixed = crate::ast_transformer::transform_with_ast(&content)
            .unwrap_or_else(|_| content.clone());
        let fixed = replace_router_hooks(&fixed);
        let fixed = fix_all_component_imports(&fixed, analysis);

        vfs.write_string(dest_path.as_str(), &fixed)
            .map_err(|e| miette!("Failed to write component: {}", e))?;
    }
    Ok(())
}
