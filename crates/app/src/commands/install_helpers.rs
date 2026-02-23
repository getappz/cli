//! Shared helpers for running the install step (used by both build and dev commands).

use crate::shell::RunOptions;
use detectors::PackageManagerInfo;
use miette::Result;
use std::path::PathBuf;
use task::{Context, Runner, TaskRegistry};
use tokio_util::sync::CancellationToken;

/// Get default install command based on package manager
pub fn get_default_install_command(package_manager: &Option<PackageManagerInfo>) -> String {
    if let Some(ref pm) = package_manager {
        match pm.manager.as_str() {
            "yarn" => "yarn install".to_string(),
            "pnpm" => "pnpm install".to_string(),
            "bun" => "bun install".to_string(),
            _ => "npm install".to_string(), // Default to npm
        }
    } else {
        "npm install".to_string() // Default fallback
    }
}

/// Execute a recipe task with cancellation support
pub async fn run_recipe_task(
    registry: &TaskRegistry,
    task_name: &str,
    working_path: PathBuf,
    verbose: bool,
) -> Result<()> {
    let mut ctx = Context::new();
    ctx.set_working_path(working_path);
    let mut runner = if verbose {
        Runner::new_verbose(registry)
    } else {
        Runner::new(registry)
    };

    // Create cancellation token for graceful shutdown on Ctrl+C
    let cancellation_token = CancellationToken::new();

    // Spawn task to listen for Ctrl+C and cancel execution
    let cancellation_token_clone = cancellation_token.clone();
    let verbose_clone = verbose;
    tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            eprintln!("Failed to listen for Ctrl+C: {}", e);
            return;
        }
        if verbose_clone {
            eprintln!("\nReceived Ctrl+C, cancelling tasks...");
        }
        cancellation_token_clone.cancel();
    });

    let res = runner
        .invoke_async(
            task_name,
            &mut ctx,
            Some(cancellation_token.clone()),
            false,
            false,
        )
        .await;

    // If cancelled via Ctrl+C, exit cleanly with code 130
    if cancellation_token.is_cancelled() {
        eprintln!("Cancelled.");
        std::process::exit(130);
    }

    res.map_err(|e| miette::miette!("{} failed: {}", task_name, e))
}

/// Handle shell script fallback on Windows for framework commands
pub async fn handle_shell_script_fallback(
    result: Result<()>,
    is_shell_script: bool,
    has_user_script: bool,
    framework_cmd: Option<&String>,
    default_cmd: Option<String>,
    ctx: &Context,
    _opts: &RunOptions,
) -> Result<()> {
    if result.is_ok() {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        if has_user_script && is_shell_script {
            eprintln!("⚠️  Warning: Shell script detected. Falling back to framework command.");
            if let Some(framework_cmd) = framework_cmd {
                println!("✓ Using framework command");
                run_local_with(ctx, framework_cmd, _opts.clone()).await?;
            } else if let Some(ref default_cmd) = default_cmd {
                println!("✓ Using default command");
                run_local_with(ctx, default_cmd, _opts.clone()).await?;
            } else {
                result?; // No fallback available, return error
            }
            Ok(())
        } else {
            result // Not a shell script issue, return error
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        result // On Unix, just return the error
    }
}
