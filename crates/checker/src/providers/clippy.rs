//! Clippy check provider — Rust linting via `cargo clippy`.
//!
//! Clippy is the official Rust linter. This provider runs
//! `cargo clippy --message-format=json` and parses Cargo's JSON messages.
//!
//! - **Check**: `cargo clippy --message-format=json --quiet`
//! - **Fix**: `cargo clippy --fix --allow-dirty --allow-staged`

use std::path::Path;

use async_trait::async_trait;

use crate::config::CheckContext;
use crate::error::{CheckResult, CheckerError};
use crate::output::{CheckIssue, FixKind, FixReport, FixSuggestion};
use crate::provider::CheckProvider;
use crate::providers::helpers;

/// Clippy check provider for Rust.
pub struct ClippyProvider;

#[async_trait]
impl CheckProvider for ClippyProvider {
    fn name(&self) -> &str {
        "Clippy"
    }

    fn slug(&self) -> &str {
        "clippy"
    }

    fn description(&self) -> &str {
        "Official Rust linter (cargo clippy)"
    }

    fn detect(&self, project_dir: &Path, _frameworks: &[&str]) -> bool {
        project_dir.join("Cargo.toml").exists()
    }

    fn tool_name(&self) -> &str {
        "cargo"
    }

    async fn ensure_tool(&self, sandbox: &dyn sandbox::SandboxProvider) -> CheckResult<()> {
        // cargo clippy is part of the Rust toolchain.
        let check = sandbox.exec("cargo clippy --version").await;
        if let Ok(output) = check {
            if output.success() {
                return Ok(());
            }
        }

        // Try to add the clippy component.
        let _ = ui::status::info("Installing clippy component...");
        let output = sandbox.exec("rustup component add clippy").await?;
        if !output.success() {
            return Err(CheckerError::ToolInstallFailed {
                tool: "clippy".to_string(),
                reason: helpers::combined_output(&output),
            });
        }
        Ok(())
    }

    async fn check(&self, ctx: &CheckContext) -> CheckResult<Vec<CheckIssue>> {
        let cmd = "cargo clippy --message-format=json --quiet -- -W clippy::all";

        let output = ctx.exec(cmd).await?;
        let stdout = output.stdout();

        if stdout.trim().is_empty() {
            return Ok(Vec::new());
        }

        parse_cargo_messages(&stdout)
    }

    async fn fix(&self, ctx: &CheckContext) -> CheckResult<FixReport> {
        let cmd = "cargo clippy --fix --allow-dirty --allow-staged --quiet -- -W clippy::all";

        let output = ctx.exec(cmd).await?;

        // After fixing, run check again to see remaining issues.
        let remaining = self.check(ctx).await.unwrap_or_default();

        let _ = output; // Fix output is on stderr.

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

/// Parse Cargo's JSON message stream into `CheckIssue` structs.
///
/// Each line is a separate JSON object. We only care about "compiler-message"
/// type entries that contain diagnostic information.
fn parse_cargo_messages(output: &str) -> CheckResult<Vec<CheckIssue>> {
    let mut issues = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }

        let msg: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Only process compiler-message entries.
        let reason = msg.get("reason").and_then(|r| r.as_str());
        if reason != Some("compiler-message") {
            continue;
        }

        let diag = match msg.get("message") {
            Some(d) => d,
            None => continue,
        };

        let level = diag.get("level").and_then(|l| l.as_str()).unwrap_or("warning");

        // Skip notes and help messages — only keep errors and warnings.
        if level == "note" || level == "help" || level == "failure-note" {
            continue;
        }

        let severity = helpers::map_severity(level);

        let message = diag
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown issue")
            .to_string();

        let code = diag
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        // Extract primary span.
        let spans = diag.get("spans").and_then(|s| s.as_array());
        let primary_span = spans.and_then(|spans| {
            spans
                .iter()
                .find(|s| s.get("is_primary").and_then(|p| p.as_bool()).unwrap_or(false))
                .or_else(|| spans.first())
        });

        let (file, line_num, col_num) = if let Some(span) = primary_span {
            let file = span
                .get("file_name")
                .and_then(|f| f.as_str())
                .unwrap_or("<unknown>");
            let line = span.get("line_start").and_then(|l| l.as_u64()).map(|v| v as u32);
            let col = span
                .get("column_start")
                .and_then(|c| c.as_u64())
                .map(|v| v as u32);
            (file.to_string(), line, col)
        } else {
            ("<unknown>".to_string(), None, None)
        };

        // Check if there's a suggestion (machine-applicable fix).
        let has_suggestion = primary_span
            .and_then(|s| s.get("suggestion_applicability"))
            .and_then(|a| a.as_str())
            .map(|a| a == "MachineApplicable" || a == "MaybeIncorrect")
            .unwrap_or(false);

        let fix = if has_suggestion {
            let kind = if primary_span
                .and_then(|s| s.get("suggestion_applicability"))
                .and_then(|a| a.as_str())
                == Some("MachineApplicable")
            {
                FixKind::Safe
            } else {
                FixKind::Unsafe
            };
            Some(FixSuggestion {
                kind,
                description: "Clippy suggestion".to_string(),
                replacement: primary_span
                    .and_then(|s| s.get("suggested_replacement"))
                    .and_then(|r| r.as_str())
                    .map(|s| s.to_string()),
            })
        } else {
            None
        };

        let mut issue = CheckIssue::new(file, severity, message, "clippy");
        if let Some(c) = code {
            issue = issue.with_code(c);
        }
        if let (Some(l), Some(c)) = (line_num, col_num) {
            issue = issue.at(l, c);
        }
        if let Some(f) = fix {
            issue = issue.with_fix(f);
        }
        issues.push(issue);
    }

    Ok(issues)
}
