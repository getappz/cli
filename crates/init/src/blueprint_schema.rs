//! Blueprint file schema — the unified format for scaffolding + operations.

use miette::{miette, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

fn is_false(v: &bool) -> bool { !v }
fn is_empty_vec<T>(v: &Vec<T>) -> bool { v.is_empty() }

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct BlueprintSchema {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<BlueprintMeta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosts: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<Vec<SetupStep>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasks: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<HashMap<String, Vec<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<HashMap<String, Vec<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub includes: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct BlueprintMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    #[serde(default, skip_serializing_if = "is_empty_vec")]
    pub categories: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_manager: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct SetupStep {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_locally: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub add_dependency: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dev: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_file: Option<WriteFileDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_file: Option<PatchFileDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_env: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mkdir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cp: Option<CopyDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub once: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct WriteFileDef {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct PatchFileDef {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replace: Option<String>,
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
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
