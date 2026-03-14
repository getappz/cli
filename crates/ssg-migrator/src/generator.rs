use crate::types::{MigrationConfig, ProjectAnalysis};
use biome_fs::BiomePath;
use biome_js_parser::{parse, JsParserOptions};
use biome_js_syntax::{JsFileSource, JsSyntaxKind, JsSyntaxNode};
use biome_rowan::AstNode;
use camino::Utf8PathBuf;
use miette::{miette, Result};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use walkdir::WalkDir;

/// Final classification result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticSafety {
    Safe,
    Unsafe,
}

/// Why a file is unsafe (for logging / reports)
#[derive(Debug, Clone)]
pub enum UnsafeReason {
    UnresolvedJsxIdentifier(String),
    FunctionScopedIdentifier(String),
    ReactHookUsage(String),
    BrowserApiUsage(String),
}

/// Classifier output
pub struct StaticSafetyResult {
    pub safety: StaticSafety,
    pub reasons: Vec<UnsafeReason>,
}

/// Tracks where an identifier is declared to determine if it's safe for static conversion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclaredKind {
    /// Imported identifier (always safe for Astro templates)
    Import,
    /// Module-level constant (safe for Astro templates)
    ModuleConst,
    /// Function-scoped variable (unsafe - requires React runtime)
    FunctionScoped,
}

/// Production-grade classifier for determining if a React component can be safely converted to Astro
pub struct StaticSafetyClassifier {
    declared: HashMap<String, DeclaredKind>,
    jsx_used: std::collections::HashSet<String>,
    reasons: Vec<UnsafeReason>,
}

impl StaticSafetyClassifier {
    /// Classify a component's static safety based on its AST
    pub fn classify(root: &JsSyntaxNode) -> StaticSafetyResult {
        let mut classifier = Self {
            declared: HashMap::new(),
            jsx_used: std::collections::HashSet::new(),
            reasons: Vec::new(),
        };

        classifier.collect_declarations(root);
        classifier.collect_jsx_usages(root);
        classifier.evaluate();

        StaticSafetyResult {
            safety: if classifier.reasons.is_empty() {
                StaticSafety::Safe
            } else {
                StaticSafety::Unsafe
            },
            reasons: classifier.reasons,
        }
    }

    /// Collect all declared identifiers with their declaration kind
    fn collect_declarations(&mut self, root: &JsSyntaxNode) {
        for node in root.descendants() {
            match node.kind() {
                // import foo from 'x'
                JsSyntaxKind::JS_IMPORT_DEFAULT_CLAUSE
                | JsSyntaxKind::JS_IMPORT_NAMESPACE_CLAUSE => {
                    if let Some(id) = node.first_token() {
                        let name = id.text_trimmed();
                        if !name.is_empty() {
                            self.declared.insert(name.to_string(), DeclaredKind::Import);
                        }
                    }
                }

                // import { foo } from 'x'
                kind if kind.to_string().map(|s| s.contains("IMPORT") && s.contains("SPECIFIER")).unwrap_or(false) => {
                    for child in node.descendants() {
                        if child.kind() == JsSyntaxKind::JS_REFERENCE_IDENTIFIER {
                            let name = child.text_trimmed();
                            if !name.is_empty() {
                                self.declared.insert(name.to_string(), DeclaredKind::Import);
                            }
                        }
                    }
                }

                // const foo = ...
                JsSyntaxKind::JS_VARIABLE_DECLARATOR => {
                    if let Some(binding) = node.children().find(|c| c.kind() == JsSyntaxKind::JS_IDENTIFIER_BINDING) {
                        let name = binding.text_trimmed();
                        if !name.is_empty() {
                            let kind = if is_inside_function(&node) {
                                DeclaredKind::FunctionScoped
                            } else {
                                DeclaredKind::ModuleConst
                            };
                            self.declared.insert(name.to_string(), kind);
                        }
                    }
                }

                // function foo() {}
                JsSyntaxKind::JS_FUNCTION_DECLARATION => {
                    if let Some(id) = node.children().find(|c| c.kind() == JsSyntaxKind::JS_IDENTIFIER_BINDING) {
                        let name = id.text_trimmed();
                        if !name.is_empty() {
                            self.declared.insert(name.to_string(), DeclaredKind::ModuleConst);
                        }
                    }
                }

                // type Foo = ...
                JsSyntaxKind::TS_TYPE_ALIAS_DECLARATION |
                JsSyntaxKind::TS_INTERFACE_DECLARATION => {
                    if let Some(id) = node.children().find(|c| c.kind() == JsSyntaxKind::TS_IDENTIFIER_BINDING) {
                        let name = id.text_trimmed();
                        if !name.is_empty() {
                            self.declared.insert(name.to_string(), DeclaredKind::ModuleConst);
                        }
                    }
                }

                _ => {}
            }
        }
    }

    /// Collect identifiers used inside JSX expressions
    fn collect_jsx_usages(&mut self, root: &JsSyntaxNode) {
        for node in root.descendants() {
            if node.kind() == JsSyntaxKind::JS_REFERENCE_IDENTIFIER && is_inside_jsx(&node) {
                let name = node.text_trimmed();

                // Ignore Astro.props, props, etc
                if name == "Astro" || name == "props" {
                    continue;
                }

                if !name.is_empty() {
                    self.jsx_used.insert(name.to_string());
                }
            }
        }
    }

    /// Evaluate safety based on collected declarations and usages
    fn evaluate(&mut self) {
        // First, check for React context-dependent components
        self.check_react_context_dependencies();
        
        for ident in &self.jsx_used {
            match self.declared.get(ident) {
                // Imported identifiers are always safe (unless they're context-dependent components)
                Some(DeclaredKind::Import) => {
                    // Check if this is a known React context-dependent component
                    if self.is_context_dependent_component(ident) {
                        self.reasons.push(UnsafeReason::ReactHookUsage(
                            format!("Component '{}' requires React context", ident)
                        ));
                    }
                }
                // Module-level constants are safe
                Some(DeclaredKind::ModuleConst) => {}
                // Function-scoped variables are unsafe
                Some(DeclaredKind::FunctionScoped) => {
                    self.reasons.push(UnsafeReason::FunctionScopedIdentifier(ident.clone()));
                }
                // Undeclared identifiers are unsafe
                None => {
                    self.reasons.push(UnsafeReason::UnresolvedJsxIdentifier(ident.clone()));
                }
            }
        }
    }
    
    /// Check for React context dependencies that require React runtime
    /// This includes Radix UI components that use React context
    fn check_react_context_dependencies(&mut self) {
        // Components that require React context (Radix UI, etc.)
        let context_dependent_components = vec![
            "Accordion", "AccordionItem", "AccordionTrigger", "AccordionContent",
            "Dialog", "DialogTrigger", "DialogContent",
            "DropdownMenu", "DropdownMenuTrigger", "DropdownMenuContent",
            "Popover", "PopoverTrigger", "PopoverContent",
            "Select", "SelectTrigger", "SelectContent",
            "Tabs", "TabsList", "TabsTrigger", "TabsContent",
            "Tooltip", "TooltipTrigger", "TooltipContent",
            "Toast", "ToastProvider",
        ];
        
        // Check if any of these components are used in JSX
        for component in context_dependent_components {
            if self.jsx_used.contains(component) {
                self.reasons.push(UnsafeReason::ReactHookUsage(
                    format!("Component '{}' requires React context", component)
                ));
            }
        }
    }
    
    /// Check if a component name is a known React context-dependent component
    fn is_context_dependent_component(&self, name: &str) -> bool {
        let context_dependent_components = vec![
            "Accordion", "AccordionItem", "AccordionTrigger", "AccordionContent",
            "Dialog", "DialogTrigger", "DialogContent",
            "DropdownMenu", "DropdownMenuTrigger", "DropdownMenuContent",
            "Popover", "PopoverTrigger", "PopoverContent",
            "Select", "SelectTrigger", "SelectContent",
            "Tabs", "TabsList", "TabsTrigger", "TabsContent",
            "Tooltip", "TooltipTrigger", "TooltipContent",
            "Toast", "ToastProvider",
        ];
        context_dependent_components.contains(&name)
    }
    
    /// Check if a node is a Radix UI import
    fn is_radix_ui_import(&self, node: &JsSyntaxNode) -> bool {
        // Look for import source that contains @radix-ui
        for descendant in node.descendants() {
            if descendant.kind() == JsSyntaxKind::JS_STRING_LITERAL_EXPRESSION ||
               descendant.kind() == JsSyntaxKind::JS_STRING_LITERAL {
                let text = descendant.text_trimmed().to_string();
                if text.contains("@radix-ui") {
                    return true;
                }
            }
        }
        false
    }
    
