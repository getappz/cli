//! On-demand WASM plugin manager for the appz CLI.
//!
//! Handles plugin discovery (from CDN manifest), subscription-gated downloads,
//! Ed25519 signature verification, WASM header validation, caching, and
//! sandbox-isolated execution.
//!
//! # Architecture
//!
//! ```text
//! PluginManager (facade)
//!   ├── PluginManifest     — CDN manifest fetch + local cache
//!   ├── EntitlementChecker — subscription validation via API
//!   ├── PluginDownloader   — WASM + sig download from CDN
//!   ├── PluginCache        — local version cache (~/.appz/plugins/)
//!   └── PluginSecurity     — Ed25519, header, handshake verification
//! ```

pub mod cache;
pub mod downloader;
pub mod entitlements;
pub mod error;
pub mod manifest;
pub mod security;
pub mod update_check;

use api::Client;
use cache::PluginCache;
use downloader::PluginDownloader;
use entitlements::EntitlementChecker;
use error::{PluginError, PluginResult};
use manifest::{PluginEntry, PluginManifest};
use security::PluginSecurity;
use starbase_utils::{dirs, fs};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A verified plugin ready to be loaded by the WASM runtime.
pub struct VerifiedPlugin {
    /// Path to the WASM binary on disk.
    pub wasm_path: PathBuf,
    /// Plugin manifest entry (metadata).
    pub entry: PluginEntry,
    /// Plugin name (key in manifest).
    pub name: String,
}

/// Facade that ties together all plugin management components.
pub struct PluginManager {
    manifest: PluginManifest,
    entitlements: EntitlementChecker,
    downloader: PluginDownloader,
    cache: PluginCache,
    plugins_dir: PathBuf,
}

impl PluginManager {
    /// Create a new PluginManager.
    ///
    /// Loads the manifest from cache/CDN and initializes all subcomponents.
    pub async fn new(api_client: Arc<Client>) -> PluginResult<Self> {
        let plugins_dir = Self::default_plugins_dir()?;
        Self::with_plugins_dir(api_client, &plugins_dir).await
    }

    /// Create a new PluginManager with a custom plugins directory.
    pub async fn with_plugins_dir(
        api_client: Arc<Client>,
        plugins_dir: &Path,
    ) -> PluginResult<Self> {
        fs::create_dir_all(plugins_dir).map_err(|e| PluginError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

        let manifest = PluginManifest::load(plugins_dir).await?;
        let entitlements = EntitlementChecker::new(api_client, plugins_dir);
        let downloader = PluginDownloader::new(plugins_dir);
        let cache = PluginCache::new(plugins_dir);

        Ok(Self {
            manifest,
            entitlements,
            downloader,
            cache,
            plugins_dir: plugins_dir.to_path_buf(),
        })
    }

    /// Ensure a plugin is available for a given command.
    ///
    /// This is the main entry point. It:
    /// 1. Looks up the command in the manifest
    /// 2. Checks subscription entitlement
    /// 3. Downloads the plugin if not cached
    /// 4. Verifies the signature and header
    /// 5. Checks CLI version compatibility
    ///
    /// Returns a `VerifiedPlugin` ready for loading.
    pub async fn ensure_plugin(&self, command_name: &str) -> PluginResult<VerifiedPlugin> {
        // 0. Check for local dev plugin override (e.g. APPZ_DEV_PLUGIN_CHECK)
        let env_key = format!(
            "APPZ_DEV_PLUGIN_{}",
            command_name.to_uppercase().replace('-', "_")
        );
        if let Ok(path) = std::env::var(&env_key) {
            let wasm_path = std::path::PathBuf::from(&path);
            if wasm_path.exists() {
                tracing::debug!("Using dev plugin from {}: {}", env_key, path);
                let entry = PluginEntry {
                    name: command_name.to_string(),
                    description: format!("Local dev build of {} plugin", command_name),
                    version: "dev".to_string(),
                    min_cli_version: "0.1.0".to_string(),
                    tier: "free".to_string(),
                    wasm_url: String::new(),
                    sig_url: String::new(),
                    checksum: String::new(),
                    commands: vec![command_name.to_string()],
                    size_bytes: 0,
                };
                return Ok(VerifiedPlugin {
                    wasm_path,
                    entry,
                    name: command_name.to_string(),
                });
            }
        }

        // 1. Find plugin in manifest
        let (plugin_name, entry) = self.manifest.find_by_command(command_name)?;
        let plugin_name = plugin_name.to_string();
        let entry = entry.clone();

        // 2. Check entitlement
        self.entitlements
            .check(&plugin_name, &entry.tier)
            .await?;

        // 3. Check cache or download
        let wasm_path = if let Some(path) = self.cache.get(&plugin_name, &entry.version) {
            path
        } else {
            let path = self.downloader.download(&plugin_name, &entry).await?;
            // Clean up old versions
            let _ = self.cache.cleanup_old_versions(&plugin_name, &entry.version);
            path
        };

        // 4. Verify signature
        PluginSecurity::verify_signature(&wasm_path, &plugin_name)?;

        // 5. Validate WASM header
        let wasm_bytes = fs::read_file_bytes(&wasm_path).map_err(|e| PluginError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        let header = PluginSecurity::validate_header(&wasm_bytes, &plugin_name)?;

        // 6. Check CLI version compatibility
        PluginSecurity::check_cli_version(&header.min_cli_version, &plugin_name)?;

        Ok(VerifiedPlugin {
            wasm_path,
            entry,
            name: plugin_name,
        })
    }

    /// Check if a command is provided by any plugin in the manifest.
    pub fn has_command(&self, command: &str) -> bool {
        self.manifest.find_by_command(command).is_ok()
    }

    /// Get plugin manifest entries for help display.
    pub fn available_plugins(&self) -> Vec<(&str, &PluginEntry)> {
        self.manifest.all_entries().collect()
    }

    /// Get the default plugins directory (~/.appz/plugins/).
    pub fn default_plugins_dir() -> PluginResult<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| {
            PluginError::Other("Could not determine home directory".to_string())
        })?;
        Ok(home.join(".appz").join("plugins"))
    }

