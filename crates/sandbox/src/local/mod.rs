//! Local filesystem sandbox provider.
//!
//! Implements [`SandboxProvider`] for running commands and managing files
//! directly on the host filesystem, scoped to a project directory.
//!
//! # Init lifecycle
//!
//! When [`SandboxProvider::init`] is called on a [`LocalProvider`], the
//! following steps execute in order (each with a spinner unless quiet mode
//! is enabled):
//!
//! 1. **Project directory** — created if it does not exist.
//! 2. **Mise availability** — checks PATH; auto-installs if
//!    `auto_install_mise` is `true` and mise is missing.
//! 3. **Tool installation** — `mise use -g tool@version` for all
//!    [`MiseToolSpec`] entries in config.
//! 4. **Project tool sync** — `mise install` if a `mise.toml`,
//!    `.mise.toml`, or `.tool-versions` file exists in the project root.
//! 5. **Environment loading** — `mise env --json` to capture PATH and
//!    other env vars set by mise.
//! 6. **Dotenv loading** — optional `.env` file (relative to project root)
//!    merged into the environment.
//!
//! After init, [`SandboxProvider::fs`] and [`SandboxProvider::exec`] are
//! ready to use.
//!
//! # Command execution
//!
//! All commands are wrapped through mise (`mise x -- <cmd>`) so they
//! automatically pick up the correct tool versions. The merged environment
//! is injected (priority: config env > mise env > system env).
//!
//! If mise is not available, commands fall back to direct execution.
//!
//! # UI behaviour
//!
//! UI output (spinners, info/success/warning messages) is driven by the
//! internal `ui` crate. Set `SandboxSettings::quiet` to suppress all
//! terminal output — useful for tests and CI.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use async_trait::async_trait;
use starbase_utils::fs as starbase_fs;

use crate::config::{MiseToolSpec, SandboxConfig};
use crate::error::{SandboxError, SandboxResult};
use crate::mise::MiseManager;
use crate::provider::{CommandOutput, SandboxProvider};
use crate::scoped_fs::ScopedFs;

/// Local filesystem provider.
///
/// All operations happen directly on the host OS. Commands are executed via
/// the `command` crate, optionally wrapped through mise for tool management.
pub struct LocalProvider {
    /// Stored config (populated during `init`).
    config: Option<SandboxConfig>,
    /// Scoped filesystem handle (populated during `init`).
    scoped_fs: Option<ScopedFs>,
    /// Mise manager (populated during `init`).
    mise: Option<MiseManager>,
    /// Cached mise environment variables.
    mise_env: HashMap<String, String>,
    /// Whether to suppress UI output.
    quiet: bool,
}

impl LocalProvider {
    /// Create a new, uninitialised local provider.
    ///
    /// Call [`SandboxProvider::init`] to set it up before using it.
    pub fn new() -> Self {
        Self {
            config: None,
            scoped_fs: None,
            mise: None,
            mise_env: HashMap::new(),
            quiet: false,
        }
    }

    /// Find node_modules/.bin directories by walking up from the project directory.
    /// Matches Vercel's getNodeBinPaths behavior.
    fn find_node_modules_bin_paths(project_path: &Path) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let canonical_start = project_path
            .canonicalize()
            .unwrap_or_else(|_| project_path.to_path_buf());
        let mut current = Some(canonical_start.as_path());