    /// Check if an import node requires React context
    fn is_react_context_import(&self, node: &JsSyntaxNode) -> bool {
        // Look for import source
        for descendant in node.descendants() {
            if descendant.kind() == JsSyntaxKind::JS_STRING_LITERAL_EXPRESSION ||
               descendant.kind() == JsSyntaxKind::JS_STRING_LITERAL {
                let text = descendant.text_trimmed().to_string();
                // Check for Radix UI or other context-dependent libraries
                if text.contains("@radix-ui") {
                    return true;
                }
            }
        }
        false
    }
}

/// Check if a node is inside a JSX context
fn is_inside_jsx(node: &JsSyntaxNode) -> bool {
    let mut parent = node.parent();
    while let Some(p) = parent {
        if let Some(kind_str) = p.kind().to_string() {
            if kind_str.starts_with("JSX_") {
                return true;
            }
        }
        parent = p.parent();
    }
    false
}

pub fn generate_astro_project(config: &MigrationConfig, analysis: &ProjectAnalysis) -> Result<()> {
    // Create output directory
    if config.output_dir.exists() && !config.force {
        return Err(miette!(
            "Output directory already exists. Use --force to overwrite."
        ));
    }

    if config.output_dir.exists() && config.force {
        fs::remove_dir_all(&config.output_dir)
            .map_err(|e| miette!("Failed to remove existing directory: {}", e))?;
    }

    fs::create_dir_all(&config.output_dir)
        .map_err(|e| miette!("Failed to create output directory: {}", e))?;

    // Generate directory structure
    let src_dir = config.output_dir.join("src");
    let pages_dir = src_dir.join("pages");
    let components_dir = src_dir.join("components");
    let layouts_dir = src_dir.join("layouts");
    let public_dir = config.output_dir.join("public");

    fs::create_dir_all(&pages_dir)
        .map_err(|e| miette!("Failed to create pages directory: {}", e))?;
    fs::create_dir_all(&components_dir)
        .map_err(|e| miette!("Failed to create components directory: {}", e))?;
    fs::create_dir_all(&layouts_dir)
        .map_err(|e| miette!("Failed to create layouts directory: {}", e))?;
    fs::create_dir_all(&public_dir)
        .map_err(|e| miette!("Failed to create public directory: {}", e))?;

    // Generate astro.config.mjs
    generate_astro_config(&config.output_dir, analysis)?;

    // Generate package.json
    generate_package_json(&config.output_dir, analysis)?;

    // Generate tsconfig.json
    generate_tsconfig(&config.output_dir, &analysis)?;

    // Copy tailwind config if exists
    if analysis.has_tailwind {
        copy_tailwind_config(&analysis.source_dir, &config.output_dir)?;
    }

    // Copy public assets
    copy_public_assets(&analysis.source_dir, &public_dir)?;

    // Copy lib directory (for utilities like utils.ts)
    let lib_dir = src_dir.join("lib");
    let source_lib_dir = analysis.source_dir.join("src/lib");
    if source_lib_dir.exists() {
        fs::create_dir_all(&lib_dir)
            .map_err(|e| miette!("Failed to create lib directory: {}", e))?;
        copy_dir_all(&source_lib_dir, &lib_dir)?;
    }

    // Copy assets directory
    let assets_dir = src_dir.join("assets");
    let source_assets_dir = analysis.source_dir.join("src/assets");
    if source_assets_dir.exists() {
        copy_dir_all(&source_assets_dir, &assets_dir)?;
    }

    // Copy CSS files from src/ (index.css, App.css, etc.)
    copy_css_files(&analysis.source_dir.join("src"), &src_dir)?;

    // Generate layout
    generate_layout(&layouts_dir)?;

    // Copy/generate components first (pages may depend on them)
    generate_components(&components_dir, analysis)?;

    // Generate pages from routes
    generate_pages(&pages_dir, analysis)?;

    // Generate README
    generate_readme(&config.output_dir, &config.project_name)?;

    Ok(())
}

fn generate_astro_config(output_dir: &Utf8PathBuf, analysis: &ProjectAnalysis) -> Result<()> {
    let config_path = output_dir.join("astro.config.mjs");
    let mut file = fs::File::create(&config_path)
        .map_err(|e| miette!("Failed to create astro.config.mjs: {}", e))?;

    let mut config = r#"import { defineConfig } from 'astro/config';
import react from '@astrojs/react';
"#.to_string();

    if analysis.has_tailwind {
        config.push_str("import tailwind from '@astrojs/tailwind';\n");
    }

    config.push_str("\nexport default defineConfig({\n");
    config.push_str("  integrations: [\n");
    config.push_str("    react(),\n");
    if analysis.has_tailwind {
        config.push_str("    tailwind(),\n");
    }
    config.push_str("  ],\n");
    config.push_str("  output: 'static',\n");
    config.push_str("  vite: {\n");
    config.push_str("    resolve: {\n");
    config.push_str("      alias: {\n");
    config.push_str("        '@': new URL('./src', import.meta.url).pathname,\n");
    config.push_str("      },\n");
    config.push_str("    },\n");
    config.push_str("  },\n");
    config.push_str("});\n");

    file.write_all(config.as_bytes())
        .map_err(|e| miette!("Failed to write astro.config.mjs: {}", e))?;

    Ok(())
}

fn generate_package_json(output_dir: &Utf8PathBuf, analysis: &ProjectAnalysis) -> Result<()> {
    use serde_json::{json, Map};

    let mut dependencies = Map::new();
    
    // Core Astro dependencies
    dependencies.insert("astro".to_string(), json!("^4.0.0"));
    dependencies.insert("@astrojs/react".to_string(), json!("^3.0.0"));
    
    if analysis.has_tailwind {
        dependencies.insert("@astrojs/tailwind".to_string(), json!("^5.0.0"));
        dependencies.insert("tailwindcss".to_string(), json!("^3.4.0"));
        // Check if tailwindcss-animate is in original dependencies
        if let Some(version) = analysis.dependencies.get("tailwindcss-animate") {
            dependencies.insert("tailwindcss-animate".to_string(), json!(version));
        } else {
            dependencies.insert("tailwindcss-animate".to_string(), json!("^1.0.7"));
        }
        // Check for tailwind-merge
        if let Some(version) = analysis.dependencies.get("tailwind-merge") {
            dependencies.insert("tailwind-merge".to_string(), json!(version));
        }
    }

    // Keep React and React DOM for client components
    dependencies.insert("react".to_string(), json!("^18.3.0"));
    dependencies.insert("react-dom".to_string(), json!("^18.3.0"));

    // Keep UI libraries and common utilities
    for (dep, version) in &analysis.dependencies {
        // Skip if already added
        if dependencies.contains_key(dep) {
            continue;
        }
        if !dep.starts_with("vite")
            && dep != "react"
            && dep != "react-dom"
            && dep != "tailwindcss" // Already added above
            && dep != "tailwindcss-animate" // Already added above if tailwind exists
            && dep != "tailwind-merge" // Already added above if tailwind exists
            && !dep.starts_with("@vitejs")
            && !dep.starts_with("react-router") // Not needed in Astro
        {
            // Include Radix UI, Lucide, and common utility libraries
            if dep.starts_with("@radix-ui")
                || dep.starts_with("lucide-react")
                || dep == "clsx"
                || dep == "class-variance-authority"
                || dep == "cmdk"
                || dep == "date-fns"
                || dep == "zod"
                || dep == "sonner"
                || dep == "embla-carousel-react"
                || dep == "input-otp"
                || dep == "vaul"
                || dep == "next-themes"
            {
                dependencies.insert(dep.clone(), json!(version));
            }
        }
    }

    let mut dev_dependencies = Map::new();
    dev_dependencies.insert("@types/react".to_string(), json!("^18.3.0"));
    dev_dependencies.insert("@types/react-dom".to_string(), json!("^18.3.0"));
    dev_dependencies.insert("typescript".to_string(), json!("^5.0.0"));

    let package_json = json!({
        "name": "migrated-astro-app",
        "type": "module",
        "version": "0.0.1",
        "scripts": {
            "dev": "astro dev",
            "start": "astro dev",
            "build": "astro build",
            "preview": "astro preview"
        },
        "dependencies": dependencies,
        "devDependencies": dev_dependencies
    });

    let package_path = output_dir.join("package.json");
    let mut file = fs::File::create(&package_path)
        .map_err(|e| miette!("Failed to create package.json: {}", e))?;

    let formatted = serde_json::to_string_pretty(&package_json)
        .map_err(|e| miette!("Failed to serialize package.json: {}", e))?;

    file.write_all(formatted.as_bytes())
        .map_err(|e| miette!("Failed to write package.json: {}", e))?;

    Ok(())
}

