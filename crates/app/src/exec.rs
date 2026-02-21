//! Command execution for appz exec (CLI + MCP).
//! Sandbox-by-default; duct for cross-platform when --no-sandbox.

use miette::Result;
use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ExecConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: std::path::PathBuf,
    pub sandbox: bool,
    pub stream: bool,
    pub json: bool,
    pub shell: bool,
    pub timeout: Option<Duration>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

pub async fn run_exec(config: ExecConfig) -> Result<ExecResult> {
    if config.sandbox {
        run_exec_sandbox(&config).await
    } else {
        run_exec_direct(&config).await
    }
}

async fn run_exec_sandbox(config: &ExecConfig) -> Result<ExecResult> {
    let sandbox_config = SandboxConfig::new(config.cwd.clone())
        .with_settings(SandboxSettings::default().quiet());
    let sandbox = create_sandbox(sandbox_config)
        .await
        .map_err(|e| miette::miette!("Sandbox creation failed: {}. Try --no-sandbox.", e))?;

    let cmd_str = build_command_string(&config.command, &config.args);
    let timeout = config.timeout;

    let result = if let Some(dur) = timeout {
        tokio::time::timeout(dur, sandbox.exec(&cmd_str)).await
    } else {
        Ok(sandbox.exec(&cmd_str).await)
    };

    match result {
        Ok(Ok(out)) => Ok(ExecResult {
            exit_code: out.exit_code().unwrap_or(1),
            stdout: out.stdout(),
            stderr: out.stderr(),
            timed_out: false,
        }),
        Ok(Err(e)) => Err(miette::miette!("Command failed: {}", e)),
        Err(_) => Ok(ExecResult {
            exit_code: 124,
            stdout: String::new(),
            stderr: format!(
                "Command timed out after {:?}",
                timeout.unwrap_or_default()
            ),
            timed_out: true,
        }),
    }
}

async fn run_exec_direct(config: &ExecConfig) -> Result<ExecResult> {
    let cmd_str = build_command_string(&config.command, &config.args);
    let parts = shell_words::split(&cmd_str)
        .unwrap_or_else(|_| vec![config.command.clone()]);
    let (program, program_args) = parts
        .split_first()
        .ok_or_else(|| miette::miette!("Empty command"))?;

    let program = program.to_string();
    let args: Vec<String> = program_args.to_vec();
    let cwd = config.cwd.clone();
    let timeout = config.timeout;

    let run = move || {
        duct::cmd(&program, &args)
            .dir(&cwd)
            .stdout_capture()
            .stderr_capture()
            .unchecked()
            .run()
    };

    let result = if let Some(dur) = timeout {
        tokio::time::timeout(
            dur,
            tokio::task::spawn_blocking(run),
        )
        .await
    } else {
        Ok(tokio::task::spawn_blocking(run).await)
    };

    match result {
        Ok(Ok(Ok(output))) => Ok(ExecResult {
            exit_code: output.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            timed_out: false,
        }),
        Ok(Ok(Err(e))) => Err(miette::miette!("Command failed: {}", e)),
        Ok(Err(e)) => Err(miette::miette!("Command failed: {}", e)),
        Err(_) => Ok(ExecResult {
            exit_code: 124,
            stdout: String::new(),
            stderr: "Command timed out".to_string(),
            timed_out: true,
        }),
    }
}

fn build_command_string(cmd: &str, args: &[String]) -> String {
    if args.is_empty() {
        cmd.to_string()
    } else {
        format!("{} {}", cmd, args.join(" "))
    }
}
