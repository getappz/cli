//! WordPress runtime abstraction.
//!
//! Defines the [`WordPressRuntime`] trait that both DDEV and WordPress Playground
//! implement. This allows the blueprint executor, generator, and CLI commands to
//! work identically regardless of the underlying runtime.
//!
//! # Adding a new runtime
//!
//! 1. Create `src/runtimes/<name>.rs` implementing [`WordPressRuntime`].
//! 2. Re-export from `src/runtimes/mod.rs`.
//! 3. Register in `RuntimeSelector` (in `crates/app/src/wp_runtime.rs`).

use std::fmt;
use std::path::{Path, PathBuf};

/// Error type for WordPress runtime operations.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("{runtime} is not available. {install_hint}")]
    NotAvailable {
        runtime: String,
        install_hint: String,
    },

    #[error("{runtime} is not configured for this project.")]
    NotConfigured { runtime: String },

    #[error("Runtime command failed: {command}\n{message}")]
    CommandFailed { command: String, message: String },

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Core trait abstracting a WordPress local development runtime.
///
/// Each runtime (DDEV, WordPress Playground, etc.) implements this trait.
/// The trait is dyn-compatible so runtimes can be stored as `Arc<dyn WordPressRuntime>`.
///
/// All command execution goes through this trait rather than direct process spawning,
/// which allows the blueprint executor and generator to be runtime-agnostic.
pub trait WordPressRuntime: Send + Sync + fmt::Debug {
    // ------------------------------------------------------------------
    // Identity
    // ------------------------------------------------------------------

    /// Human-readable runtime name (e.g. "DDEV", "WordPress Playground").
    fn name(&self) -> &str;

    /// Slug identifier (e.g. "ddev", "playground").
    fn slug(&self) -> &str;

    // ------------------------------------------------------------------
    // Availability detection
    // ------------------------------------------------------------------

    /// Check if the runtime's prerequisites are installed on the system.
    fn is_available(&self) -> bool;

    /// Check if this runtime is configured for the given project.
    fn is_configured(&self, project_path: &Path) -> bool;

    // ------------------------------------------------------------------
    // Lifecycle
    // ------------------------------------------------------------------

    /// Configure the runtime for a new project.
    fn configure(
        &self,
        project_path: &Path,
        project_type: &str,
        docroot: Option<&str>,
    ) -> Result<(), RuntimeError>;

    /// Start the runtime (containers, server, etc.).
    fn start(&self, project_path: &Path) -> Result<(), RuntimeError>;

    /// Stop the runtime.
    fn stop(&self, project_path: &Path) -> Result<(), RuntimeError>;

    /// Open the site in the default browser.
    fn open_browser(&self, project_path: &Path) -> Result<(), RuntimeError>;

    /// Stream logs to stdout (blocking — runs until interrupted).
    fn stream_logs(&self, project_path: &Path) -> Result<(), RuntimeError>;

    /// Check network connectivity from the runtime environment.
    /// Returns true if the runtime can reach the internet.
    fn check_connectivity(&self, project_path: &Path) -> bool;

    // ------------------------------------------------------------------
    // WordPress operations
    // ------------------------------------------------------------------

    /// Check if WordPress core is installed (database tables exist).
    fn wp_is_installed(&self, project_path: &Path) -> bool;

    /// Run `wp core install` with the given parameters.
    fn wp_install(
        &self,
        project_path: &Path,
        url: &str,
        admin_user: &str,
        admin_pass: &str,
    ) -> Result<(), RuntimeError>;

    /// Get the site URL for this project.
    fn site_url(&self, project_path: &Path) -> String;

    // ------------------------------------------------------------------
    // WP-CLI execution (used by executor and generator)
    // ------------------------------------------------------------------

    /// Run a WP-CLI command (fire-and-forget, inherit stdout/stderr).
    fn wp_cli(&self, project_path: &Path, args: &[&str]) -> Result<(), RuntimeError>;

    /// Run a WP-CLI command and capture stdout output.
    fn wp_cli_output(&self, project_path: &Path, args: &[&str]) -> Option<String>;

    /// Execute a shell command inside the runtime environment.
    fn exec_shell(&self, project_path: &Path, cmd: &str) -> Result<(), RuntimeError>;

    /// Execute a command with arguments inside the runtime environment.
    fn exec_args(&self, project_path: &Path, args: &[&str]) -> Result<(), RuntimeError>;

    /// Pipe SQL to the WordPress database.
    fn exec_sql(&self, project_path: &Path, sql: &str) -> Result<(), RuntimeError>;

    // ------------------------------------------------------------------
    // Configuration
    // ------------------------------------------------------------------

    /// Set the PHP version for this project.
    ///
    /// For DDEV: runs `ddev config --php-version=X`.
    /// For Playground: stores in state.json and uses `--php=X` flag.
    fn set_php_version(
        &self,
        project_path: &Path,
        version: &str,
    ) -> Result<(), RuntimeError>;

    // ------------------------------------------------------------------
    // High-level operations (with runtime-specific optimizations)
    // ------------------------------------------------------------------

    /// Download a URL to a local path inside the runtime.
    ///
    /// Default: uses `exec_shell` with curl. Playground overrides to use
    /// native URL resource fetching.
    fn download_url(
        &self,
        project_path: &Path,
        url: &str,
        dest_path: &str,
    ) -> Result<(), RuntimeError> {
        self.exec_shell(
            project_path,
            &format!("curl -sL '{}' -o '{}'", url.replace('\'', "'\\''"), dest_path.replace('\'', "'\\''"))
        )
    }

    /// Download a ZIP from a URL and extract it to a destination path.
    ///
    /// Default: uses `exec_shell` with curl+unzip. Playground overrides to use
    /// native unzip blueprint step.
    fn download_and_unzip(
        &self,
        project_path: &Path,
        url: &str,
        extract_to: &str,
    ) -> Result<(), RuntimeError> {
        let escaped_url = url.replace('\'', "'\\''");
        let escaped_dest = extract_to.replace('\'', "'\\''");
        self.exec_shell(
            project_path,
            &format!(
                "curl -sL '{}' -o /tmp/_bp_download.zip && unzip -o /tmp/_bp_download.zip -d '{}' && rm /tmp/_bp_download.zip",
                escaped_url, escaped_dest
            ),
        )
    }

    /// Evaluate a PHP expression via `wp eval`.
    ///
    /// Default: delegates to `wp_cli`. This exists so Playground can use native
    /// `runPHP` steps instead of shelling out.
    fn wp_eval(
        &self,
        project_path: &Path,
        code: &str,
    ) -> Result<(), RuntimeError> {
        self.wp_cli(project_path, &["eval", code])
    }

    /// Make an HTTP request from inside the runtime environment.
    ///
    /// Default: uses `exec_shell` with curl. Playground overrides to use
    /// native `request` blueprint step.
    fn http_request(
        &self,
        project_path: &Path,
        method: &str,
        url: &str,
    ) -> Result<(), RuntimeError> {
        self.exec_shell(
            project_path,
            &format!("curl -sS -X {} '{}'", method.replace('\'', "'\\''"), url.replace('\'', "'\\''"))
        )
    }

    // ------------------------------------------------------------------
    // Introspection (used by generator)
    // ------------------------------------------------------------------

    /// Get the PHP version configured for this project.
    fn php_version(&self, project_path: &Path) -> Option<String>;
}
