//! Init configuration and context types.

use std::path::PathBuf;
use std::sync::Arc;

use sandbox::SandboxProvider;

use crate::error::InitResult;

/// Options for an init run.
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// Project name / directory name.
    pub project_name: String,

    /// Output directory (parent of project_name).
    pub output_dir: PathBuf,

    /// Skip dependency installation.
    pub skip_install: bool,

    /// Overwrite existing directory.
    pub force: bool,

    /// Machine-readable output.
    pub json_output: bool,

    /// CI mode (non-interactive).
    pub is_ci: bool,

    /// Blueprint name, path, or URL (from --blueprint flag).
    pub blueprint: Option<String>,

    /// Skip registry cache (from --no-cache flag).
    pub no_cache: bool,
}

impl InitOptions {
    /// Full path where the project will be created.
    pub fn project_path(&self) -> PathBuf {
        self.output_dir.join(&self.project_name)
    }
}

/// Context passed to provider operations with everything needed for init.
#[derive(Clone)]
pub struct InitContext {
    /// The sandbox used for all command execution and file I/O.
    pub sandbox: Arc<dyn SandboxProvider>,

    /// Init options.
    pub options: InitOptions,

    /// Resolved source string (e.g. "astro", "https://github.com/...", "npm:create-foo").
    pub source: String,
}

impl InitContext {
    /// Create a new init context.
    pub fn new(sandbox: Arc<dyn SandboxProvider>, options: InitOptions, source: String) -> Self {
        Self {
            sandbox,
            options,
            source,
        }
    }

    /// Get the project root path.
    pub fn project_path(&self) -> PathBuf {
        self.sandbox.project_path().to_path_buf()
    }

    /// Execute a command through the sandbox.
    pub async fn exec(&self, cmd: &str) -> InitResult<sandbox::CommandOutput> {
        self.sandbox.exec(cmd).await.map_err(Into::into)
    }

    /// Execute a command interactively through the sandbox.
    pub async fn exec_interactive(&self, cmd: &str) -> InitResult<std::process::ExitStatus> {
        self.sandbox.exec_interactive(cmd).await.map_err(Into::into)
    }

    /// Access the sandbox's scoped filesystem.
    pub fn fs(&self) -> &sandbox::ScopedFs {
        self.sandbox.fs()
    }
}

/// Detect whether we're running in a CI/CD environment.
pub fn is_ci_environment() -> bool {
    common::env::is_ci()
}
