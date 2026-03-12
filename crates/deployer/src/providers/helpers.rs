//! Shared helpers for provider implementations.
//!
//! Thin wrappers around the sandbox crate for command execution and
//! pure utility functions for parsing CLI output.

use crate::error::DeployResult;

/// Check if an environment variable is set and non-empty.
pub fn has_env_var(name: &str) -> bool {
    let bag = env_var::GlobalEnvBag::instance();
    if let Some(val) = bag.get(name) {
        !val.is_empty()
    } else {
        std::env::var(name).map(|v| !v.is_empty()).unwrap_or(false)
    }
}

/// Get the value of an environment variable.
pub fn get_env_var(name: &str) -> Option<String> {
    let bag = env_var::GlobalEnvBag::instance();
    bag.get(name).or_else(|| std::env::var(name).ok())
}

/// Extract a URL from command output text.
///
/// Looks for common URL patterns in deploy command output.
pub fn extract_url_from_output(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        // Look for https:// URLs
        if let Some(start) = trimmed.find("https://") {
            let url_part = &trimmed[start..];
            // Find the end of the URL (space, newline, or end of string)
            let end = url_part
                .find(|c: char| c.is_whitespace() || c == ')' || c == ']' || c == '>')
                .unwrap_or(url_part.len());
            return Some(url_part[..end].to_string());
        }
    }
    None
}

/// Parse a JSON response string into a serde_json::Value.
#[allow(dead_code)]
pub fn parse_json_output(output: &str) -> DeployResult<serde_json::Value> {
    serde_json::from_str(output).map_err(|e| crate::error::DeployError::JsonError {
        reason: format!("Failed to parse provider output as JSON: {}", e),
    })
}

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
