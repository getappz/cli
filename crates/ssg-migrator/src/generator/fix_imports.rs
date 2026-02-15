//! Import path fixing and router hook shims for migrated React components.

use crate::generator::regex::{RE_USE_LOCATION, RE_USE_NAVIGATE};
use crate::types::ProjectAnalysis;
use regex::Regex;
use std::collections::HashMap;

pub(super) fn fix_react_imports(content: &str) -> String {
    let transformed = match crate::ast_transformer::transform_with_ast(content) {
        Ok(t) => t,
        Err(_) => transform_with_regex_fallback(content),
    };
    let transformed = replace_router_hooks(&transformed);
    fix_all_component_imports_simple(&transformed)
}

pub(super) fn replace_router_hooks(content: &str) -> String {
    let mut fixed = content.to_string();
    fixed = RE_USE_NAVIGATE
        .replace_all(&fixed, |caps: &regex::Captures| {
            let var = caps.get(1).unwrap().as_str();
            format!(
                "const {} = (path: string) => {{ window.location.href = path; }};",
                var
            )
        })
        .to_string();
    fixed = RE_USE_LOCATION
        .replace_all(&fixed, |caps: &regex::Captures| {
            let var = caps.get(1).unwrap().as_str();
            format!(
                "const {} = {{ pathname: window.location.pathname, search: window.location.search, hash: window.location.hash }};",
                var
            )
        })
        .to_string();
    fixed
}

fn fix_all_component_imports_simple(content: &str) -> String {
    content.to_string()
}

pub(super) fn fix_all_component_imports(content: &str, analysis: &ProjectAnalysis) -> String {
    let mut fixed = content.to_string();
    let mut component_paths: HashMap<String, String> = HashMap::new();

    for component in &analysis.components {
        if component.file_path.to_string().contains("/components/ui/") {
            continue;
        }
        let name = &component.name;
        if component.is_client_side {
            component_paths.insert(name.clone(), format!("@/components/ui/{}", name));
        } else {
            let imported_by_client = analysis
                .components
                .iter()
                .any(|c| c.is_client_side && c.imports.contains(name));
            if imported_by_client {
                component_paths.insert(name.clone(), format!("@/components/ui/{}", name));
            } else {
                component_paths
                    .insert(name.clone(), format!("@/components/{}.astro", name));
            }
        }
    }

    for (comp_name, import_path) in &component_paths {
        let pattern = format!(
            r#"import\s+(\w+)\s+from\s+['"]@/components/{}(?:\.tsx)?['"]"#,
            regex::escape(comp_name)
        );
        if let Ok(re) = Regex::new(&pattern) {
            fixed = re
                .replace_all(&fixed, |caps: &regex::Captures| {
                    format!("import {} from '{}'", caps.get(1).unwrap().as_str(), import_path)
                })
                .to_string();
        }
    }

    fixed
}

pub(super) fn transform_with_regex_fallback(content: &str) -> String {
    let mut fixed = content.to_string();

    let router_import_pattern =
        Regex::new(r#"import\s+.*?from\s+["']react-router-dom["'];?\s*\n?"#).unwrap();
    fixed = router_import_pattern.replace_all(&fixed, "").to_string();

    fixed = RE_USE_NAVIGATE
        .replace_all(&fixed, |caps: &regex::Captures| {
            let var = caps.get(1).unwrap().as_str();
            format!(
                "const {} = (path: string) => {{ window.location.href = path; }};",
                var
            )
        })
        .to_string();

    fixed = RE_USE_LOCATION
        .replace_all(&fixed, |caps: &regex::Captures| {
            let var = caps.get(1).unwrap().as_str();
            format!(
                "const {} = {{ pathname: window.location.pathname, search: window.location.search, hash: window.location.hash }};",
                var
            )
        })
        .to_string();

    let use_effect_location_pattern = Regex::new(
        r#"useEffect\(\(\)\s*=>\s*\{[^}]*location[^}]*\},\s*\[location\.pathname\]\);"#,
    )
    .unwrap();
    fixed = use_effect_location_pattern.replace_all(&fixed, "").to_string();

    fixed = fixed.replace("</Link>", "</a>");
    let link_to_double = Regex::new(r#"<Link(\s+)to="([^"]+)""#).unwrap();
    fixed = link_to_double
        .replace_all(&fixed, |caps: &regex::Captures| {
            format!(
                "<a{}href=\"{}\"",
                caps.get(1).unwrap().as_str(),
                caps.get(2).unwrap().as_str()
            )
        })
        .to_string();
    let link_to_single = Regex::new(r#"<Link(\s+)to='([^']+)'"#).unwrap();
    fixed = link_to_single
        .replace_all(&fixed, |caps: &regex::Captures| {
            format!(
                "<a{}href='{}'",
                caps.get(1).unwrap().as_str(),
                caps.get(2).unwrap().as_str()
            )
        })
        .to_string();
    fixed = fixed.replace("<Link ", "<a ");
    fixed = fixed.replace("<Link>", "<a>");
    fixed = fixed.replace("<Link\n", "<a\n");
    fixed = fixed.replace("<Link\t", "<a\t");

    fixed
}
