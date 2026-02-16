//! Plugin manifest: describes available plugins, their versions, tiers, and download URLs.
//!
//! The manifest is a static JSON file hosted on CDN, cached locally at
//! `~/.appz/plugins/manifest.json`.

use crate::error::{PluginError, PluginResult};
use reqwest::header::{HeaderValue, REFERER};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// How long to cache the manifest before re-fetching (in seconds).
const MANIFEST_CACHE_TTL_SECS: u64 = 3600; // 1 hour

/// Default CDN URL for the plugin manifest.
const DEFAULT_MANIFEST_URL: &str = "https://get.appz.dev/plugins/plugins.json";

/// Top-level manifest structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Schema version (currently 1).
    pub version: u32,
    /// Map of plugin name -> entry.
    pub plugins: HashMap<String, PluginEntry>,
}

/// Describes a single downloadable plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    /// Human-readable name.
    pub name: String,
    /// Short description shown in help.
    pub description: String,
    /// Semantic version.
    pub version: String,
    /// Minimum CLI version required to run this plugin.
    pub min_cli_version: String,
    /// Subscription tier required (e.g. "free", "pro", "enterprise").
    pub tier: String,
    /// URL to download the WASM binary.
    pub wasm_url: String,
    /// URL to download the detached Ed25519 signature.
    pub sig_url: String,
    /// SHA-256 checksum of the WASM binary (hex-encoded).
    pub checksum: String,
    /// CLI commands this plugin provides.
    pub commands: Vec<String>,
    /// Approximate size of the WASM binary in bytes.
    #[serde(default)]
    pub size_bytes: u64,
}

/// Cached manifest with timestamp.
#[derive(Debug, Serialize, Deserialize)]
struct CachedManifest {
    /// Unix timestamp when the manifest was fetched.
    fetched_at: u64,
    /// The manifest data.
    manifest: PluginManifest,
}

impl PluginManifest {
    /// Load manifest from cache, or fetch from CDN if stale/missing.
    pub async fn load(plugins_dir: &Path) -> PluginResult<Self> {
        let cache_path = plugins_dir.join("manifest.json");
        let manifest_url =
            std::env::var("APPZ_PLUGIN_MANIFEST_URL").unwrap_or_else(|_| DEFAULT_MANIFEST_URL.to_string());

        // Try loading from cache
        if let Some(manifest) = Self::load_from_cache(&cache_path)? {
            return Ok(manifest);
        }

        // Fetch from CDN
        tracing::debug!("Fetching plugin manifest from {}", manifest_url);
        let manifest = Self::fetch_from_url(&manifest_url).await?;

        // Cache it
        Self::save_to_cache(&cache_path, &manifest)?;

        Ok(manifest)
    }

    /// Load manifest from local cache if it exists and is not stale.
    fn load_from_cache(cache_path: &Path) -> PluginResult<Option<Self>> {
        if !cache_path.exists() {
            return Ok(None);
        }

        let data = std::fs::read_to_string(cache_path)?;
        let cached: CachedManifest = serde_json::from_str(&data)?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now - cached.fetched_at > MANIFEST_CACHE_TTL_SECS {
            tracing::debug!("Plugin manifest cache is stale, will re-fetch");
            return Ok(None);
        }

        tracing::debug!("Using cached plugin manifest");
        Ok(Some(cached.manifest))
    }

    /// Fetch manifest from a remote URL.
    async fn fetch_from_url(url: &str) -> PluginResult<Self> {
        let client = reqwest::Client::builder()
            .user_agent(
                HeaderValue::from_static("Mozilla/5.0 (compatible; Appz-CLI/0.1.0; +https://appz.dev)")
            )
            .default_headers({
                let mut h = reqwest::header::HeaderMap::new();
                h.insert(REFERER, HeaderValue::from_static("https://appz.dev/"));
                h
            })
            .build()
            .map_err(|e| PluginError::ManifestError {
                reason: format!("Failed to build HTTP client: {}", e),
            })?;

        let response = client.get(url).send().await.map_err(|e| PluginError::ManifestError {
            reason: format!("Failed to fetch manifest from {}: {}", url, e),
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let headers: Vec<_> = response
                .headers()
                .iter()
                .map(|(k, v)| format!("  {}: {:?}", k.as_str(), v.to_str().unwrap_or("(binary)")))
                .collect();
            let cf_challenge = response
                .headers()
                .get("cf-mitigated")
                .and_then(|v| v.to_str().ok())
                .map(|s| s == "challenge")
                .unwrap_or(false);
            let body = response.text().await.unwrap_or_else(|_| "(failed to read body)".to_string());
            let cf_challenge = cf_challenge || body.contains("Just a moment");

            let debug = format!(
                "HTTP {} {}\nHeaders:\n{}\nBody:\n{}",
                status.as_u16(),
                url,
                headers.join("\n"),
                if body.len() > 500 {
                    format!("{}... (truncated)", &body[..500])
                } else {
                    body
                }
            );
            eprintln!("{}", debug);

            let reason = if cf_challenge {
                format!(
                    "Manifest fetch returned HTTP {}: {}. \
                    Cloudflare is blocking this request with a bot challenge. \
                    Configure Cloudflare (Security > WAF) to allow User-Agent containing \"Appz-CLI\", \
                    or disable Bot Fight Mode for get.appz.dev.",
                    status.as_u16(),
                    url
                )
            } else {
                format!("Manifest fetch returned HTTP {}: {}", status.as_u16(), url)
            };

            return Err(PluginError::ManifestError { reason });
        }

        let manifest: PluginManifest =
            response.json().await.map_err(|e| PluginError::ManifestError {
                reason: format!("Invalid manifest JSON: {}", e),
            })?;

        Ok(manifest)
    }

    /// Save manifest to local cache.
    fn save_to_cache(cache_path: &Path, manifest: &PluginManifest) -> PluginResult<()> {
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let cached = CachedManifest {
            fetched_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            manifest: manifest.clone(),
        };

        let data = serde_json::to_string_pretty(&cached)?;
        std::fs::write(cache_path, data)?;
        Ok(())
    }

    /// Find the plugin entry that provides a given command.
    pub fn find_by_command(&self, command: &str) -> PluginResult<(&str, &PluginEntry)> {
        for (plugin_name, entry) in &self.plugins {
            if entry.commands.iter().any(|c| c == command) {
                return Ok((plugin_name, entry));
            }
        }
        Err(PluginError::PluginNotFound {
            command: command.to_string(),
        })
    }

    /// Get all available plugin entries (for --help display).
    pub fn all_entries(&self) -> impl Iterator<Item = (&str, &PluginEntry)> {
        self.plugins.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Return the local cache path for the manifest.
    pub fn cache_path(plugins_dir: &Path) -> PathBuf {
        plugins_dir.join("manifest.json")
    }
}
