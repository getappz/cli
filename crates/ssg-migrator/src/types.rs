use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    pub path: String,
    pub component: String,
    pub is_catch_all: bool,
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
}

#[derive(Debug, Clone)]
pub struct MigrationConfig {
    pub source_dir: Utf8PathBuf,
    pub output_dir: Utf8PathBuf,
    pub project_name: String,
    pub force: bool,
    /// When true, generate a static-export Next.js project (`output: 'export'` in next.config).
    pub static_export: bool,
}

