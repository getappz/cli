//! Unified deployment configuration types.
//!
//! The deployer reads its configuration from the `deploy` section of
//! `appz.json`. Each provider has its own config under `deploy.targets.<slug>`.
//!
//! # Example `appz.json`
//!
//! ```json
//! {
//!   "buildCommand": "npm run build",
//!   "outputDirectory": "dist",
//!   "deploy": {
//!     "default": "vercel",
//!     "targets": {
//!       "vercel": { "projectName": "my-app", "team": "my-team" },
//!       "netlify": { "siteId": "abc-123" }
//!     },
//!     "env": {
//!       "production": { "API_URL": "https://api.example.com" },
//!       "preview": { "API_URL": "https://staging.example.com" }
//!     },
//!     "hooks": {
//!       "before_deploy": "echo 'deploying...'",
//!       "after_deploy": "echo 'done!'"
//!     }
//!   }
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sandbox::SandboxProvider;

use crate::error::{DeployError, DeployResult};

/// Top-level deploy configuration (the `deploy` key in appz.json).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeployConfig {
    /// Default provider slug used when `appz deploy` is run without arguments.
    #[serde(default)]
    pub default: Option<String>,

    /// Per-provider target configurations keyed by provider slug.
    #[serde(default)]
    pub targets: HashMap<String, serde_json::Value>,

    /// Shared environment variables by environment name.
    #[serde(default)]
    pub env: HashMap<String, HashMap<String, String>>,

    /// Deploy lifecycle hooks.
    #[serde(default)]
    pub hooks: Option<DeployHooks>,
}

/// Lifecycle hooks executed around deployments.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeployHooks {
    /// Command to run before deployment starts.
    pub before_deploy: Option<String>,

    /// Command to run after successful deployment.
    pub after_deploy: Option<String>,
}

impl DeployConfig {
    /// Get the typed configuration for a specific provider.
    ///
    /// Deserialises the raw JSON value stored under `targets.<slug>` into
    /// the provider's config struct `T`.
    pub fn get_target_config<T: serde::de::DeserializeOwned>(
        &self,
        slug: &str,
    ) -> DeployResult<Option<T>> {
        match self.targets.get(slug) {
            Some(value) => {
                let config: T = serde_json::from_value(value.clone()).map_err(|e| {
                    DeployError::JsonError {
                        reason: format!(
                            "Failed to parse deploy target config for '{}': {}",
                            slug, e
                        ),
                    }
                })?;
                Ok(Some(config))
            }
            None => Ok(None),
        }
    }

    /// Get the environment variables for a given environment (production/preview).
    pub fn env_for(&self, environment: &str) -> HashMap<String, String> {
        self.env.get(environment).cloned().unwrap_or_default()
    }

    /// Check if a specific provider is configured as a target.
    pub fn has_target(&self, slug: &str) -> bool {
        self.targets.contains_key(slug)
    }

    /// List all configured target provider slugs.
    pub fn target_slugs(&self) -> Vec<String> {
        self.targets.keys().cloned().collect()
    }
}

/// Context passed to provider operations with everything needed for a deploy.
///
/// Holds an `Arc<dyn SandboxProvider>` for executing commands, reading files,
/// and managing tools through the sandbox crate rather than raw process spawning.
#[derive(Clone)]
pub struct DeployContext {
    /// The sandbox used for all command execution and file I/O.
    pub sandbox: Arc<dyn SandboxProvider>,

    /// Absolute path to the project root directory.
    pub project_dir: PathBuf,

    /// Build output directory (relative to project root, e.g. "dist", "build").
    pub output_dir: String,

    /// Whether this is a preview deployment.
    pub is_preview: bool,

    /// The deploy configuration from appz.json.
    pub deploy_config: DeployConfig,

    /// Environment variables to inject.
    pub env_vars: HashMap<String, String>,

    /// Whether we're running in CI/CD mode (non-interactive).
    pub is_ci: bool,

    /// Whether dry-run mode is active (show what would happen without deploying).
    pub dry_run: bool,

    /// Whether to produce JSON output.
    pub json_output: bool,
}

