//! URL utility functions.
//!
//! Most URL utilities are provided by `ufo_rs`. This module contains
//! only functions not available in that crate.

use url::Url;

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
    fn test_is_same_domain() {
        assert!(is_same_domain("https://example.com/a", "https://example.com/b"));
        assert!(is_same_domain("http://example.com", "https://example.com"));
        assert!(is_same_domain("//example.com/x", "https://example.com/y"));
        assert!(!is_same_domain("https://a.com", "https://b.com"));
    }
}
