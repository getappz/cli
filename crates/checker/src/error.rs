//! Checker error types with rich diagnostic help messages.
//!
//! Every error variant includes a [`miette::Diagnostic`] code and a
//! human-readable `help` hint so that CLI users get actionable guidance
//! when something goes wrong.

#![allow(unused_assignments)]

use miette::Diagnostic;
use thiserror::Error;

/// Result type alias for checker operations.
pub type CheckResult<T> = Result<T, CheckerError>;

/// Errors that can occur during check operations.
#[derive(Error, Debug, Diagnostic)]
pub enum CheckerError {
    #[diagnostic(
        code(checker::provider_not_found),
        help("Available checkers: biome, tsc, ruff, clippy, phpstan, stylelint, secrets.\nRun 'appz check --help' to see all options.")
    )]
    #[error("Unknown checker: {slug}")]
    ProviderNotFound { slug: String },

    #[diagnostic(
        code(checker::no_providers_detected),
        help("No applicable checkers were detected for your project.\nRun 'appz check --init' to set up checkers, or use '--checker <name>' to run a specific one.")
    )]
    #[error("No checkers detected for the current project")]
    NoProvidersDetected,

    #[diagnostic(
        code(checker::tool_not_found),
        help("Install {tool} or let appz install it automatically.\nRun 'appz check --init' to configure your project.")
    )]
    #[error("Required tool not found: {tool}")]
    ToolNotFound { tool: String },

    #[diagnostic(
        code(checker::tool_install_failed),
        help("Try installing {tool} manually.\nCheck your network connection and tool registry availability.")
    )]
    #[error("Failed to install tool {tool}: {reason}")]
    ToolInstallFailed { tool: String, reason: String },

    #[diagnostic(
        code(checker::check_failed),
        help("The checker '{provider}' encountered an error.\nRun with --verbose for more details.")
    )]
    #[error("Checker '{provider}' failed: {reason}")]
    CheckFailed { provider: String, reason: String },

    #[diagnostic(
        code(checker::fix_failed),
        help("The auto-fix for '{provider}' encountered an error.\nYou may need to fix the issues manually.")
    )]
    #[error("Auto-fix failed for '{provider}': {reason}")]
    FixFailed { provider: String, reason: String },

    #[diagnostic(
        code(checker::parse_failed),
        help("Failed to parse output from '{provider}'.\nThis may indicate a version mismatch — try updating the tool.")
    )]
    #[error("Failed to parse {provider} output: {reason}")]
    ParseFailed { provider: String, reason: String },

    #[diagnostic(
        code(checker::ai_fix_failed),
        help("AI-assisted fix failed. Check your AI provider configuration in appz.json.\nEnsure your API key is set and the model is accessible.")
    )]
    #[error("AI fix failed: {reason}")]
    AiFixFailed { reason: String },

    #[diagnostic(
        code(checker::git_error),
        help("Ensure you are inside a git repository.\nRun 'git status' to check your repository state.")
    )]
    #[error("Git operation failed: {reason}")]
    GitError { reason: String },

    #[diagnostic(
        code(checker::cache_error),
        help("The check cache may be corrupted. Delete .appz/check-cache.json and retry.")
    )]
    #[error("Cache error: {reason}")]
    CacheError { reason: String },

    #[diagnostic(
        code(checker::config_error),
        help("Check the 'check' section in appz.json for syntax errors.")
    )]
    #[error("Configuration error: {reason}")]
    ConfigError { reason: String },

    #[diagnostic(
        code(checker::io_error),
        help("Check file permissions and available disk space.")
    )]
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[diagnostic(
        code(checker::json_error),
        help("Verify the file is well-formed JSON.")
    )]
    #[error("JSON error: {reason}")]
    JsonError { reason: String },

    #[diagnostic(code(checker::other))]
    #[error("{0}")]
    Other(String),
}

impl From<serde_json::Error> for CheckerError {
    fn from(err: serde_json::Error) -> Self {
        CheckerError::JsonError {
            reason: err.to_string(),
        }
    }
}

impl From<miette::Error> for CheckerError {
    fn from(err: miette::Error) -> Self {
        CheckerError::Other(err.to_string())
    }
}

impl From<sandbox::SandboxError> for CheckerError {
    fn from(err: sandbox::SandboxError) -> Self {
        CheckerError::CheckFailed {
            provider: "sandbox".into(),
            reason: err.to_string(),
        }
    }
}

impl From<ai::AiError> for CheckerError {
    fn from(err: ai::AiError) -> Self {
        CheckerError::AiFixFailed {
            reason: err.to_string(),
        }
    }
}