        while let Some(dir) = current {
            let node_modules_bin = dir.join("node_modules").join(".bin");
            if node_modules_bin.exists() && node_modules_bin.is_dir() {
                paths.push(node_modules_bin);
            }
            let node_modules_bin_alt = dir.join("node_modules").join("bin");
            if node_modules_bin_alt.exists() && node_modules_bin_alt.is_dir() {
                paths.push(node_modules_bin_alt);
            }
            current = dir.parent();
        }
        paths
    }

    /// Build the merged environment for command execution.
    ///
    /// Priority (highest first):
    /// 1. Settings `env` from config
    /// 2. Mise environment variables (with node_modules/.bin prepended to PATH)
    /// 3. System environment (inherited by `command::Command` automatically)
    fn merged_env(&self) -> HashMap<String, String> {
        let mut env = self.mise_env.clone();

        // Prepend node_modules/.bin to PATH so project binaries (e.g. astro, next) are found.
        if let Some(cfg) = &self.config {
            let node_modules_bins = Self::find_node_modules_bin_paths(&cfg.project_path);
            if !node_modules_bins.is_empty() {
                let path_sep = if cfg!(target_os = "windows") { ";" } else { ":" };
                let node_modules_path_str: Vec<String> = node_modules_bins
                    .iter()
                    .filter_map(|p| p.to_str().map(String::from))
                    .collect();
                let prepend = node_modules_path_str.join(path_sep);
                let existing = env.get("PATH").cloned().unwrap_or_default();
                let new_path = if existing.is_empty() {
                    prepend
                } else {
                    format!("{}{}{}", prepend, path_sep, existing)
                };
                env.insert("PATH".to_string(), new_path);
            }
        }

        // Overlay config-level env vars.
        if let Some(cfg) = &self.config {
            for (k, v) in &cfg.settings.env {
                env.insert(k.clone(), v.clone());
            }
        }

        env
    }

    /// Conditionally print a status::info message.
    fn info(&self, msg: &str) {
        if !self.quiet {
            let _ = ui::status::info(msg);
        }
    }

    /// Conditionally print a status::success message.
    fn success(&self, msg: &str) {
        if !self.quiet {
            let _ = ui::status::success(msg);
        }
    }

    /// Conditionally print a status::warning message.
    fn warn(&self, msg: &str) {
        if !self.quiet {
            let _ = ui::status::warning(msg);
        }
    }

    /// Create a spinner that respects the quiet flag.
    /// Returns `None` when quiet is enabled so callers can no-op.
    fn spinner(&self, msg: &str) -> Option<ui::progress::SpinnerHandle> {
        if self.quiet {
            None
        } else {
            Some(ui::progress::spinner(msg))
        }
    }
}

impl Default for LocalProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SandboxProvider for LocalProvider {
    // ------------------------------------------------------------------
    // Lifecycle
    // ------------------------------------------------------------------

    async fn init(&mut self, config: &SandboxConfig) -> SandboxResult<()> {
        self.quiet = config.settings.quiet;
        let project_path = &config.project_path;

        // ── Step 1: Project directory ──────────────────────────────
        if !project_path.exists() {
            let sp = self.spinner("Creating project directory...");
            starbase_fs::create_dir_all(project_path)?;
            if let Some(s) = sp {
                s.finish_with_message("Project directory created");
            }
        }

        let canonical = project_path
            .canonicalize()
            .map_err(SandboxError::Io)?;

        self.info(&format!(
            "Sandbox root: {}",
            canonical.display()
        ));

        // Set up scoped filesystem.
        self.scoped_fs = Some(ScopedFs::new(&canonical)?);

        // Set up mise manager.
        let mise = MiseManager::new(&canonical);

        // ── Step 2: Mise availability ──────────────────────────────
        if config.settings.auto_install_mise {
            if MiseManager::is_available() {
                if let Some(version) = MiseManager::version() {
                    self.success(&format!("mise {} found", version));
                } else {
                    self.success("mise found");
                }
            } else {
                let sp = self.spinner("Installing mise...");
                MiseManager::ensure_installed()?;
                if let Some(s) = sp {
                    s.finish_with_message("mise installed successfully");
                }
            }
        } else if !MiseManager::is_available() {
            self.warn("mise is not installed and auto-install is disabled");
        }

        // ── Step 3: Tool installation ──────────────────────────────
        if !config.settings.mise_tools.is_empty() {
            let tool_names: Vec<String> = config
                .settings
                .mise_tools
                .iter()
                .map(|t| t.to_mise_arg())
                .collect();
            let label = format!("Installing tools: {}", tool_names.join(", "));
            let sp = self.spinner(&label);
            mise.install_tools(&config.settings.mise_tools)?;
            if let Some(s) = sp {
                s.finish_with_message(&format!(
                    "Installed {} tool{}",
                    config.settings.mise_tools.len(),
                    if config.settings.mise_tools.len() == 1 { "" } else { "s" }
                ));
            }
        }

        // ── Step 4: Sync project tools ─────────────────────────────
        let has_tool_config = canonical.join("mise.toml").exists()
            || canonical.join(".mise.toml").exists()
            || canonical.join(".tool-versions").exists();

        if has_tool_config {
            let sp = self.spinner("Syncing project tool versions...");
            mise.sync_tools()?;
            if let Some(s) = sp {
                s.finish_with_message("Project tools synced");
            }
        }

        // ── Step 5: Load environment ───────────────────────────────
        {
            let sp = self.spinner("Loading mise environment...");
            self.mise_env = mise.load_env()?;
            if let Some(s) = sp {
                let count = self.mise_env.len();
                s.finish_with_message(&format!(
                    "Loaded {} env variable{}",
                    count,
                    if count == 1 { "" } else { "s" }
                ));
            }
        }

        // ── Step 6: Dotenv file ────────────────────────────────────
        if let Some(dotenv_path) = &config.settings.dotenv {
            let scoped = self.scoped_fs.as_ref().unwrap();
            if scoped.exists(dotenv_path) {
                let sp = self.spinner(&format!(
                    "Loading {}...",
                    dotenv_path.display()
                ));
                let content = scoped.read_to_string(dotenv_path)?;
                let mut loaded = 0usize;
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((key, value)) = line.split_once('=') {
                        let key = key.trim().to_string();
                        let value = value.trim().trim_matches('"').trim_matches('\'').to_string();
                        self.mise_env.insert(key, value);
                        loaded += 1;
                    }
                }
                if let Some(s) = sp {
                    s.finish_with_message(&format!(
                        "Loaded {} variable{} from {}",
                        loaded,
                        if loaded == 1 { "" } else { "s" },
                        dotenv_path.display()
                    ));
                }
            } else {
                self.warn(&format!(
                    "dotenv file not found: {}",
                    dotenv_path.display()
                ));
            }
        }

