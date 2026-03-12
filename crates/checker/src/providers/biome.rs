//! Biome check provider — JS/TS/JSON/CSS linting and formatting.
//!
//! Biome is a Rust-based, extremely fast all-in-one toolchain that replaces
//! ESLint + Prettier for JavaScript/TypeScript/JSON/CSS projects.
//!
//! - **Check**: `biome check --reporter=json .`
//! - **Fix**: `biome check --fix --reporter=json .`
//! - **Format**: `biome format --reporter=json .`

use std::path::Path;

use async_trait::async_trait;

use crate::config::CheckContext;
use crate::error::{CheckResult, CheckerError};
use crate::output::{CheckIssue, FixKind, FixReport, FixSuggestion, Severity};
use crate::provider::CheckProvider;
use crate::providers::helpers;

/// Biome check provider for JS/TS/JSON/CSS.
pub struct BiomeProvider;

#[async_trait]
impl CheckProvider for BiomeProvider {
    fn name(&self) -> &str {
        "Biome"
    }

    fn slug(&self) -> &str {
        "biome"
    }

    fn description(&self) -> &str {
        "Fast JS/TS/JSON/CSS linter and formatter (Rust-based)"
    }

    fn detect(&self, project_dir: &Path, _frameworks: &[&str]) -> bool {
        // Detect if this is a JS/TS project.
        let has_package_json = project_dir.join("package.json").exists();
        let has_biome_config = project_dir.join("biome.json").exists()
            || project_dir.join("biome.jsonc").exists();

        // Run biome if there's a biome config, or if it's a JS/TS project
        // without an eslint config (prefer biome as default).
        if has_biome_config {
            return true;
        }

        if has_package_json {
            // Don't override existing ESLint setups unless biome is configured.
            let has_eslint = project_dir.join(".eslintrc").exists()
                || project_dir.join(".eslintrc.js").exists()
                || project_dir.join(".eslintrc.cjs").exists()
                || project_dir.join(".eslintrc.json").exists()
                || project_dir.join(".eslintrc.yml").exists()
                || project_dir.join("eslint.config.js").exists()
                || project_dir.join("eslint.config.mjs").exists()
                || project_dir.join("eslint.config.cjs").exists()
                || project_dir.join("eslint.config.ts").exists();

            return !has_eslint;
        }

        false
    }

    fn tool_name(&self) -> &str {
        "@biomejs/biome"
    }

    async fn ensure_tool(&self, sandbox: &dyn sandbox::SandboxProvider) -> CheckResult<()> {
        // Check if biome is available locally (npx) or globally.
        let check = sandbox.exec("npx biome --version").await;
        if let Ok(output) = check {
            if output.success() {
                return Ok(());
            }
        }

        // Try global install.
        let _ = ui::status::info("Installing Biome...");
        let output = sandbox.exec("npm install -g @biomejs/biome").await?;
        if !output.success() {
            return Err(CheckerError::ToolInstallFailed {
                tool: "biome".to_string(),
                reason: helpers::combined_output(&output),
            });
        }
        Ok(())
    }

    async fn check(&self, ctx: &CheckContext) -> CheckResult<Vec<CheckIssue>> {
        let files = ctx.file_args();
        let cmd = format!("npx biome check --reporter=json {}", files);

        let output = ctx.exec(&cmd).await?;
        let stdout = output.stdout();

        // Biome exits with non-zero if it finds issues — that's expected.
        if stdout.trim().is_empty() {
            return Ok(Vec::new());
        }

        parse_biome_output(&stdout)
    }

    async fn fix(&self, ctx: &CheckContext) -> CheckResult<FixReport> {
        let files = ctx.file_args();
        let cmd = format!("npx biome check --fix --reporter=json {}", files);

        let output = ctx.exec(&cmd).await?;
        let stdout = output.stdout();

        // Parse remaining issues after fix.
        let remaining = if stdout.trim().is_empty() {
            Vec::new()
        } else {
            parse_biome_output(&stdout).unwrap_or_default()
        };

        Ok(FixReport {
            fixed_count: 0, // Biome doesn't report fixed count in JSON; we infer from diff.
            remaining_count: remaining.len(),
            fixed_issues: Vec::new(),
            remaining_issues: remaining,
        })
    }

    fn supports_fix(&self) -> bool {
        true
    }

    fn supports_format(&self) -> bool {
        true
    }
}

/// Parse Biome's JSON reporter output into `CheckIssue` structs.
fn parse_biome_output(output: &str) -> CheckResult<Vec<CheckIssue>> {
    let mut issues = Vec::new();

    // Biome's JSON output is a JSON object with a "diagnostics" array.
    let value: serde_json::Value = match serde_json::from_str(output) {
        Ok(v) => v,
        Err(_) => {
            // Try to find JSON in the output (biome might print other text).
            if let Some(start) = output.find('{') {
                match serde_json::from_str(&output[start..]) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(CheckerError::ParseFailed {
                            provider: "biome".to_string(),
                            reason: format!("Invalid JSON output: {}", e),
                        });
                    }
                }
            } else {
                return Ok(Vec::new());
            }
        }
    };

    if let Some(diagnostics) = value.get("diagnostics").and_then(|d| d.as_array()) {
        for diag in diagnostics {
            let severity = diag
                .get("severity")
                .and_then(|s| s.as_str())
                .map(helpers::map_severity)
                .unwrap_or(Severity::Warning);

            let message = diag
                .get("description")
                .or_else(|| diag.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown issue")
                .to_string();

            let code = diag
                .get("category")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());

            let file = diag
                .get("location")
                .and_then(|l| l.get("path"))
                .and_then(|p| p.get("file"))
                .and_then(|f| f.as_str())
                .unwrap_or("<unknown>")
                .to_string();

            let (line, column) = extract_biome_location(diag);

            let fix = if diag.get("fixable").and_then(|f| f.as_bool()).unwrap_or(false) {
                Some(FixSuggestion {
                    kind: FixKind::Safe,
                    description: "Biome safe fix available".to_string(),
                    replacement: None,
                })
            } else {
                None
            };

            let mut issue = CheckIssue::new(file, severity, message, "biome");
            if let Some(c) = code {
                issue = issue.with_code(c);
            }
            if let (Some(l), Some(c)) = (line, column) {
                issue = issue.at(l, c);
            }
            if let Some(f) = fix {
                issue = issue.with_fix(f);
            }
            issues.push(issue);
        }
    }

    Ok(issues)
}

/// Extract line/column from Biome's diagnostic location.
fn extract_biome_location(diag: &serde_json::Value) -> (Option<u32>, Option<u32>) {
    let span = diag
        .get("location")
        .and_then(|l| l.get("span"));

    if let Some(span) = span {
        let line = span.get(0).and_then(|v| v.as_u64()).map(|v| v as u32);
        let col = span.get(1).and_then(|v| v.as_u64()).map(|v| v as u32);
        (line, col)
    } else {
        (None, None)
    }
}
