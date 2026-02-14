//! Read local project configuration (appz.json)
//!
//! Similar to Vercel's read-config.ts for vercel.json

use crate::project::ProjectSettings;
use miette::{miette, Result};
use serde::{Deserialize, Serialize};
use starbase_utils::json;
use std::path::Path;

/// Local project configuration (appz.json)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalConfig {
    #[serde(rename = "buildCommand")]
    pub build_command: Option<String>,
    #[serde(rename = "devCommand")]
    pub dev_command: Option<String>,
    #[serde(rename = "installCommand")]
    pub install_command: Option<String>,
    #[serde(rename = "outputDirectory")]
    pub output_directory: Option<String>,
    pub framework: Option<String>,
    #[serde(rename = "rootDirectory")]
    pub root_directory: Option<String>,
    /// Deployment configuration for hosting providers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deploy: Option<deployer::DeployConfig>,
    /// Check/lint configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<checker::CheckConfig>,
}

/// Read local configuration from appz.json (sync version)
///
/// Returns None if file doesn't exist or is invalid.
#[allow(dead_code)]
pub fn read_config(project_path: &Path) -> Result<Option<ProjectSettings>> {
    let config_path = project_path.join("appz.json");

    if !config_path.exists() {
        return Ok(None);
    }

    let local_config: LocalConfig = json::read_file(&config_path)
        .map_err(|e| miette!("Failed to read/parse appz.json: {}", e))?;

    let mut settings = ProjectSettings::default();
    settings.build_command = local_config.build_command;
    settings.dev_command = local_config.dev_command;
    settings.install_command = local_config.install_command;
    settings.output_directory = local_config.output_directory;
    settings.framework = local_config.framework;
    settings.root_directory = local_config.root_directory;

    Ok(Some(settings))
}

/// Read local configuration from appz.json (async version)
///
/// Returns None if file doesn't exist or is invalid.
/// Uses spawn_blocking for file I/O in async context (following workspace rules).
pub async fn read_config_async(project_path: &Path) -> Result<Option<ProjectSettings>> {
    let config_path = project_path.join("appz.json");

    // Use spawn_blocking for file I/O in async context (following workspace rules)
    let config_path_clone = config_path.clone();
    let exists = tokio::task::spawn_blocking(move || config_path_clone.exists())
        .await
        .map_err(|e| miette!("Failed to check file existence: {}", e))?;

    if !exists {
        return Ok(None);
    }

    let config_path_clone = config_path.clone();
    let local_config: LocalConfig = tokio::task::spawn_blocking(move || {
        json::read_file(&config_path_clone)
            .map_err(|e| miette!("Failed to read/parse appz.json: {}", e))
    })
    .await
    .map_err(|e| miette!("Failed to read file: {}", e))??;

    let mut settings = ProjectSettings::default();
    settings.build_command = local_config.build_command;
    settings.dev_command = local_config.dev_command;
    settings.install_command = local_config.install_command;
    settings.output_directory = local_config.output_directory;
    settings.framework = local_config.framework;
    settings.root_directory = local_config.root_directory;

    Ok(Some(settings))
}
