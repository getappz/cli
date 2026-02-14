//! Secret scan provider — credential detection via gitleaks.
//!
//! Gitleaks is a fast, Go-based tool that scans for hardcoded secrets
//! and credentials in source code.
//!
//! - **Check**: `gitleaks detect --report-format json --no-banner -v`
//! - **Fix**: Not supported (secrets must be manually removed and rotated).

use std::path::Path;

use async_trait::async_trait;

use crate::config::CheckContext;
use crate::error::{CheckResult, CheckerError};
use crate::output::{CheckIssue, FixReport, Severity};
use crate::provider::CheckProvider;
/// Secret scan provider using gitleaks.
pub struct SecretScanProvider;

#[async_trait]
impl CheckProvider for SecretScanProvider {
    fn name(&self) -> &str {
        "Secret Scanner"
    }

    fn slug(&self) -> &str {
        "secrets"
    }

    fn description(&self) -> &str {
        "Detect hardcoded secrets and credentials (gitleaks)"
    }

    fn detect(&self, project_dir: &Path, _frameworks: &[&str]) -> bool {
        // Run on any project that is a git repository.
        project_dir.join(".git").exists()
    }

    fn tool_name(&self) -> &str {
        "gitleaks"
    }

    async fn ensure_tool(&self, sandbox: &dyn sandbox::SandboxProvider) -> CheckResult<()> {
        let check = sandbox.exec("gitleaks version").await;
        if let Ok(output) = check {
            if output.success() {
                return Ok(());
            }
        }

        // Install via mise (preferred) or brew.
        let _ = ui::status::info("Installing gitleaks...");

        let mise_output = sandbox.exec("mise use -g gitleaks@latest").await;
        if let Ok(output) = mise_output {
            if output.success() {
                return Ok(());
            }
        }

        // Fallback: brew.
        let brew_output = sandbox.exec("brew install gitleaks").await;
        if let Ok(output) = brew_output {
            if output.success() {
                return Ok(());
            }
        }

        Err(CheckerError::ToolInstallFailed {
            tool: "gitleaks".to_string(),
            reason: "Could not install gitleaks via mise or brew. Install manually: https://github.com/gitleaks/gitleaks#installing".to_string(),
        })
    }

    async fn check(&self, ctx: &CheckContext) -> CheckResult<Vec<CheckIssue>> {
        let cmd = "gitleaks detect --report-format json --no-banner --exit-code 0 -v --source .";

        let output = ctx.exec(cmd).await?;
        let stdout = output.stdout();

        if stdout.trim().is_empty() || stdout.trim() == "[]" {
            return Ok(Vec::new());
        }

        parse_gitleaks_output(&stdout)
    }

    async fn fix(&self, _ctx: &CheckContext) -> CheckResult<FixReport> {
        // Secrets can't be auto-fixed; they need manual removal and rotation.
        Ok(FixReport::default())
    }

    fn supports_fix(&self) -> bool {
        false
    }
}

/// Parse gitleaks JSON output into `CheckIssue` structs.
///
/// Gitleaks JSON format (array of findings):
/// ```json
/// [
///   {
///     "Description": "AWS Access Key",
///     "File": "config.js",
///     "StartLine": 10,
///     "StartColumn": 1,
///     "EndLine": 10,
///     "EndColumn": 30,
///     "RuleID": "aws-access-key-id",
///     "Secret": "AKIA...",
///     "Match": "..."
///   }
/// ]
/// ```
fn parse_gitleaks_output(output: &str) -> CheckResult<Vec<CheckIssue>> {
    let mut issues = Vec::new();

    let findings: Vec<serde_json::Value> =
        serde_json::from_str(output).map_err(|e| CheckerError::ParseFailed {
            provider: "secrets".to_string(),
            reason: format!("Invalid JSON: {}", e),
        })?;

    for finding in &findings {
        let description = finding
            .get("Description")
            .and_then(|d| d.as_str())
            .unwrap_or("Secret detected");

        let file = finding
            .get("File")
            .and_then(|f| f.as_str())
            .unwrap_or("<unknown>");

        let rule_id = finding
            .get("RuleID")
            .and_then(|r| r.as_str())
            .map(|s| s.to_string());

        let start_line = finding
            .get("StartLine")
            .and_then(|l| l.as_u64())
            .map(|v| v as u32);
        let start_col = finding
            .get("StartColumn")
            .and_then(|c| c.as_u64())
            .map(|v| v as u32);
        let end_line = finding
            .get("EndLine")
            .and_then(|l| l.as_u64())
            .map(|v| v as u32);
        let end_col = finding
            .get("EndColumn")
            .and_then(|c| c.as_u64())
            .map(|v| v as u32);

        let message = format!(
            "Potential secret detected: {}. Remove the secret and rotate credentials immediately.",
            description
        );

        let mut issue = CheckIssue::new(file, Severity::Error, message, "secrets");
        if let Some(r) = rule_id {
            issue = issue.with_code(r);
        }
        if let (Some(l), Some(c)) = (start_line, start_col) {
            issue = issue.at(l, c);
        }
        if let (Some(el), Some(ec)) = (end_line, end_col) {
            issue = issue.to(el, ec);
        }
        issues.push(issue);
    }

    Ok(issues)
}
