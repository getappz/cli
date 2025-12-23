use crate::types::{ComponentInfo, ProjectAnalysis, RouteInfo};
use biome_fs::BiomePath;
use biome_js_parser::{parse, JsParserOptions};
use biome_js_syntax::{JsFileSource, JsSyntaxKind, JsSyntaxNode, JsImport};
use biome_rowan::AstNode;
use camino::Utf8PathBuf;
use miette::{miette, Result};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use walkdir::WalkDir;

pub fn analyze_project(source_dir: &Utf8PathBuf) -> Result<ProjectAnalysis> {
    let app_path = source_dir.join("src/App.tsx");
    let package_json_path = source_dir.join("package.json");

    // Parse routes from App.tsx using regex (simpler than AST traversal)
    let routes = if app_path.exists() {
        parse_routes(&app_path)?
    } else {
        vec![]
    };

    // Analyze components
    let components = analyze_components(source_dir)?;

    // Parse dependencies
    let dependencies = parse_dependencies(&package_json_path)?;

    // Check for config files
    let has_vite_config = source_dir.join("vite.config.ts").exists()
        || source_dir.join("vite.config.js").exists();
    let has_tailwind = source_dir.join("tailwind.config.ts").exists()
        || source_dir.join("tailwind.config.js").exists();

    Ok(ProjectAnalysis {
        routes,
        components,
        dependencies,
        has_vite_config,
        has_tailwind,
        source_dir: source_dir.clone(),
    })
}

