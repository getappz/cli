//! Mise tool version manager integration.
//!
//! Provides [`MiseManager`] which wraps all interactions with the
//! [mise](https://mise.jdx.dev/) CLI tool. Mise manages tool versions
//! (Node, Hugo, Bun, Python, Go, etc.) and per-project environments.
//!
//! # Responsibilities
//!
//! | Area | Methods |
//! |------|---------|
//! | Availability | `is_available`, `version` |
//! | Installation | `ensure_installed` (auto-detects OS/package manager) |
//! | Tool management | `install_tool`, `install_tools`, `sync_tools` |
//! | Command execution | `exec_in_env`, `exec_with_tool`, `exec_interactive`, `exec_raw` |
//! | Environment | `load_env` (parses `mise env --json`) |
//!
//! # Platform-specific installation
//!
//! [`MiseManager::ensure_installed`] tries the following installers in order:
//!
//! **Unix/macOS:** `brew` → `apk` → `pacman` → `curl https://mise.run | sh`
//!
//! **Windows:** `winget` → `scoop`
//!
//! If the install command returns a non-zero exit code but mise becomes
//! available on PATH afterwards, the error is silently ignored (handles
//! edge cases like "already installed" exit codes).
//!
//! # Fallback behaviour
//!
//! - [`exec_in_env`](MiseManager::exec_in_env) falls back to direct command
//!   execution if mise is not available on PATH.
//! - [`load_env`](MiseManager::load_env) returns an empty map if mise is
//!   not available or `mise env --json` fails.
//!
//! # Thread safety
//!
//! `MiseManager` is `Send + Sync + Clone`. It holds only a `PathBuf` and
//! delegates all work to the `command` crate, so it can safely be shared
//! across async tasks.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Output;

use command::Command;

use crate::config::MiseToolSpec;
use crate::error::{SandboxError, SandboxResult};

/// Manages mise interactions scoped to a project directory.
#[derive(Debug, Clone)]
pub struct MiseManager {
    /// The project directory where mise commands are executed.
    project_path: PathBuf,
}

impl MiseManager {
    /// Create a new `MiseManager` for the given project path.
    pub fn new(project_path: impl Into<PathBuf>) -> Self {
        Self {
            project_path: project_path.into(),
        }
    }

    /// Return the project path this manager is scoped to.
    pub fn project_path(&self) -> &Path {
        &self.project_path
    }

    // ------------------------------------------------------------------
    // Mise availability
    // ------------------------------------------------------------------

    /// Check whether `mise` is available on the system PATH.
    pub fn is_available() -> bool {
        which::which("mise").is_ok()
    }

    // ------------------------------------------------------------------
    // Mise installation
    // ------------------------------------------------------------------

    /// Ensure mise is installed on the system.
    ///
    /// If mise is already on PATH, this is a no-op.
    /// Otherwise it attempts to install it using the platform-appropriate
    /// package manager (brew, apt, winget, scoop, etc.) or the official
    /// install script.
    pub fn ensure_installed() -> SandboxResult<()> {
        if Self::is_available() {
            return Ok(());
        }

        let result = {
            #[cfg(target_os = "windows")]
            {
                Self::install_windows()
            }
            #[cfg(not(target_os = "windows"))]
            {
                Self::install_unix()
            }
        };

        // Even if the install command returned an error, mise might now be on
        // PATH (e.g. winget returning non-zero for "already installed").
        if result.is_err() && Self::is_available() {
            return Ok(());
        }

        result
    }

    #[cfg(not(target_os = "windows"))]
    fn install_unix() -> SandboxResult<()> {
        // Try the most common package managers in order of preference.
        let installers: &[(&str, &str)] = &[
            ("brew", "brew install mise"),
            ("apk", "apk add mise"),
            ("pacman", "sudo pacman -S --noconfirm mise"),
        ];

        for (check, cmd) in installers {
            if which::which(check).is_ok() {
                return run_simple(cmd).map_err(|e| SandboxError::MiseSetupFailed {
                    reason: format!("{} install failed: {}", check, e),
                });
            }
        }

        // Fallback: official install script.
        run_simple("curl https://mise.run | sh").map_err(|e| SandboxError::MiseSetupFailed {
            reason: format!("curl installer failed: {}", e),
        })
    }

