//! TypeScript compiler check provider — type checking via `tsc --noEmit`.
//!
//! The TypeScript compiler is the only reliable way to check TypeScript types.
//! This provider runs `tsc --noEmit` and parses the compiler's diagnostic output.

use std::path::Path;

use async_trait::async_trait;

use crate::config::CheckContext;
use crate::error::{CheckResult, CheckerError};
use crate::output::{CheckIssue, FixReport, Severity};
use crate::provider::CheckProvider;
use crate::providers::helpers;

/// TypeScript compiler check provider.
pub struct TypeScriptProvider;

#[async_trait]
impl CheckProvider for TypeScriptProvider {
    fn name(&self) -> &str {
        "TypeScript Compiler"
    }

    fn slug(&self) -> &str {
        "tsc"
    }

    fn description(&self) -> &str {
        "TypeScript type checking via tsc --noEmit"
    }

    fn detect(&self, project_dir: &Path, _frameworks: &[&str]) -> bool {
        project_dir.join("tsconfig.json").exists()
    }

    fn tool_name(&self) -> &str {
        "tsc"
    }

    async fn ensure_tool(&self, sandbox: &dyn sandbox::SandboxProvider) -> CheckResult<()> {
        // tsc is usually a devDependency; check npx first.
        let check = sandbox.exec("npx tsc --version").await;
        if let Ok(output) = check {
            if output.success() {
                return Ok(());
            }
        }

        // Try global install.
        let _ = ui::status::info("Installing TypeScript...");
        let output = sandbox.exec("npm install -g typescript").await?;
        if !output.success() {
            return Err(CheckerError::ToolInstallFailed {
                tool: "typescript".to_string(),
                reason: helpers::combined_output(&output),
            });
        }
        Ok(())
    }

    async fn check(&self, ctx: &CheckContext) -> CheckResult<Vec<CheckIssue>> {
        // tsc --noEmit --pretty false produces machine-readable output.
        let cmd = "npx tsc --noEmit --pretty false";

        let output = ctx.exec(cmd).await?;
        let stdout = output.stdout();

        // tsc exits with non-zero if it finds type errors — that's expected.
        if stdout.trim().is_empty() && output.success() {
            return Ok(Vec::new());
        }

        parse_tsc_output(&stdout)
    }

    async fn fix(&self, _ctx: &CheckContext) -> CheckResult<FixReport> {
        // tsc doesn't support auto-fix; type errors require manual intervention.
        Ok(FixReport::default())
    }

    fn supports_fix(&self) -> bool {
        false
    }
}

/// Parse tsc output into `CheckIssue` structs.
///
/// tsc output format: `file(line,col): error TSxxxx: message`
fn parse_tsc_output(output: &str) -> CheckResult<Vec<CheckIssue>> {
    let mut issues = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse: file(line,col): error TSxxxx: message
        // or: file(line,col): warning TSxxxx: message
        if let Some(issue) = parse_tsc_line(line) {
            issues.push(issue);
        }
    }

    Ok(issues)
}

/// Parse a single tsc diagnostic line.
fn parse_tsc_line(line: &str) -> Option<CheckIssue> {
    // Format: path/to/file.ts(10,5): error TS2304: Cannot find name 'foo'.
    let paren_pos = line.find('(')?;
    let close_paren = line[paren_pos..].find(')')? + paren_pos;

    let file = &line[..paren_pos];
    let location = &line[paren_pos + 1..close_paren];

    // Parse line,col.
    let parts: Vec<&str> = location.split(',').collect();
    let line_num = parts.first().and_then(|s| s.parse::<u32>().ok());
    let col_num = parts.get(1).and_then(|s| s.parse::<u32>().ok());

    // Parse ": error TSxxxx: message" or ": warning TSxxxx: message".
    let rest = &line[close_paren + 1..].trim_start_matches(':').trim();

    let (severity, rest) = if rest.starts_with("error") {
        (Severity::Error, rest.strip_prefix("error")?.trim())
    } else if rest.starts_with("warning") {
        (Severity::Warning, rest.strip_prefix("warning")?.trim())
    } else {
        (Severity::Error, *rest)
    };

    // Extract code (TSxxxx) and message.
    let (code, message) = if let Some(colon_pos) = rest.find(':') {
        let code = rest[..colon_pos].trim().to_string();
        let msg = rest[colon_pos + 1..].trim().to_string();
        (Some(code), msg)
    } else {
        (None, rest.to_string())
    };

    let mut issue = CheckIssue::new(file, severity, message, "tsc");
    if let (Some(l), Some(c)) = (line_num, col_num) {
        issue = issue.at(l, c);
    }
    if let Some(c) = code {
        issue = issue.with_code(c);
    }

    Some(issue)
}
