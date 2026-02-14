//! Auto-fix orchestrator for `--fix` mode.
//!
//! Runs each provider's fix command for safe fixes only, then collects
//! remaining issues and generates a summary report.

use crate::config::CheckContext;
use crate::error::CheckResult;
use crate::output::{CheckReport, FixReport, Severity};
use crate::provider::CheckProvider;

/// Run auto-fix across all applicable providers.
///
/// This is called by the runner when `--fix` is active. Each provider
/// that supports fix runs its own fix command (e.g. `biome check --fix`,
/// `ruff check --fix`, `cargo clippy --fix`).
///
/// Returns the aggregated fix report.
pub async fn run_fixes(
    providers: &[Box<dyn CheckProvider>],
    ctx: &CheckContext,
) -> CheckResult<FixReport> {
    let mut total_fixed = 0usize;
    let mut all_remaining = Vec::new();
    let mut fixed_descriptions = Vec::new();

    for provider in providers {
        if !provider.supports_fix() {
            continue;
        }

        if !ctx.check_config.strict.unwrap_or(false) {
            let _ = ui::status::info(&format!("Fixing with {}...", provider.name()));
        }

        match provider.fix(ctx).await {
            Ok(report) => {
                total_fixed += report.fixed_count;
                all_remaining.extend(report.remaining_issues);
                fixed_descriptions.extend(report.fixed_issues);
            }
            Err(e) => {
                let _ = ui::status::warning(&format!(
                    "Auto-fix failed for {}: {}",
                    provider.name(),
                    e
                ));
            }
        }
    }

    Ok(FixReport {
        fixed_count: total_fixed,
        remaining_count: all_remaining.len(),
        fixed_issues: fixed_descriptions,
        remaining_issues: all_remaining,
    })
}

/// Display a summary of the fix results.
pub fn display_fix_summary(report: &FixReport) {
    if report.fixed_count > 0 {
        let _ = ui::status::success(&format!(
            "Fixed {} issue{}",
            report.fixed_count,
            if report.fixed_count == 1 { "" } else { "s" }
        ));
    }

    if report.remaining_count > 0 {
        let _ = ui::status::warning(&format!(
            "{} issue{} remaining after auto-fix",
            report.remaining_count,
            if report.remaining_count == 1 { "" } else { "s" }
        ));
    }
}

/// Display the final check report summary.
pub fn display_report_summary(report: &CheckReport, strict: bool) {
    let _ = ui::layout::blank_line();

    if report.issues.is_empty() {
        let _ = ui::status::success("No issues found!");
        return;
    }

    // Count by severity.
    let errors = report
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .count();
    let warnings = report
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Warning)
        .count();
    let infos = report
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Info || i.severity == Severity::Hint)
        .count();

    // Display issues grouped by file.
    let mut current_file = String::new();
    for issue in &report.issues {
        let file_str = issue.file.display().to_string();
        if file_str != current_file {
            let _ = ui::layout::blank_line();
            let _ = ui::status::info(&file_str);
            current_file = file_str;
        }

        let location = match (issue.line, issue.column) {
            (Some(l), Some(c)) => format!("{}:{}", l, c),
            (Some(l), None) => format!("{}:1", l),
            _ => String::new(),
        };

        let code_str = issue
            .code
            .as_deref()
            .map(|c| format!(" [{}]", c))
            .unwrap_or_default();

        let fix_indicator = if issue.has_safe_fix() { " (fixable)" } else { "" };

        let line = format!(
            "  {:>8} {:>10}  {}{}{}",
            issue.severity, location, issue.message, code_str, fix_indicator
        );

        match issue.severity {
            Severity::Error => {
                let _ = ui::status::error(&line);
            }
            Severity::Warning => {
                let _ = ui::status::warning(&line);
            }
            _ => {
                let _ = ui::status::info(&line);
            }
        }
    }

    let _ = ui::layout::blank_line();

    // Summary line.
    let mut parts = Vec::new();
    if errors > 0 {
        parts.push(format!("{} error{}", errors, if errors == 1 { "" } else { "s" }));
    }
    if warnings > 0 {
        parts.push(format!(
            "{} warning{}",
            warnings,
            if warnings == 1 { "" } else { "s" }
        ));
    }
    if infos > 0 {
        parts.push(format!("{} info", infos));
    }

    let summary = parts.join(", ");
    let duration_ms = report.duration.as_millis();

    if report.passed(strict) {
        let _ = ui::status::success(&format!("Check passed ({}) in {}ms", summary, duration_ms));
    } else {
        let _ = ui::status::error(&format!("Check failed: {} ({}ms)", summary, duration_ms));
    }

    // Fix report.
    if let Some(ref fix_report) = report.fix_report {
        display_fix_summary(fix_report);
    }

    // Providers run.
    let _ = ui::status::info(&format!(
        "Checkers: {}",
        report.providers_run.join(", ")
    ));
}
