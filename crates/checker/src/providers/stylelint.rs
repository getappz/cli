//! Stylelint check provider — CSS/SCSS linting.
//!
//! Stylelint is the industry-standard CSS linter.
//!
//! - **Check**: `stylelint --formatter json "**/*.{css,scss,less}"`
//! - **Fix**: `stylelint --fix "**/*.{css,scss,less}"`

use std::path::Path;

use async_trait::async_trait;

use crate::config::CheckContext;
use crate::error::{CheckResult, CheckerError};
use crate::output::{CheckIssue, FixReport, Severity};
use crate::provider::CheckProvider;
use crate::providers::helpers;

/// Stylelint check provider for CSS/SCSS.
pub struct StylelintProvider;

#[async_trait]
impl CheckProvider for StylelintProvider {
    fn name(&self) -> &str {
        "Stylelint"
    }

    fn slug(&self) -> &str {
        "stylelint"
    }

    fn description(&self) -> &str {
        "Industry-standard CSS/SCSS linter"
    }

    fn detect(&self, project_dir: &Path, _frameworks: &[&str]) -> bool {
        // Only run if there's an explicit Stylelint config.
        project_dir.join(".stylelintrc").exists()
            || project_dir.join(".stylelintrc.json").exists()
            || project_dir.join(".stylelintrc.yml").exists()
            || project_dir.join(".stylelintrc.yaml").exists()
            || project_dir.join("stylelint.config.js").exists()
            || project_dir.join("stylelint.config.mjs").exists()
            || project_dir.join("stylelint.config.cjs").exists()
    }

    fn tool_name(&self) -> &str {
        "stylelint"
    }

    async fn ensure_tool(&self, sandbox: &dyn sandbox::SandboxProvider) -> CheckResult<()> {
        let check = sandbox.exec("npx stylelint --version").await;
        if let Ok(output) = check {
            if output.success() {
                return Ok(());
            }
        }

        let _ = ui::status::info("Installing Stylelint...");
        let output = sandbox.exec("npm install -g stylelint").await?;
        if !output.success() {
            return Err(CheckerError::ToolInstallFailed {
                tool: "stylelint".to_string(),
                reason: helpers::combined_output(&output),
            });
        }
        Ok(())
    }

    async fn check(&self, ctx: &CheckContext) -> CheckResult<Vec<CheckIssue>> {
        let cmd = "npx stylelint --formatter json \"**/*.{css,scss,less}\"";

        let output = ctx.exec(cmd).await?;
        let stdout = output.stdout();

        if stdout.trim().is_empty() {
            return Ok(Vec::new());
        }

        parse_stylelint_output(&stdout)
    }

    async fn fix(&self, ctx: &CheckContext) -> CheckResult<FixReport> {
        let cmd = "npx stylelint --fix \"**/*.{css,scss,less}\"";

        let output = ctx.exec(cmd).await?;
        let _ = output;

        // Run check again to see remaining issues.
        let remaining = self.check(ctx).await.unwrap_or_default();

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
}

/// Parse Stylelint's JSON output into `CheckIssue` structs.
///
/// Stylelint JSON format (array of file results):
/// ```json
/// [
///   {
///     "source": "/path/to/file.css",
///     "warnings": [
///       {
///         "line": 10,
///         "column": 5,
///         "rule": "color-no-invalid-hex",
///         "severity": "error",
///         "text": "Unexpected invalid hex color \"#zzz\""
///       }
///     ]
///   }
/// ]
/// ```
fn parse_stylelint_output(output: &str) -> CheckResult<Vec<CheckIssue>> {
    let mut issues = Vec::new();

    let results: Vec<serde_json::Value> =
        serde_json::from_str(output).map_err(|e| CheckerError::ParseFailed {
            provider: "stylelint".to_string(),
            reason: format!("Invalid JSON: {}", e),
        })?;

    for result in &results {
        let file = result
            .get("source")
            .and_then(|s| s.as_str())
            .unwrap_or("<unknown>");

        if let Some(warnings) = result.get("warnings").and_then(|w| w.as_array()) {
            for warning in warnings {
                let severity = warning
                    .get("severity")
                    .and_then(|s| s.as_str())
                    .map(helpers::map_severity)
                    .unwrap_or(Severity::Warning);

                let message = warning
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or("Unknown issue")
                    .to_string();

                let code = warning
                    .get("rule")
                    .and_then(|r| r.as_str())
                    .map(|s| s.to_string());

                let line = warning.get("line").and_then(|l| l.as_u64()).map(|v| v as u32);
                let column = warning
                    .get("column")
                    .and_then(|c| c.as_u64())
                    .map(|v| v as u32);

                let mut issue = CheckIssue::new(file, severity, message, "stylelint");
                if let Some(c) = code {
                    issue = issue.with_code(c);
                }
                if let (Some(l), Some(c)) = (line, column) {
                    issue = issue.at(l, c);
                }
                issues.push(issue);
            }
        }
    }

    Ok(issues)
}