    /// Get a reference to the manifest.
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// Get the plugins directory path.
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    /// Force-refresh the manifest from CDN, bypassing the cache TTL.
    pub async fn force_refresh_manifest(&mut self) -> PluginResult<()> {
        // Remove the cached manifest so the next load hits the CDN
        let cache_path = PluginManifest::cache_path(&self.plugins_dir);
        if cache_path.exists() {
            fs::remove_file(&cache_path).map_err(|e| PluginError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        }
        self.manifest = PluginManifest::load(&self.plugins_dir).await?;
        Ok(())
    }

    /// Update a single plugin (or all plugins if `name` is None).
    ///
    /// Re-downloads even if the cached version matches the manifest.
    /// Returns a list of what was updated.
    pub async fn update_plugins(
        &self,
        name: Option<&str>,
    ) -> PluginResult<Vec<update_check::InstalledPlugin>> {
        let mut results = Vec::new();

        let entries: Vec<(String, PluginEntry)> = if let Some(name) = name {
            // Find the specific plugin
            let entry = self
                .manifest
                .plugins
                .get(name)
                .cloned()
                .ok_or_else(|| PluginError::PluginNotFound {
                    command: name.to_string(),
                })?;
            vec![(name.to_string(), entry)]
        } else {
            // All plugins
            self.manifest
                .plugins
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };

        for (plugin_name, entry) in entries {
            let old_versions = self.cache.list_versions(&plugin_name).unwrap_or_default();
            let old_version = old_versions.first().cloned().unwrap_or_default();
            let has_update = !old_version.is_empty() && old_version != entry.version;

            // Always re-download to get the latest
            if self.cache.get(&plugin_name, &entry.version).is_none() || has_update {
                let _ = self.downloader.download(&plugin_name, &entry).await?;
                let _ = self.cache.cleanup_old_versions(&plugin_name, &entry.version);
            }

            results.push(update_check::InstalledPlugin {
                name: plugin_name,
                version: entry.version.clone(),
                manifest_version: entry.version,
                has_update,
            });
        }

        Ok(results)
    }

    /// List all installed (cached) plugins with their versions.
    pub fn installed_plugins(&self) -> Vec<update_check::InstalledPlugin> {
        let mut results = Vec::new();

        for (plugin_name, entry) in &self.manifest.plugins {
            let versions = self.cache.list_versions(plugin_name).unwrap_or_default();
            let installed_version = versions.first().cloned().unwrap_or_else(|| "—".to_string());
            let has_update =
                installed_version != "—" && installed_version != entry.version;

            results.push(update_check::InstalledPlugin {
                name: plugin_name.clone(),
                version: installed_version,
                manifest_version: entry.version.clone(),
                has_update,
            });
        }

        results.sort_by(|a, b| a.name.cmp(&b.name));
        results
    }

    /// Get the default cache directory (~/.appz/cache/).
    pub fn default_cache_dir() -> PluginResult<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| {
            PluginError::Other("Could not determine home directory".to_string())
        })?;
        Ok(home.join(".appz").join("cache"))
    }
}
