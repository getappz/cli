//! Humanized values shared by every command: ages, sizes, durations, status.

use super::theme::out;

pub(crate) fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Absolute UTC time for detail views: `2026-11-25 03:19 UTC`.
/// Accepts seconds or milliseconds since the epoch.
pub(crate) fn datetime(ts: i64) -> String {
    if ts <= 0 {
        return "-".into();
    }
    chrono::DateTime::from_timestamp_millis(to_ms(ts))
        .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "-".into())
}

/// Epoch seconds or milliseconds → milliseconds.
pub(crate) fn to_ms(ts: i64) -> i64 {
    if ts.abs() < 100_000_000_000 {
        ts * 1000
    } else {
        ts
    }
}

/// `age` against now, for a timestamp in seconds or milliseconds.
pub(crate) fn ago(ts: i64) -> String {
    if ts <= 0 {
        return String::new();
    }
    age(to_ms(ts), now_ms())
}

/// `just now` / `5m ago` for detail views next to an absolute time; `None`
/// once `age` would fall back to a date (the absolute time already says it).
pub(crate) fn relative(ts_ms: i64, now_ms: i64) -> Option<String> {
    match age(ts_ms, now_ms) {
        a if a == "just now" => Some(a),
        a if a.ends_with(['m', 'h', 'd']) => Some(format!("{a} ago")),
        _ => None,
    }
}

/// Compact relative age for tables: `just now`, `5m`, `3h`, `5d`, then a date.
pub(crate) fn age(ts_ms: i64, now_ms: i64) -> String {
    let secs = (now_ms - ts_ms) / 1000;
    match secs {
        s if s < 45 => "just now".into(),
        s if s < 3600 => format!("{}m", (s / 60).max(1)),
        s if s < 86_400 => format!("{}h", s / 3600),
        s if s < 30 * 86_400 => format!("{}d", s / 86_400),
        _ => chrono::DateTime::from_timestamp_millis(ts_ms)
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "-".into()),
    }
}

/// `319.5 MB`. Same formatter `appz clean` has always used.
pub(crate) use appz_core::format_size as bytes;

/// Uppercase the first character: `kept permanently` → `Kept permanently`.
pub(crate) fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

pub(crate) fn duration_ms(ms: u64) -> String {
    match ms {
        m if m < 1000 => format!("{m}ms"),
        m if m < 10_000 => format!("{:.1}s", m as f64 / 1000.0),
        m if m < 60_000 => format!("{}s", m / 1000),
        m => format!("{}m {:02}s", m / 60_000, (m % 60_000) / 1000),
    }
}

/// `● Ready` (green) / `● Error` (red) / `● Building` (yellow) / other dim.
/// Styled for stdout, since statuses live in tables.
pub(crate) fn status(s: &str) -> String {
    if s.trim().is_empty() {
        return "-".into();
    }
    let lower = s.trim().to_lowercase().replace(['_', '-'], " ");
    let label = capitalize(&lower);
    let dot = match lower.split(' ').next().unwrap_or("") {
        "ready" | "active" | "live" | "succeeded" | "success" | "verified" | "current" | "ok"
        | "delivered" => out::green("●"),
        "error" | "failed" | "canceled" | "cancelled" | "expired" | "blocked" => out::red("●"),
        "building" | "queued" | "pending" | "initializing" | "deploying" | "uploading"
        | "staged" | "paused" | "expiring" | "retrying" => out::yellow("●"),
        _ => out::dim("●"),
    };
    format!("{dot} {label}")
}

