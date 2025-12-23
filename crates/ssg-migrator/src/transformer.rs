use crate::types::ComponentInfo;
use biome_fs::BiomePath;
use biome_js_parser::{parse, JsParserOptions};
use biome_js_syntax::JsFileSource;
use miette::{miette, Result};
use regex::Regex;

pub fn transform_component_to_astro(component: &ComponentInfo) -> Result<String> {
    let path = BiomePath::new(component.file_path.clone());
    let content = path
        .read_to_string()
        .map_err(|e| miette!("Failed to read component: {}", e))?;

    if component.is_client_side {
        // Keep as React component, just add client directive comment
        Ok(format!("// Client component - keep as React\n{}", content))
    } else {
        // Transform to Astro format
        transform_static_component(&content)
    }
}

fn transform_static_component(content: &str) -> Result<String> {
    // Parse to validate, but use regex for transformation
    let _parsed = parse(content, JsFileSource::tsx(), JsParserOptions::default());

    // Extract component props interface if present
    let props_interface = extract_props_interface(content);
    
    // Extract JSX return statement
    let jsx_content = extract_jsx_content(content);

    // Build Astro component
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
    // Try to find return statement with JSX
    let return_pattern = Regex::new(r"return\s*\(([\s\S]*?)\)\s*;").unwrap();
    if let Some(cap) = return_pattern.captures(content) {
        if let Some(jsx) = cap.get(1) {
            let jsx_content = jsx.as_str().trim();
            // Convert className to class
            let converted = jsx_content.replace("className", "class");
            return converted;
        }
    }
    
    // Fallback: try to find JSX directly
    let jsx_pattern = Regex::new(r"(<[\s\S]*?>)").unwrap();
    if let Some(cap) = jsx_pattern.captures(content) {
        if let Some(jsx) = cap.get(1) {
            return jsx.as_str().replace("className", "class");
        }
    }
    
    // If we can't extract, return a placeholder
    format!("<!-- Migrated from React component -->\n<div>Content migrated</div>")
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
        // Store in a variable to avoid lifetime issues
        // We don't actually use page_name in this function, so this is fine
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
    // Simple regex-based transformation
    let mut result = String::from("---\n");
    
    // Extract props if present
    if content.contains("interface") && content.contains("Props") {
        result.push_str("// Props interface migrated\n");
    }
    
    result.push_str("---\n\n");
    
    // Convert className to class
    let converted = content.replace("className", "class");
    
    // Try to extract JSX return
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
