//! URL utility functions inlined from legacy urlz/ufo crate.

use url::Url;

/// Remove leading slash from a path.
pub fn without_leading_slash(input: &str) -> &str {
    input.strip_prefix('/').unwrap_or(input)
}

/// Remove trailing slash, respecting query strings and fragments.
pub fn without_trailing_slash(input: &str) -> String {
    let end_pos = input
        .find('?')
        .or_else(|| input.find('#'))
        .unwrap_or(input.len());
    let (path, suffix) = input.split_at(end_pos);
    let trimmed = path.trim_end_matches('/');
    format!("{}{}", trimmed, suffix)
}

/// Check if two URLs share the same host (case-insensitive, ignores port/scheme).
pub fn is_same_domain(a: &str, b: &str) -> bool {
    let na = ensure_https(a);
    let nb = ensure_https(b);
    match (Url::parse(&na), Url::parse(&nb)) {
        (Ok(ua), Ok(ub)) => match (ua.host_str(), ub.host_str()) {
            (Some(ha), Some(hb)) => ha.eq_ignore_ascii_case(hb),
            _ => false,
        },
        _ => false,
    }
}

/// Normalize scheme-relative and triple-slash URLs.
pub fn normalize_url(input: &str) -> String {
    if input.starts_with("///") {
        input.replacen("///", "https://", 1)
    } else if input.starts_with("//") {
        input.replacen("//", "https://", 1)
    } else {
        input.to_string()
    }
}

fn ensure_https(input: &str) -> String {
    if input.starts_with("//") {
        format!("https:{}", input)
    } else {
        input.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_without_leading_slash() {
        assert_eq!(without_leading_slash("/foo"), "foo");
        assert_eq!(without_leading_slash("foo"), "foo");
        assert_eq!(without_leading_slash(""), "");
    }

    #[test]
    fn test_without_trailing_slash() {
        assert_eq!(without_trailing_slash("/foo/"), "/foo");
        assert_eq!(without_trailing_slash("/foo"), "/foo");
        assert_eq!(without_trailing_slash("/path/?q=1"), "/path?q=1");
    }

    #[test]
    fn test_is_same_domain() {
        assert!(is_same_domain("https://example.com/a", "https://example.com/b"));
        assert!(is_same_domain("http://example.com", "https://example.com"));
        assert!(is_same_domain("//example.com/x", "https://example.com/y"));
        assert!(!is_same_domain("https://a.com", "https://b.com"));
    }

    #[test]
    fn test_normalize_url() {
        assert_eq!(normalize_url("///example.com/"), "https://example.com/");
        assert_eq!(normalize_url("//example.com/"), "https://example.com/");
        assert_eq!(normalize_url("https://example.com"), "https://example.com");
    }
}
