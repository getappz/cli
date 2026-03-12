//! Sandbox provider trait and extension trait.
//!
//! # Trait hierarchy
//!
//! ```text
//! SandboxProvider          Core trait (dyn-compatible)
//!   ├── init / teardown    Lifecycle
//!   ├── fs                 ScopedFs access
//!   ├── exec / exec_*      Command execution
//!   ├── ensure_tool        Tool management
//!   └── project_path / config  Info
//!
//! SandboxProviderExt       Extension trait (auto-impl, NOT dyn-compatible)
//!   ├── write_files_progress   Batch write with progress bar
//!   ├── read_files_progress    Batch read with progress bar
//!   ├── remove_files_progress  Batch remove with progress bar
//!   └── copy_progress          Copy tree with spinner
//! ```
//!
//! # Provider lifecycle
//!
//! 1. Construct the provider: `LocalProvider::new()`
//! 2. Call `init(&config)` — sets up the project dir, mise, tools, and env.
//! 3. Use `fs()`, `exec()`, `ensure_tool()`, etc.
//! 4. Call `teardown()` when done (optional for local; required for Docker).
//!
//! Prefer [`crate::create_sandbox`] which handles steps 1–2 automatically.
//!
//! # Why two traits?
//!
//! [`SandboxProvider`] is **dyn-compatible** so it can be used as
//! `Box<dyn SandboxProvider>`. The extension trait [`SandboxProviderExt`]
//! has generic methods (for batch operations) which break dyn-compatibility,
//! so they live in a separate auto-implemented trait. Import both:
//!
//! ```rust,ignore
//! use sandbox::{SandboxProvider, SandboxProviderExt};
//! ```

use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future;

use crate::config::{MiseToolSpec, SandboxConfig};
use crate::error::{SandboxError, SandboxResult};
use crate::scoped_fs::ScopedFs;

/// Output from a non-interactive command execution.
///
/// Wraps [`std::process::Output`] with convenience accessors for stdout/stderr
/// as strings and trimmed output. Used by [`SandboxProvider::exec`],
/// [`SandboxProvider::exec_with_tool`], and [`SandboxProvider::exec_all`].
///
/// # Example
///
/// ```rust,no_run
/// # async fn demo(sandbox: &dyn sandbox::SandboxProvider) -> Result<(), Box<dyn std::error::Error>> {
/// let out = sandbox.exec("node --version").await?;
/// if out.success() {
///     println!("Node version: {}", out.stdout_trimmed());
/// } else {
///     eprintln!("Error: {}", out.stderr());
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct CommandOutput {
    /// The raw process output (stdout, stderr, exit status).
    pub inner: Output,
}

impl CommandOutput {
    pub fn new(output: Output) -> Self {
        Self { inner: output }
    }

    /// Whether the command exited successfully (exit code 0).
    pub fn success(&self) -> bool {
        self.inner.status.success()
    }

    /// The exit code, if available.
    pub fn exit_code(&self) -> Option<i32> {
        self.inner.status.code()
    }

    /// Stdout as a UTF-8 string (lossy).
    pub fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.inner.stdout).to_string()
    }

    /// Stderr as a UTF-8 string (lossy).
    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.inner.stderr).to_string()
    }

    /// Stdout trimmed.
    pub fn stdout_trimmed(&self) -> String {
        self.stdout().trim().to_string()
    }
}

/// Core trait abstracting a sandbox execution environment.
///
/// Implementations provide scoped filesystem access, command execution,
/// and tool management for a single project directory.
///
/// See [`SandboxProviderExt`] for additional convenience methods with
/// progress indicators (auto-implemented for all `SandboxProvider`).
#[async_trait]
pub trait SandboxProvider: Send + Sync {
    // ------------------------------------------------------------------
    // Lifecycle
    // ------------------------------------------------------------------

    /// Initialise the sandbox: validate project path, set up mise, install tools.
    async fn init(&mut self, config: &SandboxConfig) -> SandboxResult<()>;

    /// Tear down the sandbox (cleanup resources).
    ///
    /// For the local provider this is a no-op; Docker providers would stop
    /// and remove containers here.
    async fn teardown(&mut self) -> SandboxResult<()>;

    // ------------------------------------------------------------------
    // Filesystem
    // ------------------------------------------------------------------

    /// Access the scoped filesystem handle.
    fn fs(&self) -> &ScopedFs;

    // ------------------------------------------------------------------
    // Command execution
    // ------------------------------------------------------------------

    /// Execute a command (captures stdout/stderr).
    ///
    /// The command runs in the project root with the mise-managed environment.
    async fn exec(&self, cmd: &str) -> SandboxResult<CommandOutput>;

    /// Execute a command interactively (inherits stdin/stdout/stderr).
    async fn exec_interactive(&self, cmd: &str) -> SandboxResult<ExitStatus>;

    // ------------------------------------------------------------------
    // Tool management
    // ------------------------------------------------------------------

    /// Ensure a specific tool is available via mise.
    async fn ensure_tool(&self, tool: &MiseToolSpec) -> SandboxResult<()>;

    /// Execute a command with a specific tool version pinned.
    ///
    /// Produces: `mise x <tool>@<version> -- <cmd>`
    async fn exec_with_tool(
        &self,
        tool: &MiseToolSpec,
        cmd: &str,
    ) -> SandboxResult<CommandOutput>;

