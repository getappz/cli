//! Formatting utilities for dates, statuses, numbers, and strings.

use crate::theme;
use chrono::{DateTime, Utc};
use design::ColorRole;

/// Format a Unix timestamp to a readable date/time string.
///
/// # Arguments
/// * `timestamp` - Unix timestamp in seconds
///
/// # Returns
/// Formatted date string in "YYYY-MM-DD HH:MM:SS" format, or "N/A" if invalid
pub fn timestamp(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "N/A".to_string();
    }

    match DateTime::from_timestamp(timestamp, 0) {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        None => "N/A".to_string(),
    }
}

/// Format a timestamp that may be in seconds, milliseconds, or microseconds.
///
/// Automatically detects the unit from magnitude and converts to seconds
/// before formatting. Some backends (e.g., D1) return microseconds.
///
/// # Arguments
/// * `ts` - Unix timestamp in seconds, milliseconds, or microseconds
///
/// # Returns
/// Formatted date string in "YYYY-MM-DD HH:MM:SS" format, or "N/A" if invalid
pub fn timestamp_auto(ts: i64) -> String {
    if ts <= 0 {
        return "N/A".to_string();
    }

    // Normalize to seconds based on magnitude. Thresholds:
    // - > 1e15: microseconds (e.g., 1739961333000000)
    // - > 4.1e9: milliseconds (year 2100 in seconds)
    // - else: already in seconds
    let ts_seconds = if ts > 1_000_000_000_000_000 {
        ts / 1_000_000
    } else if ts > 4_102_444_800 {
        ts / 1000
    } else {
        ts
    };

    timestamp(ts_seconds)
}

/// Format a Unix timestamp to a relative time string (e.g., "2 hours ago").
///
/// # Arguments
/// * `timestamp` - Unix timestamp in seconds
///
/// # Returns
/// Relative time string or absolute date if too old
pub fn timestamp_relative(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "N/A".to_string();
    }

    match DateTime::from_timestamp(timestamp, 0) {
        Some(dt) => {
            let now = Utc::now();
            let duration = now.signed_duration_since(dt);

            if duration.num_seconds() < 60 {
                "just now".to_string()
            } else if duration.num_minutes() < 60 {
                format!("{} minutes ago", duration.num_minutes())
            } else if duration.num_hours() < 24 {
                format!("{} hours ago", duration.num_hours())
            } else if duration.num_days() < 7 {
                format!("{} days ago", duration.num_days())
            } else {
                dt.format("%Y-%m-%d").to_string()
            }
        }
        None => "N/A".to_string(),
    }
}

/// Format a status string as a colored badge.
///
/// # Arguments
/// * `status` - Status string (e.g., "active", "pending", "failed")
///
/// # Returns
/// Colored status badge
pub fn status_badge(status: &str) -> String {
    let status_lower = status.to_lowercase();
    let role = match status_lower.as_str() {
        "active" | "success" | "completed" | "ready" | "safe" => ColorRole::Success,
        "pending" | "processing" | "queued" | "low" | "medium" => ColorRole::Warning,
        "failed" | "error" | "cancelled" | "high" | "critical" => ColorRole::Error,
        "inactive" | "stopped" => ColorRole::Muted,
        _ => return status.to_string(),
    };
    theme::style(status, role)
}

/// Format a number with thousand separators.
///
/// # Arguments
/// * `n` - Number to format
///
/// # Returns
/// Formatted number string
pub fn number(n: i64) -> String {
    let s = n.to_string();
    let mut result = String::new();

    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }

    result.chars().rev().collect()
}

/// Truncate a string to a maximum length with ellipsis.
///
/// # Arguments
/// * `s` - String to truncate
/// * `max_len` - Maximum length
///
/// # Returns
/// Truncated string with ellipsis if needed
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Format a unified diff string with colors for easier reading.
/// - Red for removed lines (-)
/// - Green for added lines (+)
/// - Cyan for hunk headers (@@)
/// Respects NO_COLOR environment variable.
pub fn colored_diff(plain: &str) -> String {
    let mut out = String::new();
    for line in plain.lines() {
        let colored = match line.chars().next() {
            Some('-') if !line.starts_with("---") => {
                theme::style(line, ColorRole::Error)
            }
            Some('+') if !line.starts_with("+++") => {
                theme::style(line, ColorRole::Success)
            }
            Some('@') => theme::style(line, ColorRole::Accent),
            _ => line.to_string(),
        };
        out.push_str(&colored);
        out.push('\n');
    }
    out
}

/// Format a duration in seconds to a human-readable string.
///
/// # Arguments
/// * `seconds` - Duration in seconds
///
/// # Returns
/// Human-readable duration (e.g., "2h 30m 15s")
pub fn duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        let minutes = seconds / 60;
        let secs = seconds % 60;
        if secs == 0 {
            format!("{}m", minutes)
        } else {
            format!("{}m {}s", minutes, secs)
        }
    } else {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let secs = seconds % 60;
        if secs == 0 && minutes == 0 {
            format!("{}h", hours)
        } else if secs == 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}h {}m {}s", hours, minutes, secs)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp() {
        let ts = 1609459200; // 2021-01-01 00:00:00 UTC
        assert!(timestamp(ts).contains("2021"));
        assert_eq!(timestamp(0), "N/A");
        assert_eq!(timestamp(-1), "N/A");
    }

    #[test]
    fn test_timestamp_auto_units() {
        // Seconds (2021-01-01 00:00:00 UTC)
        assert!(timestamp_auto(1609459200).contains("2021"));

        // Milliseconds (Feb 19, 2026)
        let ms = 1739961333000i64;
        assert!(
            timestamp_auto(ms).contains("2026"),
            "milliseconds should format correctly"
        );

        // Microseconds (CLI-created teams from D1/etc.)
        let us = 1739961333000000i64;
        assert!(
            timestamp_auto(us).contains("2026"),
            "microseconds should format correctly, not overflow to year 58108"
        );
    }

    #[test]
    fn test_status_badge() {
        let active = status_badge("active");
        assert!(active.contains("active"));
        let failed = status_badge("failed");
        assert!(failed.contains("failed"));
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "he...");
        assert_eq!(truncate("hi", 5), "hi");
    }

    #[test]
    fn test_duration() {
        assert_eq!(duration(30), "30s");
        assert_eq!(duration(90), "1m 30s");
        assert_eq!(duration(3665), "1h 1m 5s");
    }

    #[test]
    fn test_colored_diff() {
        let plain = "--- a/file\n+++ b/file\n@@ -1,3 +1,3 @@\n-old\n+new\n same\n";
        let result = colored_diff(plain);
        assert!(result.contains("old"));
        assert!(result.contains("new"));
        assert!(result.contains("same"));
    }
}
