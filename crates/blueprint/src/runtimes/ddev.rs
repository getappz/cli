//! DDEV runtime — WordPress local development via Docker.
//!
//! Implements [`WordPressRuntime`] by delegating all operations to `ddev` CLI commands.
//! This is a direct extraction of the logic previously embedded in `executor.rs`,
//! `generator.rs`, and `crates/app/src/ddev_helpers.rs`.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::runtime::{RuntimeError, WordPressRuntime};

/// DDEV-based WordPress runtime.
#[derive(Debug)]
pub struct DdevRuntime;

impl DdevRuntime {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DdevRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl WordPressRuntime for DdevRuntime {
    fn name(&self) -> &str {
        "DDEV"
    }

    fn slug(&self) -> &str {
        "ddev"
    }

    fn is_available(&self) -> bool {
        which::which("ddev").is_ok()
    }

    fn is_configured(&self, project_path: &Path) -> bool {
        project_path.join(".ddev").join("config.yaml").exists()
    }

    fn configure(
        &self,
        project_path: &Path,
        project_type: &str,
        docroot: Option<&str>,
    ) -> Result<(), RuntimeError> {
        let mut args = vec![
            "config".to_string(),
            format!("--project-type={}", project_type),
        ];
        if let Some(dr) = docroot {
            args.push(format!("--docroot={}", dr));
        }
        if project_type == "php" {
            args.push("--php-version=8.2".to_string());
        }
        self.run_ddev(project_path, &args.iter().map(|s| s.as_str()).collect::<Vec<_>>())
    }

    fn start(&self, project_path: &Path) -> Result<(), RuntimeError> {
        // Skip if DDEV containers are already running
        if self.is_running(project_path) {
            return Ok(());
        }
        self.run_ddev(project_path, &["start"])
    }

    fn stop(&self, project_path: &Path) -> Result<(), RuntimeError> {
        self.run_ddev(project_path, &["stop"])
    }

    fn open_browser(&self, project_path: &Path) -> Result<(), RuntimeError> {
        // ddev launch may fail (e.g. no browser), but that's not fatal
        let _ = self.run_ddev(project_path, &["launch"]);
        Ok(())
    }

    fn stream_logs(&self, project_path: &Path) -> Result<(), RuntimeError> {
        self.run_ddev(project_path, &["logs", "-f"])
    }

