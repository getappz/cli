//! Shared verification logic — build and test execution.
//!
//! Used by `appz check --verify` and `appz git worktree create --verify`.
//! Uses `appz_build` for framework-detected projects (JS/TS); manual fallback for Cargo/Go.

use appz_build::{detect_framework, run_build as appz_run_build};
use miette::{miette, Result};
use sandbox::{create_sandbox, SandboxConfig};
use std::path::Path;

use crate::sandbox_helpers::mise_tools_for_execution;

/// Run project-appropriate build command via appz_build (auto-detects framework)
/// or manual fallback for Cargo/Go when no framework is detected.
pub async fn run_build(workdir: &Path) -> Result<()> {
    // Try appz_build framework detection first (covers JS/TS: Next.js, Vite, etc.)
    if let Ok(Some(detected)) = detect_framework(workdir).await {
        ui::status::info(&format!("Running build ({}): {}", detected.name, detected.build_command));

        let config = SandboxConfig::new(workdir.to_path_buf())
            .with_settings(mise_tools_for_execution(&detected.package_manager, None));

        if let Ok(sandbox) = create_sandbox(config).await {
            appz_run_build(sandbox.as_ref(), &detected)
                .await
                .map_err(|e| miette!("Build failed: {}", e))?;
        } else {
            // Sandbox creation failed — run build command directly
            let status = tokio::process::Command::new("sh")
                .args(["-c", &detected.build_command])
                .current_dir(workdir)
                .status()
                .await
                .map_err(|e| miette!("Build failed: {}", e))?;
            if !status.success() {
                return Err(miette!("Build failed"));
            }
        }
        return Ok(());
    }

    // Fallback for non-framework projects: Cargo, Go (appz_build doesn't detect these)
    if workdir.join("Cargo.toml").exists() {
        ui::status::info("Running cargo build...");
        let status = tokio::process::Command::new("cargo")
            .arg("build")
            .current_dir(workdir)
            .status()
            .await
            .map_err(|e| miette!("cargo build failed: {}", e))?;
        if !status.success() {
            return Err(miette!("Build failed"));
        }
    }
    if workdir.join("go.mod").exists() {
        ui::status::info("Running go build...");
        let status = tokio::process::Command::new("go")
            .arg("build")
            .current_dir(workdir)
            .status()
            .await
            .map_err(|e| miette!("go build failed: {}", e))?;
        if !status.success() {
            return Err(miette!("Build failed"));
        }
    }
    Ok(())
}

/// Run project-appropriate test command (npm test, cargo test, pytest, go test).
pub async fn run_tests(workdir: &Path) -> Result<()> {
    if workdir.join("package.json").exists() {
        ui::status::info("Running npm test...");
        let status = tokio::process::Command::new("npm")
            .arg("test")
            .current_dir(workdir)
            .status()
            .await
            .map_err(|e| miette::miette!("npm test failed: {}", e))?;
        if !status.success() {
            return Err(miette::miette!("Tests failed"));
        }
    }
    if workdir.join("Cargo.toml").exists() {
        ui::status::info("Running cargo test...");
        let status = tokio::process::Command::new("cargo")
            .arg("test")
            .current_dir(workdir)
            .status()
            .await
            .map_err(|e| miette::miette!("cargo test failed: {}", e))?;
        if !status.success() {
            return Err(miette::miette!("Tests failed"));
        }
    }
    if workdir.join("pyproject.toml").exists() || workdir.join("setup.py").exists() {
        ui::status::info("Running pytest...");
        let status = tokio::process::Command::new("pytest")
            .current_dir(workdir)
            .status()
            .await
            .map_err(|e| miette::miette!("pytest failed: {}", e))?;
        if !status.success() {
            return Err(miette::miette!("Tests failed"));
        }
    }
    if workdir.join("go.mod").exists() {
        ui::status::info("Running go test...");
        let status = tokio::process::Command::new("go")
            .args(["test", "./..."])
            .current_dir(workdir)
            .status()
            .await
            .map_err(|e| miette::miette!("go test failed: {}", e))?;
        if !status.success() {
            return Err(miette::miette!("Tests failed"));
        }
    }
    Ok(())
}
