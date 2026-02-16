//! App Router page creation and cleanup.

use super::regex::RE_DYNAMIC_PARAM;
use super::templates::PAGE_TEMPLATE;
use crate::types::{ProjectAnalysis, RouteInfo};
use crate::vfs::Vfs;
use camino::Utf8PathBuf;
use miette::{miette, Result};
use regex::Regex;

pub(super) fn create_app_router_pages(
    vfs: &dyn Vfs,
    output_dir: &Utf8PathBuf,
    analysis: &ProjectAnalysis,
    static_export: bool,
) -> Result<()> {
    for route in &analysis.routes {
        let (app_segment, page_name, component) = route_to_page_info(route)?;
        let page_path = if app_segment.is_empty() {
            output_dir.join(format!("src/app/{}", page_name))
        } else {
            let dir = output_dir.join(format!("src/app/{}", app_segment));
            vfs.create_dir_all(dir.as_str())
                .map_err(|e| miette!("Failed to create dir: {}", e))?;
            output_dir.join(format!("src/app/{}/{}", app_segment, page_name))
        };

        let has_dynamic = app_segment.contains('[');
        let content = if static_export && has_dynamic {
            let params = extract_dynamic_params(&app_segment);
            let params_fn = build_generate_static_params(&params);
            format!("{}\n\n{}", PAGE_TEMPLATE.replace("PAGENAME", &component), params_fn)
        } else {
            PAGE_TEMPLATE.replace("PAGENAME", &component)
        };

        vfs.write_string(page_path.as_str(), &content)
            .map_err(|e| miette!("Failed to write {}: {}", page_path, e))?;
    }
    Ok(())
}

fn route_to_page_info(route: &RouteInfo) -> Result<(String, String, String)> {
    if route.component == "Index" {
        return Ok(("".into(), "page.tsx".into(), "Index".into()));
    }
    if route.is_catch_all || route.component == "NotFound" {
        return Ok(("".into(), "not-found.tsx".into(), route.component.clone()));
    }
    let path = route.path.trim_start_matches('/');
    let segment = RE_DYNAMIC_PARAM.replace_all(path, "[$1]").to_string();
    Ok((segment, "page.tsx".into(), route.component.clone()))
}

fn extract_dynamic_params(segment: &str) -> Vec<String> {
    Regex::new(r"\[(\w+)\]")
        .unwrap()
        .captures_iter(segment)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn build_generate_static_params(params: &[String]) -> String {
    let fields: Vec<String> = params.iter().map(|p| format!("{p}: \"\"")).collect();
    format!(r#"// TODO: Populate for static export
export function generateStaticParams() {{
  return [{{ {} }}];
}}"#, fields.join(", "))
}

pub(super) fn cleanup_client_files(vfs: &dyn Vfs, output_dir: &Utf8PathBuf) -> Result<()> {
    for rel in &["src/client/main.tsx", "src/client/App.tsx"] {
        let path = output_dir.join(rel);
        if vfs.exists(path.as_str()) {
            vfs.remove_file(path.as_str())
                .map_err(|e| miette!("Failed to remove {}: {}", rel, e))?;
        }
    }
    // Find vite-related files in src/client
    let client_dir = output_dir.join("src/client");
    if vfs.exists(client_dir.as_str()) {
        for entry in vfs.list_dir(client_dir.as_str()).unwrap_or_default() {
            if entry.is_file && entry.path.contains("vite") {
                let _ = vfs.remove_file(&entry.path);
            }
        }
    }
    Ok(())
}
