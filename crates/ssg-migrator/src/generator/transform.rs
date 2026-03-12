//! React-to-Astro transformation logic.

use super::classifier::{StaticSafety, StaticSafetyClassifier};
use super::regex::{
    RE_CHILDREN, RE_COMPONENT_START, RE_DEFAULT_IMPORT, RE_EMPTY_FRAGMENT, RE_JSX_COMMENT,
    RE_LAST_IMPORT, RE_LINK_TAG, RE_NAMED_IMPORT, RE_REACT_FRAGMENT, RE_REACT_HOOK,
    RE_STYLE_KV, RE_STYLE_OBJECT,
};
use super::templates::generate_client_only_page;
use crate::types::ProjectAnalysis;
use biome_js_parser::{parse, JsParserOptions};
use biome_js_syntax::JsFileSource;
use biome_rowan::AstNode;
use regex::Regex;

pub(super) fn transform_page_to_astro(
    content: &str,
    component_name: &str,
    analysis: &ProjectAnalysis,
) -> String {
    if RE_REACT_HOOK.is_match(content) {
        return generate_client_only_page(component_name);
    }

    let parsed = parse(content, JsFileSource::tsx(), JsParserOptions::default());
    if parsed.has_errors() {
        return generate_client_only_page(component_name);
    }
    let tree = parsed.tree();
    let result = StaticSafetyClassifier::classify(tree.syntax());
    if result.safety == StaticSafety::Unsafe {
        return generate_client_only_page(component_name);
    }
    transform_page_to_astro_static(content, analysis)
}

pub(super) fn transform_page_to_astro_static(content: &str, analysis: &ProjectAnalysis) -> String {
    let mut result = String::from("---\n");
    result.push_str("import Layout from '../layouts/Layout.astro';\n");

    let (component_imports, component_usage) = collect_imports(content, analysis, "../components");
    for import in &component_imports {
        result.push_str(&format!("{}\n", import));
    }

    let mut last_import_end = 0;
    for m in RE_LAST_IMPORT.find_iter(content) {
        last_import_end = m.end();
    }
    let component_start = RE_COMPONENT_START
        .find(content)
        .map(|m| m.start())
        .unwrap_or(content.len());
    if component_start > last_import_end {
        let constants_section = &content[last_import_end..component_start].trim();
        if !constants_section.is_empty() {
            result.push('\n');
            result.push_str(constants_section);
            result.push('\n');
        }
    }

    result.push_str("---\n\n");
    result.push_str("<Layout>\n");

    let jsx_content = extract_and_convert_jsx(content, &component_usage);
    result.push_str("  ");
    result.push_str(&jsx_content);
    result.push('\n');
    result.push_str("</Layout>\n");
    result
}

pub(super) fn transform_component_to_astro(
    content: &str,
    component_name: &str,
    analysis: &ProjectAnalysis,
) -> String {
    if RE_REACT_HOOK.is_match(content) {
        return content.to_string();
    }

    let parsed = parse(content, JsFileSource::tsx(), JsParserOptions::default());
    if parsed.has_errors() {
        return content.to_string();
    }
    let tree = parsed.tree();
    let result = StaticSafetyClassifier::classify(tree.syntax());
    if result.safety == StaticSafety::Unsafe {
        return content.to_string();
    }
    transform_component_to_astro_static(content, component_name, analysis)
}

pub(super) fn transform_component_to_astro_static(
    content: &str,
    _component_name: &str,
    analysis: &ProjectAnalysis,
) -> String {
    let mut result = String::from("---\n");

    let (component_imports, component_usage) = collect_imports(content, analysis, ".");
    for import in &component_imports {
        result.push_str(&format!("{}\n", import));
    }

    let mut last_import_end = 0;
    for m in RE_LAST_IMPORT.find_iter(content) {
        last_import_end = m.end();
    }
    let component_start = RE_COMPONENT_START
        .find(content)
        .map(|m| m.start())
        .unwrap_or(content.len());
    if component_start > last_import_end {
        let constants_section = &content[last_import_end..component_start].trim();
        if !constants_section.is_empty() {
            result.push('\n');
            result.push_str(constants_section);
            result.push('\n');
        }
    }

    result.push_str("---\n\n");

    let jsx_content = extract_and_convert_jsx(content, &component_usage);
    result.push_str(&jsx_content);
    result.push('\n');
    result
}

