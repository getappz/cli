//! Client-side parsing and validation of `appz ls` filters, matching the
//! appz.dev `GET /v0/deployments` query schema.

use chrono::{DateTime, Duration, NaiveDate, Utc};

const STATUSES: [&str; 4] = ["ready", "building", "error", "canceled"];

/// Lower-case, validate and de-duplicate `--status` values.
pub fn parse_statuses(values: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for v in values {
        let s = v.trim().to_ascii_lowercase();
        if !STATUSES.contains(&s.as_str()) {
            return Err(format!(
                "invalid --status '{v}': expected one of {}",
                STATUSES.join(", ")
            ));
        }
        if !out.contains(&s) {
            out.push(s);
        }
    }
    Ok(out)
}

/// An environment slug: 1-40 chars of `a-z0-9-`, not starting or ending with `-`.
pub fn parse_environment(v: &str) -> Result<String, String> {
    let ok = !v.is_empty()
        && v.len() <= 40
        && !v.starts_with('-')
        && !v.ends_with('-')
        && v.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if ok {
        Ok(v.to_string())
    } else {
        Err(format!(
            "invalid --environment '{v}': use a lowercase slug such as production, preview or staging"
        ))
    }
}

/// `key=value` with a key of 1-40 chars from `A-Za-z0-9_.-`.
pub fn parse_meta(kv: &str) -> Result<String, String> {
    let (k, _) = kv
        .split_once('=')
        .ok_or_else(|| format!("invalid --meta '{kv}': expected key=value"))?;
    let ok = !k.is_empty()
        && k.len() <= 40
        && k.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'));
    if ok {
        Ok(kv.to_string())
    } else {
        Err(format!(
            "invalid --meta key '{k}': use 1-40 characters from A-Z a-z 0-9 _ . -"
        ))
    }
}

/// Parse `--since` / `--until` into epoch milliseconds.
///
/// Accepts a relative age (`30m`, `12h`, `7d`, `2w`, meaning that long ago), a date
/// (`2026-11-01`, UTC; as an upper bound it covers the whole day), an RFC 3339
/// timestamp, or raw epoch milliseconds.
pub fn parse_time(v: &str, upper_bound: bool, now: DateTime<Utc>) -> Result<i64, String> {
    let bad = || {
        format!(
            "invalid time '{v}': use 7d, 12h, 30m, 2w, YYYY-MM-DD, an RFC 3339 timestamp or epoch milliseconds"
        )
    };
    let v = v.trim();
    if v.len() >= 12 && v.bytes().all(|b| b.is_ascii_digit()) {
        return v.parse().map_err(|_| bad());
    }
    let (num, unit) = v.split_at(v.len().saturating_sub(1));
    if !num.is_empty() && num.bytes().all(|b| b.is_ascii_digit()) {
        let unit = unit.chars().next().unwrap_or(' ');
        let n: i64 = num.parse().map_err(|_| bad())?;
        let d = match unit {
            'm' => Duration::try_minutes(n),
            'h' => Duration::try_hours(n),
            'd' => Duration::try_days(n),
            'w' => Duration::try_weeks(n),
            _ => return Err(bad()),
        }
        .ok_or_else(bad)?;
        return now
            .checked_sub_signed(d)
            .map(|t| t.timestamp_millis())
            .ok_or_else(bad);
    }
    if let Ok(day) = NaiveDate::parse_from_str(v, "%Y-%m-%d") {
        let start = day
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        return Ok(if upper_bound {
            start + 86_400_000 - 1
        } else {
            start
        });
    }
    DateTime::parse_from_rfc3339(v)
        .map(|t| t.timestamp_millis())
        .map_err(|_| bad())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-24T00:00:00Z")
            .unwrap()
            .into()
    }

    #[test]
    fn statuses_validate_and_dedupe() {
        let v = |s: &[&str]| s.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            parse_statuses(&v(&["Ready", "error", "ready"])).unwrap(),
            ["ready", "error"]
        );
        assert!(
            parse_statuses(&v(&["failed"]))
                .unwrap_err()
                .contains("ready, building")
        );
    }

    #[test]
    fn environment_and_meta() {
        assert!(parse_environment("staging-2").is_ok());
        assert!(parse_environment("Prod").is_err());
        assert!(parse_environment("-x").is_err());
        assert!(parse_meta("git.sha=abc").is_ok());
        assert!(parse_meta("novalue").is_err());
        assert!(parse_meta("bad key=v").is_err());
    }

    #[test]
    fn times() {
        let day = 86_400_000;
        let n = now().timestamp_millis();
        assert_eq!(parse_time("7d", false, now()).unwrap(), n - 7 * day);
        assert_eq!(parse_time("2w", false, now()).unwrap(), n - 14 * day);
        assert_eq!(parse_time("90m", false, now()).unwrap(), n - 90 * 60_000);
        let d = parse_time("2026-11-01", false, now()).unwrap();
        assert_eq!(parse_time("2026-11-01", true, now()).unwrap(), d + day - 1);
        assert_eq!(parse_time("2026-11-01T00:00:00Z", false, now()).unwrap(), d);
        assert_eq!(
            parse_time("1762732800000", false, now()).unwrap(),
            1762732800000
        );
        assert!(parse_time("yesterday", false, now()).is_err());
        assert!(parse_time("7x", false, now()).is_err());
        assert!(parse_time("-7d", false, now()).is_err());
    }
}
