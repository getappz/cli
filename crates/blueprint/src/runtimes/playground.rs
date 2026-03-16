//! WordPress Playground runtime — local development via Node.js WASM.
//!
//! Implements [`WordPressRuntime`] using the `@wp-playground/cli` npm package.
//! Requires Node.js 20.18+ and npx. No Docker or PHP installation needed.
//!
//! The Playground CLI runs WordPress entirely in WASM with SQLite for the database.
//! It supports native blueprint execution via `run-blueprint` and WP-CLI via
//! `run-blueprint` with inline steps.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::runtime::{RuntimeError, WordPressRuntime};

/// Playground state directory name inside the project.
const PLAYGROUND_DIR: &str = ".playground";

/// Playground state file inside the state directory.
const PLAYGROUND_STATE: &str = "state.json";

/// Default port for the Playground server.
const DEFAULT_PORT: u16 = 9400;

/// WordPress Playground CLI runtime.
#[derive(Debug)]
pub struct PlaygroundRuntime;

impl PlaygroundRuntime {
    pub fn new() -> Self {
        Self
    }

    /// Check if the installed Node.js version meets the minimum requirement (20.18+).
    pub fn check_node_version() -> bool {
        let output = Command::new("node")
            .args(["--version"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                // version is like "v20.18.0"
                let version = version.strip_prefix('v').unwrap_or(&version);
                parse_node_version_ok(version)
            }
            _ => false,
        }
    }
}

impl Default for PlaygroundRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl WordPressRuntime for PlaygroundRuntime {
    fn name(&self) -> &str {
        "WordPress Playground"
    }

    fn slug(&self) -> &str {
        "playground"
    }

    fn is_available(&self) -> bool {
        which::which("node").is_ok()
            && which::which("npx").is_ok()
            && Self::check_node_version()
    }

    fn is_configured(&self, project_path: &Path) -> bool {
        project_path.join(PLAYGROUND_DIR).join(PLAYGROUND_STATE).exists()
    }

    fn configure(
        &self,
        project_path: &Path,
        _project_type: &str,
        _docroot: Option<&str>,
    ) -> Result<(), RuntimeError> {
        let state_dir = project_path.join(PLAYGROUND_DIR);
        std::fs::create_dir_all(&state_dir).map_err(|e| RuntimeError::Io {
            path: state_dir.clone(),
            source: e,
        })?;

        let state_file = state_dir.join(PLAYGROUND_STATE);
        let state = serde_json::json!({
            "runtime": "playground",
        });

        let state_str = serde_json::to_string_pretty(&state).unwrap_or_else(|_| {
            r#"{"runtime":"playground"}"#.to_string()
        });

        std::fs::write(&state_file, state_str).map_err(|e| RuntimeError::Io {
            path: state_file,
            source: e,
        })?;

        Ok(())
    }

    fn start(&self, project_path: &Path) -> Result<(), RuntimeError> {
        // The Playground server is started interactively (blocks until Ctrl+C).
        // For `appz dev`, this is called via stream_logs which runs the server process.
        // For blueprint operations, we use `run-blueprint` directly — no server needed.
        //
        // If the state directory doesn't exist, configure first.
        if !self.is_configured(project_path) {
            self.configure(project_path, "wordpress", None)?;
        }
        Ok(())
    }

