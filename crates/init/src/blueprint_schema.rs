//! Blueprint file schema — the unified format for scaffolding + operations.

use miette::{miette, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Default)]
pub struct BlueprintSchema {
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default)]
    pub meta: Option<BlueprintMeta>,
    #[serde(default)]
    pub config: Option<serde_json::Value>,
    #[serde(default)]
    pub hosts: Option<serde_json::Value>,
    #[serde(default)]
    pub tools: Option<serde_json::Value>,
    #[serde(default)]
    pub setup: Option<Vec<SetupStep>>,
    #[serde(default)]
    pub tasks: Option<serde_json::Value>,
    #[serde(default)]
    pub before: Option<HashMap<String, Vec<String>>>,
    #[serde(default)]
    pub after: Option<HashMap<String, Vec<String>>>,
    #[serde(default)]
    pub includes: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct BlueprintMeta {
    pub name: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub framework: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    pub create_command: Option<String>,
    pub package_manager: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct SetupStep {
    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default)]
    pub run_locally: Option<String>,
    #[serde(default)]
    pub run: Option<String>,
    #[serde(default)]
    pub cd: Option<String>,
    #[serde(default)]
    pub add_dependency: Option<Vec<String>>,
    #[serde(default)]
    pub dev: Option<bool>,
    #[serde(default)]
    pub write_file: Option<WriteFileDef>,
    #[serde(default)]
    pub patch_file: Option<PatchFileDef>,
    #[serde(default)]
    pub set_env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub mkdir: Option<String>,
    #[serde(default)]
    pub cp: Option<CopyDef>,
    #[serde(default)]
    pub rm: Option<String>,
    #[serde(default)]
    pub once: Option<bool>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct WriteFileDef {
    pub path: String,
    pub content: Option<String>,
    pub template: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct PatchFileDef {
    pub path: String,
    pub after: Option<String>,
    pub before: Option<String>,
    pub replace: Option<String>,
    pub content: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct CopyDef {
    pub src: String,
    pub dest: String,
}

pub fn parse_blueprint<P: AsRef<Path>>(path: P) -> Result<BlueprintSchema> {
    let path = path.as_ref();
    let raw = starbase_utils::fs::read_file(path)
        .map_err(|e| miette!("Failed to read blueprint file {}: {}", path.display(), e))?;

    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

    match ext.to_lowercase().as_str() {
        "yaml" | "yml" => {
            serde_yaml::from_str(&raw)
                .map_err(|e| miette!("Invalid YAML in {}: {}", path.display(), e))
        }
        "jsonc" => {
            let stripped = json_comments::StripComments::new(raw.as_bytes());
            serde_json::from_reader(stripped)
                .map_err(|e| miette!("Invalid JSONC in {}: {}", path.display(), e))
        }
        _ => {
            serde_json::from_str(&raw)
                .map_err(|e| miette!("Invalid JSON in {}: {}", path.display(), e))
        }
    }
}
