use miette::{miette, Result};
use ssh::{self};
use std::collections::HashMap;
use std::io::Write;
use std::time::Duration;

use crate::host::HostConfig;

/// Options for remote command execution
#[derive(Debug, Clone)]
pub struct RemoteRunOptions {
    pub cwd: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub show_output: bool,
    pub timeout: Option<Duration>,
}

impl Default for RemoteRunOptions {
    fn default() -> Self {
        Self {
            cwd: None,
            env: None,
            show_output: true,
            timeout: Some(Duration::from_secs(300)), // 5 minute default timeout
        }
    }
}

/// SSH client for executing commands on remote hosts
pub struct SshClient {
    config: HostConfig,
}

impl SshClient {
    /// Create a new SSH client for the given host configuration
    pub fn new(config: HostConfig) -> Self {
        Self { config }
    }

    /// Execute a command on the remote host
    pub fn run_remote(&self, command: &str, options: RemoteRunOptions) -> Result<String> {
        // Build the command with options
        let full_command = self.build_command(command, &options);

        // Connect to the remote host
        let addr = format!("{}:{}", self.config.hostname, self.config.port());

        // Create session builder
        let mut session_builder = ssh::create_session();

        // Set username
        let username = if let Some(user) = &self.config.user {
            user.clone()
        } else {
            // Use current user as default
            std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "root".to_string())
        };
        session_builder = session_builder.username(&username);

        // Authenticate with private key if provided
        if let Some(key_path) = self.config.identity_file_path() {
            if key_path.exists() {
                session_builder =
                    session_builder.private_key_path(key_path.to_string_lossy().as_ref());
            }
        }

        // Set timeout if provided
        if let Some(timeout) = options.timeout {
            session_builder = session_builder.timeout(Some(timeout));
        }

        // Connect to the remote host and get a local session
        let mut session = session_builder
            .connect(&addr)
            .map_err(|e| miette!("Failed to connect to {}: {}", addr, e))?
            .run_local();

        // Execute the command
        let exec = session
            .open_exec()
            .map_err(|e| miette!("Failed to open exec channel: {}", e))?;

        let output = exec.send_command(&full_command).map_err(|e| {
            miette!(
                "Failed to execute command '{}' on {}: {}",
                full_command,
                self.config.alias(),
                e
            )
        })?;

        // Convert output to string
        let output_str = String::from_utf8_lossy(&output).to_string();

        // Show output if requested
        if options.show_output {
            print!("{}", output_str);
            std::io::stdout()
                .flush()
                .map_err(|e| miette!("Failed to flush stdout: {}", e))?;
        }

        // Close the session
        session.close();

        // Note: ssh-rs's send_command may include stderr in the output
        // If the command fails, ssh-rs might return an error or the output might indicate failure
        // For now, we consider execution successful if no error is thrown
        // Users should check command output for errors if needed

        Ok(output_str)
    }

    /// Build the full command with working directory and environment variables
    fn build_command(&self, command: &str, options: &RemoteRunOptions) -> String {
        let mut parts = Vec::new();

        // Change directory if specified
        if let Some(cwd) = &options.cwd {
            parts.push(format!("cd {}", shell_escape::unix::escape(cwd)));
        }

        // Set environment variables if specified
        if let Some(env) = &options.env {
            for (key, value) in env {
                parts.push(format!(
                    "export {}={}",
                    shell_escape::unix::escape(key),
                    shell_escape::unix::escape(value)
                ));
            }
        }

        // Add the actual command
        parts.push(command.to_string());

        parts.join(" && ")
    }
}

// Helper for shell escaping
mod shell_escape {
    pub mod unix {
        pub fn escape(s: &str) -> String {
            if s.is_empty() {
                return "''".to_string();
            }
            if s.chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '/' | '.' | ':'))
            {
                return s.to_string();
            }
            format!("'{}'", s.replace('\'', "'\"'\"'"))
        }
    }
}
