//! Ruff check provider — Python linting and formatting.
//!
//! Ruff is a Rust-based, extremely fast Python linter that replaces
//! flake8, isort, pyupgrade, bandit, and many more tools.
//!
//! - **Check**: `ruff check --output-format=json .`
//! - **Fix**: `ruff check --fix --output-format=json .`
//! - **Format**: `ruff format --check .`

use std::path::Path;

use async_trait::async_trait;

use crate::config::CheckContext;
use crate::error::{CheckResult, CheckerError};
use crate::output::{CheckIssue, FixKind, FixReport, FixSuggestion, Severity};
use crate::provider::CheckProvider;
use crate::providers::helpers;

/// Ruff check provider for Python.
pub struct RuffProvider;

#[async_trait]
impl CheckProvider for RuffProvider {
    fn name(&self) -> &str {
        "Ruff"
    }

    fn slug(&self) -> &str {
        "ruff"
    }

    fn description(&self) -> &str {
        "Extremely fast Python linter and formatter (Rust-based)"
    }

    fn detect(&self, project_dir: &Path, _frameworks: &[&str]) -> bool {
        project_dir.join("pyproject.toml").exists()
            || project_dir.join("requirements.txt").exists()
            || project_dir.join("setup.py").exists()
            || project_dir.join("setup.cfg").exists()
            || project_dir.join("Pipfile").exists()
            || project_dir.join("ruff.toml").exists()
            || project_dir.join(".ruff.toml").exists()
    }

    fn tool_name(&self) -> &str {
        "ruff"
    }

    async fn ensure_tool(&self, sandbox: &dyn sandbox::SandboxProvider) -> CheckResult<()> {
        let check = sandbox.exec("ruff --version").await;
        if let Ok(output) = check {
            if output.success() {
                return Ok(());
            }
        }

        // Install via mise (preferred for Python tools) or pip.
        let _ = ui::status::info("Installing Ruff...");

        // Try mise first.
        let mise_output = sandbox.exec("mise use -g ruff@latest").await;
        if let Ok(output) = mise_output {
            if output.success() {
                return Ok(());
            }
        }

        // Fallback to pip/pipx.
        let output = sandbox.exec("pip install ruff").await?;
        if !output.success() {
            return Err(CheckerError::ToolInstallFailed {
                tool: "ruff".to_string(),
                reason: helpers::combined_output(&output),
            });
        }
        Ok(())
    }

    async fn check(&self, ctx: &CheckContext) -> CheckResult<Vec<CheckIssue>> {
        let files = ctx.file_args();
        let cmd = format!("ruff check --output-format=json {}", files);

        let output = ctx.exec(&cmd).await?;
        let stdout = output.stdout();

        // Ruff exits non-zero when issues are found.
        if stdout.trim().is_empty() {
            return Ok(Vec::new());
        }

        parse_ruff_output(&stdout)
    }

    async fn fix(&self, ctx: &CheckContext) -> CheckResult<FixReport> {
        let files = ctx.file_args();
        let cmd = format!("ruff check --fix --output-format=json {}", files);

        let output = ctx.exec(&cmd).await?;
        let stdout = output.stdout();

        let remaining = if stdout.trim().is_empty() {
            Vec::new()
        } else {
            parse_ruff_output(&stdout).unwrap_or_default()
        };

        Ok(FixReport {
            fixed_count: 0,
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

/// Parse Ruff's JSON output into `CheckIssue` structs.
///
/// Ruff outputs a JSON array of diagnostics:
/// ```json
/// [
///   {
///     "code": "E501",
///     "message": "Line too long (120 > 88 characters)",
///     "filename": "src/main.py",
///     "location": { "row": 10, "column": 1 },
///     "end_location": { "row": 10, "column": 120 },
///     "fix": { "applicability": "safe", "message": "..." },
///     "url": "..."
///   }
/// ]
/// ```
fn parse_ruff_output(output: &str) -> CheckResult<Vec<CheckIssue>> {
    let mut issues = Vec::new();

    let diagnostics: Vec<serde_json::Value> =
        serde_json::from_str(output).map_err(|e| CheckerError::ParseFailed {
            provider: "ruff".to_string(),
            reason: format!("Invalid JSON: {}", e),
        })?;

    for diag in &diagnostics {
        let code = diag.get("code").and_then(|c| c.as_str()).map(|s| s.to_string());

        let message = diag
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown issue")
            .to_string();

        let file = diag
            .get("filename")
            .and_then(|f| f.as_str())
            .unwrap_or("<unknown>")
            .to_string();

        let line = diag
            .get("location")
            .and_then(|l| l.get("row"))
            .and_then(|r| r.as_u64())
            .map(|v| v as u32);
        let column = diag
            .get("location")
            .and_then(|l| l.get("column"))
            .and_then(|c| c.as_u64())
            .map(|v| v as u32);

        let end_line = diag
            .get("end_location")
            .and_then(|l| l.get("row"))
            .and_then(|r| r.as_u64())
            .map(|v| v as u32);
        let end_column = diag
            .get("end_location")
            .and_then(|l| l.get("column"))
            .and_then(|c| c.as_u64())
            .map(|v| v as u32);

        // Ruff uses code prefixes to categorize: E=error, W=warning, etc.
        let severity = code
            .as_deref()
            .map(|c| {
                if c.starts_with('E') || c.starts_with('F') {
                    Severity::Error
                } else if c.starts_with('W') || c.starts_with('C') {
                    Severity::Warning
                } else {
                    Severity::Warning
                }
            })
            .unwrap_or(Severity::Warning);

        let fix = diag.get("fix").map(|f| {
            let applicability = f
                .get("applicability")
                .and_then(|a| a.as_str())
                .unwrap_or("unsafe");
            let kind = if applicability == "safe" {
                FixKind::Safe
            } else {
                FixKind::Unsafe
            };
            let desc = f
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Fix available")
                .to_string();
            FixSuggestion {
                kind,
                description: desc,
                replacement: None,
            }
        });

        let mut issue = CheckIssue::new(file, severity, message, "ruff");
        if let Some(c) = code {
            issue = issue.with_code(c);
        }
        if let (Some(l), Some(c)) = (line, column) {
            issue = issue.at(l, c);
        }
        if let (Some(el), Some(ec)) = (end_line, end_column) {
            issue = issue.to(el, ec);
        }
        if let Some(f) = fix {
            issue = issue.with_fix(f);
        }
        issues.push(issue);
    }

    Ok(issues)
}