fn generate_tsconfig(output_dir: &Utf8PathBuf, analysis: &ProjectAnalysis) -> Result<()> {
    let tsconfig_path = output_dir.join("tsconfig.json");
    
    // Build client-required components set to create path aliases
    let client_required = collect_client_required_components(analysis);
    
    // Build path aliases - we don't need component-specific aliases anymore
    // since we're updating import paths directly to use react/ directory
    let path_aliases = vec!["\"@/*\": [\"./src/*\"]".to_string()];
    
    let paths_json = path_aliases.join(",\n      ");
    
    let tsconfig = format!(
        r#"{{
  "extends": "astro/tsconfigs/strict",
  "compilerOptions": {{
    "baseUrl": ".",
    "paths": {{
      {}
    }}
  }}
}}
"#,
        paths_json
    );

    let mut file = fs::File::create(&tsconfig_path)
        .map_err(|e| miette!("Failed to create tsconfig.json: {}", e))?;
    file.write_all(tsconfig.as_bytes())
        .map_err(|e| miette!("Failed to write tsconfig.json: {}", e))?;

    Ok(())
}

fn copy_tailwind_config(source_dir: &Utf8PathBuf, output_dir: &Utf8PathBuf) -> Result<()> {
    let source_config = source_dir.join("tailwind.config.ts");
    if source_config.exists() {
        let dest_config = output_dir.join("tailwind.config.ts");
        fs::copy(&source_config, &dest_config)
            .map_err(|e| miette!("Failed to copy tailwind.config.ts: {}", e))?;
    }
    // Also check for .js extension
    let source_config_js = source_dir.join("tailwind.config.js");
    if source_config_js.exists() {
        let dest_config = output_dir.join("tailwind.config.js");
        fs::copy(&source_config_js, &dest_config)
            .map_err(|e| miette!("Failed to copy tailwind.config.js: {}", e))?;
    }
    Ok(())
}

fn copy_public_assets(source_dir: &Utf8PathBuf, public_dir: &Utf8PathBuf) -> Result<()> {
    let source_public = source_dir.join("public");
    if source_public.exists() {
        copy_dir_all(&source_public, public_dir)?;
    }
    Ok(())
}

fn copy_css_files(source_src_dir: &Utf8PathBuf, dest_src_dir: &Utf8PathBuf) -> Result<()> {
    // Copy CSS files from src/ root (index.css, App.css, etc.)
    for entry in fs::read_dir(source_src_dir)
        .map_err(|e| miette!("Failed to read src directory: {}", e))? {
        let entry = entry.map_err(|e| miette!("Failed to read entry: {}", e))?;
        let path = entry.path();
        
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "css" {
                    let file_name = path.file_name()
                        .and_then(|n| n.to_str())
                        .ok_or_else(|| miette!("Invalid file name"))?;
                    let dest_path = dest_src_dir.join(file_name);
                    fs::copy(&path, &dest_path)
                        .map_err(|e| miette!("Failed to copy CSS file {}: {}", file_name, e))?;
                }
            }
        }
    }
    Ok(())
}

fn copy_dir_all(src: &Utf8PathBuf, dst: &Utf8PathBuf) -> Result<()> {
    fs::create_dir_all(dst)
        .map_err(|e| miette!("Failed to create directory: {}", e))?;
    for entry in fs::read_dir(src)
        .map_err(|e| miette!("Failed to read directory: {}", e))? {
        let entry = entry.map_err(|e| miette!("Failed to read entry: {}", e))?;
        let path = entry.path();
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| miette!("Invalid file name"))?;
        let dst_path = dst.join(file_name);

        if path.is_dir() {
            copy_dir_all(
                &Utf8PathBuf::from_path_buf(path).map_err(|_| miette!("Invalid path"))?,
                &dst_path,
            )?;
        } else {
            fs::copy(&path, &dst_path)
                .map_err(|e| miette!("Failed to copy file: {}", e))?;
        }
    }
    Ok(())
}

