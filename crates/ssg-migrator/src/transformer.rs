use crate::types::ComponentInfo;
use crate::vfs::Vfs;
use biome_js_parser::{parse, JsParserOptions};
use biome_js_syntax::JsFileSource;
use miette::{miette, Result};
use regex::Regex;

/// Options for Astro conversion (client directive style, slot vs children).
#[derive(Debug, Clone, Default)]
pub struct AstroConvertOptions {
    /// Client directive: "comment" | "only" (e.g. client:only="react")
    pub client_directive: Option<String>,
    /// Slot style: "slot" | "children"
    pub slot_style: Option<String>,
    /// File extension for parsing: tsx, ts, jsx, js (defaults to tsx)
    pub file_extension: Option<String>,
}

fn js_file_source_from_ext(ext: Option<&str>) -> JsFileSource {
    match ext {
        Some("ts") => JsFileSource::ts(),
        Some("jsx") | Some("js") => JsFileSource::jsx(),
        _ => JsFileSource::tsx(),
    }
}

/// Convert React/TSX content to Astro format (content-only, no project context).
pub fn convert_to_astro(content: &str, opts: AstroConvertOptions) -> Result<String> {
    transform_static_component(content, opts)
}

pub fn transform_component_to_astro(vfs: &dyn Vfs, component: &ComponentInfo) -> Result<String> {
    let content = vfs
        .read_to_string(component.file_path.as_str())
        .map_err(|e| miette!("Failed to read component: {}", e))?;

    if component.is_client_side {
        Ok(format!("// Client component - keep as React\n{}", content))
    } else {
        transform_static_component(&content, AstroConvertOptions::default())
    }
}

fn transform_static_component(content: &str, opts: AstroConvertOptions) -> Result<String> {
    let source = js_file_source_from_ext(opts.file_extension.as_deref());
    let _parsed = parse(content, source, JsParserOptions::default());

    let props_interface = extract_props_interface(content);

    let mut jsx_content = extract_jsx_content(content);

    if opts.slot_style.as_deref() == Some("slot") {
        jsx_content = jsx_content.replace("{children}", "<slot />");
    }

    let mut astro = String::from("---\n");

    if let Some(props) = props_interface {
        astro.push_str(&format!("interface Props {}\n", props));
        astro.push_str("const { } = Astro.props;\n");
    }

    astro.push_str("---\n\n");
    astro.push_str(&jsx_content);

    Ok(astro)
}

fn extract_props_interface(content: &str) -> Option<String> {
    let interface_pattern = Regex::new(r"interface\s+(\w+)\s*\{([^}]*)\}").unwrap();

    if let Some(cap) = interface_pattern.captures(content) {
        if let Some(props_body) = cap.get(2) {
            return Some(props_body.as_str().trim().to_string());
        }
    }

    None
}

fn extract_jsx_content(content: &str) -> String {
    let return_pattern = Regex::new(r"return\s*\(([\s\S]*?)\)\s*;").unwrap();
    if let Some(cap) = return_pattern.captures(content) {
        if let Some(jsx) = cap.get(1) {
            let jsx_content = jsx.as_str().trim();
            let converted = jsx_content.replace("className", "class");
            return converted;
        }
    }

    let jsx_pattern = Regex::new(r"(<[\s\S]*?>)").unwrap();
    if let Some(cap) = jsx_pattern.captures(content) {
        if let Some(jsx) = cap.get(1) {
            return jsx.as_str().replace("className", "class");
        }
    }

    "<!-- Migrated from React component -->\n<div>Content migrated</div>".to_string()
}

pub fn transform_route_to_astro_page(
    route_path: &str,
    component_name: &str,
    component_content: &str,
) -> String {
    let _page_name = if route_path == "/" {
        "index"
    } else if route_path == "*" {
        "404"
    } else {
        return format!(
            r#"---
import Layout from '../layouts/Layout.astro';
import {} from '../components/{}.astro';
---
<Layout>
  {}
</Layout>
"#,
            component_name, component_name, component_content
        );
    };

    format!(
        r#"---
import Layout from '../layouts/Layout.astro';
import {} from '../components/{}.astro';
---
<Layout>
  {}
</Layout>
"#,
        component_name, component_name, component_content
    )
}

pub fn transform_to_astro_simple(content: &str) -> String {
    let mut result = String::from("---\n");

    if content.contains("interface") && content.contains("Props") {
        result.push_str("// Props interface migrated\n");
    }

    result.push_str("---\n\n");

    let converted = content.replace("className", "class");

    if let Some(return_start) = converted.find("return (") {
        if let Some(return_end) = converted[return_start..].find(");") {
            let jsx = &converted[return_start + 8..return_start + return_end].trim();
            result.push_str(jsx);
        } else {
            result.push_str(&converted);
        }
    } else {
        result.push_str(&converted);
    }

    result
}