        self.mise = Some(mise);

        // Store config (with canonical project path).
        let mut stored_config = config.clone();
        stored_config.project_path = canonical;
        self.config = Some(stored_config);

        self.success("Sandbox ready");

        Ok(())
    }

    async fn teardown(&mut self) -> SandboxResult<()> {
        self.info("Tearing down sandbox...");
        self.config = None;
        self.scoped_fs = None;
        self.mise = None;
        self.mise_env.clear();
        self.success("Sandbox torn down");
        Ok(())
    }

    // ------------------------------------------------------------------
    // Filesystem
    // ------------------------------------------------------------------

    fn fs(&self) -> &ScopedFs {
        self.scoped_fs
            .as_ref()
            .expect("LocalProvider::init() must be called before fs()")
    }

    // ------------------------------------------------------------------
    // Command execution
    // ------------------------------------------------------------------

    async fn exec(&self, cmd: &str) -> SandboxResult<CommandOutput> {
        let mise = self
            .mise
            .as_ref()
            .expect("LocalProvider::init() must be called before exec()");
        let env = self.merged_env();
        let output = mise.exec_in_env(cmd, Some(&env))?;
        Ok(CommandOutput::new(output))
    }

    async fn exec_interactive(&self, cmd: &str) -> SandboxResult<ExitStatus> {
        let mise = self
            .mise
            .as_ref()
            .expect("LocalProvider::init() must be called before exec_interactive()");
        let env = self.merged_env();
        mise.exec_interactive(cmd, Some(&env))
    }

    // ------------------------------------------------------------------
    // Tool management
    // ------------------------------------------------------------------

    async fn ensure_tool(&self, tool: &MiseToolSpec) -> SandboxResult<()> {
        let mise = self
            .mise
            .as_ref()
            .expect("LocalProvider::init() must be called before ensure_tool()");
        let sp = self.spinner(&format!("Ensuring {}...", tool.to_mise_arg()));
        let result = mise.install_tool(tool);
        if let Some(s) = sp {
            match &result {
                Ok(()) => s.finish_with_message(&format!("{} ready", tool.to_mise_arg())),
                Err(_) => s.finish_with_message(&format!("{} failed", tool.to_mise_arg())),
            }
        }
        result
    }

    async fn exec_with_tool(
        &self,
        tool: &MiseToolSpec,
        cmd: &str,
    ) -> SandboxResult<CommandOutput> {
        let mise = self
            .mise
            .as_ref()
            .expect("LocalProvider::init() must be called before exec_with_tool()");
        let env = self.merged_env();
        let output = mise.exec_with_tool(tool, cmd, Some(&env))?;
        Ok(CommandOutput::new(output))
    }

    // ------------------------------------------------------------------
    // Info
    // ------------------------------------------------------------------

    fn project_path(&self) -> &Path {
        self.config
            .as_ref()
            .map(|c| c.project_path.as_path())
            .expect("LocalProvider::init() must be called before project_path()")
    }

    fn config(&self) -> &SandboxConfig {
        self.config
            .as_ref()
            .expect("LocalProvider::init() must be called before config()")
    }
}
