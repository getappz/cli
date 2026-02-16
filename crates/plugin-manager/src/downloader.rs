//! Plugin downloader: fetches WASM binaries and signatures from CDN,
//! verifies checksums, and stores them in the local plugin cache.

use crate::error::{PluginError, PluginResult};
use crate::manifest::PluginEntry;
use crate::security::PluginSecurity;
use grab::{download_to_path, DownloadOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

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

        let options = DownloadOptions {
            timeout: Duration::from_secs(120),
            user_agent: "appz-cli".to_string(),
            parallel_threshold_bytes: 5 * 1024 * 1024,
            max_concurrent_chunks: 4,
            chunk_size: 1024 * 1024,
            resume: false,
            headers: None,
        };

        download_to_path(&entry.wasm_url, &wasm_path, options.clone(), None).await.map_err(
            |e| PluginError::DownloadFailed {
                plugin: plugin_name.to_string(),
                reason: format!("WASM download failed: {}", e),
            },
        )?;

        download_to_path(&entry.sig_url, &sig_path, options, None).await.map_err(|e| {
            PluginError::DownloadFailed {
                plugin: plugin_name.to_string(),
                reason: format!("Signature download failed: {}", e),
            }
        })?;

        PluginSecurity::verify_checksum(&wasm_path, &entry.checksum, plugin_name)?;

        tracing::info!("Plugin '{}' downloaded and verified", plugin_name);
        Ok(wasm_path)
    }
}
