//! AST transform host functions for downloadable plugins.
//!
//! The host performs Biome AST operations natively on behalf of the plugin.
//! This keeps heavy parser dependencies on the native side.

use extism::{convert::Json, host_fn};

use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;

// ============================================================================
// AST Transform
// ============================================================================

host_fn!(pub appz_past_transform(
    _user_data: PluginHostData;
    args: Json<PluginAstTransformInput>
) -> Json<PluginAstTransformOutput> {
    let input = args.into_inner();

    // Apply rules as simple text transformations for now.
    // This will be extended to use Biome AST when the ssg-migrator
    // is fully extracted as a plugin.
    let mut code = input.code.clone();

    for rule in &input.rules {
        match rule.rule_type.as_str() {
            "replace_import" => {
                // Replace an import source with another
                if let (Some(from), Some(to)) = (rule.params.get("from"), rule.params.get("to")) {
                    code = code.replace(
                        &format!("from '{}'", from),
                        &format!("from '{}'", to),
                    );
                    code = code.replace(
                        &format!("from \"{}\"", from),
                        &format!("from \"{}\"", to),
                    );
                }
            }
            "remove_import" => {
                // Remove import lines matching a source
                if let Some(source) = rule.params.get("source") {
                    let lines: Vec<&str> = code.lines().collect();
                    let filtered: Vec<&str> = lines
                        .into_iter()
                        .filter(|line| {
                            let trimmed = line.trim();
                            !(trimmed.starts_with("import ")
                                && (trimmed.contains(&format!("from '{}'", source))
                                    || trimmed.contains(&format!("from \"{}\"", source))))
                        })
                        .collect();
                    code = filtered.join("\n");
                }
            }
            "replace_text" => {
                // Simple text replacement
                if let (Some(from), Some(to)) = (rule.params.get("from"), rule.params.get("to")) {
                    code = code.replace(from.as_str(), to.as_str());
                }
            }
            "replace_jsx_tag" => {
                // Replace a JSX tag name (e.g., <Link> -> <a>)
                if let (Some(from), Some(to)) = (rule.params.get("from"), rule.params.get("to")) {
                    code = code.replace(
                        &format!("<{}", from),
                        &format!("<{}", to),
                    );
                    code = code.replace(
                        &format!("</{}>", from),
                        &format!("</{}>", to),
                    );
                }
            }
            "replace_jsx_attr" => {
                // Replace a JSX attribute name (e.g., to -> href)
                if let (Some(from), Some(to)) = (rule.params.get("from"), rule.params.get("to")) {
                    // Simple attribute replacement (handles common cases)
                    code = code.replace(
                        &format!(" {}=", from),
                        &format!(" {}=", to),
                    );
                }
            }
            _ => {
                // Unknown rule type - skip silently
                tracing::warn!("Unknown AST rule type: {}", rule.rule_type);
            }
        }
    }

    Ok(Json(PluginAstTransformOutput {
        success: true,
        code: Some(code),
        error: None,
    }))
});

// ============================================================================
// AST Parse JSX
// ============================================================================

host_fn!(pub appz_past_parse_jsx(
    _user_data: PluginHostData;
    args: Json<PluginAstParseInput>
) -> Json<PluginAstParseOutput> {
    let input = args.into_inner();

    // Simple JSX parser that extracts key information.
    // Returns a structured representation rather than a full AST.
    let code = &input.code;

    let mut imports: Vec<serde_json::Value> = Vec::new();
    let mut components: Vec<String> = Vec::new();

    for line in code.lines() {
        let trimmed = line.trim();

        // Parse imports
        if trimmed.starts_with("import ") {
            imports.push(serde_json::json!({
                "line": trimmed,
            }));
        }

        // Detect component usage (simple: look for PascalCase JSX tags)
        if let Some(start) = trimmed.find('<') {
            let rest = &trimmed[start + 1..];
            if let Some(end) = rest.find(|c: char| !c.is_alphanumeric()) {
                let tag = &rest[..end];
                if !tag.is_empty()
                    && tag.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                {
                    if !components.contains(&tag.to_string()) {
                        components.push(tag.to_string());
                    }
                }
            }
        }
    }

    let ast = serde_json::json!({
        "imports": imports,
        "components": components,
        "line_count": code.lines().count(),
    });

    Ok(Json(PluginAstParseOutput {
        success: true,
        ast: Some(ast),
        error: None,
    }))
});