    /// Execute multiple independent commands concurrently.
    ///
    /// Returns results in the same order as the input commands. Each command
    /// runs in its own process; all are spawned concurrently via tokio.
    async fn exec_all(&self, cmds: &[&str]) -> Vec<SandboxResult<CommandOutput>> {
        let futs: Vec<_> = cmds.iter().map(|cmd| self.exec(cmd)).collect();
        future::join_all(futs).await
    }

    // ------------------------------------------------------------------
    // Info
    // ------------------------------------------------------------------

    /// The absolute project root path.
    fn project_path(&self) -> &Path;

    /// The configuration this sandbox was initialised with.
    fn config(&self) -> &SandboxConfig;
}

// ===========================================================================
// Extension trait — batch operations with progress indicators
// ===========================================================================

/// Extension trait providing batch filesystem operations with visual progress.
///
/// Automatically implemented for every [`SandboxProvider`]. Methods respect
/// the `quiet` flag in [`SandboxConfig::settings`]:
/// - When quiet, operations execute silently (no terminal output).
/// - Otherwise, a progress bar or spinner is displayed.
pub trait SandboxProviderExt: SandboxProvider {
    /// Write multiple files with a visual progress bar.
    ///
    /// Returns a [`SandboxError::BatchError`] if any writes fail.
    fn write_files_progress<P, C>(
        &self,
        items: &[(P, C)],
        label: &str,
    ) -> SandboxResult<()>
    where
        P: AsRef<Path> + Sync,
        C: AsRef<[u8]> + Sync,
    {
        let total = items.len();
        let quiet = self.config().settings.quiet;

        let results = if quiet || total == 0 {
            self.fs().write_files(items)
        } else {
            let pb = Arc::new(ui::progress::progress_bar(total as u64, label));
            let pb_cb = pb.clone();
            let results = self.fs().write_files_with_progress(
                items,
                Some(move || pb_cb.inc()),
            );
            pb.finish_with_message(&format!("{} - done", label));
            results
        };

        summarise_batch_results_unit(&results, total)
    }

    /// Read multiple files with a visual progress bar.
    ///
    /// Returns all `(path, Result<content>)` pairs.
    fn read_files_progress<P: AsRef<Path> + Sync>(
        &self,
        rel_paths: &[P],
        label: &str,
    ) -> Vec<(PathBuf, SandboxResult<String>)> {
        let total = rel_paths.len();
        let quiet = self.config().settings.quiet;

        if quiet || total == 0 {
            self.fs().read_files(rel_paths)
        } else {
            let pb = Arc::new(ui::progress::progress_bar(total as u64, label));
            let pb_cb = pb.clone();
            let results = self.fs().read_files_with_progress(
                rel_paths,
                Some(move || pb_cb.inc()),
            );
            pb.finish_with_message(&format!("{} - done", label));
            results
        }
    }

    /// Remove multiple files with a visual progress bar.
    ///
    /// Returns a [`SandboxError::BatchError`] if any removals fail.
    fn remove_files_progress<P: AsRef<Path> + Sync>(
        &self,
        rel_paths: &[P],
        label: &str,
    ) -> SandboxResult<()> {
        let total = rel_paths.len();
        let quiet = self.config().settings.quiet;

        let results = if quiet || total == 0 {
            self.fs().remove_files(rel_paths)
        } else {
            let pb = Arc::new(ui::progress::progress_bar(total as u64, label));
            let pb_cb = pb.clone();
            let results = self.fs().remove_files_with_progress(
                rel_paths,
                Some(move || pb_cb.inc()),
            );
            pb.finish_with_message(&format!("{} - done", label));
            results
        };

        summarise_batch_results_unit(&results, total)
    }

    /// Copy a file or directory tree with a spinner.
    ///
    /// For directory trees, a spinner is shown since the total file count
    /// is unknown ahead of time.
    fn copy_progress(
        &self,
        from: impl AsRef<Path>,
        to: impl AsRef<Path>,
        label: &str,
    ) -> SandboxResult<()> {
        let quiet = self.config().settings.quiet;

        if quiet {
            self.fs().copy(&from, &to)
        } else {
            let sp = ui::progress::spinner(label);
            let result = self.fs().copy(&from, &to);
            match &result {
                Ok(()) => sp.finish_with_message(&format!("{} - done", label)),
                Err(_) => sp.finish_with_message(&format!("{} - failed", label)),
            }
            result
        }
    }
}

/// Blanket implementation: every `SandboxProvider` gets `SandboxProviderExt` for free.
impl<T: SandboxProvider + ?Sized> SandboxProviderExt for T {}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check batch results for errors and produce a summary [`SandboxError::BatchError`].
fn summarise_batch_results_unit(
    results: &[(PathBuf, SandboxResult<()>)],
    total: usize,
) -> SandboxResult<()> {
    let errors: Vec<_> = results
        .iter()
        .filter(|(_, r)| r.is_err())
        .collect();

    if errors.is_empty() {
        return Ok(());
    }

    let first_error = errors[0]
        .1
        .as_ref()
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();

    Err(SandboxError::BatchError {
        count: errors.len(),
        total,
        first_error,
    })
}