impl std::fmt::Debug for DeployContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeployContext")
            .field("project_dir", &self.project_dir)
            .field("output_dir", &self.output_dir)
            .field("is_preview", &self.is_preview)
            .field("is_ci", &self.is_ci)
            .field("dry_run", &self.dry_run)
            .field("json_output", &self.json_output)
            .finish()
    }
}

impl DeployContext {
    /// Create a new deploy context with a sandbox and minimal required fields.
    pub fn new(sandbox: Arc<dyn SandboxProvider>, output_dir: String) -> Self {
        let project_dir = sandbox.project_path().to_path_buf();
        Self {
            sandbox,
            project_dir,
            output_dir,
            is_preview: false,
            deploy_config: DeployConfig::default(),
            env_vars: HashMap::new(),
            is_ci: is_ci_environment(),
            dry_run: false,
            json_output: false,
        }
    }

    /// Set preview mode.
    pub fn with_preview(mut self, preview: bool) -> Self {
        self.is_preview = preview;
        self
    }

    /// Set the deploy config.
    pub fn with_config(mut self, config: DeployConfig) -> Self {
        self.deploy_config = config;
        self
    }

    /// Set environment variables.
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env_vars = env;
        self
    }

    /// Set dry-run mode.
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Set JSON output mode.
    pub fn with_json_output(mut self, json: bool) -> Self {
        self.json_output = json;
        self
    }

    /// Get the absolute path to the output directory.
    pub fn output_path(&self) -> PathBuf {
        self.project_dir.join(&self.output_dir)
    }

    /// Get the environment name ("preview" or "production").
    pub fn environment_name(&self) -> &str {
        if self.is_preview {
            "preview"
        } else {
            "production"
        }
    }

    /// Execute a command through the sandbox (captured output).
    pub async fn exec(&self, cmd: &str) -> DeployResult<sandbox::CommandOutput> {
        self.sandbox.exec(cmd).await.map_err(Into::into)
    }

    /// Execute a command interactively through the sandbox (inherited stdio).
    pub async fn exec_interactive(&self, cmd: &str) -> DeployResult<std::process::ExitStatus> {
        self.sandbox.exec_interactive(cmd).await.map_err(Into::into)
    }

    /// Access the sandbox's scoped filesystem.
    pub fn fs(&self) -> &sandbox::ScopedFs {
        self.sandbox.fs()
    }
}

/// Context for the interactive setup wizard.
pub struct SetupContext {
    /// The sandbox used for command execution and file I/O during setup.
    pub sandbox: Arc<dyn SandboxProvider>,

    /// Absolute path to the project root.
    pub project_dir: PathBuf,

    /// Whether we're in CI/CD (non-interactive) mode.
    pub is_ci: bool,

    /// Existing deploy config (may be empty/default).
    pub deploy_config: DeployConfig,

    /// Framework detected in the project (if any).
    pub framework: Option<String>,

    /// Build output directory (if known).
    pub output_dir: Option<String>,
}

impl std::fmt::Debug for SetupContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SetupContext")
            .field("project_dir", &self.project_dir)
            .field("is_ci", &self.is_ci)
            .field("framework", &self.framework)
            .field("output_dir", &self.output_dir)
            .finish()
    }
}

impl SetupContext {
    pub fn new(sandbox: Arc<dyn SandboxProvider>) -> Self {
        let project_dir = sandbox.project_path().to_path_buf();
        Self {
            sandbox,
            project_dir,
            is_ci: is_ci_environment(),
            deploy_config: DeployConfig::default(),
            framework: None,
            output_dir: None,
        }
    }

    /// Execute a command through the sandbox (captured output).
    pub async fn exec(&self, cmd: &str) -> DeployResult<sandbox::CommandOutput> {
        self.sandbox.exec(cmd).await.map_err(Into::into)
    }

    /// Execute a command interactively through the sandbox (inherited stdio).
    pub async fn exec_interactive(&self, cmd: &str) -> DeployResult<std::process::ExitStatus> {
        self.sandbox.exec_interactive(cmd).await.map_err(Into::into)
    }

    /// Access the sandbox's scoped filesystem.
    pub fn fs(&self) -> &sandbox::ScopedFs {
        self.sandbox.fs()
    }
}