pub(crate) fn plural(n: usize, noun: &str) -> String {
    if n == 1 {
        return format!("1 {noun}");
    }
    if let Some(stem) = noun.strip_suffix('y')
        && !stem.ends_with(['a', 'e', 'i', 'o', 'u'])
    {
        return format!("{n} {stem}ies");
    }
    if noun.ends_with(['s', 'x', 'z']) || noun.ends_with("sh") || noun.ends_with("ch") {
        return format!("{n} {noun}es");
    }
    format!("{n} {noun}s")
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;
    const HOUR: i64 = 60 * MIN;
    const DAY: i64 = 24 * HOUR;

    fn plain() {
        console::set_colors_enabled(false);
        console::set_colors_enabled_stderr(false);
    }

    #[test]
    fn age_buckets() {
        let now = 1_795_000_000_000;
        assert_eq!(age(now - 10_000, now), "just now");
        assert_eq!(age(now - 5 * MIN, now), "5m");
        assert_eq!(age(now - 3 * HOUR, now), "3h");
        assert_eq!(age(now - 5 * DAY, now), "5d");
    }

    #[test]
    fn age_old_dates_are_absolute() {
        // 2026-10-01T00:00:00Z
        let ts = 1_790_812_800_000;
        assert_eq!(age(ts, ts + 40 * DAY), "2026-10-01");
    }

    #[test]
    fn age_in_the_future_is_just_now() {
        let now = 1_795_000_000_000;
        assert_eq!(age(now + MIN, now), "just now");
    }

    #[test]
    fn datetime_accepts_secs_and_ms() {
        assert_eq!(datetime(1_790_812_800), "2026-10-01 00:00 UTC");
        assert_eq!(datetime(1_790_812_800_000), "2026-10-01 00:00 UTC");
    }

    #[test]
    fn to_ms_normalizes_seconds() {
        assert_eq!(to_ms(1_790_812_800), 1_790_812_800_000);
        assert_eq!(to_ms(1_790_812_800_000), 1_790_812_800_000);
    }

    #[test]
    fn now_is_after_2026() {
        assert!(now_ms() > 1_767_225_600_000);
    }

    #[test]
    fn bytes_humanized() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(1536), "1.5 KB");
        assert_eq!(bytes(335_005_295), "319.5 MB");
        assert_eq!(bytes(5_368_709_120), "5.0 GB");
    }

    #[test]
    fn durations() {
        assert_eq!(duration_ms(850), "850ms");
        assert_eq!(duration_ms(1200), "1.2s");
        assert_eq!(duration_ms(42_000), "42s");
        assert_eq!(duration_ms(185_000), "3m 05s");
    }

    #[test]
    fn status_titlecases_with_dot() {
        plain();
        assert_eq!(status("READY"), "● Ready");
        assert_eq!(status("pending_validation"), "● Pending validation");
        assert_eq!(status(""), "-");
    }

    #[test]
    fn relative_phrase_for_detail_views() {
        let now = 1_795_000_000_000;
        assert_eq!(relative(now - 10_000, now).as_deref(), Some("just now"));
        assert_eq!(relative(now - 5 * MIN, now).as_deref(), Some("5m ago"));
        assert_eq!(relative(now - 40 * DAY, now), None);
    }

    #[test]
    fn capitalize_first_letter_only() {
        assert_eq!(capitalize("kept permanently"), "Kept permanently");
        assert_eq!(capitalize(""), "");
        assert_eq!(capitalize("éclair"), "Éclair");
    }

    #[test]
    fn plurals_of_sibilant_nouns() {
        assert_eq!(plural(2, "alias"), "2 aliases");
        assert_eq!(plural(3, "process"), "3 processes");
        assert_eq!(plural(2, "box"), "2 boxes");
    }

    #[test]
    fn status_colors_ok_and_expiring() {
        console::set_colors_enabled(true);
        assert_ne!(status("ok"), status("unknown-state"));
        assert_ne!(status("expiring"), status("unknown-state"));
        console::set_colors_enabled(false);
    }

    #[test]
    fn missing_epoch_timestamps_render_dash() {
        assert_eq!(datetime(0), "-");
    }

    #[test]
    fn plurals() {
        assert_eq!(plural(1, "project"), "1 project");
        assert_eq!(plural(20, "project"), "20 projects");
        assert_eq!(plural(2, "proxy"), "2 proxies");
        assert_eq!(plural(0, "webhook"), "0 webhooks");
    }
}
