//! Checker configuration types.
//!
//! The checker reads its configuration from the `check` section of
//! `appz.json`. Settings control which providers run, strictness,
//! ignore patterns, and AI fix configuration.
//!
//! # Example `appz.json`
//!
//! ```json
//! {
//!   "buildCommand": "npm run build",
//!   "check": {
//!     "strict": true,
//!     "ignore": ["dist/**", "node_modules/**"],
//!     "providers": ["biome", "tsc"],
//!     "disabled": ["secrets"],
//!     "aiProvider": "openai",
//!     "aiModel": "gpt-4o"
//!   }
//! }
//! ```

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use sandbox::SandboxProvider;

use crate::error::{CheckResult, CheckerError};

/// Per-role model configuration for the AI repair pipeline.
///
/// Allows using different (cheaper) models for planning and verification
/// while reserving the stronger model for patch generation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiModelConfig {
    /// Model for the Planner agent (default: cheap variant of `aiModel`).
    #[serde(default)]
    pub planner: Option<String>,
    /// Model for the Fix agent (default: `aiModel`).
    #[serde(default)]
    pub fixer: Option<String>,
    /// Model for the Verify agent (default: cheap variant of `aiModel`).
    #[serde(default)]
    pub verifier: Option<String>,
}

/// Top-level check configuration (the `check` key in appz.json).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CheckConfig {
    /// Treat warnings as errors.
    #[serde(default)]
    pub strict: Option<bool>,

    /// Glob patterns to ignore (relative to project root).
    #[serde(default)]
    pub ignore: Option<Vec<String>>,

    /// Explicit list of provider slugs to run (overrides auto-detection).
    #[serde(default)]
    pub providers: Option<Vec<String>>,

    /// Provider slugs to disable even if detected.
    #[serde(default)]
    pub disabled: Option<Vec<String>>,

    /// AI provider for `--ai-fix` ("openai", "anthropic", "ollama").
    #[serde(default, rename = "aiProvider")]
    pub ai_provider: Option<String>,

    /// AI model name — used as default for all AI roles (e.g. "gpt-4o").
    #[serde(default, rename = "aiModel")]
    pub ai_model: Option<String>,

    /// Per-role model overrides for the AI repair pipeline.
    #[serde(default, rename = "aiModels")]
    pub ai_models: Option<AiModelConfig>,

    /// Safety configuration for AI repair.
    #[serde(default, rename = "aiSafety")]
    pub ai_safety: Option<crate::ai_fixer::safety::SafetyConfig>,

    /// Maximum AI repair retry attempts.
    #[serde(default, rename = "aiMaxAttempts")]
    pub ai_max_attempts: Option<u32>,
}

/// Context passed to check provider operations.
///
/// Holds everything a provider needs: the sandbox, project paths,
/// file lists, and configuration flags.
#[derive(Clone)]
pub struct CheckContext {
    /// The sandbox used for all command execution and file I/O.
    pub sandbox: Arc<dyn SandboxProvider>,

    /// Absolute path to the project root directory.
    pub project_dir: std::path::PathBuf,

    /// The check configuration from appz.json.
    pub check_config: CheckConfig,

    /// Whether to run in fix mode.
    pub fix: bool,

    /// Whether to check formatting.
    pub format: bool,

    /// Whether strict mode is active (warnings are errors).
    pub strict: bool,

    /// Scoped file list (from --changed / --staged), or None for all files.
    pub file_filter: Option<Vec<String>>,

    /// Whether dry-run mode is active.
    pub json_output: bool,

    /// Whether we're running in CI/CD mode (non-interactive).
    pub is_ci: bool,
}

impl std::fmt::Debug for CheckContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckContext")
            .field("project_dir", &self.project_dir)
            .field("fix", &self.fix)
            .field("format", &self.format)
            .field("strict", &self.strict)
            .field("file_filter", &self.file_filter)
            .field("json_output", &self.json_output)
            .field("is_ci", &self.is_ci)
            .finish()
    }
}

impl CheckContext {
    /// Create a new check context with a sandbox.
    pub fn new(sandbox: Arc<dyn SandboxProvider>) -> Self {
        let project_dir = sandbox.project_path().to_path_buf();
        Self {
            sandbox,
            project_dir,
            check_config: CheckConfig::default(),
            fix: false,
            format: false,
            strict: false,
            file_filter: None,
            json_output: false,
            is_ci: false,
        }
    }

    /// Set the check config.
    pub fn with_config(mut self, config: CheckConfig) -> Self {
        // Apply strict from config if not overridden.
        if let Some(strict) = config.strict {
            self.strict = strict;
        }
        self.check_config = config;
        self
    }

    /// Set fix mode.
    pub fn with_fix(mut self, fix: bool) -> Self {
        self.fix = fix;
        self
    }

    /// Set format mode.
    pub fn with_format(mut self, format: bool) -> Self {
        self.format = format;
        self
    }

    /// Set strict mode.
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    /// Set the file filter (from git integration).
    pub fn with_file_filter(mut self, files: Option<Vec<String>>) -> Self {
        self.file_filter = files;
        self
    }

    /// Set JSON output mode.
    pub fn with_json_output(mut self, json: bool) -> Self {
        self.json_output = json;
        self
    }

    /// Set CI mode.
    pub fn with_ci(mut self, is_ci: bool) -> Self {
        self.is_ci = is_ci;
        self
    }

    /// Execute a command through the sandbox (captured output).
    pub async fn exec(&self, cmd: &str) -> CheckResult<sandbox::CommandOutput> {
        self.sandbox.exec(cmd).await.map_err(Into::into)
    }

    /// Access the sandbox's scoped filesystem.
    pub fn fs(&self) -> &sandbox::ScopedFs {
        self.sandbox.fs()
    }

    /// Build a file argument string for linters.
    ///
    /// If a file filter is set, returns the files joined by spaces.
    /// Otherwise returns `"."` for the project root.
    pub fn file_args(&self) -> String {
        match &self.file_filter {
            Some(files) if !files.is_empty() => files.join(" "),
            _ => ".".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Config I/O
// ---------------------------------------------------------------------------

/// Read the check configuration from `appz.json`.
///
/// Returns `None` if the file doesn't exist or has no `check` section.
pub fn read_check_config(project_dir: &Path) -> CheckResult<Option<CheckConfig>> {
    let config_path = project_dir.join("appz.json");

    if !config_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&config_path)?;
    let root: serde_json::Value = serde_json::from_str(&content)?;

    match root.get("check") {
        Some(check_value) => {
            let config: CheckConfig = serde_json::from_value(check_value.clone())?;
            Ok(Some(config))
        }
        None => Ok(None),
    }
}

/// Read the check configuration asynchronously.
pub async fn read_check_config_async(project_dir: &Path) -> CheckResult<Option<CheckConfig>> {
    let project_dir = project_dir.to_path_buf();
    tokio::task::spawn_blocking(move || read_check_config(&project_dir))
        .await
        .map_err(|e| CheckerError::Other(format!("Failed to read config: {}", e)))?
}
