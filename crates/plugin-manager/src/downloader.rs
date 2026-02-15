//! Plugin downloader: fetches WASM binaries and signatures from CDN,
//! verifies checksums, and stores them in the local plugin cache.

use crate::error::{PluginError, PluginResult};
use crate::manifest::PluginEntry;
use crate::security::PluginSecurity;
use std::path::{Path, PathBuf};

/// Downloads plugin WASM binaries from CDN and stores them locally.
pub struct PluginDownloader {
    plugins_dir: PathBuf,
}

impl PluginDownloader {
    pub fn new(plugins_dir: &Path) -> Self {
        Self {
            plugins_dir: plugins_dir.to_path_buf(),
        }
    }

    /// Download a plugin's WASM binary and signature, verify checksum,
    /// and store in the local cache directory.
    ///
    /// Returns the path to the downloaded WASM file.
    pub async fn download(&self, plugin_name: &str, entry: &PluginEntry) -> PluginResult<PathBuf> {
        let plugin_dir = self.plugins_dir.join(plugin_name).join(&entry.version);
        std::fs::create_dir_all(&plugin_dir)?;

        let wasm_path = plugin_dir.join("plugin.wasm");
        let sig_path = plugin_dir.join("plugin.wasm.sig");

        tracing::info!(
            "Downloading plugin '{}' v{} ({} bytes)...",
            plugin_name,
            entry.version,
            entry.size_bytes
        );

        // Download WASM binary
        Self::download_file(&entry.wasm_url, &wasm_path, plugin_name).await?;

        // Download signature
        Self::download_file(&entry.sig_url, &sig_path, plugin_name).await?;

        // Verify checksum
        PluginSecurity::verify_checksum(&wasm_path, &entry.checksum, plugin_name)?;

        tracing::info!("Plugin '{}' downloaded and verified", plugin_name);
        Ok(wasm_path)
    }

    /// Download a single file from a URL.
    async fn download_file(url: &str, dest: &Path, plugin_name: &str) -> PluginResult<()> {
        let response = reqwest::get(url).await.map_err(|e| PluginError::DownloadFailed {
            plugin: plugin_name.to_string(),
            reason: format!("HTTP request failed: {}", e),
        })?;

        if !response.status().is_success() {
            return Err(PluginError::DownloadFailed {
                plugin: plugin_name.to_string(),
                reason: format!("HTTP {}: {}", response.status().as_u16(), url),
            });
        }

        let bytes = response.bytes().await.map_err(|e| PluginError::DownloadFailed {
            plugin: plugin_name.to_string(),
            reason: format!("Failed to read response body: {}", e),
        })?;

        std::fs::write(dest, &bytes)?;

        tracing::debug!("Downloaded {} bytes to {:?}", bytes.len(), dest);
        Ok(())
    }
}