    fn stop(&self, project_path: &Path) -> Result<(), RuntimeError> {
        // Kill any running Playground server by finding the process.
        // This is best-effort — the server process is typically stopped via Ctrl+C.
        let _ = Command::new("pkill")
            .args(["-f", &format!("wp-playground.*{}", project_path.display())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        Ok(())
    }

    fn open_browser(&self, _project_path: &Path) -> Result<(), RuntimeError> {
        let url = format!("http://localhost:{}", DEFAULT_PORT);
        let _ = open_url(&url);
        Ok(())
    }

    fn stream_logs(&self, project_path: &Path) -> Result<(), RuntimeError> {
        // Run the Playground server — this blocks until the process exits (Ctrl+C).
        let mount_arg = format!("{}:/wordpress", project_path.display());
        let port_arg = DEFAULT_PORT.to_string();

        let mut args = vec![
            "@wp-playground/cli@latest".to_string(),
            "server".to_string(),
            "--mount".to_string(), mount_arg,
            "--port".to_string(), port_arg,
            "--login".to_string(),
        ];

        // Apply stored PHP version if set
        if let Some(php) = self.php_version(project_path) {
            args.push("--php".to_string());
            args.push(php);
        }

        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let status = Command::new("npx")
            .args(&args_refs)
            .current_dir(project_path)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| RuntimeError::CommandFailed {
                command: "npx @wp-playground/cli server".into(),
                message: e.to_string(),
            })?;

        if !status.success() {
            return Err(RuntimeError::CommandFailed {
                command: "npx @wp-playground/cli server".into(),
                message: format!("exit code: {}", status.code().unwrap_or(-1)),
            });
        }
        Ok(())
    }

    fn check_connectivity(&self, _project_path: &Path) -> bool {
        // Playground runs in WASM — no container DNS issues.
        true
    }

    fn wp_is_installed(&self, project_path: &Path) -> bool {
        // Check if wp-config.php or WordPress files exist.
        // In Playground mode, WordPress is always "installed" once the server runs.
        project_path.join("wp-config.php").exists()
            || project_path.join("wp-config-sample.php").exists()
    }

    fn wp_install(
        &self,
        project_path: &Path,
        _url: &str,
        admin_user: &str,
        admin_pass: &str,
    ) -> Result<(), RuntimeError> {
        // Use a blueprint step to install WordPress
        let blueprint = serde_json::json!({
            "steps": [{
                "step": "runWpInstallationWizard",
                "options": {
                    "adminUsername": admin_user,
                    "adminPassword": admin_pass
                }
            }]
        });
        self.run_blueprint_json(project_path, &blueprint)
    }

    fn site_url(&self, _project_path: &Path) -> String {
        format!("http://localhost:{}", DEFAULT_PORT)
    }

    fn wp_cli(&self, project_path: &Path, args: &[&str]) -> Result<(), RuntimeError> {
        // Execute WP-CLI via a blueprint with a wp-cli step.
        // Use array form to preserve argument boundaries (avoids space-splitting issues).
        let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let blueprint = serde_json::json!({
            "steps": [{
                "step": "wp-cli",
                "command": args_vec
            }]
        });
        self.run_blueprint_json(project_path, &blueprint)
    }

    fn wp_cli_output(&self, project_path: &Path, args: &[&str]) -> Option<String> {
        // Execute WP-CLI via blueprint and capture output
        let command_str = args.join(" ");
        let blueprint = serde_json::json!({
            "steps": [{
                "step": "wp-cli",
                "command": command_str
            }]
        });

        let bp_json = serde_json::to_string(&blueprint).ok()?;
        let mount_arg = format!("{}:/wordpress", project_path.display());

        let output = Command::new("npx")
            .args([
                "@wp-playground/cli@latest",
                "run-blueprint",
                "--mount", &mount_arg,
                "--blueprint", "-",
            ])
            .current_dir(project_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()
            .and_then(|mut child| {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(bp_json.as_bytes());
                }
                child.wait_with_output().ok()
            })?;

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

    fn set_php_version(
        &self,
        project_path: &Path,
        version: &str,
    ) -> Result<(), RuntimeError> {
        // Persist to .playground/state.json so php_version() can read it back
        let state_path = project_path.join(PLAYGROUND_DIR).join(PLAYGROUND_STATE);
        let mut state: serde_json::Value = if let Ok(content) = std::fs::read_to_string(&state_path) {
            serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
        } else {
            serde_json::json!({"runtime": "playground"})
        };

        if let Some(obj) = state.as_object_mut() {
            obj.insert("php_version".to_string(), serde_json::Value::String(version.to_string()));
        }

        let state_str = serde_json::to_string_pretty(&state).unwrap_or_default();
        std::fs::write(&state_path, state_str).map_err(|e| RuntimeError::Io {
            path: state_path,
            source: e,
        })?;

        // Note: the --php flag is passed at server/run-blueprint launch time.
        // The stored version is used by stream_logs() and run_blueprint_json().
        Ok(())
    }

    fn download_url(
        &self,
        project_path: &Path,
        url: &str,
        dest_path: &str,
    ) -> Result<(), RuntimeError> {
        // Use Playground-native writeFile step with URL resource
        let blueprint = serde_json::json!({
            "steps": [{
                "step": "writeFile",
                "path": dest_path,
                "data": {
                    "resource": "url",
                    "url": url
                }
            }]
        });
        self.run_blueprint_json(project_path, &blueprint)
    }

    fn download_and_unzip(
        &self,
        project_path: &Path,
        url: &str,
        extract_to: &str,
    ) -> Result<(), RuntimeError> {
        // Use Playground-native unzip step with URL resource
        let blueprint = serde_json::json!({
            "steps": [{
                "step": "unzip",
                "zipFile": {
                    "resource": "url",
                    "url": url
                },
                "extractToPath": extract_to
            }]
        });
        self.run_blueprint_json(project_path, &blueprint)
    }

    fn wp_eval(
        &self,
        project_path: &Path,
        code: &str,
    ) -> Result<(), RuntimeError> {
        // Use native runPHP step instead of shelling out via wp eval
        let full_code = if code.starts_with("<?php") {
            code.to_string()
        } else {
            format!("<?php {}", code)
        };
        let blueprint = serde_json::json!({
            "steps": [{
                "step": "runPHP",
                "code": full_code
            }]
        });
        self.run_blueprint_json(project_path, &blueprint)
    }

    fn http_request(
        &self,
        project_path: &Path,
        method: &str,
        url: &str,
    ) -> Result<(), RuntimeError> {
        // Use Playground-native request step
        let blueprint = serde_json::json!({
            "steps": [{
                "step": "request",
                "request": {
                    "method": method,
                    "url": url
                }
            }]
        });
        self.run_blueprint_json(project_path, &blueprint)
    }

    fn exec_shell(&self, project_path: &Path, cmd: &str) -> Result<(), RuntimeError> {
        // WordPress Playground runs in WASM — standard Unix tools (curl, bash, unzip)
        // are not available via shell_exec(). Log a warning for commands that are likely
        // to fail silently in the WASM environment.
        let likely_unix = cmd.contains("curl ")
            || cmd.contains("unzip ")
            || cmd.contains("bash ")
            || cmd.contains("wget ");
        if likely_unix {
            tracing::warn!(
                "exec_shell in Playground: command may not work in WASM environment: {}",
                cmd
            );
        }

        let php_code = format!("<?php shell_exec({});", php_escape_string(cmd));
        let blueprint = serde_json::json!({
            "steps": [{
                "step": "runPHP",
                "code": php_code
            }]
        });
        self.run_blueprint_json(project_path, &blueprint)
    }

    fn exec_args(&self, project_path: &Path, args: &[&str]) -> Result<(), RuntimeError> {
        // For Playground, exec_args maps to exec_shell
        let cmd = args.join(" ");
        self.exec_shell(project_path, &cmd)
    }

    fn exec_sql(&self, project_path: &Path, sql: &str) -> Result<(), RuntimeError> {
        // Run SQL via wp db query
        let blueprint = serde_json::json!({
            "steps": [{
                "step": "runSql",
                "sql": {
                    "resource": "literal",
                    "name": "query.sql",
                    "contents": sql
                }
            }]
        });
        self.run_blueprint_json(project_path, &blueprint)
    }

    fn php_version(&self, project_path: &Path) -> Option<String> {
        // Read from .playground/state.json if configured, otherwise return default
        let state_path = project_path.join(PLAYGROUND_DIR).join(PLAYGROUND_STATE);
        if let Ok(content) = std::fs::read_to_string(&state_path) {
            if let Ok(state) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(php) = state.get("php_version").and_then(|v| v.as_str()) {
                    return Some(php.to_string());
                }
            }
        }
        // Default PHP version for Playground
        Some("8.5".to_string())
    }
}

impl PlaygroundRuntime {
    /// Run a blueprint JSON object via the Playground CLI's `run-blueprint` command.
    fn run_blueprint_json(
        &self,
        project_path: &Path,
        blueprint: &serde_json::Value,
    ) -> Result<(), RuntimeError> {
        let bp_json = serde_json::to_string(blueprint).map_err(|e| RuntimeError::CommandFailed {
            command: "serialize blueprint".into(),
            message: e.to_string(),
        })?;

        let mount_arg = format!("{}:/wordpress", project_path.display());

        let mut child = Command::new("npx")
            .args([
                "@wp-playground/cli@latest",
                "run-blueprint",
                "--mount", &mount_arg,
                "--blueprint", "-",
            ])
            .current_dir(project_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| RuntimeError::CommandFailed {
                command: "npx @wp-playground/cli run-blueprint".into(),
                message: e.to_string(),
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(bp_json.as_bytes()).map_err(|e| RuntimeError::CommandFailed {
                command: "npx @wp-playground/cli run-blueprint (stdin)".into(),
                message: e.to_string(),
            })?;
        }

        let status = child.wait().map_err(|e| RuntimeError::CommandFailed {
            command: "npx @wp-playground/cli run-blueprint".into(),
            message: e.to_string(),
        })?;

        if !status.success() {
            return Err(RuntimeError::CommandFailed {
                command: "npx @wp-playground/cli run-blueprint".into(),
                message: format!("exit code: {}", status.code().unwrap_or(-1)),
            });
        }
        Ok(())
    }

    /// Run a blueprint file directly via the Playground CLI.
    pub fn run_blueprint_file(
        &self,
        project_path: &Path,
        blueprint_path: &Path,
    ) -> Result<(), RuntimeError> {
        let mount_arg = format!("{}:/wordpress", project_path.display());
        let bp_path_str = blueprint_path.display().to_string();

        let status = Command::new("npx")
            .args([
                "@wp-playground/cli@latest",
                "run-blueprint",
                "--mount", &mount_arg,
                "--blueprint", &bp_path_str,
            ])
            .current_dir(project_path)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| RuntimeError::CommandFailed {
                command: "npx @wp-playground/cli run-blueprint".into(),
                message: e.to_string(),
            })?;

        if !status.success() {
            return Err(RuntimeError::CommandFailed {
                command: "npx @wp-playground/cli run-blueprint".into(),
                message: format!("exit code: {}", status.code().unwrap_or(-1)),
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check if a Node.js version string meets the minimum 20.18 requirement.
fn parse_node_version_ok(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    let major: u32 = parts[0].parse().unwrap_or(0);
    let minor: u32 = parts[1].parse().unwrap_or(0);
    major > 20 || (major == 20 && minor >= 18)
}

/// Escape a string for use inside PHP single-quoted strings.
fn php_escape_string(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// Open a URL in the default browser (best-effort).
fn open_url(url: &str) -> Result<(), std::io::Error> {
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(url).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd").args(["/c", "start", url]).spawn()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_version_check() {
        assert!(parse_node_version_ok("20.18.0"));
        assert!(parse_node_version_ok("20.19.0"));
        assert!(parse_node_version_ok("22.0.0"));
        assert!(!parse_node_version_ok("20.17.0"));
        assert!(!parse_node_version_ok("18.20.0"));
        assert!(!parse_node_version_ok("16.0.0"));
    }

    #[test]
    fn test_php_escape_string() {
        assert_eq!(php_escape_string("hello"), "'hello'");
        assert_eq!(php_escape_string("it's"), "'it\\'s'");
        assert_eq!(php_escape_string("path\\to"), "'path\\\\to'");
    }
}
