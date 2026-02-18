use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    pub path: String,
    pub component: String,
    pub is_catch_all: bool,
    /// True when path uses optional catch-all syntax, e.g. [[...slug]]
    #[serde(default)]
    pub is_optional_catch_all: bool,
}

#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub name: String,
    pub file_path: Utf8PathBuf,
    pub is_client_side: bool,
    pub is_react_context_boundary: bool, // NEW: Marks components that import Radix UI or other context-dependent libraries
    pub uses_hooks: Vec<String>,
    pub uses_browser_apis: Vec<String>,
    pub imports: Vec<String>, // NEW: Track imported component names for graph propagation
}

#[derive(Debug, Clone)]
pub struct ProjectAnalysis {
    pub routes: Vec<RouteInfo>,
    pub components: Vec<ComponentInfo>,
    pub dependencies: HashMap<String, String>,
    pub has_vite_config: bool,
    pub has_tailwind: bool,
    pub source_dir: Utf8PathBuf,
    /// True when react-scripts is in dependencies (Create React App).
    pub is_cra: bool,
    /// Unique REACT_APP_ variable names found in source and .env files.
    pub react_app_vars: Vec<String>,
    /// True when socket.io or ws is in dependencies (needs webpack fallback).
    pub has_websocket_deps: bool,
    /// True when :export found in SCSS files (Turbopack may break).
    pub has_scss_export: bool,
    /// True when ReactComponent SVG imports found (needs SVGR config).
    pub has_svg_react_component: bool,
    /// True when extraReducers object notation found (RTK v2 builder required).
    pub has_extra_reducers_object: bool,
    /// True when /app/ paths found in href/to/push/redirect (route group conflict).
    pub has_app_path_in_nav: bool,
}

#[derive(Debug, Clone)]
pub struct MigrationConfig {
    pub source_dir: Utf8PathBuf,
    pub output_dir: Utf8PathBuf,
    pub project_name: String,
    pub force: bool,
    /// When true, generate a static-export Next.js project (`output: 'export'` in next.config).
    pub static_export: bool,
    /// Comma-separated transforms: router, client, helmet, context, image, all (default when None)
    pub transforms: Option<String>,
}

/// Severity level for SSG verification warnings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsgSeverity {
    /// Will cause the build to fail.
    Error,
    /// May cause runtime issues or unexpected behaviour.
    Warning,
}

/// A single SSG compatibility issue found during verification.
#[derive(Debug, Clone)]
pub struct SsgWarning {
    pub severity: SsgSeverity,
    pub message: String,
    /// Relative file path where the issue was found (if applicable).
    pub file: Option<String>,
    /// CRA-to-Next rule ID for traceability, e.g. gotchas-window-undefined.
    pub rule_id: Option<String>,
}

/// Result of a pre-migration scan with recommended rules/transforms.
#[derive(Debug, Clone)]
pub struct PreMigrationReport {
    pub analysis: ProjectAnalysis,
    /// Recommended rule IDs based on scan results.
    pub recommended_rules: Vec<String>,
}

