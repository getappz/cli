//! Version check module for automatic update notifications.
//!
//! Checks for new versions of appz and displays notifications during command execution.
//! Uses caching to avoid frequent API calls.

use miette::{miette, Result};
use std::path::PathBuf;
use std::time::Duration;
use tracing::debug;

/// Cache duration for version checks (24 hours)
pub const CACHE_DURATION_DAILY: Duration = Duration::from_secs(60 * 60 * 24);

/// Check for new version and display notification if available.
///
/// This function is called during command execution to notify users of available updates.
/// It skips checking in CI environments and non-interactive terminals.
pub async fn check_for_new_version() -> Result<()> {
    // Skip in test environments or non-interactive terminals
    if std::env::var("CI").is_ok() || !atty::is(atty::Stream::Stdout) {
        return Ok(());
    }

    // Check cached version
    if let Some(latest_version) = get_latest_version_cached(CACHE_DURATION_DAILY).await? {
        let current_version = env!("CARGO_PKG_VERSION");

        if version_compare(&latest_version, current_version) > 0 {
            // New version available, show notification
            eprintln!(
                "\nℹ There's a new version of appz available, {} (currently on {})!",
                latest_version, current_version
            );
            eprintln!("  Run `appz self-update` to update appz\n");
        }
    }

    Ok(())
}

/// Get the latest version with caching support.
///
/// Checks cache first, and only fetches from API if cache is stale or missing.
pub async fn get_latest_version_cached(cache_duration: Duration) -> Result<Option<String>> {
    let cache_file = get_cache_file_path()?;

    // Check if cache exists and is fresh
    if let Some(age) = get_file_age(&cache_file) {
        if age < cache_duration {
            // Cache is fresh, try to read it
            if let Ok(cached_version) = starbase_utils::fs::read_file(&cache_file) {
                let cached_version = cached_version.trim().to_string();
                let current_version = env!("CARGO_PKG_VERSION");
                // Verify cached version is >= current version
                if version_compare(&cached_version, current_version) >= 0 {
                    debug!("Using cached version: {}", cached_version);
                    return Ok(Some(cached_version));
                }
            }
        }
    }

    // Cache miss or stale, fetch from API
    let version = get_latest_version_from_api().await?;

    // Update cache
    if let Some(ref version) = version {
        if let Some(parent) = cache_file.parent() {
            starbase_utils::fs::create_dir_all(parent)
                .map_err(|e| miette!("Failed to create cache directory: {}", e))?;
        }
        starbase_utils::fs::write_file(&cache_file, version)
            .map_err(|e| miette!("Failed to write version cache: {}", e))?;
    }

    Ok(version)
}

/// Configure a self_update Update builder with common settings.
///
/// This is a shared helper to avoid code duplication between version_check and self_upgrade.
/// Returns a configured Update builder that can be further customized.
#[cfg(feature = "self_update")]
pub fn configure_update_builder() -> self_update::backends::github::UpdateBuilder {
    use common::consts::github;
    use self_update::backends::github::Update;
    use self_update::cargo_crate_version;

    let mut update = Update::configure();
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        update.auth_token(&token);
    }

    update
        .repo_owner(&github::repo_owner())
        .repo_name(&github::repo_name())
        .bin_name("appz")
        .current_version(cargo_crate_version!());

    update
}

/// Fetch the latest version from GitHub Releases API.
pub async fn get_latest_version_from_api() -> Result<Option<String>> {
    #[cfg(feature = "self_update")]
    {
        // Configure update builder
        let update_builder = configure_update_builder();

        // Wrap blocking operations in spawn_blocking to avoid runtime issues
        let result = tokio::task::spawn_blocking(move || match update_builder.build() {
            Ok(built) => match built.get_latest_release() {
                Ok(release) => {
                    debug!("Fetched latest version from API: {}", release.version);
                    Ok(Some(release.version))
                }
                Err(e) => {
                    debug!("Failed to get latest release: {}", e);
                    Ok(None)
                }
            },
            Err(e) => {
                debug!("Failed to build update: {}", e);
                Ok(None)
            }
        })
        .await
        .map_err(|e| miette!("Task join error: {}", e))?;

        result
    }

    #[cfg(not(feature = "self_update"))]
    {
        // If self_update feature is disabled, we can't fetch versions
        Ok(None)
    }
}

/// Get the cache file path for storing latest version.
fn get_cache_file_path() -> Result<PathBuf> {
    use starbase_utils::dirs;
    let cache_dir = dirs::home_dir()
        .ok_or_else(|| miette::miette!("Could not determine home directory"))?
        .join(".appz")
        .join("cache");
    Ok(cache_dir.join("latest-version"))
}

/// Get the age of a file (time since last modification).
///
/// This is a synchronous helper that performs a quick metadata check.
/// It's called from async contexts but the operation is fast enough that
/// blocking briefly is acceptable.
fn get_file_age(path: &PathBuf) -> Option<Duration> {
    let metadata = starbase_utils::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    modified.elapsed().ok()
}

/// Compare two version strings.
///
/// Returns:
/// - `-1` if v1 < v2
/// - `0` if v1 == v2
/// - `1` if v1 > v2
fn version_compare(v1: &str, v2: &str) -> i32 {
    // Remove 'v' prefix if present
    let v1 = v1.strip_prefix('v').unwrap_or(v1);
    let v2 = v2.strip_prefix('v').unwrap_or(v2);

    // Simple string comparison (can be enhanced with semver crate if needed)
    match v1.cmp(v2) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}
