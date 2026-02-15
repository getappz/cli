//! Entitlement checker: validates user subscription tiers against plugin requirements.
//!
//! Calls `GET /api/v1/plugins/entitlements` and caches the result locally with a TTL.

use crate::error::{PluginError, PluginResult};
use api::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

/// How long to cache entitlements before re-checking (in seconds).
const ENTITLEMENT_CACHE_TTL_SECS: u64 = 3600; // 1 hour

/// The "free" tier is always granted even without authentication.
const FREE_TIER: &str = "free";

/// Response from the entitlements API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementResponse {
    /// List of tier names the user has access to (e.g., ["free", "pro"]).
    pub tiers: Vec<String>,
}

/// Cached entitlements with expiry.
#[derive(Debug, Serialize, Deserialize)]
struct CachedEntitlements {
    /// Unix timestamp when entitlements were fetched.
    fetched_at: u64,
    /// The entitlement data.
    tiers: Vec<String>,
}

/// Checks whether the authenticated user is entitled to use a plugin.
pub struct EntitlementChecker {
    api_client: Arc<Client>,
    plugins_dir: std::path::PathBuf,
}

impl EntitlementChecker {
    pub fn new(api_client: Arc<Client>, plugins_dir: &Path) -> Self {
        Self {
            api_client,
            plugins_dir: plugins_dir.to_path_buf(),
        }
    }

    /// Check if the user is entitled to use a plugin with the given tier.
    pub async fn check(&self, plugin_name: &str, required_tier: &str) -> PluginResult<()> {
        // Free tier is always available
        if required_tier == FREE_TIER {
            return Ok(());
        }

        let tiers = self.get_tiers().await?;

        if tiers.iter().any(|t| t == required_tier) {
            Ok(())
        } else {
            Err(PluginError::EntitlementRequired {
                plugin: plugin_name.to_string(),
                tier: required_tier.to_string(),
            })
        }
    }

    /// Get the user's entitled tiers (from cache or API).
    async fn get_tiers(&self) -> PluginResult<Vec<String>> {
        let cache_path = self.plugins_dir.join("entitlements.json");

        // Try cache first
        if let Some(tiers) = Self::load_from_cache(&cache_path)? {
            return Ok(tiers);
        }

        // Fetch from API
        tracing::debug!("Fetching plugin entitlements from API");
        let tiers = self.fetch_from_api().await?;

        // Cache result
        Self::save_to_cache(&cache_path, &tiers)?;

        Ok(tiers)
    }

    /// Load entitlements from local cache if not expired.
    fn load_from_cache(cache_path: &Path) -> PluginResult<Option<Vec<String>>> {
        if !cache_path.exists() {
            return Ok(None);
        }

        let data = std::fs::read_to_string(cache_path)?;
        let cached: CachedEntitlements = match serde_json::from_str(&data) {
            Ok(c) => c,
            Err(_) => return Ok(None), // corrupt cache, re-fetch
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now - cached.fetched_at > ENTITLEMENT_CACHE_TTL_SECS {
            tracing::debug!("Entitlement cache is stale, will re-fetch");
            return Ok(None);
        }

        tracing::debug!("Using cached entitlements: {:?}", cached.tiers);
        Ok(Some(cached.tiers))
    }

    /// Fetch entitlements from the Appz API.
    async fn fetch_from_api(&self) -> PluginResult<Vec<String>> {
        let response: EntitlementResponse = self
            .api_client
            .get("/api/v1/plugins/entitlements")
            .await
            .map_err(|e| PluginError::Other(format!("Failed to fetch entitlements: {}", e)))?;

        Ok(response.tiers)
    }

    /// Save entitlements to local cache.
    fn save_to_cache(cache_path: &Path, tiers: &[String]) -> PluginResult<()> {
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let cached = CachedEntitlements {
            fetched_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            tiers: tiers.to_vec(),
        };

        let data = serde_json::to_string_pretty(&cached)?;
        std::fs::write(cache_path, data)?;
        Ok(())
    }
}
