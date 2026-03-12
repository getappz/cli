//! Pack project with Repomix via sandbox exec.

use std::path::Path;

use crate::error::CodeSearchError;
use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};
use tracing::instrument;

const REPOMIX_OUTPUT: &str = "repomix-output.md";

#[instrument(skip_all)]
pub async fn pack(workdir: &Path) -> Result<std::path::PathBuf, CodeSearchError> {
    let config = SandboxConfig::new(workdir)
        .with_settings(SandboxSettings::default().with_tool("node", Some("22")));
    let sandbox = create_sandbox(config)
        .await
        .map_err(|e| format!("Failed to create sandbox: {}", e))?;

    let output_path = workdir.join(REPOMIX_OUTPUT);
    // Restrict to source, config, and docs - avoid build output and noise
    let include_patterns =
        "**/*.ts,**/*.tsx,**/*.astro,**/*.js,**/*.jsx,**/*.md,**/*.mdx,**/astro.config.*,**/tailwind.config.*,**/vite.config.*,**/*.config.*";
    // Exclude AI agent folders and blog content (skills, rules, plans, data posts)
    let ignore_patterns = ".claude/**,.cursor/**,.codex/**,.aider/**,.continue/**,.github/copilot/**,src/data/**";
    let cmd = format!(
        "npx repomix@latest --style markdown --output {} --include \"{}\" --ignore \"{}\" .",
        output_path.display(),
        include_patterns,
        ignore_patterns
    );

    let out = sandbox.exec(&cmd).await.map_err(|e| {
        CodeSearchError(format!("Repomix failed: {}", e))
    })?;

    if !out.success() {
        return Err(CodeSearchError(format!(
            "Repomix failed: {}",
            out.stderr().trim()
        )));
    }

    if !output_path.exists() {
        return Err(CodeSearchError("Repomix did not produce output file".into()));
    }

    Ok(output_path)
}
