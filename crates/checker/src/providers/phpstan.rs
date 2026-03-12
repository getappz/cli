//! PHPStan check provider — PHP static analysis.
//!
//! PHPStan is the industry-standard static analysis tool for PHP projects.
//!
//! - **Check**: `phpstan analyse --error-format=json --no-progress`
//! - **Fix**: Not supported (PHPStan is analysis-only).

use std::path::Path;

use async_trait::async_trait;

use crate::config::CheckContext;
use crate::error::{CheckResult, CheckerError};
use crate::output::{CheckIssue, FixReport, Severity};
use crate::provider::CheckProvider;
use crate::providers::helpers;

/// PHPStan check provider for PHP.
pub struct PHPStanProvider;

#[async_trait]
impl CheckProvider for PHPStanProvider {
    fn name(&self) -> &str {
        "PHPStan"
    }

    fn slug(&self) -> &str {
        "phpstan"
    }

    fn description(&self) -> &str {
        "PHP static analysis tool"
    }

    fn detect(&self, project_dir: &Path, _frameworks: &[&str]) -> bool {
        project_dir.join("composer.json").exists()
    }

    fn tool_name(&self) -> &str {
        "phpstan"
    }

    async fn ensure_tool(&self, sandbox: &dyn sandbox::SandboxProvider) -> CheckResult<()> {
        // Check if phpstan is available via vendor/bin or globally.
        let check = sandbox.exec("./vendor/bin/phpstan --version").await;
        if let Ok(output) = check {
            if output.success() {
                return Ok(());
            }
        }

        let check_global = sandbox.exec("phpstan --version").await;
        if let Ok(output) = check_global {
            if output.success() {
                return Ok(());
            }
        }

        // Install via composer.
        let _ = ui::status::info("Installing PHPStan...");
        let output = sandbox
            .exec("composer require --dev phpstan/phpstan")
            .await?;
        if !output.success() {
            return Err(CheckerError::ToolInstallFailed {
                tool: "phpstan".to_string(),
                reason: helpers::combined_output(&output),
            });
        }
        Ok(())
    }

    async fn check(&self, ctx: &CheckContext) -> CheckResult<Vec<CheckIssue>> {
        // Try vendor/bin first, then global.
        let has_vendor = ctx.fs().exists("vendor/bin/phpstan");
        let phpstan_cmd = if has_vendor {
            "./vendor/bin/phpstan"
        } else {
            "phpstan"
        };

        let cmd = format!(
            "{} analyse --error-format=json --no-progress",
            phpstan_cmd
        );

        let output = ctx.exec(&cmd).await?;
        let stdout = output.stdout();

        if stdout.trim().is_empty() {
            return Ok(Vec::new());
        }

        parse_phpstan_output(&stdout)
    }

    async fn fix(&self, _ctx: &CheckContext) -> CheckResult<FixReport> {
        // PHPStan doesn't support auto-fix.
        Ok(FixReport::default())
    }

    fn supports_fix(&self) -> bool {
        false
    }
}

/// Parse PHPStan's JSON output into `CheckIssue` structs.
///
/// PHPStan JSON format:
/// ```json
/// {
///   "totals": { "errors": 0, "file_errors": 5 },
///   "files": {
///     "/path/to/file.php": {
///       "errors": 2,
///       "messages": [
///         { "message": "...", "line": 10, "ignorable": true }
///       ]
///     }
///   },
///   "errors": []
/// }
/// ```
fn parse_phpstan_output(output: &str) -> CheckResult<Vec<CheckIssue>> {
    let mut issues = Vec::new();

    let value: serde_json::Value =
        serde_json::from_str(output).map_err(|e| CheckerError::ParseFailed {
            provider: "phpstan".to_string(),
            reason: format!("Invalid JSON: {}", e),
        })?;

    if let Some(files) = value.get("files").and_then(|f| f.as_object()) {
        for (file_path, file_data) in files {
            if let Some(messages) = file_data.get("messages").and_then(|m| m.as_array()) {
                for msg in messages {
                    let message = msg
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown issue")
                        .to_string();

                    let line = msg.get("line").and_then(|l| l.as_u64()).map(|v| v as u32);

                    let mut issue =
                        CheckIssue::new(file_path.as_str(), Severity::Error, message, "phpstan");
                    if let Some(l) = line {
                        issue = issue.at(l, 1);
                    }
                    issues.push(issue);
                }
            }
        }
    }

    // Also include general errors.
    if let Some(errors) = value.get("errors").and_then(|e| e.as_array()) {
        for err in errors {
            if let Some(msg) = err.as_str() {
                issues.push(CheckIssue::new("<project>", Severity::Error, msg, "phpstan"));
            }
        }
    }

    Ok(issues)
}
