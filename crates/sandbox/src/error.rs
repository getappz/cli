//! Sandbox error types with rich diagnostic help messages.
//!
//! Every error variant includes a [`miette::Diagnostic`] code (e.g.
//! `sandbox::path_escape`) and a human-readable `help` hint so that CLI
//! users get actionable guidance when something goes wrong.
//!
//! # Error variants (quick reference)
//!
//! | Variant | Diagnostic code | Typical cause |
//! |---------|----------------|---------------|
//! | `PathEscape` | `sandbox::path_escape` | Absolute path or `..` traversal |
//! | `ProjectNotFound` | `sandbox::project_not_found` | Missing project directory |
//! | `FileNotFound` | `sandbox::file_not_found` | Read/remove on non-existent file |
//! | `DirectoryNotFound` | `sandbox::directory_not_found` | `list_dir` on non-existent dir |
//! | `MiseSetupFailed` | `sandbox::mise_setup_failed` | Mise install/detection failure |
//! | `CommandFailed` | `sandbox::command_failed` | Non-zero exit or spawn error |
//! | `JsonError` | `sandbox::json_error` | Invalid JSON syntax or schema |
//! | `TomlError` | `sandbox::toml_error` | Invalid TOML syntax or schema |
//! | `GlobError` | `sandbox::glob_error` | Bad glob pattern |
//! | `Io` | `sandbox::io_error` | Low-level I/O failure |
//! | `BatchError` | `sandbox::batch_error` | One or more items in a batch op failed |
//! | `Other` | `sandbox::other` | Catch-all for uncommon situations |
//!
//! # Conversions
//!
//! The following `From` impls allow `?` to propagate errors transparently:
//!
//! - `std::io::Error` → `SandboxError::Io`
//! - `serde_json::Error` → `SandboxError::JsonError`
//! - `toml::de::Error` / `toml::ser::Error` → `SandboxError::TomlError`
//! - `glob::PatternError` / `glob::GlobError` → `SandboxError::GlobError`
//! - `starbase_utils::fs::FsError` → `SandboxError::Other`
//! - `miette::Error` → `SandboxError::Other`

// The miette::Diagnostic derive generates assignment code that triggers
// spurious `unused_assignments` warnings on every named field.
#![allow(unused_assignments)]

use miette::Diagnostic;
use thiserror::Error;

/// Result type alias for sandbox operations.
pub type SandboxResult<T> = Result<T, SandboxError>;

/// Errors that can occur during sandbox operations.
///
/// Each variant carries a diagnostic code and a `help` message. When
/// rendered through `miette`, the user sees both the error and the
/// suggested remedy, for example:
///
/// ```text
/// × Path escapes sandbox root: ../../etc/passwd
///   help: All paths must be relative to the project root.
///         Remove any leading '/' or '..' segments.
/// ```
#[derive(Error, Debug, Diagnostic)]
pub enum SandboxError {
    #[diagnostic(
        code(sandbox::path_escape),
        help("All paths must be relative to the project root. Remove any leading '/' or '..' segments.")
    )]
    #[error("Path escapes sandbox root: {path}")]
    PathEscape { path: String },

    #[diagnostic(
        code(sandbox::project_not_found),
        help("Verify the project path exists and you have read permissions, or let the sandbox create it by enabling auto-create.")
    )]
    #[error("Project path does not exist: {path}")]
    ProjectNotFound { path: String },

    #[diagnostic(
        code(sandbox::file_not_found),
        help("Check the file path for typos. Use sandbox.fs().exists(path) to verify before reading.")
    )]
    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[diagnostic(
        code(sandbox::directory_not_found),
        help("The directory does not exist. Create it first with sandbox.fs().create_dir_all(path).")
    )]
    #[error("Directory not found: {path}")]
    DirectoryNotFound { path: String },

    #[diagnostic(
        code(sandbox::mise_setup_failed),
        help(
            "Install mise manually: https://mise.jdx.dev/getting-started.html\n\
             Or set auto_install_mise: true in SandboxSettings to let the sandbox install it."
        )
    )]
    #[error("Mise setup failed: {reason}")]
    MiseSetupFailed { reason: String },

    #[diagnostic(
        code(sandbox::command_failed),
        help("Check the command syntax and ensure all required tools are installed via mise.\nRun 'mise ls' to see installed tools.")
    )]
    #[error("Command failed: {command}\n{reason}")]
    CommandFailed { command: String, reason: String },

    #[diagnostic(
        code(sandbox::json_error),
        help("Verify the JSON file is well-formed. Use a linter or 'jq .' to check syntax.")
    )]
    #[error("JSON operation failed: {reason}")]
    JsonError { reason: String },

    #[diagnostic(
        code(sandbox::toml_error),
        help("Verify the TOML file is well-formed. Check for unclosed quotes, missing commas, or invalid keys.")
    )]
    #[error("TOML operation failed: {reason}")]
    TomlError { reason: String },

    #[diagnostic(
        code(sandbox::glob_error),
        help("Check the glob pattern syntax. Common patterns: '**/*.ts', 'src/**/*.rs', '*.{{js,jsx}}'.")
    )]
    #[error("Glob pattern error: {reason}")]
    GlobError { reason: String },

    #[diagnostic(
        code(sandbox::io_error),
        help("Check file permissions and available disk space. Ensure the path is accessible.")
    )]
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[diagnostic(
        code(sandbox::batch_error),
        help("One or more operations in the batch failed. Check the individual errors for details.")
    )]
    #[error("Batch operation failed: {count} of {total} operations failed. First error: {first_error}")]
    BatchError {
        count: usize,
        total: usize,
        first_error: String,
    },

    #[diagnostic(code(sandbox::other))]
    #[error("{0}")]
    Other(String),
}

impl From<miette::Error> for SandboxError {
    fn from(err: miette::Error) -> Self {
        SandboxError::Other(err.to_string())
    }
}

impl From<starbase_utils::fs::FsError> for SandboxError {
    fn from(err: starbase_utils::fs::FsError) -> Self {
        SandboxError::Other(err.to_string())
    }
}

impl From<serde_json::Error> for SandboxError {
    fn from(err: serde_json::Error) -> Self {
        SandboxError::JsonError {
            reason: err.to_string(),
        }
    }
}

impl From<toml::de::Error> for SandboxError {
    fn from(err: toml::de::Error) -> Self {
        SandboxError::TomlError {
            reason: err.to_string(),
        }
    }
}

impl From<toml::ser::Error> for SandboxError {
    fn from(err: toml::ser::Error) -> Self {
        SandboxError::TomlError {
            reason: err.to_string(),
        }
    }
}

impl From<glob::PatternError> for SandboxError {
    fn from(err: glob::PatternError) -> Self {
        SandboxError::GlobError {
            reason: err.to_string(),
        }
    }
}

impl From<glob::GlobError> for SandboxError {
    fn from(err: glob::GlobError) -> Self {
        SandboxError::GlobError {
            reason: err.to_string(),
        }
    }
}