// ---------------------------------------------------------------------------
// Config I/O
// ---------------------------------------------------------------------------

/// Read the deploy configuration from `appz.json`.
///
/// Returns `None` if the file doesn't exist or has no `deploy` section.
pub fn read_deploy_config(project_dir: &Path) -> DeployResult<Option<DeployConfig>> {
    let config_path = project_dir.join("appz.json");

    if !config_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&config_path)?;
    let root: serde_json::Value = serde_json::from_str(&content)?;

    match root.get("deploy") {
        Some(deploy_value) => {
            let config: DeployConfig = serde_json::from_value(deploy_value.clone())?;
            Ok(Some(config))
        }
        None => Ok(None),
    }
}

/// Read the deploy configuration asynchronously.
pub async fn read_deploy_config_async(project_dir: &Path) -> DeployResult<Option<DeployConfig>> {
    let project_dir = project_dir.to_path_buf();
    tokio::task::spawn_blocking(move || read_deploy_config(&project_dir))
        .await
        .map_err(|e| DeployError::Other(format!("Failed to read config: {}", e)))?
}

/// Write the deploy configuration to `appz.json`.
///
/// Merges the deploy section into the existing appz.json, preserving other fields.
pub fn write_deploy_config(project_dir: &Path, config: &DeployConfig) -> DeployResult<()> {
    let config_path = project_dir.join("appz.json");

    let mut root: serde_json::Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        serde_json::from_str(&content)?
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    let deploy_value = serde_json::to_value(config)?;
    root.as_object_mut()
        .ok_or_else(|| DeployError::JsonError {
            reason: "appz.json root must be a JSON object".into(),
        })?
        .insert("deploy".to_string(), deploy_value);

    let formatted = serde_json::to_string_pretty(&root)?;
    std::fs::write(&config_path, formatted)?;

    Ok(())
}

/// Write the deploy configuration asynchronously.
pub async fn write_deploy_config_async(
    project_dir: &Path,
    config: &DeployConfig,
) -> DeployResult<()> {
    let project_dir = project_dir.to_path_buf();
    let config = config.clone();
    tokio::task::spawn_blocking(move || write_deploy_config(&project_dir, &config))
        .await
        .map_err(|e| DeployError::Other(format!("Failed to write config: {}", e)))?
}

// ---------------------------------------------------------------------------
// CI detection
// ---------------------------------------------------------------------------

/// Detect whether we're running in a CI/CD environment.
pub fn is_ci_environment() -> bool {
    let bag = env_var::GlobalEnvBag::instance();

    // Standard CI env var
    if bag.has("CI") {
        return true;
    }

    // Appz-specific non-interactive flags
    if bag.has("APPZ_NO_INPUT") || bag.has("APPZ_YES") {
        return true;
    }

    // Common CI platform env vars
    let ci_vars = [
        "GITHUB_ACTIONS",
        "GITLAB_CI",
        "CIRCLECI",
        "TRAVIS",
        "JENKINS_URL",
        "BUILDKITE",
        "CODEBUILD_BUILD_ID",
        "TF_BUILD",
        "BITBUCKET_PIPELINE",
    ];

    ci_vars.iter().any(|var| bag.has(var))
}

/// Detect which CI platform we're running on, if any.
pub fn detect_ci_platform() -> Option<CiPlatform> {
    let bag = env_var::GlobalEnvBag::instance();

    if bag.has("GITHUB_ACTIONS") {
        Some(CiPlatform::GitHubActions)
    } else if bag.has("GITLAB_CI") {
        Some(CiPlatform::GitLabCi)
    } else if bag.has("CIRCLECI") {
        Some(CiPlatform::CircleCi)
    } else if bag.has("TRAVIS") {
        Some(CiPlatform::Travis)
    } else if bag.has("BUILDKITE") {
        Some(CiPlatform::Buildkite)
    } else if bag.has("CI") {
        Some(CiPlatform::Unknown)
    } else {
        None
    }
}

/// Known CI/CD platforms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CiPlatform {
    GitHubActions,
    GitLabCi,
    CircleCi,
    Travis,
    Buildkite,
    Unknown,
}