fn parse_routes(app_path: &Utf8PathBuf) -> Result<Vec<RouteInfo>> {
    let path = BiomePath::new(app_path.clone());
    let content = path
        .read_to_string()
        .map_err(|e| miette!("Failed to read App.tsx: {}", e))?;

    let mut routes = Vec::new();

    // Use regex to find Route components
    // Pattern: <Route path="..." element={<ComponentName />} />
    let route_pattern = Regex::new(r#"<Route\s+path=["']([^"']+)["']\s+element=\{<(\w+)\s*/>\}"#).unwrap();
    
    for cap in route_pattern.captures_iter(&content) {
        let path = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_else(|| "/".to_string());
        let component = cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_else(|| "NotFound".to_string());
        let is_catch_all = path == "*";
        
        routes.push(RouteInfo {
            path,
            component,
            is_catch_all,
        });
    }

    // Also check for catch-all route
    if content.contains(r#"path="*""#) || content.contains(r#"path='*'"#) {
        if !routes.iter().any(|r| r.is_catch_all) {
            routes.push(RouteInfo {
                path: "*".to_string(),
                component: "NotFound".to_string(),
                is_catch_all: true,
            });
        }
    }

    // If no routes found, add default index route
    if routes.is_empty() {
        routes.push(RouteInfo {
            path: "/".to_string(),
            component: "Index".to_string(),
            is_catch_all: false,
        });
    }

    Ok(routes)
}

fn analyze_components(source_dir: &Utf8PathBuf) -> Result<Vec<ComponentInfo>> {
    let mut components = Vec::new();
    let components_dir = source_dir.join("src/components");
    let pages_dir = source_dir.join("src/pages");

    // Analyze components directory
    if components_dir.exists() {
        analyze_directory(&components_dir, &mut components)?;
    }

    // Analyze pages directory
    if pages_dir.exists() {
        analyze_directory(&pages_dir, &mut components)?;
    }

    // Propagate context boundaries up the import graph
    propagate_context_boundaries(&mut components);

    Ok(components)
}

fn analyze_directory(dir: &Utf8PathBuf, components: &mut Vec<ComponentInfo>) -> Result<()> {
    for entry in WalkDir::new(dir) {
        let entry = entry.map_err(|e| miette!("Failed to read directory: {}", e))?;
        let path = Utf8PathBuf::from_path_buf(entry.path().to_path_buf())
            .map_err(|_| miette!("Invalid UTF-8 path"))?;

        if path.extension() == Some("tsx") || path.extension() == Some("jsx") {
            if let Ok(component_info) = analyze_component_file(&path) {
                components.push(component_info);
            }
        }
    }

    Ok(())
}

fn analyze_component_file(file_path: &Utf8PathBuf) -> Result<ComponentInfo> {
    let path = BiomePath::new(file_path.clone());
    let content = path
        .read_to_string()
        .map_err(|e| miette!("Failed to read component file: {}", e))?;

    // Parse to AST for context boundary detection
    let parsed = parse(&content, JsFileSource::tsx(), JsParserOptions::default());
    let tree = parsed.tree();
    let syntax = tree.syntax();

    // Detect React context boundaries (Radix UI imports)
    let is_react_context_boundary = detects_react_context_boundary(syntax);
    
    // Extract imports for graph propagation
    let imports = extract_component_imports(syntax);

    let (uses_hooks, uses_browser_apis) = detect_client_features(&content);
    let is_client_side = !uses_hooks.is_empty() || !uses_browser_apis.is_empty() || is_react_context_boundary;

    let name = file_path
        .file_stem()
        .unwrap_or("Unknown")
        .to_string();

    Ok(ComponentInfo {
        name,
        file_path: file_path.clone(),
        is_client_side,
        is_react_context_boundary,
        uses_hooks,
        uses_browser_apis,
        imports,
    })
}

fn detect_client_features(content: &str) -> (Vec<String>, Vec<String>) {
    let mut hooks = Vec::new();
    let mut browser_apis = Vec::new();

    // Common React hooks
    let hook_pattern = Regex::new(r"use(State|Effect|Ref|Callback|Memo|Context|Reducer|LayoutEffect)").unwrap();
    
    // Browser API patterns
    let browser_patterns = vec![
        Regex::new(r"\bwindow\b").unwrap(),
        Regex::new(r"\bdocument\b").unwrap(),
        Regex::new(r"\blocalStorage\b").unwrap(),
        Regex::new(r"\bsessionStorage\b").unwrap(),
        Regex::new(r"\bnavigator\b").unwrap(),
    ];

    // Check for hooks
    for cap in hook_pattern.captures_iter(content) {
        if let Some(m) = cap.get(0) {
            let hook = m.as_str().to_string();
            if !hooks.contains(&hook) {
                hooks.push(hook);
            }
        }
    }

    // Check for browser APIs
    for pattern in &browser_patterns {
        if pattern.is_match(content) {
            let api_name = pattern.as_str().trim_matches('\\').trim_matches('b');
            if !browser_apis.contains(&api_name.to_string()) {
                browser_apis.push(api_name.to_string());
            }
        }
    }

    (hooks, browser_apis)
}

fn parse_dependencies(package_json_path: &Utf8PathBuf) -> Result<HashMap<String, String>> {
    if !package_json_path.exists() {
        return Ok(HashMap::new());
    }

    let path = BiomePath::new(package_json_path.clone());
    let content = path
        .read_to_string()
        .map_err(|e| miette!("Failed to read package.json: {}", e))?;

    let package_json: Value = serde_json::from_str(&content)
        .map_err(|e| miette!("Failed to parse package.json: {}", e))?;

    let mut deps = HashMap::new();

    if let Some(dependencies) = package_json.get("dependencies").and_then(|v| v.as_object()) {
        for (key, value) in dependencies {
            if let Some(version) = value.as_str() {
                deps.insert(key.clone(), version.to_string());
            }
        }
    }

    Ok(deps)
}

/// Detect if a component imports Radix UI or other React context-dependent libraries
fn detects_react_context_boundary(syntax: &JsSyntaxNode) -> bool {
    for node in syntax.descendants() {
        // Check import statements
        if let Some(import_decl) = JsImport::cast(node.clone()) {
            if let Ok(import_clause) = import_decl.import_clause() {
                if let Ok(source) = import_clause.source() {
                    if let Ok(source_text) = source.inner_string_text() {
                        let value = source_text.text();
                        
                        // Check for Radix UI imports
                        if value.contains("@radix-ui/")
                            || value.contains("react-accordion")
                            || value.contains("react-dialog")
                            || value.contains("react-dropdown-menu")
                            || value.contains("react-popover")
                            || value.contains("react-select")
                            || value.contains("react-tabs")
                            || value.contains("react-tooltip")
                            || value.contains("react-toast")
                        {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Extract component names imported from @/components paths
/// Uses regex for reliability (simpler than AST traversal for this use case)
fn extract_component_imports(syntax: &JsSyntaxNode) -> Vec<String> {
    let mut imports = Vec::new();
    
    // Get the source text
    let source_text = syntax.text_trimmed().to_string();
    
    // Pattern for default imports: import Component from '@/components/...'
    let default_pattern = Regex::new(r#"import\s+(\w+)\s+from\s+['"]@/components/([^'"]+)['"]"#).unwrap();
    for cap in default_pattern.captures_iter(&source_text) {
        if let Some(name) = cap.get(1) {
            imports.push(name.as_str().to_string());
        }
    }
    
    // Pattern for named imports: import { Component1, Component2 } from '@/components/...'
    let named_pattern = Regex::new(r#"import\s+\{([^}]+)\}\s+from\s+['"]@/components/([^'"]+)['"]"#).unwrap();
    for cap in named_pattern.captures_iter(&source_text) {
        if let Some(imports_list) = cap.get(1) {
            // Split by comma and extract individual names
            for name in imports_list.as_str().split(',') {
                let trimmed = name.trim();
                // Handle "as" aliases: import { Component as Alias } from '...'
                let actual_name = if let Some(pos) = trimmed.find(" as ") {
                    trimmed[..pos].trim()
                } else {
                    trimmed
                };
                if !actual_name.is_empty() {
                    imports.push(actual_name.to_string());
                }
            }
        }
    }
    
    // Pattern for namespace imports: import * as Something from '@/components/...'
    let namespace_pattern = Regex::new(r#"import\s+\*\s+as\s+(\w+)\s+from\s+['"]@/components/([^'"]+)['"]"#).unwrap();
    for cap in namespace_pattern.captures_iter(&source_text) {
        if let Some(name) = cap.get(1) {
            imports.push(name.as_str().to_string());
        }
    }
    
    imports
}

/// Propagate context boundaries up the import graph
/// If a component imports a context-boundary component, it must also be client-side
fn propagate_context_boundaries(components: &mut Vec<ComponentInfo>) {
    // Iterate until convergence (usually 1-2 passes)
    let mut changed = true;
    while changed {
        changed = false;
        
        // Create a snapshot of context boundaries for this pass
        let context_boundaries: HashMap<String, bool> = components
            .iter()
            .map(|c| (c.name.clone(), c.is_react_context_boundary || c.is_client_side))
            .collect();
        
        for component in components.iter_mut() {
            // Check if this component imports any context-boundary components
            let imports_context_boundary = component.imports.iter().any(|import_name| {
                context_boundaries
                    .get(import_name)
                    .copied()
                    .unwrap_or(false)
            });
            
            if imports_context_boundary && !component.is_client_side {
                component.is_client_side = true;
                changed = true;
            }
        }
    }
}
