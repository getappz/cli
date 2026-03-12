//! Global user configuration (~/.appz/config.json).
//! Stores preferences like telemetry enable/disable (Vercel-aligned).

use miette::Result;
use serde::{Deserialize, Serialize};
use starbase_utils::dirs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry: Option<TelemetryConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Whether telemetry collection is enabled. Default: true when not set.
    pub enabled: bool,
}

impl UserConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether telemetry is enabled. Defaults to true when not explicitly disabled.
    pub fn telemetry_enabled(&self) -> bool {
        self.telemetry.as_ref().is_none_or(|t| t.enabled)
    }
}

/// Get the path to the config.json file
pub fn get_config_path() -> Result<PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| miette::miette!("Could not determine home directory"))?;
    Ok(home_dir.join(".appz").join("config.json"))
}

/// Load user config from ~/.appz/config.json
pub fn load_config() -> Result<UserConfig> {
    let config_path = get_config_path()?;

    if !config_path.exists() {
        return Ok(UserConfig::new());
    }

    use starbase_utils::{fs, json};

    match json::read_file(&config_path) {
        Ok(config) => Ok(config),
        Err(e) => {
            if let Ok(content) = fs::read_file(&config_path) {
                if content.trim().is_empty() {
                    return Ok(UserConfig::new());
                }
            }
            Err(miette::miette!(
                "Failed to read config file {}: {}",
                common::user_config::path_for_display(&config_path),
                e
            ))
        }
    }
}

/// Save user config to ~/.appz/config.json
pub fn save_config(config: &UserConfig) -> Result<()> {
    let config_path = get_config_path()?;
    let config_dir = config_path
        .parent()
        .ok_or_else(|| miette::miette!("Invalid config path"))?;

    use starbase_utils::{fs, json};

    if !config_dir.exists() {
        fs::create_dir_all(config_dir).map_err(|e| {
            miette::miette!(
                "Failed to create directory {}: {}",
                common::user_config::path_for_display(config_dir),
                e
            )
        })?;
    }

    json::write_file(&config_path, config, true).map_err(|e| {
        miette::miette!(
            "Failed to write config file {}: {}",
            common::user_config::path_for_display(&config_path),
            e
        )
    })?;

    Ok(())
}