    #[cfg(target_os = "windows")]
    fn install_windows() -> SandboxResult<()> {
        if which::which("winget").is_ok() {
            return run_simple(
                "winget install --id jdx.mise -e --accept-source-agreements --accept-package-agreements",
            )
            .map_err(|e| SandboxError::MiseSetupFailed {
                reason: format!("winget install failed: {}", e),
            });
        }
        if which::which("scoop").is_ok() {
            return run_simple("scoop install mise").map_err(|e| SandboxError::MiseSetupFailed {
                reason: format!("scoop install failed: {}", e),
            });
        }
        Err(SandboxError::MiseSetupFailed {
            reason: "No supported Windows package manager found (winget or scoop required)"
                .to_string(),
        })
    }

    // ------------------------------------------------------------------
    // Tool installation
    // ------------------------------------------------------------------

    /// Install a specific tool via mise (user-global scope).
    ///
    /// Runs `mise use -g <tool>@<version>` which both installs the tool and
    /// activates it globally.
    pub fn install_tool(&self, spec: &MiseToolSpec) -> SandboxResult<()> {
        if !Self::is_available() {
            return Err(SandboxError::MiseSetupFailed {
                reason: "mise is not installed".to_string(),
            });
        }

        let arg = spec.to_mise_arg();
        let mut cmd = Command::new("mise");
        cmd.arg("use").arg("-g").arg(&arg);
        cmd.cwd(&self.project_path);
        cmd.set_error_on_nonzero(false);

        let output = cmd.exec().map_err(|e| SandboxError::CommandFailed {
            command: format!("mise use -g {}", arg),
            reason: e.to_string(),
        })?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(SandboxError::CommandFailed {
                command: format!("mise use -g {}", arg),
                reason: stderr.to_string(),
            })
        }
    }

    /// Install multiple tools in a single mise command.
    ///
    /// Runs `mise use -g tool1@v1 tool2@v2 ...` which is significantly
    /// faster than installing each tool individually because mise resolves
    /// and downloads them in parallel internally.
    pub fn install_tools(&self, specs: &[MiseToolSpec]) -> SandboxResult<()> {
        if specs.is_empty() {
            return Ok(());
        }
        if !Self::is_available() {
            return Err(SandboxError::MiseSetupFailed {
                reason: "mise is not installed".to_string(),
            });
        }

        let args: Vec<String> = specs.iter().map(|s| s.to_mise_arg()).collect();
        let mut cmd = Command::new("mise");
        cmd.arg("use").arg("-g");
        for arg in &args {
            cmd.arg(arg);
        }
        cmd.cwd(&self.project_path);
        cmd.set_error_on_nonzero(false);

        let display = format!("mise use -g {}", args.join(" "));
        let output = cmd.exec().map_err(|e| SandboxError::CommandFailed {
            command: display.clone(),
            reason: e.to_string(),
        })?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(SandboxError::CommandFailed {
                command: display,
                reason: stderr.to_string(),
            })
        }
    }

    /// Sync tools from `.tool-versions` or `mise.toml` in the project directory.
    ///
    /// Runs `mise install` in the project directory.
    pub fn sync_tools(&self) -> SandboxResult<()> {
        if !Self::is_available() {
            return Err(SandboxError::MiseSetupFailed {
                reason: "mise is not installed".to_string(),
            });
        }

        let mut cmd = Command::new("mise");
        cmd.arg("install");
        cmd.cwd(&self.project_path);
        cmd.set_error_on_nonzero(false);

        let output = cmd.exec().map_err(|e| SandboxError::CommandFailed {
            command: "mise install".to_string(),
            reason: e.to_string(),
        })?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(SandboxError::CommandFailed {
                command: "mise install".to_string(),
                reason: stderr.to_string(),
            })
        }
    }

    // ------------------------------------------------------------------
    // Command execution through mise
    // ------------------------------------------------------------------

    /// Execute a command wrapped with mise for a specific tool version.
    ///
    /// Produces: `mise x <tool>@<version> -- <cmd>`
    pub fn exec_with_tool(
        &self,
        spec: &MiseToolSpec,
        cmd_str: &str,
        extra_env: Option<&HashMap<String, String>>,
    ) -> SandboxResult<Output> {
        if !Self::is_available() {
            return Err(SandboxError::MiseSetupFailed {
                reason: "mise is not installed".to_string(),
            });
        }

        let mise_cmd = format!("mise x {} -- {}", spec.to_mise_arg(), cmd_str);
        let mut cmd = Command::new(&mise_cmd);
        cmd.cwd(&self.project_path);
        cmd.set_error_on_nonzero(false);

        if let Some(env) = extra_env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }

        cmd.exec().map_err(|e| SandboxError::CommandFailed {
            command: mise_cmd,
            reason: e.to_string(),
        })
    }

    /// Execute a command through the mise environment (no specific tool pinned).
    ///
    /// Produces: `mise x -- <cmd>`
    pub fn exec_in_env(
        &self,
        cmd_str: &str,
        extra_env: Option<&HashMap<String, String>>,
    ) -> SandboxResult<Output> {
        if !Self::is_available() {
            // If mise is not available, fall back to running the command directly.
            return self.exec_raw(cmd_str, extra_env);
        }

        let mise_cmd = format!("mise x -- {}", cmd_str);
        let mut cmd = Command::new(&mise_cmd);
        cmd.cwd(&self.project_path);
        cmd.set_error_on_nonzero(false);

        if let Some(env) = extra_env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }

        cmd.exec().map_err(|e| SandboxError::CommandFailed {
            command: mise_cmd,
            reason: e.to_string(),
        })
    }

    /// Execute a command interactively through mise (inherits stdin/stdout/stderr).
    pub fn exec_interactive(
        &self,
        cmd_str: &str,
        extra_env: Option<&HashMap<String, String>>,
    ) -> SandboxResult<std::process::ExitStatus> {
        let final_cmd = if Self::is_available() {
            format!("mise x -- {}", cmd_str)
        } else {
            cmd_str.to_string()
        };

        let mut cmd = Command::new(&final_cmd);
        cmd.cwd(&self.project_path);

        if let Some(env) = extra_env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }

        cmd.exec_interactive()
            .map_err(|e| SandboxError::CommandFailed {
                command: final_cmd,
                reason: e.to_string(),
            })
    }

    /// Execute a raw command (no mise wrapping) scoped to the project path.
    pub fn exec_raw(
        &self,
        cmd_str: &str,
        extra_env: Option<&HashMap<String, String>>,
    ) -> SandboxResult<Output> {
        let mut cmd = Command::new(cmd_str);
        cmd.cwd(&self.project_path);
        cmd.set_error_on_nonzero(false);

        if let Some(env) = extra_env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }

        cmd.exec().map_err(|e| SandboxError::CommandFailed {
            command: cmd_str.to_string(),
            reason: e.to_string(),
        })
    }

    // ------------------------------------------------------------------
    // Environment
    // ------------------------------------------------------------------

    /// Load the mise environment variables for the project directory.
    ///
    /// Runs `mise env --json` and returns the parsed key-value pairs.
    /// Returns an empty map if mise is not available.
    pub fn load_env(&self) -> SandboxResult<HashMap<String, String>> {
        if !Self::is_available() {
            return Ok(HashMap::new());
        }

        let mut cmd = Command::new("mise");
        cmd.arg("env").arg("--json");
        cmd.cwd(&self.project_path);
        cmd.set_error_on_nonzero(false);

        let output = cmd.exec().map_err(|e| SandboxError::CommandFailed {
            command: "mise env --json".to_string(),
            reason: e.to_string(),
        })?;

        if !output.status.success() {
            // Non-fatal: return empty env if mise env fails (e.g. no config).
            return Ok(HashMap::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let env_json: serde_json::Value =
            serde_json::from_str(stdout.trim()).unwrap_or(serde_json::Value::Object(
                serde_json::Map::new(),
            ));

        let mut env = HashMap::new();
        if let serde_json::Value::Object(map) = env_json {
            for (key, value) in map {
                if let serde_json::Value::String(val) = value {
                    env.insert(key, val);
                }
            }
        }

        Ok(env)
    }

    /// Get the mise version string, or `None` if mise is not available.
    pub fn version() -> Option<String> {
        if !Self::is_available() {
            return None;
        }

        let mut cmd = Command::new("mise");
        cmd.arg("--version");
        cmd.set_error_on_nonzero(false);

        cmd.exec().ok().and_then(|output| {
            if output.status.success() {
                let v = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if v.is_empty() {
                    None
                } else {
                    Some(v)
                }
            } else {
                None
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Run a simple shell command and return Ok/Err based on exit status.
fn run_simple(cmd_str: &str) -> SandboxResult<()> {
    let mut cmd = Command::new(cmd_str);
    cmd.set_error_on_nonzero(false);

    let output = cmd.exec().map_err(|e| SandboxError::CommandFailed {
        command: cmd_str.to_string(),
        reason: e.to_string(),
    })?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(SandboxError::CommandFailed {
            command: cmd_str.to_string(),
            reason: stderr.to_string(),
        })
    }
}
