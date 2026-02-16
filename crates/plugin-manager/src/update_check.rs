//! Periodic plugin update check with daily caching.
//!
//! After a plugin executes, the CLI can show a non-intrusive hint suggesting
//! `appz plugin update`. The hint is suppressed for 7 days (or in CI) so
//! it doesn't annoy users on every run.

use crate::error::{PluginError, PluginResult};
use serde::{Deserialize, Serialize};
use starbase_utils::fs;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How often to show the "run `appz plugin update`" hint (7 days).
const HINT_TTL_SECS: u64 = 60 * 60 * 24 * 7;

/// State persisted at `~/.appz/cache/plugin-updates.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct UpdateState {
    /// Unix timestamp of the last time we showed an update hint.
    #[serde(default)]
    last_hint_shown: u64,
    /// Per-plugin: unix timestamp of the last time we checked for updates.
    #[serde(default)]
    plugin_checks: HashMap<String, u64>,
}

/// Lightweight checker that decides whether to show an update hint.
pub struct PluginUpdateChecker {
    state_path: PathBuf,
}

impl PluginUpdateChecker {
    /// Create a checker that stores state in the given cache directory.
    ///
    /// Typically `~/.appz/cache`.
    pub fn new(cache_dir: &Path) -> Self {
        Self {
            state_path: cache_dir.join("plugin-updates.json"),
        }
    }

    /// Determine whether to show the update hint to the user.
    ///
    /// Returns `true` if:
    /// - We're in an interactive terminal (not CI)
    /// - More than 7 days have passed since the last hint
    pub fn should_show_hint(&self) -> bool {
        // Skip in CI or non-interactive terminals
        if std::env::var("CI").is_ok() || !atty::is(atty::Stream::Stderr) {
            return false;
        }

        let state = self.load_state();
        let now = now_secs();

        now.saturating_sub(state.last_hint_shown) > HINT_TTL_SECS
    }

    /// Record that the hint was shown so it won't appear again for 7 days.
    pub fn record_hint_shown(&self) {
        let mut state = self.load_state();
        state.last_hint_shown = now_secs();
        let _ = self.save_state(&state);
    }

    /// Print the update hint to stderr (non-blocking, won't fail the command).
    pub fn show_hint() {
        eprintln!(
            "\n  Tip: Run `appz plugin update` to check for plugin updates.\n"
        );
    }

    // ── internal ──────────────────────────────────────────────────────

    fn load_state(&self) -> UpdateState {
        starbase_utils::json::read_file(&self.state_path).unwrap_or_default()
    }

    fn save_state(&self, state: &UpdateState) -> PluginResult<()> {
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent).map_err(|e| PluginError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        }
        starbase_utils::json::write_file(&self.state_path, state, true)
            .map_err(|e| PluginError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        Ok(())
    }
}

/// Info about an installed (cached) plugin.
#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    pub name: String,
    pub version: String,
    pub manifest_version: String,
    pub has_update: bool,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}
