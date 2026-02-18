use crate::types::{ComponentInfo, PreMigrationReport, ProjectAnalysis, RouteInfo};
use crate::vfs::Vfs;
use biome_js_parser::{parse, JsParserOptions};
use biome_js_syntax::{JsFileSource, JsImport, JsSyntaxNode};
use biome_rowan::AstNode;
use camino::Utf8PathBuf;
use miette::{miette, Result};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;

// ── Pre-compiled regexes ────────────────────────────────────────────────

static RE_ROUTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<Route\s+path=["']([^"']+)["']\s+element=\{<(\w+)\s*/>\}"#).unwrap()
});
static RE_HOOKS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"use(State|Effect|Ref|Callback|Memo|Context|Reducer|LayoutEffect)").unwrap()
});
static RE_WINDOW: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bwindow\b").unwrap());
static RE_DOCUMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bdocument\b").unwrap());
static RE_LOCALSTORAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\blocalStorage\b").unwrap());
static RE_SESSIONSTORAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bsessionStorage\b").unwrap());
static RE_NAVIGATOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bnavigator\b").unwrap());

static RE_IMPORT_DEFAULT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import\s+(\w+)\s+from\s+['"]@/components/([^'"]+)['"]"#).unwrap()
});
static RE_IMPORT_NAMED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import\s+\{([^}]+)\}\s+from\s+['"]@/components/([^'"]+)['"]"#).unwrap()
});
static RE_IMPORT_NAMESPACE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import\s+\*\s+as\s+(\w+)\s+from\s+['"]@/components/([^'"]+)['"]"#).unwrap()
});

static RE_REACT_APP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"REACT_APP_[A-Za-z0-9_]+").unwrap());
static RE_EXTRA_REDUCERS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"extraReducers\s*:").unwrap());
static RE_SVG_REACT_COMPONENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"ReactComponent").unwrap());
static RE_APP_PATH_IN_NAV: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"["']/app/"#).unwrap());

static BROWSER_PATTERNS: LazyLock<Vec<(&str, &Regex)>> = LazyLock::new(|| {
    vec![
        ("window", &RE_WINDOW),
        ("document", &RE_DOCUMENT),
        ("localStorage", &RE_LOCALSTORAGE),
        ("sessionStorage", &RE_SESSIONSTORAGE),
        ("navigator", &RE_NAVIGATOR),
    ]
});

// ── Public API ──────────────────────────────────────────────────────────

/// Run pre-migration scan and return analysis with recommended rule IDs.
pub fn run_pre_migration_scan(
    vfs: &dyn Vfs,
    source_dir: &Utf8PathBuf,
) -> Result<PreMigrationReport> {
    let analysis = analyze_project(vfs, source_dir)?;
    let recommended_rules = build_recommended_rules(&analysis);
    Ok(PreMigrationReport {
        analysis,
        recommended_rules,
    })
}

fn build_recommended_rules(analysis: &ProjectAnalysis) -> Vec<String> {
    let mut rules = Vec::new();
    if !analysis.react_app_vars.is_empty() {
        rules.push("env-prefix-change".to_string());
    }
    if analysis.has_websocket_deps {
        rules.push("gotchas-websocket-optional-deps".to_string());
        rules.push("setup-next-config".to_string());
    }
    if analysis.has_scss_export {
        rules.push("gotchas-turbopack".to_string());
    }
    if analysis.has_svg_react_component {
        rules.push("assets-static-imports".to_string());
    }
    if analysis.has_extra_reducers_object {
        rules.push("state-redux".to_string());
    }
    if analysis.has_app_path_in_nav {
        rules.push("routing-route-groups".to_string());
    }
    rules
}

