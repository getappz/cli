//! Plugin cache: manages locally stored WASM plugins, version tracking,
//! and cleanup of old versions.

use crate::error::PluginResult;
use std::path::{Path, PathBuf};

/// Manages the local plugin cache at `~/.appz/plugins/`.
pub struct PluginCache {
    plugins_dir: PathBuf,
}

impl PluginCache {
    pub fn new(plugins_dir: &Path) -> Self {
        Self {
            plugins_dir: plugins_dir.to_path_buf(),
        }
    }

    /// Get the path to a cached plugin WASM, if it exists.
    pub fn get(&self, plugin_name: &str, version: &str) -> Option<PathBuf> {
        let wasm_path = self
            .plugins_dir
            .join(plugin_name)
            .join(version)
            .join("plugin.wasm");

        if wasm_path.exists() {
            tracing::debug!("Plugin '{}' v{} found in cache", plugin_name, version);
            Some(wasm_path)
        } else {
            tracing::debug!("Plugin '{}' v{} not in cache", plugin_name, version);
            None
        }
    }

    /// Check if a plugin exists in the cache (any version).
    pub fn has_any_version(&self, plugin_name: &str) -> bool {
        let plugin_dir = self.plugins_dir.join(plugin_name);
        plugin_dir.exists() && plugin_dir.is_dir()
    }

    /// List all cached versions of a plugin.
    pub fn list_versions(&self, plugin_name: &str) -> PluginResult<Vec<String>> {
        let plugin_dir = self.plugins_dir.join(plugin_name);
        if !plugin_dir.exists() {
            return Ok(Vec::new());
        }

        let mut versions = Vec::new();
        for entry in std::fs::read_dir(&plugin_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    // Check that the version directory actually contains a plugin
                    let wasm_path = entry.path().join("plugin.wasm");
                    if wasm_path.exists() {
                        versions.push(name.to_string());
                    }
                }
            }
        }

        Ok(versions)
    }

    /// Remove old versions of a plugin, keeping only the specified current version.
    pub fn cleanup_old_versions(
        &self,
        plugin_name: &str,
        current_version: &str,
    ) -> PluginResult<()> {
        let versions = self.list_versions(plugin_name)?;

        for version in versions {
            if version != current_version {
                let version_dir = self.plugins_dir.join(plugin_name).join(&version);
                tracing::debug!(
                    "Removing old plugin version: {} v{}",
                    plugin_name,
                    version
                );
                std::fs::remove_dir_all(&version_dir)?;
            }
        }

        Ok(())
    }

    /// Remove a plugin entirely from the cache.
    pub fn remove(&self, plugin_name: &str) -> PluginResult<()> {
        let plugin_dir = self.plugins_dir.join(plugin_name);
        if plugin_dir.exists() {
            std::fs::remove_dir_all(&plugin_dir)?;
            tracing::debug!("Removed plugin '{}' from cache", plugin_name);
        }
        Ok(())
    }

    /// Ensure the plugins directory exists.
    pub fn ensure_dir(&self) -> PluginResult<()> {
        std::fs::create_dir_all(&self.plugins_dir)?;
        Ok(())
    }
}
