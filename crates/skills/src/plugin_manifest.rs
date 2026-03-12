//! Plugin manifest discovery for skills.
//!
//! Reads .claude-plugin/marketplace.json and plugin.json to find skill paths.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct MarketplaceManifest {
    metadata: Option<MarketplaceMetadata>,
    plugins: Option<Vec<PluginEntry>>,
}

#[derive(Deserialize)]
struct MarketplaceMetadata {
    #[serde(rename = "pluginRoot")]
    plugin_root: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PluginSource {
    String(String),
    Object { source: String, repo: Option<String> },
}

#[derive(Deserialize)]
struct PluginEntry {
    source: Option<PluginSource>,
    skills: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct PluginManifest {
    skills: Option<Vec<String>>,
}

fn is_contained_in(target: &Path, base: &Path) -> bool {
    match (target.canonicalize(), base.canonicalize()) {
        (Ok(t), Ok(b)) => t.starts_with(&b) || t == b,
        _ => target.starts_with(base) || target == base,
    }
}

/// Extract skill search directories from plugin manifests.
pub fn get_plugin_skill_paths(base_path: &Path) -> Vec<PathBuf> {
    let mut search_dirs = Vec::new();

    let mut add_plugin_skills = |plugin_base: &Path, skills: Option<&Vec<String>>| {
        if !is_contained_in(plugin_base, base_path) {
            return;
        }
        if let Some(skills) = skills {
            for skill_path in skills {
                if skill_path.starts_with("./") {
                    let full = plugin_base.join(skill_path);
                    let skill_dir = full.parent().unwrap_or(plugin_base);
                    if is_contained_in(skill_dir, base_path) {
                        search_dirs.push(skill_dir.to_path_buf());
                    }
                }
            }
        }
        search_dirs.push(plugin_base.join("skills"));
    };

    let marketplace_path = base_path.join(".claude-plugin").join("marketplace.json");
    if marketplace_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&marketplace_path) {
            if let Ok(manifest) = serde_json::from_str::<MarketplaceManifest>(&content) {
                let plugin_root = manifest
                    .metadata
                    .as_ref()
                    .and_then(|m| m.plugin_root.as_ref())
                    .filter(|r| r.starts_with("./"))
                    .map(|r| base_path.join(r.trim_start_matches("./")))
                    .unwrap_or_else(|| base_path.to_path_buf());
                for plugin in manifest.plugins.unwrap_or_default() {
                    if let Some(PluginSource::String(source)) = plugin.source {
                        if source.starts_with("./") {
                            let plugin_base = plugin_root.join(source.trim_start_matches("./"));
                            add_plugin_skills(&plugin_base, plugin.skills.as_ref());
                        }
                    }
                }
            }
        }
    }

    let plugin_path = base_path.join(".claude-plugin").join("plugin.json");
    if plugin_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&plugin_path) {
            if let Ok(manifest) = serde_json::from_str::<PluginManifest>(&content) {
                add_plugin_skills(base_path, manifest.skills.as_ref());
            }
        }
    }

    search_dirs
}