pub fn analyze_project(vfs: &dyn Vfs, source_dir: &Utf8PathBuf) -> Result<ProjectAnalysis> {
    let app_path = source_dir.join("src/App.tsx");
    let app_path_jsx = source_dir.join("src/App.jsx");
    let package_json_path = source_dir.join("package.json");

    let app_file = if vfs.exists(app_path.as_str()) {
        app_path
    } else if vfs.exists(app_path_jsx.as_str()) {
        app_path_jsx
    } else {
        app_path.clone()
    };

    let routes = if vfs.exists(app_file.as_str()) {
        parse_routes(vfs, &app_file)?
    } else {
        vec![]
    };

    let components = analyze_components(vfs, source_dir)?;
    let dependencies = parse_dependencies(vfs, &package_json_path)?;

    let has_vite_config = vfs.exists(source_dir.join("vite.config.ts").as_str())
        || vfs.exists(source_dir.join("vite.config.js").as_str());
    let has_tailwind = vfs.exists(source_dir.join("tailwind.config.ts").as_str())
        || vfs.exists(source_dir.join("tailwind.config.js").as_str());

    let is_cra = dependencies
        .keys()
        .any(|k| k == "react-scripts" || k.starts_with("react-scripts"));
    let has_websocket_deps = dependencies
        .keys()
        .any(|k| k.contains("socket.io") || k == "ws");
    let react_app_vars = scan_react_app_vars(vfs, source_dir)?;
    let has_scss_export = scan_scss_export(vfs, source_dir)?;
    let has_svg_react_component = scan_svg_react_component(vfs, source_dir)?;
    let has_extra_reducers_object = scan_extra_reducers_object(vfs, source_dir)?;
    let has_app_path_in_nav = scan_app_path_in_nav(vfs, source_dir)?;

    Ok(ProjectAnalysis {
        routes,
        components,
        dependencies,
        has_vite_config,
        has_tailwind,
        source_dir: source_dir.clone(),
        is_cra,
        react_app_vars,
        has_websocket_deps,
        has_scss_export,
        has_svg_react_component,
        has_extra_reducers_object,
        has_app_path_in_nav,
    })
}

// ── Routes ──────────────────────────────────────────────────────────────

/// Normalize React Router path to canonical form and detect catch-all variants.
/// - "/blog/*" -> ("/blog/[...slug]", true, false)
/// - "/docs/[[...slug]]" -> ("/docs/[[...slug]]", false, true)
/// - "/files/[...slug]" -> ("/files/[...slug]", true, false)
/// - "*" -> ("*", true, false)
fn normalize_route_path(path: &str) -> (String, bool, bool) {
    let path = path.trim();
    if path == "*" {
        return ("*".to_string(), true, false);
    }
    if path.contains("[[...") && path.contains("]]") {
        return (path.to_string(), false, true);
    }
    if path.contains("[...") && path.contains("]") {
        return (path.to_string(), true, false);
    }
    if path.ends_with("/*") {
        let prefix = path.trim_end_matches("/*").trim_end_matches('/');
        if prefix.is_empty() {
            return ("*".to_string(), true, false);
        }
        let next_segment = format!("{}/[...slug]", prefix.trim_start_matches('/'));
        return (next_segment, true, false);
    }
    (path.to_string(), false, false)
}

