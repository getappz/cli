//! Formatting utilities for dates, statuses, numbers, and strings.

use chrono::{DateTime, Utc};
use owo_colors::OwoColorize;

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

/// Format a timestamp that may be in seconds or milliseconds.
///
/// Automatically detects if the timestamp is in milliseconds (very large values)
/// and converts it to seconds before formatting.
///
/// # Arguments
/// * `ts` - Unix timestamp in seconds or milliseconds
///
/// # Returns
/// Formatted date string in "YYYY-MM-DD HH:MM:SS" format, or "N/A" if invalid
pub fn timestamp_auto(ts: i64) -> String {
    if ts <= 0 {
        return "N/A".to_string();
    }

    // If timestamp is very large (> year 2100 in seconds), assume it's in milliseconds
    // Jan 1, 2100 00:00:00 UTC = 4_102_444_800 seconds
    let ts_seconds = if ts > 4_102_444_800 { ts / 1000 } else { ts };

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
    match status_lower.as_str() {
        "active" | "success" | "completed" | "ready" | "safe" => status.green().to_string(),
        "pending" | "processing" | "queued" | "low" | "medium" => status.yellow().to_string(),
        "failed" | "error" | "cancelled" | "high" | "critical" => status.red().to_string(),
        "inactive" | "stopped" => status.bright_black().to_string(),
        _ => status.to_string(),
    }
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
}
