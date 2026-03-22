//! GitHub blueprint registry client — URL resolution, type definitions, and optional caching.

use serde::Deserialize;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const REGISTRY_REPO: &str = "getappz/blueprints";
pub const REGISTRY_BRANCH: &str = "main";

const RAW_BASE: &str = "https://raw.githubusercontent.com";

// ---------------------------------------------------------------------------
// Registry index types
// ---------------------------------------------------------------------------

/// Top-level registry.json structure.
#[derive(Debug, Deserialize)]
pub struct RegistryIndex {
    pub version: u32,
    pub frameworks: HashMap<String, FrameworkEntry>,
    #[serde(default)]
    pub deploy: HashMap<String, DeployTargetEntry>,
}

impl RegistryIndex {
    /// Returns `true` if `framework` exists in the index.
    pub fn has_framework(&self, framework: &str) -> bool {
        self.frameworks.contains_key(framework)
    }

    /// Returns `true` if both `framework` and `blueprint` exist in the index.
    pub fn has_blueprint(&self, framework: &str, blueprint: &str) -> bool {
        self.frameworks
            .get(framework)
            .map(|f| f.blueprints.contains_key(blueprint))
            .unwrap_or(false)
    }

    /// Returns `true` if a deploy target exists.
    pub fn has_deploy_target(&self, target: &str) -> bool {
        self.deploy.contains_key(target)
    }
}

/// Per-deploy-target entry in the registry.
#[derive(Debug, Deserialize)]
pub struct DeployTargetEntry {
    pub description: String,
}

/// Per-framework entry in the registry.
#[derive(Debug, Deserialize)]
pub struct FrameworkEntry {
    pub name: String,
    pub blueprints: HashMap<String, BlueprintEntry>,
}

/// Per-blueprint entry in the registry.
#[derive(Debug, Deserialize)]
pub struct BlueprintEntry {
    pub description: String,
}

// ---------------------------------------------------------------------------
// URL helpers
// ---------------------------------------------------------------------------

/// Returns the raw GitHub URL for `registry.json`.
pub fn registry_index_url() -> String {
    format!(
        "{RAW_BASE}/{REGISTRY_REPO}/{REGISTRY_BRANCH}/registry.json"
    )
}

/// Returns the raw GitHub URL for a blueprint's `blueprint.yaml`.
///
/// e.g. `resolve_blueprint_url("nextjs", "ecommerce")`
/// → `https://raw.githubusercontent.com/AviHS/appz-blueprints/main/nextjs/ecommerce/blueprint.yaml`
pub fn resolve_blueprint_url(framework: &str, blueprint: &str) -> String {
    format!(
        "{RAW_BASE}/{REGISTRY_REPO}/{REGISTRY_BRANCH}/{framework}/{blueprint}/blueprint.yaml"
    )
}

/// Returns the raw GitHub URL for a named template file inside a blueprint directory.
///
/// e.g. `resolve_template_url("nextjs", "ecommerce", "template.zip")`
pub fn resolve_template_url(framework: &str, blueprint: &str, template: &str) -> String {
    format!(
        "{RAW_BASE}/{REGISTRY_REPO}/{REGISTRY_BRANCH}/{framework}/{blueprint}/{template}"
    )
}

// ---------------------------------------------------------------------------
// Cache helpers
// ---------------------------------------------------------------------------

/// Returns the path used for the local registry cache: `~/.appz/cache/registry.json`.
fn cache_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".appz").join("cache").join("registry.json"))
}