fn parse_routes(vfs: &dyn Vfs, app_path: &Utf8PathBuf) -> Result<Vec<RouteInfo>> {
    let content = vfs
        .read_to_string(app_path.as_str())
        .map_err(|e| miette!("Failed to read App.tsx: {}", e))?;

    let mut routes = Vec::new();

    for cap in RE_ROUTE.captures_iter(&content) {
        let path = cap
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "/".to_string());
        let component = cap
            .get(2)
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "NotFound".to_string());
        let (path, is_catch_all, is_optional_catch_all) =
            normalize_route_path(&path);

        routes.push(RouteInfo {
            path,
            component,
            is_catch_all,
            is_optional_catch_all,
        });
    }

    if (content.contains(r#"path="*""#) || content.contains(r#"path='*'"#))
        && !routes.iter().any(|r| r.is_catch_all && r.path == "*")
    {
        routes.push(RouteInfo {
            path: "*".to_string(),
            component: "NotFound".to_string(),
            is_catch_all: true,
            is_optional_catch_all: false,
        });
    }

    if routes.is_empty() {
        routes.push(RouteInfo {
            path: "/".to_string(),
            component: "Index".to_string(),
            is_catch_all: false,
            is_optional_catch_all: false,
        });
    }

    Ok(routes)
}

// ── Components ──────────────────────────────────────────────────────────

fn analyze_components(vfs: &dyn Vfs, source_dir: &Utf8PathBuf) -> Result<Vec<ComponentInfo>> {
    let mut components = Vec::new();
    let components_dir = source_dir.join("src/components");
    let pages_dir = source_dir.join("src/pages");

    if vfs.exists(components_dir.as_str()) {
        analyze_directory(vfs, &components_dir, &mut components)?;
    }
    if vfs.exists(pages_dir.as_str()) {
        analyze_directory(vfs, &pages_dir, &mut components)?;
    }

    propagate_context_boundaries(&mut components);
    Ok(components)
}

fn analyze_directory(
    vfs: &dyn Vfs,
    dir: &Utf8PathBuf,
    components: &mut Vec<ComponentInfo>,
) -> Result<()> {
    for entry in vfs.walk_dir(dir.as_str())? {
        if !entry.is_file {
            continue;
        }
        let path = Utf8PathBuf::from(&entry.path);
        if path.extension() == Some("tsx") || path.extension() == Some("jsx") {
            if let Ok(component_info) = analyze_component_file(vfs, &path) {
                components.push(component_info);
            }
        }
    }
    Ok(())
}

fn analyze_component_file(vfs: &dyn Vfs, file_path: &Utf8PathBuf) -> Result<ComponentInfo> {
    let content = vfs
        .read_to_string(file_path.as_str())
        .map_err(|e| miette!("Failed to read component file: {}", e))?;

    let parsed = parse(&content, JsFileSource::tsx(), JsParserOptions::default());
    let tree = parsed.tree();
    let syntax = tree.syntax();

    let is_react_context_boundary = detects_react_context_boundary(syntax);
    let imports = extract_component_imports(syntax);
    let (uses_hooks, uses_browser_apis) = detect_client_features(&content);
    let is_client_side =
        !uses_hooks.is_empty() || !uses_browser_apis.is_empty() || is_react_context_boundary;

    let name = file_path.file_stem().unwrap_or("Unknown").to_string();

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

// ── Client feature detection ────────────────────────────────────────────

fn detect_client_features(content: &str) -> (Vec<String>, Vec<String>) {
    let mut hooks = Vec::new();
    let mut browser_apis = Vec::new();

    for cap in RE_HOOKS.captures_iter(content) {
        if let Some(m) = cap.get(0) {
            let hook = m.as_str().to_string();
            if !hooks.contains(&hook) {
                hooks.push(hook);
            }
        }
    }

    for &(api_name, pattern) in BROWSER_PATTERNS.iter() {
        if pattern.is_match(content) && !browser_apis.contains(&api_name.to_string()) {
            browser_apis.push(api_name.to_string());
        }
    }

    (hooks, browser_apis)
}

// ── Pre-migration scans ──────────────────────────────────────────────────

fn scan_react_app_vars(vfs: &dyn Vfs, source_dir: &Utf8PathBuf) -> Result<Vec<String>> {
    let mut vars = std::collections::HashSet::new();
    let src_dir = source_dir.join("src");
    if !vfs.exists(src_dir.as_str()) {
        return Ok(vec![]);
    }
    for entry in vfs.walk_dir(src_dir.as_str())? {
        if !entry.is_file {
            continue;
        }
        let path = Utf8PathBuf::from(&entry.path);
        let ext = path.extension().unwrap_or("");
        if !matches!(ext, "ts" | "tsx" | "js" | "jsx") {
            continue;
        }
        let content = match vfs.read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for cap in RE_REACT_APP.captures_iter(&content) {
            if let Some(m) = cap.get(0) {
                vars.insert(m.as_str().to_string());
            }
        }
    }
    for env_name in &[".env", ".env.local", ".env.development", ".env.production"] {
        let env_path = source_dir.join(env_name);
        if vfs.exists(env_path.as_str()) {
            let content = match vfs.read_to_string(env_path.as_str()) {
                Ok(c) => c,
                Err(_) => continue,
            };
            for cap in RE_REACT_APP.captures_iter(&content) {
                if let Some(m) = cap.get(0) {
                    vars.insert(m.as_str().to_string());
                }
            }
        }
    }
    let mut out: Vec<String> = vars.into_iter().collect();
    out.sort();
    Ok(out)
}

fn scan_scss_export(vfs: &dyn Vfs, source_dir: &Utf8PathBuf) -> Result<bool> {
    let src_dir = source_dir.join("src");
    if !vfs.exists(src_dir.as_str()) {
        return Ok(false);
    }
    for entry in vfs.walk_dir(src_dir.as_str())? {
        if !entry.is_file {
            continue;
        }
        let path = Utf8PathBuf::from(&entry.path);
        if path.extension() != Some("scss") && path.extension() != Some("sass") {
            continue;
        }
        let content = match vfs.read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if content.contains(":export") {
            return Ok(true);
        }
    }
    Ok(false)
}

fn scan_svg_react_component(vfs: &dyn Vfs, source_dir: &Utf8PathBuf) -> Result<bool> {
    let src_dir = source_dir.join("src");
    if !vfs.exists(src_dir.as_str()) {
        return Ok(false);
    }
    for entry in vfs.walk_dir(src_dir.as_str())? {
        if !entry.is_file {
            continue;
        }
        let path = Utf8PathBuf::from(&entry.path);
        if path.extension() != Some("ts") && path.extension() != Some("tsx") {
            continue;
        }
        let content = match vfs.read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if RE_SVG_REACT_COMPONENT.is_match(&content) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn scan_extra_reducers_object(vfs: &dyn Vfs, source_dir: &Utf8PathBuf) -> Result<bool> {
    let src_dir = source_dir.join("src");
    if !vfs.exists(src_dir.as_str()) {
        return Ok(false);
    }
    for entry in vfs.walk_dir(src_dir.as_str())? {
        if !entry.is_file {
            continue;
        }
        let path = Utf8PathBuf::from(&entry.path);
        let ext = path.extension().unwrap_or("");
        if !matches!(ext, "js" | "jsx" | "ts" | "tsx") {
            continue;
        }
        let content = match vfs.read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if RE_EXTRA_REDUCERS.is_match(&content) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn scan_app_path_in_nav(vfs: &dyn Vfs, source_dir: &Utf8PathBuf) -> Result<bool> {
    let src_dir = source_dir.join("src");
    if !vfs.exists(src_dir.as_str()) {
        return Ok(false);
    }
    for entry in vfs.walk_dir(src_dir.as_str())? {
        if !entry.is_file {
            continue;
        }
        let path = Utf8PathBuf::from(&entry.path);
        let ext = path.extension().unwrap_or("");
        if !matches!(ext, "js" | "jsx" | "ts" | "tsx") {
            continue;
        }
        let content = match vfs.read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if RE_APP_PATH_IN_NAV.is_match(&content) {
            return Ok(true);
        }
    }
    Ok(false)
}

// ── Dependencies ────────────────────────────────────────────────────────

fn parse_dependencies(
    vfs: &dyn Vfs,
    package_json_path: &Utf8PathBuf,
) -> Result<HashMap<String, String>> {
    if !vfs.exists(package_json_path.as_str()) {
        return Ok(HashMap::new());
    }

    let content = vfs
        .read_to_string(package_json_path.as_str())
        .map_err(|e| miette!("Failed to read package.json: {}", e))?;

    let package_json: Value =
        serde_json::from_str(&content).map_err(|e| miette!("Failed to parse package.json: {}", e))?;

    let mut deps = HashMap::new();

    for section in ["dependencies", "devDependencies"] {
        if let Some(obj) = package_json.get(section).and_then(|v| v.as_object()) {
            for (key, value) in obj {
                if let Some(version) = value.as_str() {
                    deps.entry(key.clone()).or_insert_with(|| version.to_string());
                }
            }
        }
    }

    Ok(deps)
}

// ── AST helpers ─────────────────────────────────────────────────────────

fn detects_react_context_boundary(syntax: &JsSyntaxNode) -> bool {
    for node in syntax.descendants() {
        if let Some(import_decl) = JsImport::cast(node.clone()) {
            if let Ok(import_clause) = import_decl.import_clause() {
                if let Ok(source) = import_clause.source() {
                    if let Ok(source_text) = source.inner_string_text() {
                        let value = source_text.text();
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

fn extract_component_imports(syntax: &JsSyntaxNode) -> Vec<String> {
    let mut imports = Vec::new();
    let source_text = syntax.text_trimmed().to_string();

    for cap in RE_IMPORT_DEFAULT.captures_iter(&source_text) {
        if let Some(name) = cap.get(1) {
            imports.push(name.as_str().to_string());
        }
    }

    for cap in RE_IMPORT_NAMED.captures_iter(&source_text) {
        if let Some(imports_list) = cap.get(1) {
            for name in imports_list.as_str().split(',') {
                let trimmed = name.trim();
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

    for cap in RE_IMPORT_NAMESPACE.captures_iter(&source_text) {
        if let Some(name) = cap.get(1) {
            imports.push(name.as_str().to_string());
        }
    }

    imports
}

// ── Context boundary propagation ────────────────────────────────────────

fn propagate_context_boundaries(components: &mut Vec<ComponentInfo>) {
    let mut client_flags: HashMap<String, bool> = components
        .iter()
        .map(|c| {
            (
                c.name.clone(),
                c.is_react_context_boundary || c.is_client_side,
            )
        })
        .collect();

    let mut changed = true;
    while changed {
        changed = false;
        for component in components.iter_mut() {
            if component.is_client_side {
                continue;
            }
            let imports_boundary = component
                .imports
                .iter()
                .any(|name| client_flags.get(name).copied().unwrap_or(false));
            if imports_boundary {
                component.is_client_side = true;
                client_flags.insert(component.name.clone(), true);
                changed = true;
            }
        }
    }
}
