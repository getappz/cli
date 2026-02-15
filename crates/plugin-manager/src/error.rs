//! Plugin manager error types with rich diagnostic help messages.

use miette::Diagnostic;
use thiserror::Error;

/// Result type alias for plugin manager operations.
pub type PluginResult<T> = Result<T, PluginError>;

/// Errors that can occur during plugin management operations.
#[derive(Error, Debug, Diagnostic)]
pub enum PluginError {
    #[diagnostic(
        code(plugin::not_found),
        help("Run 'appz --help' to see available commands and plugins.")
    )]
    #[error("No plugin provides the command '{command}'")]
    PluginNotFound { command: String },

    #[diagnostic(
        code(plugin::entitlement_required),
        help(
            "This plugin requires a '{tier}' subscription. Upgrade at https://appz.dev/pricing"
        )
    )]
    #[error("Plugin '{plugin}' requires '{tier}' subscription tier")]
    EntitlementRequired { plugin: String, tier: String },

    #[diagnostic(
        code(plugin::signature_invalid),
        help("The plugin file may be corrupted or tampered with. Try removing it and re-downloading.")
    )]
    #[error("Plugin signature verification failed for '{plugin}'")]
    SignatureInvalid { plugin: String },

    #[diagnostic(
        code(plugin::header_invalid),
        help("This WASM file is not a valid appz plugin. Only plugins built with the appz PDK are supported.")
    )]
    #[error("Invalid appz plugin header in '{plugin}': {reason}")]
    HeaderInvalid { plugin: String, reason: String },

    #[diagnostic(
        code(plugin::handshake_failed),
        help("The plugin could not authenticate with the appz CLI. It may be built for a different CLI version.")
    )]
    #[error("Plugin handshake failed for '{plugin}'")]
    HandshakeFailed { plugin: String },

    #[diagnostic(
        code(plugin::download_failed),
        help("Check your internet connection and try again. The plugin CDN may be temporarily unavailable.")
    )]
    #[error("Failed to download plugin '{plugin}': {reason}")]
    DownloadFailed { plugin: String, reason: String },

    #[diagnostic(
        code(plugin::checksum_mismatch),
        help("The downloaded file does not match the expected checksum. Try downloading again.")
    )]
    #[error("Checksum mismatch for plugin '{plugin}': expected {expected}, got {actual}")]
    ChecksumMismatch {
        plugin: String,
        expected: String,
        actual: String,
    },

    #[diagnostic(
        code(plugin::version_incompatible),
        help("Update appz to the latest version: 'appz self-update'")
    )]
    #[error(
        "Plugin '{plugin}' requires appz CLI >= {required}, but current version is {current}"
    )]
    VersionIncompatible {
        plugin: String,
        required: String,
        current: String,
    },

    #[diagnostic(
        code(plugin::manifest_error),
        help("Try removing ~/.appz/plugins/manifest.json and retrying.")
    )]
    #[error("Failed to load plugin manifest: {reason}")]
    ManifestError { reason: String },

    #[diagnostic(
        code(plugin::execution_error),
        help("Check the plugin documentation for usage instructions.")
    )]
    #[error("Plugin execution failed: {reason}")]
    ExecutionError { reason: String },

    #[diagnostic(
        code(plugin::io_error),
        help("Check file permissions and available disk space.")
    )]
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[diagnostic(code(plugin::other))]
    #[error("{0}")]
    Other(String),
}

impl From<api::ApiError> for PluginError {
    fn from(err: api::ApiError) -> Self {
        PluginError::Other(format!("API error: {}", err))
    }
}

impl From<serde_json::Error> for PluginError {
    fn from(err: serde_json::Error) -> Self {
        PluginError::ManifestError {
            reason: err.to_string(),
        }
    }
}

impl From<reqwest::Error> for PluginError {
    fn from(err: reqwest::Error) -> Self {
        PluginError::DownloadFailed {
            plugin: "unknown".to_string(),
            reason: err.to_string(),
        }
    }
}
