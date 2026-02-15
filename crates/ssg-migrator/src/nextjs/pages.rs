//! App Router page creation and cleanup.

use super::regex::RE_DYNAMIC_PARAM;
use super::templates::PAGE_TEMPLATE;
use crate::types::{ProjectAnalysis, RouteInfo};
use miette::{miette, Result};
use regex::Regex;
use sandbox::ScopedFs;

pub(super) fn create_app_router_pages(
    fs: &ScopedFs,
    analysis: &ProjectAnalysis,
    static_export: bool,
) -> Result<()> {
    for route in &analysis.routes {
        let (app_segment, page_name, component) = route_to_page_info(route)?;
        let page_path = if app_segment.is_empty() {
            format!("src/app/{}", page_name)
        } else {
            fs.create_dir_all(&format!("src/app/{}", app_segment))
                .map_err(|e| miette!("Failed to create dir: {}", e))?;
            format!("src/app/{}/{}", app_segment, page_name)
        };

        let has_dynamic = app_segment.contains('[');
        let content = if static_export && has_dynamic {
            let params = extract_dynamic_params(&app_segment);
            let params_fn = build_generate_static_params(&params);
            format!("{}\n\n{}", PAGE_TEMPLATE.replace("PAGENAME", &component), params_fn)
        } else {
            PAGE_TEMPLATE.replace("PAGENAME", &component)
        };

        fs.write_string(&page_path, &content)
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

pub(super) fn cleanup_client_files(fs: &ScopedFs) -> Result<()> {
    for rel in &["src/client/main.tsx", "src/client/App.tsx"] {
        if fs.exists(rel) {
            fs.remove_file(rel).map_err(|e| miette!("Failed to remove {}: {}", rel, e))?;
        }
    }
    for rel in &fs.glob("src/client/*vite*").map_err(|e| miette!("Glob failed: {}", e))? {
        if fs.is_file(rel) {
            fs.remove_file(rel).map_err(|e| miette!("Failed to remove: {}", e))?;
        }
    }
    Ok(())
}