fn collect_imports(
    content: &str,
    analysis: &ProjectAnalysis,
    components_base: &str,
) -> (Vec<String>, Vec<(String, bool)>) {
    let mut imports = Vec::new();
    let mut usage = Vec::new();

    for cap in RE_DEFAULT_IMPORT.captures_iter(content) {
        let (Some(name_m), Some(path_m)) = (cap.get(1), cap.get(2)) else {
            continue;
        };
        let comp_name = name_m.as_str();
        let path = path_m.as_str();

        if path == "react-router-dom" || path.starts_with("react") {
            continue;
        }

        if let Some(comp_path) = path.strip_prefix("@/components/") {
            let comp_clean = comp_path.trim_end_matches(".tsx").trim_end_matches(".ts");
            let is_client = analysis
                .components
                .iter()
                .any(|c| (c.name == comp_clean || c.name == comp_name) && c.is_client_side);

            if is_client {
                imports.push(format!(
                    "import {} from '{}/ui/{}.tsx';",
                    comp_name, components_base, comp_clean
                ));
                usage.push((comp_name.to_string(), true));
            } else {
                imports.push(format!(
                    "import {} from '{}/{}.astro';",
                    comp_name, components_base, comp_clean
                ));
                usage.push((comp_name.to_string(), false));
            }
        } else if path.starts_with("@/") {
            imports.push(format!("import {} from '{}';", comp_name, path));
        } else if !path.starts_with(".") {
            imports.push(format!("import {} from '{}';", comp_name, path));
        }
    }

    for cap in RE_NAMED_IMPORT.captures_iter(content) {
        let (Some(list_m), Some(path_m)) = (cap.get(1), cap.get(2)) else {
            continue;
        };
        let path = path_m.as_str();
        if path == "react-router-dom" || path.starts_with("react") {
            continue;
        }

        let imports_str: String = list_m
            .as_str()
            .split(',')
            .map(|s| s.trim())
            .collect::<Vec<_>>()
            .join(", ");

        if path.starts_with("@/components/ui/") {
            let comp_path = path.strip_prefix("@/components/ui/").unwrap();
            imports.push(format!(
                "import {{ {} }} from '{}/ui/{}';",
                imports_str, components_base, comp_path
            ));
        } else if path.starts_with("@/") {
            imports.push(format!("import {{ {} }} from '{}';", imports_str, path));
        } else if !path.starts_with(".") {
            imports.push(format!("import {{ {} }} from '{}';", imports_str, path));
        }
    }

    (imports, usage)
}

fn extract_and_convert_jsx(content: &str, component_usage: &[(String, bool)]) -> String {
    let mut converted = content.to_string();

    converted = converted.replace("className", "class");
    converted = RE_CHILDREN.replace_all(&converted, "<slot />").to_string();

    converted = RE_STYLE_OBJECT
        .replace_all(&converted, |caps: &regex::Captures| {
            if let Some(style_content) = caps.get(1) {
                let style_str = style_content.as_str();
                let mut css_parts = Vec::new();
                for kv_cap in RE_STYLE_KV.captures_iter(style_str) {
                    if let (Some(key), Some(value)) = (kv_cap.get(1), kv_cap.get(2)) {
                        let css_key: String = key
                            .as_str()
                            .chars()
                            .enumerate()
                            .flat_map(|(i, c)| {
                                if c.is_uppercase() && i > 0 {
                                    vec!['-', c.to_lowercase().next().unwrap()]
                                } else {
                                    vec![c.to_lowercase().next().unwrap_or(c)]
                                }
                            })
                            .collect();
                        css_parts
                            .push(format!("{}:{}", css_key.trim(), value.as_str().trim()));
                    }
                }
                if !css_parts.is_empty() {
                    format!("style=\"{}\"", css_parts.join("; "))
                } else {
                    caps.get(0).unwrap().as_str().to_string()
                }
            } else {
                caps.get(0).unwrap().as_str().to_string()
            }
        })
        .to_string();

    converted = RE_LINK_TAG
        .replace_all(&converted, |caps: &regex::Captures| {
            let href = caps.get(1).unwrap().as_str();
            let attrs = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            format!("<a href=\"{}\"{}>", href, attrs)
        })
        .to_string();
    converted = converted.replace("</Link>", "</a>");
    converted = converted.replace("react-router-dom", "");

    let mut jsx_content = extract_return_body(&converted);

    for (comp_name, is_client) in component_usage {
        if !*is_client {
            continue;
        }
        let pattern = Regex::new(&format!(r#"<{}\s*/>"#, regex::escape(comp_name))).unwrap();
        jsx_content = pattern
            .replace_all(&jsx_content, format!("<{} client:only=\"react\" />", comp_name).as_str())
            .to_string();
        let pattern_open = Regex::new(&format!(r#"<{}\s*>"#, regex::escape(comp_name))).unwrap();
        jsx_content = pattern_open
            .replace_all(
                &jsx_content,
                format!("<{} client:only=\"react\">", comp_name).as_str(),
            )
            .to_string();
    }

    jsx_content = RE_EMPTY_FRAGMENT.replace_all(&jsx_content, "").to_string();
    jsx_content = RE_REACT_FRAGMENT.replace_all(&jsx_content, "").to_string();

    jsx_content = RE_JSX_COMMENT
        .replace_all(&jsx_content, |caps: &regex::Captures| {
            let text = caps.get(0).unwrap().as_str();
            let inner = text
                .strip_prefix("/*")
                .and_then(|s| s.strip_suffix("*/"))
                .unwrap_or(text)
                .trim();
            format!("<!-- {} -->", inner)
        })
        .to_string();

    jsx_content
}

pub(super) fn extract_return_body(content: &str) -> String {
    if let Some(return_pos) = content.find("return (") {
        let start = return_pos + "return (".len();
        let bytes = content.as_bytes();
        let mut depth = 1i32;
        let mut end = start;
        while end < bytes.len() && depth > 0 {
            match bytes[end] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            if depth > 0 {
                end += 1;
            }
        }
        if depth == 0 {
            return content[start..end].trim().to_string();
        }
    }

    if let Some(return_pos) = content.find("return ") {
        let after = &content[return_pos + 7..];
        if let Some(end) = after.find("\n  };") {
            return after[..end].trim().to_string();
        }
        return after.trim().to_string();
    }

    content.to_string()
}
