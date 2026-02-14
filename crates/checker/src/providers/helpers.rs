//! Shared helpers for check provider implementations.
//!
//! Utility functions for parsing tool output, mapping severities,
//! and building commands.

use crate::error::{CheckResult, CheckerError};
use crate::output::Severity;

/// Build a combined output string from stdout and stderr of a
/// [`sandbox::CommandOutput`] for error reporting.
pub fn combined_output(output: &sandbox::CommandOutput) -> String {
    let stdout = output.stdout();
    let stderr = output.stderr();
    let mut combined = String::new();
    if !stdout.is_empty() {
        combined.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&stderr);
    }
    combined
}

/// Parse a JSON string into a `serde_json::Value`.
pub fn parse_json_output(output: &str, provider: &str) -> CheckResult<serde_json::Value> {
    serde_json::from_str(output).map_err(|e| CheckerError::ParseFailed {
        provider: provider.to_string(),
        reason: format!("Invalid JSON: {}", e),
    })
}

/// Map a string severity to our unified `Severity` enum.
pub fn map_severity(s: &str) -> Severity {
    match s.to_lowercase().as_str() {
        "error" | "fatal" | "err" => Severity::Error,
        "warning" | "warn" => Severity::Warning,
        "info" | "information" | "note" => Severity::Info,
        "hint" | "suggestion" => Severity::Hint,
        _ => Severity::Warning,
    }
}

/// Map a numeric severity level (common in many tools) to `Severity`.
pub fn map_severity_number(level: u32) -> Severity {
    match level {
        0 => Severity::Hint,
        1 => Severity::Warning,
        2 => Severity::Error,
        _ => Severity::Error,
    }
}
