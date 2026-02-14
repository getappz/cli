//! Shared helpers for check provider implementations.
//!
//! Utility functions for parsing tool output, mapping severities,
//! and building commands.

use crate::error::{CheckResult, CheckerError};
use crate::output::Severity;
use common::HeadTailBuffer;

/// Default output buffer capacity (1 MiB).
const OUTPUT_BUFFER_CAPACITY: usize = 1024 * 1024;

/// Build a combined output string from stdout and stderr of a
/// [`sandbox::CommandOutput`] for error reporting.
///
/// Uses a [`HeadTailBuffer`] to cap memory usage on large outputs.
pub fn combined_output(output: &sandbox::CommandOutput) -> String {
    let stdout = output.stdout();
    let stderr = output.stderr();

    let total_len = stdout.len() + stderr.len();
    if total_len <= OUTPUT_BUFFER_CAPACITY {
        // Small enough — no need for the buffer overhead.
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
        return combined;
    }

    // Large output: use HeadTailBuffer.
    let mut buf = HeadTailBuffer::new(OUTPUT_BUFFER_CAPACITY);
    buf.write(stdout.as_bytes());
    if !stderr.is_empty() {
        buf.write(b"\n");
        buf.write(stderr.as_bytes());
    }
    buf.to_string_lossy()
}

/// Build a combined output string from stdout and stderr, capped to the
/// given capacity using a [`HeadTailBuffer`].
pub fn combined_output_capped(output: &sandbox::CommandOutput, max_bytes: usize) -> String {
    let mut buf = HeadTailBuffer::new(max_bytes);
    buf.write(output.stdout().as_bytes());
    let stderr = output.stderr();
    if !stderr.is_empty() {
        buf.write(b"\n");
        buf.write(stderr.as_bytes());
    }
    buf.to_string_lossy()
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
