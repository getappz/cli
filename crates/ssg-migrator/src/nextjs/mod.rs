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

fn build_layout_content(
    vfs: &dyn Vfs,
    source_dir: &Utf8PathBuf,
    transforms_opt: Option<&str>,
) -> Result<String> {
    let transforms: Vec<convert::NextJsTransform> =
        transforms_opt.map(convert::parse_transforms).unwrap_or_default();
    let apply_helmet = transforms.is_empty()
        || transforms.contains(&convert::NextJsTransform::All)
        || transforms.contains(&convert::NextJsTransform::Helmet);

    let helmet_title = if apply_helmet {
        extract_helmet_title_from_source(vfs, source_dir)?
    } else {
        None
    };

    let metadata_block = helmet_title
        .map(|t| {
            let escaped = t.replace('\\', "\\\\").replace('\'', "\\'");
            format!(
                "\nexport const metadata = {{ title: '{}' }};\n",
                escaped
            )
        })
        .unwrap_or_default();

    let layout = if metadata_block.is_empty() {
        LAYOUT_TEMPLATE.to_string()
    } else {
        format!("{metadata_block}{}", LAYOUT_TEMPLATE)
    };

    Ok(layout)
}

fn extract_helmet_title_from_source(
    vfs: &dyn Vfs,
    source_dir: &Utf8PathBuf,
) -> Result<Option<String>> {
    let helmet_title_re = ::regex::Regex::new(r#"<Helmet[^>]*>\s*<title>([^<]*)</title>"#).unwrap();
    let src_dir = source_dir.join("src");
    if !vfs.exists(src_dir.as_str()) {
        return Ok(None);
    }
    for entry in vfs.walk_dir(src_dir.as_str())? {
        if !entry.is_file {
            continue;
        }
        let path = camino::Utf8PathBuf::from(&entry.path);
        let ext = path.extension().unwrap_or("");
        if !matches!(ext, "tsx" | "jsx" | "ts" | "js") {
            continue;
        }
        let content = match vfs.read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if let Some(cap) = helmet_title_re.captures(&content) {
            if let Some(m) = cap.get(1) {
                let title = m.as_str().trim();
                if !title.is_empty() {
                    return Ok(Some(title.to_string()));
                }
            }
        }
    }
    Ok(None)
}

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

    let layout_content =
        build_layout_content(vfs, source_dir, config.transforms.as_deref())?;
    vfs.write_string(
        output_dir.join("src/app/layout.tsx").as_str(),
        &layout_content,
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
    copy_config_files(vfs, output_dir, source_dir, analysis, config.static_export)?;

    copy_public_assets(vfs, source_dir, output_dir.join("public").as_str())?;

    let warnings = if config.static_export {
        verify_static_export(vfs, output_dir)?
    } else {
        Vec::new()
    };

    Ok(warnings)
}
