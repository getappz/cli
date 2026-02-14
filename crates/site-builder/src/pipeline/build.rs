//! Phase 5: Build the generated Astro project.

use sandbox::SandboxProvider;

use crate::config::SiteBuilderConfig;
use crate::error::{SiteBuilderError, SiteBuilderResult};

/// Run the build phase: npm install + astro build.
pub async fn run(
    _config: &SiteBuilderConfig,
    sandbox: &dyn SandboxProvider,
) -> SiteBuilderResult<()> {
    let _ = ui::status::info("Phase 5: Building project...");

    // npm install
    let _ = ui::status::info("  Running npm install...");
    let install_out = sandbox
        .exec("npm install")
        .await
        .map_err(|e| SiteBuilderError::BuildFailed {
            reason: format!("Failed to run npm install: {}", e),
        })?;

    if !install_out.success() {
        let stderr = install_out.stderr();
        let err = stderr.trim();
        return Err(SiteBuilderError::BuildFailed {
            reason: format!(
                "npm install failed: {}",
                if err.is_empty() { "see output above" } else { err }
            ),
        });
    }

    // astro build
    let _ = ui::status::info("  Running astro build...");
    let build_out = sandbox
        .exec("npx astro build")
        .await
        .map_err(|e| SiteBuilderError::BuildFailed {
            reason: format!("Failed to run astro build: {}", e),
        })?;

    if !build_out.success() {
        let stderr = build_out.stderr();
        let err = stderr.trim();
        return Err(SiteBuilderError::BuildFailed {
            reason: format!(
                "astro build failed: {}",
                if err.is_empty() { "see output above" } else { err }
            ),
        });
    }

    let _ = ui::status::success("Build complete.");
    Ok(())
}