    fn check_connectivity(&self, project_path: &Path) -> bool {
        let status = Command::new("ddev")
            .args(["exec", "curl", "-sSf", "-o", "/dev/null", "--connect-timeout", "5", "https://github.com"])
            .current_dir(project_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        status.map(|s| s.success()).unwrap_or(false)
    }

    fn wp_is_installed(&self, project_path: &Path) -> bool {
        let status = Command::new("ddev")
            .args(["exec", "wp", "core", "is-installed"])
            .current_dir(project_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        status.map(|s| s.success()).unwrap_or(false)
    }

    fn wp_install(
        &self,
        project_path: &Path,
        url: &str,
        admin_user: &str,
        admin_pass: &str,
    ) -> Result<(), RuntimeError> {
        self.wp_cli(project_path, &[
            "core", "install",
            &format!("--url={}", url),
            "--title=WordPress",
            &format!("--admin_user={}", admin_user),
            &format!("--admin_password={}", admin_pass),
            "--admin_email=admin@example.com",
            "--skip-email",
        ])
    }

    fn site_url(&self, project_path: &Path) -> String {
        let project_name = project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("wordpress");
        format!("https://{}.ddev.site", project_name)
    }

    fn wp_cli(&self, project_path: &Path, args: &[&str]) -> Result<(), RuntimeError> {
        let mut ddev_args = vec!["exec", "wp"];
        ddev_args.extend_from_slice(args);
        self.run_ddev(project_path, &ddev_args)
    }

    fn wp_cli_output(&self, project_path: &Path, args: &[&str]) -> Option<String> {
        let mut ddev_args = vec!["exec", "wp"];
        ddev_args.extend_from_slice(args);

        let output = Command::new("ddev")
            .args(&ddev_args)
            .current_dir(project_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if stdout.is_empty() {
                None
            } else {
                Some(stdout)
            }
        } else {
            None
        }
    }

    fn exec_shell(&self, project_path: &Path, cmd: &str) -> Result<(), RuntimeError> {
        self.run_ddev(project_path, &["exec", "bash", "-c", cmd])
    }

    fn exec_args(&self, project_path: &Path, args: &[&str]) -> Result<(), RuntimeError> {
        self.run_ddev(project_path, args)
    }

    fn exec_sql(&self, project_path: &Path, sql: &str) -> Result<(), RuntimeError> {
        let mut child = Command::new("ddev")
            .args(["mysql"])
            .current_dir(project_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| RuntimeError::CommandFailed {
                command: "ddev mysql".into(),
                message: e.to_string(),
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(sql.as_bytes()).map_err(|e| RuntimeError::CommandFailed {
                command: "ddev mysql (stdin write)".into(),
                message: e.to_string(),
            })?;
        }

        let status = child.wait().map_err(|e| RuntimeError::CommandFailed {
            command: "ddev mysql".into(),
            message: e.to_string(),
        })?;

        if !status.success() {
            return Err(RuntimeError::CommandFailed {
                command: "ddev mysql".into(),
                message: format!("exit code: {}", status.code().unwrap_or(-1)),
            });
        }
        Ok(())
    }

    fn set_php_version(
        &self,
        project_path: &Path,
        version: &str,
    ) -> Result<(), RuntimeError> {
        self.run_ddev(project_path, &["config", &format!("--php-version={}", version)])
    }

    // DdevRuntime uses the default implementations for download_url,
    // download_and_unzip, wp_eval, and http_request (which shell out via
    // exec_shell with curl/unzip inside the DDEV container).

    fn php_version(&self, project_path: &Path) -> Option<String> {
        let config_path = project_path.join(".ddev/config.yaml");
        let content = std::fs::read_to_string(&config_path).ok()?;
        for line in content.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("php_version:") {
                let ver = rest.trim().trim_matches('"').trim_matches('\'');
                if !ver.is_empty() {
                    return Some(ver.to_string());
                }
            }
        }
        None
    }
}

impl DdevRuntime {
    /// Check if DDEV containers are already running for this project.
    fn is_running(&self, project_path: &Path) -> bool {
        let output = Command::new("ddev")
            .args(["describe", "-j"])
            .current_dir(project_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        match output {
            Ok(out) if out.status.success() => {
                // ddev outputs JSON to stdout; try both stdout and stderr
                let stdout = String::from_utf8_lossy(&out.stdout);
                let to_try = if stdout.trim().is_empty() {
                    String::from_utf8_lossy(&out.stderr).to_string()
                } else {
                    stdout.to_string()
                };
                // ddev describe -j returns {"level":"info","msg":"...","raw":{...},"time":"..."}
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&to_try) {
                    val.get("raw")
                        .and_then(|r| r.get("status"))
                        .and_then(|s| s.as_str())
                        .map(|s| s == "running")
                        .unwrap_or(false)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Run `ddev <args>` with safe argument passing (no shell interpolation).
    fn run_ddev(&self, project_path: &Path, args: &[&str]) -> Result<(), RuntimeError> {
        let status = Command::new("ddev")
            .args(args)
            .current_dir(project_path)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| RuntimeError::CommandFailed {
                command: format!("ddev {}", args.join(" ")),
                message: e.to_string(),
            })?;

        if !status.success() {
            return Err(RuntimeError::CommandFailed {
                command: format!("ddev {}", args.join(" ")),
                message: format!("exit code: {}", status.code().unwrap_or(-1)),
            });
        }
        Ok(())
    }
}