fn generate_layout(layouts_dir: &Utf8PathBuf) -> Result<()> {
    let layout_path = layouts_dir.join("Layout.astro");
    let mut file = fs::File::create(&layout_path)
        .map_err(|e| miette!("Failed to create Layout.astro: {}", e))?;

    // Check if CSS files exist to import them
    let src_dir = layouts_dir.parent().ok_or_else(|| miette!("Invalid layout directory"))?;
    let mut css_imports = String::new();
    
    // Check for common CSS files
    let css_files = vec!["index.css", "App.css"];
    for css_file in css_files {
        let css_path = src_dir.join(css_file);
        if css_path.exists() {
            css_imports.push_str(&format!("import '../{}';\n", css_file));
        }
    }

    let layout = format!(r#"---
interface Props {{
  title?: string;
}}
const {{ title = "Migrated Astro App" }} = Astro.props;
{}
---
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{{title}}</title>
  </head>
  <body>
    <slot />
  </body>
</html>
"#, css_imports);

    file.write_all(layout.as_bytes())
        .map_err(|e| miette!("Failed to write Layout.astro: {}", e))?;

    Ok(())
}

fn generate_pages(pages_dir: &Utf8PathBuf, analysis: &ProjectAnalysis) -> Result<()> {
    for route in &analysis.routes {
        let page_name: String = if route.path == "/" {
            "index".to_string()
        } else if route.is_catch_all {
            "404".to_string()
        } else {
            route.path.trim_start_matches('/').replace('/', "-")
        };

        // Find the actual page component file
        let source_page_path = analysis.source_dir.join("src/pages").join(format!("{}.tsx", route.component));
        let source_page_path_ts = analysis.source_dir.join("src/pages").join(format!("{}.ts", route.component));
        
        let page_path = pages_dir.join(format!("{}.astro", page_name));
        
        // Check if the page component exists in source
        if source_page_path.exists() || source_page_path_ts.exists() {
            let actual_path = if source_page_path.exists() { &source_page_path } else { &source_page_path_ts };
            
            // Check if this page component uses React hooks (is client-side)
            let is_client_side = analysis.components.iter()
                .any(|c| c.file_path == *actual_path && c.is_client_side);
            
            if is_client_side {
                // Keep as React component - it should already be copied to components/ui
                // Create the page that imports the React component
                let page_content = format!(
                    r#"---
import Layout from '../layouts/Layout.astro';
import {} from '../components/react/{}.tsx';
---
<Layout>
  <{} client:load />
</Layout>
"#,
                    route.component, route.component, route.component
                );
                
                let mut file = fs::File::create(&page_path)
                    .map_err(|e| miette!("Failed to create page: {}", e))?;
                
                file.write_all(page_content.as_bytes())
                    .map_err(|e| miette!("Failed to write page: {}", e))?;
            } else {
                // Convert to Astro page
                let source_path = BiomePath::new(actual_path.clone());
                let content = source_path
                    .read_to_string()
                    .map_err(|e| miette!("Failed to read page component: {}", e))?;
                
                // Transform the page component to Astro
                let astro_content = transform_page_to_astro(&content, &route.component, analysis);
                
                let mut file = fs::File::create(&page_path)
                    .map_err(|e| miette!("Failed to create page: {}", e))?;
                
                file.write_all(astro_content.as_bytes())
                    .map_err(|e| miette!("Failed to write page: {}", e))?;
            }
        } else {
            // Fallback: create a simple page if component not found
            let page_content = format!(
                r#"---
import Layout from '../layouts/Layout.astro';
---
<Layout>
  <h1>{}</h1>
  <p>Page content migrated from React component</p>
</Layout>
"#,
                route.component
            );

            let mut file = fs::File::create(&page_path)
                .map_err(|e| miette!("Failed to create page: {}", e))?;

            file.write_all(page_content.as_bytes())
                .map_err(|e| miette!("Failed to write page: {}", e))?;
        }
    }

    Ok(())
}

/// Check if a node is inside a function (component function or any function)
fn is_inside_function(node: &JsSyntaxNode) -> bool {
    let mut parent = node.parent();
    while let Some(p) = parent {
        match p.kind() {
            JsSyntaxKind::JS_FUNCTION_DECLARATION |
            JsSyntaxKind::JS_FUNCTION_EXPRESSION |
            JsSyntaxKind::JS_ARROW_FUNCTION_EXPRESSION => {
                return true;
            }
            _ => {
                parent = p.parent();
            }
        }
    }
    false
}

/// Generate a client-only page when static conversion is unsafe
fn generate_client_only_page(component_name: &str) -> String {
    format!(
        r#"---
import Layout from '../layouts/Layout.astro';
import {} from '../components/react/{}.tsx';
---
<Layout>
  <{} client:load />
</Layout>
"#,
        component_name, component_name, component_name
    )
}

fn transform_page_to_astro(content: &str, component_name: &str, analysis: &ProjectAnalysis) -> String {
    // Parse with Biome to check for unresolved identifiers
    let parsed = parse(
        content,
        JsFileSource::tsx(),
        JsParserOptions::default(),
    );

    // If parsing fails, bail out to client-only
    if parsed.has_errors() {
        return generate_client_only_page(component_name);
    }

    let tree = parsed.tree();
    let syntax = tree.syntax();

    // 🚨 CRITICAL GUARDRAIL: Classify static safety
    let result = StaticSafetyClassifier::classify(syntax);
    
    if result.safety == StaticSafety::Unsafe {
        // Unsafe to convert statically - use client-only React component
        // Optionally log reasons for debugging:
        // eprintln!("Unsafe to convert {}: {:?}", component_name, result.reasons);
        return generate_client_only_page(component_name);
    }

    // ✅ Safe to continue static conversion
    transform_page_to_astro_static(content, component_name, analysis)
}

fn transform_page_to_astro_static(content: &str, _component_name: &str, analysis: &ProjectAnalysis) -> String {
    // Transform following official Astro migration guide:
    // https://docs.astro.build/en/guides/migrate-to-astro/from-create-react-app/#converting-jsx-files-to-astro-files
    // Key conversions:
    // 1. Use returned JSX as HTML template
    // 2. Move JavaScript/imports into code fence (---)
    // 3. Convert className to class
    // 4. Convert {children} to <slot />
    // 5. Convert inline style objects to HTML style attributes
    // 6. Use client directives for React components
    let mut result = String::from("---\n");
    result.push_str("import Layout from '../layouts/Layout.astro';\n");
    
    // Extract imports and convert them
    // Handle both default imports: import X from 'path'
    // and named imports: import { X, Y, Z } from 'path'
    let default_import_pattern = Regex::new(r#"import\s+(\w+)\s+from\s+['"]([^'"]+)['"]"#).unwrap();
    let named_import_pattern = Regex::new(r#"import\s+\{([^}]+)\}\s+from\s+['"]([^'"]+)['"]"#).unwrap();
    let mut component_imports = Vec::new();
    let mut component_usage = Vec::new();
    
    // Handle default imports
    for cap in default_import_pattern.captures_iter(content) {
        if let (Some(component_name), Some(import_path)) = (cap.get(1), cap.get(2)) {
            let comp_name = component_name.as_str();
            let path = import_path.as_str();
            
            // Skip React Router imports
            if path == "react-router-dom" {
                continue;
            }
            
            // Handle component imports from @/components
            if path.starts_with("@/components/") {
                let comp_path = path.strip_prefix("@/components/").unwrap();
                let comp_name_str = comp_name.to_string();
                
                // Check if this component is client-side
                let comp_name_clean = comp_path.trim_end_matches(".tsx").trim_end_matches(".ts");
                
                // Known React components that are always client-side (even if they don't use hooks)
                let known_react_components = vec!["Navbar", "Footer", "WhatsAppButton"];
                let is_known_react = known_react_components.contains(&comp_name_clean);
                
                let is_client_side = is_known_react || analysis.components.iter()
                    .any(|c| {
                        // Match by component name (which is the file stem)
                        (c.name == comp_name_clean || c.name == comp_name) && c.is_client_side
                    });
                
                if is_client_side {
                    // Client-side component - import from react directory
                    component_imports.push(format!("import {} from '../components/react/{}.tsx';", comp_name_str, comp_path.trim_end_matches(".tsx").trim_end_matches(".ts")));
                    component_usage.push((comp_name_str.clone(), true));
                } else {
                    // Static component - import as Astro component
                    component_imports.push(format!("import {} from '../components/{}.astro';", comp_name_str, comp_path.trim_end_matches(".tsx").trim_end_matches(".ts")));
                    component_usage.push((comp_name_str.clone(), false));
                }
            } else if path.starts_with("@/") {
                // Other @/ imports (lib, assets, etc.)
                let adjusted = path.replace("@/", "../../");
                component_imports.push(format!("import {} from '{}';", comp_name, adjusted));
            } else if !path.starts_with("react") && !path.starts_with(".") {
                // External imports (lucide-react, etc.) - but these are default imports, skip for now
                // Named imports from external packages are handled below
            }
        }
    }
    
    // Handle named imports (e.g., import { Sun, CheckCircle } from 'lucide-react')
    for cap in named_import_pattern.captures_iter(content) {
        if let (Some(imports_list), Some(import_path)) = (cap.get(1), cap.get(2)) {
            let path = import_path.as_str();
            
            // Skip React Router imports
            if path == "react-router-dom" {
                continue;
            }
            
            // Extract individual import names from the list
            let imports: Vec<&str> = imports_list.as_str()
                .split(',')
                .map(|s| s.trim())
                .collect();
            
            if !path.starts_with("react") && !path.starts_with(".") && !path.starts_with("@/") {
                // External named imports (lucide-react, etc.)
                let imports_str = imports.join(", ");
                component_imports.push(format!("import {{ {} }} from '{}';", imports_str, path));
            } else if path.starts_with("@/components/ui/") {
                // Named imports from @/components/ui (like Button from @/components/ui/button)
                let comp_path = path.strip_prefix("@/components/ui/").unwrap();
                let adjusted = format!("../components/ui/{}", comp_path);
                let imports_str = imports.join(", ");
                component_imports.push(format!("import {{ {} }} from '{}';", imports_str, adjusted));
            } else if path.starts_with("@/") {
                // Named imports from other @/ paths
                let adjusted = path.replace("@/", "../../");
                let imports_str = imports.join(", ");
                component_imports.push(format!("import {{ {} }} from '{}';", imports_str, adjusted));
            }
        }
    }
    
    for import in component_imports {
        result.push_str(&format!("{}\n", import));
    }
    
    // Extract constants and data structures defined before the component function
    // Find where the component function starts (const ComponentName = () => or function ComponentName()
    let component_start_pattern = Regex::new(r#"(?m)^(const\s+\w+\s*=\s*\(\)\s*=>|function\s+\w+\s*\()"#).unwrap();
    
    // Find the last import statement
    let last_import_pattern = Regex::new(r#"(?m)^import\s+.*?from\s+['"][^'"]+['"];?\s*$"#).unwrap();
    let mut last_import_end = 0;
    for cap in last_import_pattern.find_iter(content) {
        last_import_end = cap.end();
    }
    
    // Find where the component function starts
    let component_start = if let Some(m) = component_start_pattern.find(content) {
        m.start()
    } else {
        content.len()
    };
    
    // Extract everything between the last import and the component function
    // This includes all constants, type definitions, etc.
    if component_start > last_import_end {
        let constants_section = &content[last_import_end..component_start].trim();
        if !constants_section.is_empty() {
            result.push_str("\n");
            result.push_str(constants_section);
            result.push_str("\n");
        }
    }
    
    result.push_str("---\n\n");
    result.push_str("<Layout>\n");
    
    // Extract JSX content and convert component usage
    // Following official Astro migration guide: https://docs.astro.build/en/guides/migrate-to-astro/from-create-react-app/
    let mut converted = content.to_string();
    
    // 1. Convert className to class (Astro uses HTML standard)
    converted = converted.replace("className", "class");
    
    // 2. Convert {children} to <slot /> (Astro uses slots instead of children prop)
    // Handle both {children} and {props.children}
    let children_pattern = Regex::new(r#"\{(?:props\.)?children\}"#).unwrap();
    converted = children_pattern.replace_all(&converted, "<slot />").to_string();
    
    // 3. Convert inline style objects to HTML style attributes
    // Pattern: style={{ key: "value", key2: "value2" }}
    // Convert to: style="key:value;key2:value2"
    let style_object_pattern = Regex::new(r#"style=\{\{\s*([^}]+)\s*\}\}"#).unwrap();
    converted = style_object_pattern.replace_all(&converted, |caps: &regex::Captures| {
        if let Some(style_content) = caps.get(1) {
            let style_str = style_content.as_str();
            // Parse key-value pairs and convert to CSS string
            let kv_pattern = Regex::new(r#"(\w+):\s*['"]?([^,'"]+)['"]?"#).unwrap();
            let mut css_parts = Vec::new();
            for kv_cap in kv_pattern.captures_iter(style_str) {
                if let (Some(key), Some(value)) = (kv_cap.get(1), kv_cap.get(2)) {
                    // Convert camelCase to kebab-case
                    let css_key = key.as_str()
                        .chars()
                        .enumerate()
                        .flat_map(|(i, c)| {
                            if c.is_uppercase() && i > 0 {
                                vec!['-', c.to_lowercase().next().unwrap()]
                            } else {
                                vec![c.to_lowercase().next().unwrap_or(c)]
                            }
                        })
                        .collect::<String>();
                    css_parts.push(format!("{}:{}", css_key.trim(), value.as_str().trim()));
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
    }).to_string();
    
    // Remove react-router-dom imports (already handled above, but keep for safety)
    converted = converted.replace("react-router-dom", "");
    
    // Replace Link components from react-router-dom with <a> tags
    let link_pattern = Regex::new(r#"<Link\s+to=["']([^"']+)["']([^>]*)>"#).unwrap();
    converted = link_pattern.replace_all(&converted, |caps: &regex::Captures| {
        let href = caps.get(1).unwrap().as_str();
        let attrs = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        format!("<a href=\"{}\"{}>", href, attrs)
    }).to_string();
    converted = converted.replace("</Link>", "</a>");
    
    // Extract JSX return statement (following Astro guide: use returned JSX as HTML template)
    let mut jsx_content = if let Some(return_start) = converted.find("return (") {
        // Handle return with parentheses: return (<div>...</div>)
        if let Some(return_end) = converted[return_start..].find(");") {
            converted[return_start + 8..return_start + return_end].trim().to_string()
        } else {
            converted
        }
    } else if let Some(return_start) = converted.find("return ") {
        // Handle return without parentheses: return <div>...</div>
        // Find the matching closing brace or tag
        let after_return = &converted[return_start + 7..];
        if let Some(return_end) = after_return.find("\n  };") {
            after_return[..return_end].trim().to_string()
        } else {
            after_return.trim().to_string()
        }
    } else {
        converted
    };
    
    // Replace component tags with client directives for client-side components
    // Following Astro guide: use client:load for immediate interactivity
    for (comp_name, is_client_side) in &component_usage {
        if *is_client_side {
            // Replace <Component /> with <Component client:load />
            let pattern = format!(r#"<{}\s*/>"#, comp_name);
            let replacement = format!(r#"<{} client:load />"#, comp_name);
            jsx_content = Regex::new(&pattern).unwrap()
                .replace_all(&jsx_content, &replacement)
                .to_string();
            
            // Also handle <Component>...</Component>
            let pattern_open = format!(r#"<{}\s*>"#, comp_name);
            let replacement_open = format!(r#"<{} client:load>"#, comp_name);
            jsx_content = Regex::new(&pattern_open).unwrap()
                .replace_all(&jsx_content, &replacement_open)
                .to_string();
        }
    }
    
    // Clean up React-specific syntax
    // Remove empty fragments: <>...</> or <React.Fragment>...</React.Fragment>
    jsx_content = Regex::new(r#"<>\s*</>"#).unwrap().replace_all(&jsx_content, "").to_string();
    jsx_content = Regex::new(r#"<React\.Fragment>\s*</React\.Fragment>"#).unwrap()
        .replace_all(&jsx_content, "")
        .to_string();
    
    // Convert JSX comments to HTML comments
    // Pattern: {/* comment */}
    let jsx_comment_pattern = Regex::new(r#"/\*([^*]|\*[^/])*\*/"#).unwrap();
    jsx_content = jsx_comment_pattern.replace_all(&jsx_content, |caps: &regex::Captures| {
        let comment_text = caps.get(0).unwrap().as_str();
        // Extract comment content (remove /* and */)
        let content = comment_text
            .strip_prefix("/*")
            .and_then(|s| s.strip_suffix("*/"))
            .unwrap_or(comment_text)
            .trim();
        format!("<!-- {} -->", content)
    }).to_string();
    
    // Fix image src attributes: src={imageVar} -> src={imageVar.src}
    // In React, imported images can be used directly, but Astro needs .src property
    // Pattern: src={variableName} where variableName is an imported image
    let src_expression_pattern = Regex::new(r#"src=\{(?:([a-zA-Z_$][a-zA-Z0-9_$]*)|([a-zA-Z_$][a-zA-Z0-9_$]*\.[a-zA-Z0-9_$]+))\}"#).unwrap();
    jsx_content = src_expression_pattern.replace_all(&jsx_content, |caps: &regex::Captures| {
        if let Some(simple_var) = caps.get(1) {
            // Simple variable: src={imageVar} -> src={imageVar.src}
            let var_name = simple_var.as_str();
            format!("src={{{}.src}}", var_name)
        } else if let Some(prop_access) = caps.get(2) {
            // Already has property access: src={obj.prop} -> keep as-is
            format!("src={{{}}}", prop_access.as_str())
        } else {
            caps.get(0).unwrap().as_str().to_string()
        }
    }).to_string();
    
    result.push_str("  ");
    result.push_str(&jsx_content);
    result.push_str("\n");
    
    result.push_str("</Layout>\n");
    result
}

/// Transform a React component to an Astro component (without Layout wrapper)
fn transform_component_to_astro(content: &str, _component_name: &str, analysis: &ProjectAnalysis) -> String {
    // Parse with Biome to check for unresolved identifiers
    let parsed = parse(
        content,
        JsFileSource::tsx(),
        JsParserOptions::default(),
    );

    // If parsing fails, bail out to client-only
    if parsed.has_errors() {
        // For components, we can't use client-only fallback easily, so just return the content
        // The component will need to be kept as React
        return content.to_string();
    }

    let tree = parsed.tree();
    let syntax = tree.syntax();

    // 🚨 CRITICAL GUARDRAIL: Classify static safety
    let result = StaticSafetyClassifier::classify(syntax);
    
    if result.safety == StaticSafety::Unsafe {
        // Unsafe to convert statically - keep as React component
        return content.to_string();
    }

    // ✅ Safe to continue static conversion
    transform_component_to_astro_static(content, _component_name, analysis)
}

fn transform_component_to_astro_static(content: &str, _component_name: &str, analysis: &ProjectAnalysis) -> String {
    // Transform component following official Astro migration guide
    let mut result = String::from("---\n");
    
    // Extract imports and convert them (same logic as pages)
    let default_import_pattern = Regex::new(r#"import\s+(\w+)\s+from\s+['"]([^'"]+)['"]"#).unwrap();
    let named_import_pattern = Regex::new(r#"import\s+\{([^}]+)\}\s+from\s+['"]([^'"]+)['"]"#).unwrap();
    let mut component_imports = Vec::new();
    let mut component_usage = Vec::new();
    
    // Handle default imports
    for cap in default_import_pattern.captures_iter(content) {
        if let (Some(component_name), Some(import_path)) = (cap.get(1), cap.get(2)) {
            let comp_name = component_name.as_str();
            let path = import_path.as_str();
            
            // Skip React Router imports
            if path == "react-router-dom" {
                continue;
            }
            
            // Handle component imports from @/components
            if path.starts_with("@/components/") {
                let comp_path = path.strip_prefix("@/components/").unwrap();
                let comp_name_str = comp_name.to_string();
                let comp_name_clean = comp_path.trim_end_matches(".tsx").trim_end_matches(".ts");
                
                let known_react_components = vec!["Navbar", "Footer", "WhatsAppButton"];
                let is_known_react = known_react_components.contains(&comp_name_clean);
                
                let is_client_side = is_known_react || analysis.components.iter()
                    .any(|c| (c.name == comp_name_clean || c.name == comp_name) && c.is_client_side);
                
                if is_client_side {
                    component_imports.push(format!("import {} from './ui/{}.tsx';", comp_name_str, comp_path.trim_end_matches(".tsx").trim_end_matches(".ts")));
                    component_usage.push((comp_name_str.clone(), true));
                } else {
                    component_imports.push(format!("import {} from './{}.astro';", comp_name_str, comp_path.trim_end_matches(".tsx").trim_end_matches(".ts")));
                    component_usage.push((comp_name_str.clone(), false));
                }
            } else if path.starts_with("@/") {
                // Other @/ imports (lib, assets, etc.)
                // For components, @/ resolves to ../ (one level up from components/)
                let adjusted = path.replace("@/", "../");
                component_imports.push(format!("import {} from '{}';", comp_name, adjusted));
            } else if !path.starts_with("react") && !path.starts_with(".") && !path.starts_with("@/") {
                // External default imports - these should be included too
                // For example: import something from 'some-package'
                component_imports.push(format!("import {} from '{}';", comp_name, path));
            } else if path.ends_with(".jpg") || path.ends_with(".jpeg") || path.ends_with(".png") 
                || path.ends_with(".svg") || path.ends_with(".webp") || path.ends_with(".gif") {
                // Image imports - keep as-is, but we'll need to convert src={imageVar} to src={imageVar.src}
                component_imports.push(format!("import {} from '{}';", comp_name, path));
            }
        }
    }
    
    // Handle named imports
    for cap in named_import_pattern.captures_iter(content) {
        if let (Some(imports_list), Some(import_path)) = (cap.get(1), cap.get(2)) {
            let path = import_path.as_str();
            
            if path == "react-router-dom" {
                continue;
            }
            
            let imports: Vec<&str> = imports_list.as_str()
                .split(',')
                .map(|s| s.trim())
                .collect();
            
            if !path.starts_with("react") && !path.starts_with(".") && !path.starts_with("@/") {
                // External named imports (lucide-react, etc.)
                let imports_str = imports.join(", ");
                component_imports.push(format!("import {{ {} }} from '{}';", imports_str, path));
            } else if path.starts_with("@/components/ui/") {
                let comp_path = path.strip_prefix("@/components/ui/").unwrap();
                let adjusted = format!("./ui/{}", comp_path);
                let imports_str = imports.join(", ");
                component_imports.push(format!("import {{ {} }} from '{}';", imports_str, adjusted));
            } else if path.starts_with("@/") {
                let adjusted = path.replace("@/", "../");
                let imports_str = imports.join(", ");
                component_imports.push(format!("import {{ {} }} from '{}';", imports_str, adjusted));
            }
        }
    }
    
    for import in component_imports {
        result.push_str(&format!("{}\n", import));
    }
    
    // Extract constants before component function
    let component_start_pattern = Regex::new(r#"(?m)^(const\s+\w+\s*=\s*\(\)\s*=>|function\s+\w+\s*\()"#).unwrap();
    let last_import_pattern = Regex::new(r#"(?m)^import\s+.*?from\s+['"][^'"]+['"];?\s*$"#).unwrap();
    let mut last_import_end = 0;
    for cap in last_import_pattern.find_iter(content) {
        last_import_end = cap.end();
    }
    
    let component_start = if let Some(m) = component_start_pattern.find(content) {
        m.start()
    } else {
        content.len()
    };
    
    if component_start > last_import_end {
        let constants_section = &content[last_import_end..component_start].trim();
        if !constants_section.is_empty() {
            result.push_str("\n");
            result.push_str(constants_section);
            result.push_str("\n");
        }
    }
    
    result.push_str("---\n\n");
    
    // Extract and convert JSX content (same as pages but without Layout)
    let mut converted = content.to_string();
    converted = converted.replace("className", "class");
    
    let children_pattern = Regex::new(r#"\{(?:props\.)?children\}"#).unwrap();
    converted = children_pattern.replace_all(&converted, "<slot />").to_string();
    
    // Convert inline style objects
    let style_object_pattern = Regex::new(r#"style=\{\{\s*([^}]+)\s*\}\}"#).unwrap();
    converted = style_object_pattern.replace_all(&converted, |caps: &regex::Captures| {
        if let Some(style_content) = caps.get(1) {
            let style_str = style_content.as_str();
            let kv_pattern = Regex::new(r#"(\w+):\s*['"]?([^,'"]+)['"]?"#).unwrap();
            let mut css_parts = Vec::new();
            for kv_cap in kv_pattern.captures_iter(style_str) {
                if let (Some(key), Some(value)) = (kv_cap.get(1), kv_cap.get(2)) {
                    let css_key = key.as_str()
                        .chars()
                        .enumerate()
                        .flat_map(|(i, c)| {
                            if c.is_uppercase() && i > 0 {
                                vec!['-', c.to_lowercase().next().unwrap()]
                            } else {
                                vec![c.to_lowercase().next().unwrap_or(c)]
                            }
                        })
                        .collect::<String>();
                    css_parts.push(format!("{}:{}", css_key.trim(), value.as_str().trim()));
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
    }).to_string();
    
    converted = converted.replace("react-router-dom", "");
    
    let link_pattern = Regex::new(r#"<Link\s+to=["']([^"']+)["']([^>]*)>"#).unwrap();
    converted = link_pattern.replace_all(&converted, |caps: &regex::Captures| {
        let href = caps.get(1).unwrap().as_str();
        let attrs = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        format!("<a href=\"{}\"{}>", href, attrs)
    }).to_string();
    converted = converted.replace("</Link>", "</a>");
    
    // Extract JSX return statement
    let mut jsx_content = if let Some(return_start) = converted.find("return (") {
        if let Some(return_end) = converted[return_start..].find(");") {
            converted[return_start + 8..return_start + return_end].trim().to_string()
        } else {
            converted
        }
    } else if let Some(return_start) = converted.find("return ") {
        let after_return = &converted[return_start + 7..];
        if let Some(return_end) = after_return.find("\n  };") {
            after_return[..return_end].trim().to_string()
        } else {
            after_return.trim().to_string()
        }
    } else {
        converted
    };
    
    // Replace component tags with client directives
    for (comp_name, is_client_side) in &component_usage {
        if *is_client_side {
            let pattern = format!(r#"<{}\s*/>"#, comp_name);
            let replacement = format!(r#"<{} client:load />"#, comp_name);
            jsx_content = Regex::new(&pattern).unwrap()
                .replace_all(&jsx_content, &replacement)
                .to_string();
            
            let pattern_open = format!(r#"<{}\s*>"#, comp_name);
            let replacement_open = format!(r#"<{} client:load>"#, comp_name);
            jsx_content = Regex::new(&pattern_open).unwrap()
                .replace_all(&jsx_content, &replacement_open)
                .to_string();
        }
    }
    
    // Clean up React-specific syntax
    jsx_content = Regex::new(r#"<>\s*</>"#).unwrap().replace_all(&jsx_content, "").to_string();
    jsx_content = Regex::new(r#"<React\.Fragment>\s*</React\.Fragment>"#).unwrap()
        .replace_all(&jsx_content, "")
        .to_string();
    
    // Convert JSX comments to HTML comments
    let jsx_comment_pattern = Regex::new(r#"/\*([^*]|\*[^/])*\*/"#).unwrap();
    jsx_content = jsx_comment_pattern.replace_all(&jsx_content, |caps: &regex::Captures| {
        let comment_text = caps.get(0).unwrap().as_str();
        let content = comment_text
            .strip_prefix("/*")
            .and_then(|s| s.strip_suffix("*/"))
            .unwrap_or(comment_text)
            .trim();
        format!("<!-- {} -->", content)
    }).to_string();
    
    // Fix image src attributes: src={imageVar} -> src={imageVar.src}
    // In React, imported images can be used directly, but Astro needs .src property
    // Pattern: src={variableName} where variableName is an imported image
    let src_expression_pattern = Regex::new(r#"src=\{(?:([a-zA-Z_$][a-zA-Z0-9_$]*)|([a-zA-Z_$][a-zA-Z0-9_$]*\.[a-zA-Z0-9_$]+))\}"#).unwrap();
    jsx_content = src_expression_pattern.replace_all(&jsx_content, |caps: &regex::Captures| {
        if let Some(simple_var) = caps.get(1) {
            // Simple variable: src={imageVar} -> src={imageVar.src}
            let var_name = simple_var.as_str();
            format!("src={{{}.src}}", var_name)
        } else if let Some(prop_access) = caps.get(2) {
            // Already has property access: src={obj.prop} -> keep as-is
            format!("src={{{}}}", prop_access.as_str())
        } else {
            caps.get(0).unwrap().as_str().to_string()
        }
    }).to_string();
    
    result.push_str(&jsx_content);
    result.push_str("\n");
    
    result
}

/// Collect all component names that are imported by client-side components
/// This is the single source of truth for what needs to be copied to ui/
fn collect_client_required_components(
    analysis: &ProjectAnalysis,
) -> std::collections::HashSet<String> {
    let mut required = std::collections::HashSet::new();

    for component in &analysis.components {
        if component.is_client_side {
            for imported in &component.imports {
                required.insert(imported.clone());
            }
        }
    }

    required
}

fn generate_components(components_dir: &Utf8PathBuf, analysis: &ProjectAnalysis) -> Result<()> {
    // Create ui subdirectory for shadcn/ui components (Button, etc.)
    let ui_dir = components_dir.join("ui");
    fs::create_dir_all(&ui_dir)
        .map_err(|e| miette!("Failed to create ui directory: {}", e))?;

    // Create react subdirectory for React components that need hydration
    let react_dir = components_dir.join("react");
    fs::create_dir_all(&react_dir)
        .map_err(|e| miette!("Failed to create react directory: {}", e))?;

    // Build the client-required component set once (single source of truth)
    let client_required = collect_client_required_components(analysis);

    // First, copy all UI components from src/components/ui (shadcn/ui components)
    let source_ui_dir = analysis.source_dir.join("src/components/ui");
    if source_ui_dir.exists() {
        copy_ui_components(&source_ui_dir, &ui_dir, analysis, &client_required)?;
    }

    // Copy React components from src/components/ that are imported by other React components
    // These are components required by client-side components - they go to react/ directory
    let source_components_dir = analysis.source_dir.join("src/components");
    if source_components_dir.exists() {
        copy_react_components_from_components_dir(&source_components_dir, &react_dir, &client_required, analysis)?;
    }

    // Now handle other components
    for component in &analysis.components {
        // Check if this is a page component (in src/pages/)
        let is_page_component = component.file_path.to_string().contains("/pages/");
        
        // Check if this is already a UI component (we copied them above)
        let is_ui_component = component.file_path.to_string().contains("/components/ui/");
        
        if is_ui_component {
            // Already handled by copy_ui_components
            continue;
        }
        
        if component.is_client_side {
            // Copy React component as-is to react directory (for page components)
            if is_page_component {
                let dest_path = react_dir.join(format!("{}.tsx", component.name));
                let source_path = BiomePath::new(component.file_path.clone());
                let content = source_path
                    .read_to_string()
                    .map_err(|e| miette!("Failed to read component: {}", e))?;
                
                // Fix imports in the React component
                let fixed_content = fix_react_imports(&content);
                let fixed_content = fix_all_component_imports(&fixed_content, analysis, &client_required);
                
                let mut file = fs::File::create(&dest_path)
                    .map_err(|e| miette!("Failed to create component: {}", e))?;
                file.write_all(fixed_content.as_bytes())
                    .map_err(|e| miette!("Failed to write component: {}", e))?;
            } else {
                // Regular client-side component - copy to react with fixed imports
                let dest_path = react_dir.join(format!("{}.tsx", component.name));
                let source_path = BiomePath::new(component.file_path.clone());
                let content = source_path
                    .read_to_string()
                    .map_err(|e| miette!("Failed to read component: {}", e))?;
                
                let fixed_content = fix_react_imports(&content);
                let fixed_content = fix_all_component_imports(&fixed_content, analysis, &client_required);
                
                let mut file = fs::File::create(&dest_path)
                    .map_err(|e| miette!("Failed to create component: {}", e))?;
                file.write_all(fixed_content.as_bytes())
                    .map_err(|e| miette!("Failed to write component: {}", e))?;
            }
        } else {
            // Static component - check if it's imported by any client-side component
            let is_imported_by_client = client_required.contains(&component.name);
            
            if is_imported_by_client {
                // If imported by a client-side component, copy it to react/ as React component
                // BUT DO NOT convert to .astro - it's needed as React component
                let dest_path = react_dir.join(format!("{}.tsx", component.name));
                let source_path = BiomePath::new(component.file_path.clone());
                let content = source_path
                    .read_to_string()
                    .map_err(|e| miette!("Failed to read component: {}", e))?;
                
                let fixed_content = fix_react_imports(&content);
                let fixed_content = fix_all_component_imports(&fixed_content, analysis, &client_required);
                
                let mut file = fs::File::create(&dest_path)
                    .map_err(|e| miette!("Failed to create component: {}", e))?;
                file.write_all(fixed_content.as_bytes())
                    .map_err(|e| miette!("Failed to write component: {}", e))?;
                
                // Skip Astro conversion - this component is needed as React
            } else {
                // Pure static component - transform to Astro (only for non-page, non-ui components)
                if !is_page_component {
                    let dest_path = components_dir.join(format!("{}.astro", component.name));
                    let source_path = BiomePath::new(component.file_path.clone());
                    let content = source_path
                        .read_to_string()
                        .map_err(|e| miette!("Failed to read component: {}", e))?;
                    
                    // Use the proper component transformer with safety checks
                    let astro_content = transform_component_to_astro(&content, &component.name, analysis);
                    
                    let mut file = fs::File::create(&dest_path)
                        .map_err(|e| miette!("Failed to create component: {}", e))?;
                    file.write_all(astro_content.as_bytes())
                        .map_err(|e| miette!("Failed to write Astro component: {}", e))?;
                }
            }
        }
    }

    Ok(())
}

fn copy_ui_components(
    source_ui_dir: &Utf8PathBuf, 
    dest_ui_dir: &Utf8PathBuf, 
    analysis: &ProjectAnalysis,
    client_required: &std::collections::HashSet<String>
) -> Result<()> {
    for entry in WalkDir::new(source_ui_dir) {
        let entry = entry.map_err(|e| miette!("Failed to read ui directory: {}", e))?;
        let path = Utf8PathBuf::from_path_buf(entry.path().to_path_buf())
            .map_err(|_| miette!("Invalid UTF-8 path"))?;

        if path.extension() == Some("tsx") || path.extension() == Some("ts") {
            let file_name = path.file_name()
                .ok_or_else(|| miette!("Invalid file name"))?;
            let dest_path = dest_ui_dir.join(file_name);
            
            let source_path = BiomePath::new(path.clone());
            let content = source_path
                .read_to_string()
                .map_err(|e| miette!("Failed to read ui component: {}", e))?;
            
            // Fix imports in UI components
            let fixed_content = fix_react_imports(&content);
            let fixed_content = fix_all_component_imports(&fixed_content, analysis, client_required);
            
            let mut file = fs::File::create(&dest_path)
                .map_err(|e| miette!("Failed to create ui component: {}", e))?;
            file.write_all(fixed_content.as_bytes())
                .map_err(|e| miette!("Failed to write ui component: {}", e))?;
        }
    }
    
    Ok(())
}

fn copy_react_components_from_components_dir(
    source_components_dir: &Utf8PathBuf, 
    dest_ui_dir: &Utf8PathBuf, 
    client_required: &std::collections::HashSet<String>,
    analysis: &ProjectAnalysis
) -> Result<()> {
    // Copy all components that are required by client-side components
    // This guarantees any component imported by a React component is available as React
    
    for entry in WalkDir::new(source_components_dir) {
        let entry = entry.map_err(|e| miette!("Failed to read components directory: {}", e))?;
        let path = Utf8PathBuf::from_path_buf(entry.path().to_path_buf())
            .map_err(|_| miette!("Invalid UTF-8 path"))?;

        // Skip the ui subdirectory (already handled)
        if path.to_string().contains("/components/ui/") {
            continue;
        }

        if path.extension() == Some("tsx") || path.extension() == Some("ts") {
            let file_stem = path.file_stem()
                .ok_or_else(|| miette!("Invalid file name"))?
                .to_string();
            
            // Copy if it's required by any client-side component
            let should_copy = client_required.contains(&file_stem);
            
            if should_copy {
                let file_name = path.file_name()
                    .ok_or_else(|| miette!("Invalid file name"))?;
                let dest_path = dest_ui_dir.join(file_name);
                
                let source_path = BiomePath::new(path.clone());
                let content = source_path
                    .read_to_string()
                    .map_err(|e| miette!("Failed to read component: {}", e))?;
                
                // Transform the component using AST transformer
                let fixed_content = crate::ast_transformer::transform_with_ast(&content)
                    .unwrap_or_else(|_| content.clone());
                
                // Fix all component imports based on analysis
                let fixed_content = fix_all_component_imports(&fixed_content, analysis, client_required);
                
                let mut file = fs::File::create(&dest_path)
                    .map_err(|e| miette!("Failed to create component: {}", e))?;
                file.write_all(fixed_content.as_bytes())
                    .map_err(|e| miette!("Failed to write component: {}", e))?;
            }
        }
    }
    
    Ok(())
}

fn transform_to_astro_simple(content: &str) -> String {
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

fn fix_react_imports(content: &str) -> String {
    // Use AST-validated transformation (uses regex internally but validates with AST first)
    let transformed = match crate::ast_transformer::transform_with_ast(content) {
        Ok(transformed) => transformed,
        Err(_) => {
            // If AST parsing completely fails, use pure regex fallback
            transform_with_regex_fallback(content)
        }
    };
    
    // Fix component import paths: @/components/X -> @/components/ui/X
    // This handles components like Navbar, Footer, WhatsAppButton that are copied to ui/
    fix_component_import_paths(&transformed)
}

fn fix_component_import_paths(content: &str) -> String {
    // This function is now deprecated - import paths are fixed by fix_all_component_imports
    // which uses client_required set and maintains original paths via tsconfig aliases
    // Keeping this for backward compatibility but it shouldn't be needed
    content.to_string()
}

/// Fix all component imports in a React component based on analysis data
/// This ensures imports point to the correct locations (react/ for client-side, .astro for static)
fn fix_all_component_imports(
    content: &str, 
    analysis: &ProjectAnalysis,
    client_required: &std::collections::HashSet<String>
) -> String {
    let mut fixed = content.to_string();
    
    // Create a map of component names to their destination paths
    let mut component_paths: HashMap<String, String> = HashMap::new();
    
    for component in &analysis.components {
        let comp_name = &component.name;
        
        // Skip UI components (they're already in the right place)
        if component.file_path.to_string().contains("/components/ui/") {
            continue;
        }
        
        // Determine the correct import path
        if component.is_client_side {
            // Client-side components go to react/
            component_paths.insert(comp_name.clone(), format!("@/components/react/{}", comp_name));
        } else {
            // Static components become .astro files
            // But if they're imported by React components, they need to be available as React components too
            // Check if this component is imported by any client-side component
            let is_imported_by_client = client_required.contains(comp_name);
            
            if is_imported_by_client {
                // If imported by a client-side component, it needs to be in react/ as well
                component_paths.insert(comp_name.clone(), format!("@/components/react/{}", comp_name));
            } else {
                // Pure static component - becomes .astro
                component_paths.insert(comp_name.clone(), format!("@/components/{}.astro", comp_name));
            }
        }
    }
    
    // Also add components that are imported by client-side components but might not be in analysis.components
    // This handles cases where components are copied to react/ but weren't analyzed
    for imported_name in client_required {
        if !component_paths.contains_key(imported_name) {
            // If it's imported by a client-side component, it should be in react/
            component_paths.insert(imported_name.clone(), format!("@/components/react/{}", imported_name));
        }
    }
    
    // Fix default imports: import Component from '@/components/Component'
    for (comp_name, import_path) in &component_paths {
        // Pattern: import Component from '@/components/Component'
        let pattern = format!(r#"import\s+(\w+)\s+from\s+['"]@/components/{}(?:\.tsx)?['"]"#, comp_name);
        let regex = Regex::new(&pattern).unwrap();
        fixed = regex.replace_all(&fixed, |caps: &regex::Captures| {
            let import_name = caps.get(1).unwrap().as_str();
            format!("import {} from '{}'", import_name, import_path)
        }).to_string();
    }
    
    // Fix named imports: import { Component } from '@/components/Component'
    for (comp_name, import_path) in &component_paths {
        // Pattern: import { Component } from '@/components/Component'
        let pattern = format!(r#"import\s+\{{([^}}]*{}(?:,\s*[^}}]*)?)\}}\s+from\s+['"]@/components/{}(?:\.tsx)?['"]"#, comp_name, comp_name);
        let regex = Regex::new(&pattern).unwrap();
        fixed = regex.replace_all(&fixed, |caps: &regex::Captures| {
            let imports_list = caps.get(1).unwrap().as_str();
            format!("import {{ {} }} from '{}'", imports_list, import_path)
        }).to_string();
    }
    
    fixed
}

fn transform_with_regex_fallback(content: &str) -> String {
    // Don't transform @/ imports - they're handled by tsconfig.json path aliases and vite alias config
    // The tsconfig.json has "@/*": ["./src/*"] and vite config has the alias configured
    let mut fixed = content.to_string();
    
    // Remove React Router imports and usage
    let router_import_pattern = Regex::new(r#"import\s+.*?from\s+["']react-router-dom["'];?\s*\n?"#).unwrap();
    fixed = router_import_pattern.replace_all(&fixed, "").to_string();
    
    // Remove useLocation hook calls
    let location_pattern = Regex::new(r#"const\s+location\s*=\s*useLocation\(\);"#).unwrap();
    fixed = location_pattern.replace_all(&fixed, "").to_string();
    
    // Remove useNavigate hook calls
    let navigate_pattern = Regex::new(r#"const\s+navigate\s*=\s*useNavigate\(\);"#).unwrap();
    fixed = navigate_pattern.replace_all(&fixed, "").to_string();
    
    // Replace navigate('/path') calls with window.location.href = '/path'
    // Pattern: navigate('/path') or navigate("/path")
    let navigate_call_pattern = Regex::new(r#"navigate\((['"])([^'"]+)\1\)"#).unwrap();
    fixed = navigate_call_pattern.replace_all(&fixed, |caps: &regex::Captures| {
        let quote = caps.get(1).unwrap().as_str();
        let path = caps.get(2).unwrap().as_str();
        format!("window.location.href = {}{}{}", quote, path, quote)
    }).to_string();
    
    // Remove useEffect hooks that use location
    let useEffect_location_pattern = Regex::new(r#"useEffect\(\(\)\s*=>\s*\{[^}]*location[^}]*\},\s*\[location\.pathname\]\);"#).unwrap();
    fixed = useEffect_location_pattern.replace_all(&fixed, "").to_string();
    
    // Replace React Router Link with regular anchor tags
    // Strategy: Replace closing tags first, then handle opening tags with attributes
    
    // First, replace closing tags: </Link> -> </a>
    fixed = fixed.replace("</Link>", "</a>");
    
    // Then handle opening tags with to= attribute
    // Handle double quotes: <Link to="/path"> -> <a href="/path">
    let link_to_double = Regex::new(r#"<Link(\s+)to="([^"]+)""#).unwrap();
    fixed = link_to_double.replace_all(&fixed, |caps: &regex::Captures| {
        format!("<a{}href=\"{}\"", 
            caps.get(1).unwrap().as_str(),
            caps.get(2).unwrap().as_str())
    }).to_string();
    
    // Handle single quotes: <Link to='/path'> -> <a href='/path'>
    let link_to_single = Regex::new(r#"<Link(\s+)to='([^']+)'"#).unwrap();
    fixed = link_to_single.replace_all(&fixed, |caps: &regex::Captures| {
        format!("<a{}href='{}'", 
            caps.get(1).unwrap().as_str(),
            caps.get(2).unwrap().as_str())
    }).to_string();
    
    // Replace any remaining <Link with <a (for cases without to= attribute or already processed)
    fixed = fixed.replace("<Link ", "<a ");
    fixed = fixed.replace("<Link>", "<a>");
    fixed = fixed.replace("<Link\n", "<a\n");
    fixed = fixed.replace("<Link\t", "<a\t");
    
    // Keep className as-is for React components (don't convert to class)
    // React components should use className
    
    fixed
}

fn generate_readme(output_dir: &Utf8PathBuf, project_name: &str) -> Result<()> {
    let readme_path = output_dir.join("README.md");
    let mut file = fs::File::create(&readme_path)
        .map_err(|e| miette!("Failed to create README.md: {}", e))?;

    let readme = format!(
        r#"# {}

This project was migrated from a React SPA to Astro using the SSG migrator.

## Getting Started

1. Install dependencies:
```bash
npm install
```

2. Start the development server:
```bash
npm run dev
```

3. Build for production:
```bash
npm run build
```

## Migration Notes

- React components that use hooks or browser APIs are kept as React components in `src/components/ui/`
- Static components have been converted to Astro components
- Routes have been converted to Astro pages in `src/pages/`
"#,
        project_name
    );

    file.write_all(readme.as_bytes())
        .map_err(|e| miette!("Failed to write README.md: {}", e))?;

    Ok(())
}