/// Returns `true` if `path` exists and was modified within the last 24 hours.
fn cache_is_fresh(path: &std::path::Path) -> bool {
    path.metadata()
        .and_then(|m| m.modified())
        .map(|modified| {
            modified
                .elapsed()
                .map(|age| age < std::time::Duration::from_secs(24 * 60 * 60))
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

/// Try reading a cached registry index (even if stale).
fn read_cached_index(path: &std::path::Path) -> Result<RegistryIndex, ()> {
    let raw = std::fs::read_to_string(path).map_err(|_| ())?;
    serde_json::from_str(&raw).map_err(|_| ())
}

// ---------------------------------------------------------------------------
// RegistryClient
// ---------------------------------------------------------------------------

/// Async client for the appz blueprint registry.
pub struct RegistryClient {
    http: reqwest::Client,
}

impl RegistryClient {
    /// Creates a new `RegistryClient` with a default `reqwest::Client`.
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Creates a `RegistryClient` backed by the given `reqwest::Client`.
    pub fn with_client(http: reqwest::Client) -> Self {
        Self { http }
    }

    /// Fetches the registry index.
    ///
    /// - If `no_cache` is `false` and a fresh cache exists, returns the cached copy.
    /// - Otherwise fetches from GitHub and (if possible) writes the result to
    ///   `~/.appz/cache/registry.json` for future use.
    pub async fn fetch_index(&self, no_cache: bool) -> miette::Result<RegistryIndex> {
        use miette::IntoDiagnostic;

        // Try fresh cache first (skip if --no-cache)
        if !no_cache {
            if let Some(cp) = cache_path() {
                if cache_is_fresh(&cp) {
                    if let Ok(index) = read_cached_index(&cp) {
                        return Ok(index);
                    }
                }
            }
        }

        // Try fetching from network
        let url = registry_index_url();
        let fetch_result = async {
            let response = self.http.get(&url).send().await.into_diagnostic()?;
            let status = response.status();
            if !status.is_success() {
                return Err(miette::miette!(
                    "HTTP {status} from {url}"
                ));
            }
            let raw = response.text().await.into_diagnostic()?;
            let index: RegistryIndex = serde_json::from_str(&raw).into_diagnostic()?;

            // Best-effort cache write
            if let Some(cp) = cache_path() {
                if let Some(parent) = cp.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&cp, &raw);
            }

            Ok(index)
        }
        .await;

        match fetch_result {
            Ok(index) => Ok(index),
            Err(network_err) => {
                // Network failed — try stale cache as fallback
                if let Some(cp) = cache_path() {
                    if let Ok(index) = read_cached_index(&cp) {
                        eprintln!(
                            "Warning: Could not refresh registry ({}). Using cached data.",
                            network_err
                        );
                        return Ok(index);
                    }
                }
                // No cache at all — return helpful error
                Err(miette::miette!(
                    "Could not fetch blueprint registry ({}).\n\nAlternatives:\n  appz init <framework> --blueprint ./local-blueprint.yaml\n  appz init <framework> --blueprint https://example.com/blueprint.yaml",
                    network_err
                ))
            }
        }
    }

    /// Fetches a blueprint's `blueprint.yaml` as a raw string.
    pub async fn fetch_blueprint(
        &self,
        framework: &str,
        blueprint: &str,
    ) -> miette::Result<String> {
        use miette::IntoDiagnostic;

        let url = resolve_blueprint_url(framework, blueprint);
        let result = async {
            let response = self.http.get(&url).send().await.into_diagnostic()?;
            let status = response.status();
            if !status.is_success() {
                return Err(miette::miette!("HTTP {status}"));
            }
            response.text().await.into_diagnostic()
        }
        .await;

        result.map_err(|e| {
            miette::miette!(
                "Could not fetch blueprint '{}/{}' ({}).\n\nAlternatives:\n  appz init {} --blueprint ./local-blueprint.yaml\n  appz init {} --blueprint https://example.com/blueprint.yaml",
                framework, blueprint, e, framework, framework
            )
        })
    }

    /// Fetches an arbitrary template file inside a blueprint directory as raw bytes.
    pub async fn fetch_template(
        &self,
        framework: &str,
        blueprint: &str,
        template: &str,
    ) -> miette::Result<bytes::Bytes> {
        use miette::IntoDiagnostic;

        let url = resolve_template_url(framework, blueprint, template);
        let response = self
            .http
            .get(&url)
            .send()
            .await
            .into_diagnostic()?;

        let status = response.status();
        if !status.is_success() {
            return Err(miette::miette!(
                "Failed to fetch template {framework}/{blueprint}/{template} from {url}: HTTP {status}"
            ));
        }

        response.bytes().await.into_diagnostic()
    }
}

impl Default for RegistryClient {
    fn default() -> Self {
        Self::new()
    }
}
