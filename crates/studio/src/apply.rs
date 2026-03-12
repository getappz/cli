//! Write parsed files and run npm install / commands.

use crate::parse::ParsedResponse;
use miette::{miette, Result};
use starbase_utils::fs;
use std::path::Path;
use tracing::instrument;

fn normalize_path(path: &str) -> &str {
    path.trim_start_matches('/')
}

#[instrument(skip_all)]
pub async fn apply(parsed: &ParsedResponse, output_dir: &Path) -> Result<()> {
    for file in &parsed.files {
        let rel = normalize_path(&file.path);
        if rel.is_empty() || rel.contains("..") {
            tracing::warn!("Skipping invalid file path: {}", file.path);
            continue;
        }
        let full = output_dir.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).map_err(|e| miette!("Failed to create dir {}: {}", parent.display(), e))?;
        }
        fs::write_file(&full, &file.content).map_err(|e| miette!("Failed to write {}: {}", full.display(), e))?;
    }

    let output_dir_buf = output_dir.to_path_buf();
    tokio::task::spawn_blocking(move || run_npm_install(&output_dir_buf))
        .await
        .map_err(|e| miette!("Task join error: {}", e))??;

    if !parsed.packages.is_empty() {
        let output_dir_buf = output_dir.to_path_buf();
        let packages = parsed.packages.clone();
        tokio::task::spawn_blocking(move || run_npm_install_packages(&output_dir_buf, &packages))
            .await
            .map_err(|e| miette!("Task join error: {}", e))??;
    }

    for cmd in &parsed.commands {
        let output_dir_buf = output_dir.to_path_buf();
        let cmd = cmd.clone();
        tokio::task::spawn_blocking(move || run_command(&output_dir_buf, &cmd))
            .await
            .map_err(|e| miette!("Task join error: {}", e))??;
    }

    Ok(())
}

fn run_npm_install(cwd: &Path) -> Result<()> {
    let status = std::process::Command::new("npm")
        .arg("install")
        .current_dir(cwd)
        .status()
        .map_err(|e| miette!("Failed to run npm install: {}", e))?;
    if !status.success() {
        return Err(miette!("npm install failed with status: {}", status));
    }
    Ok(())
}

fn run_npm_install_packages(cwd: &Path, packages: &[String]) -> Result<()> {
    let status = std::process::Command::new("npm")
        .arg("install")
        .args(packages)
        .current_dir(cwd)
        .status()
        .map_err(|e| miette!("Failed to run npm install packages: {}", e))?;
    if !status.success() {
        return Err(miette!("npm install packages failed with status: {}", status));
    }
    Ok(())
}

fn run_command(cwd: &Path, command: &str) -> Result<()> {
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .status()
        .map_err(|e| miette!("Failed to run command: {}", e))?;
    if !status.success() {
        return Err(miette!("Command failed with status: {}: {}", status, command));
    }
    Ok(())
}
