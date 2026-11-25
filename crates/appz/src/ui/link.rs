//! OSC 8 terminal hyperlinks (`ESC ] 8 ; ; URL ST text ESC ] 8 ; ; ST`).
//!
//! `console` does not know these sequences: its width/strip/truncate helpers
//! would count the URL as visible text. Everything that measures cells goes
//! through [`width`] and [`truncate`] here instead.

use std::borrow::Cow;

const OPEN: &str = "\x1b]8;;";
const ST: &str = "\x1b\\";

/// `text` as a hyperlink to `target`. Pure: callers decide when to use it.
pub(crate) fn osc8(target: &str, text: &str) -> String {
    format!("{OPEN}{target}{ST}{text}{OPEN}{ST}")
}

/// Link target for a displayed URL or bare host (`site.appz.page` -> https).
pub(crate) fn target_of(s: &str) -> String {
    if s.contains("://") {
        s.to_string()
    } else {
        format!("https://{s}")
    }
}

/// `text` styled and linked to `url` when `enabled`, else plain `text`.
pub(crate) fn linked(enabled: bool, url: &str, text: &str, style: fn(&str) -> String) -> String {
    if enabled {
        osc8(&target_of(url), &style(text))
    } else {
        text.to_string()
    }
}

/// Split the first OSC 8 link of `s` into `(before, target, text, after)`.
fn split(s: &str) -> Option<(&str, &str, &str, &str)> {
    let start = s.find(OPEN)?;
    let rest = &s[start + OPEN.len()..];
    let t_end = rest.find(ST)?;
    let target = &rest[..t_end];
    let body = &rest[t_end + ST.len()..];
    let close = body.find(OPEN)?;
    let after = &body[close + OPEN.len()..];
    let after = after.strip_prefix(ST)?;
    Some((&s[..start], target, &body[..close], after))
}

/// `s` without OSC 8 sequences (ANSI styling is left for `console`).
pub(crate) fn strip(s: &str) -> Cow<'_, str> {
    if !s.contains(OPEN) {
        return Cow::Borrowed(s);
    }
    let mut out = String::new();
    let mut rest = s;
    while let Some((before, _, text, after)) = split(rest) {
        out.push_str(before);
        out.push_str(text);
        rest = after;
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// Display width of `s`, ignoring ANSI styling and OSC 8 links.
pub(crate) fn width(s: &str) -> usize {
    console::measure_text_width(&strip(s))
}

/// Truncate to `max` display columns with `tail`. A link cell keeps its whole
/// target and only shortens the visible text, so the escape is never cut.
pub(crate) fn truncate(s: &str, max: usize, tail: &str) -> String {
    match split(s) {
        Some((before, target, text, after)) if !after.contains(OPEN) => {
            let room = max.saturating_sub(width(before) + width(after));
            let text = console::truncate_str(text, room, tail);
            format!("{before}{}{after}", osc8(target, &text))
        }
        Some(_) => s.to_string(),
        None => console::truncate_str(s, max, tail).into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_shape() {
        assert_eq!(
            osc8("https://a.test", "a"),
            "\x1b]8;;https://a.test\x1b\\a\x1b]8;;\x1b\\"
        );
    }

    #[test]
    fn width_ignores_the_escape() {
        let l = osc8("https://a.test/long/target", "a.test");
        assert_eq!(width(&l), 6);
        assert_eq!(strip(&l), "a.test");
        let styled = format!("x {} y", console::style(&l).cyan().force_styling(true));
        assert_eq!(width(&styled), "x a.test y".len());
    }

    #[test]
    fn truncate_keeps_the_target_whole() {
        let l = osc8("https://example.test/target", "example.test/very/long");
        let t = truncate(&l, 10, "…");
        assert_eq!(width(&t), 10);
        assert!(t.contains("https://example.test/target"));
        assert!(t.ends_with("\x1b]8;;\x1b\\"));
    }

    #[test]
    fn plain_when_disabled_escaped_when_enabled() {
        let id = |s: &str| s.to_string();
        assert_eq!(linked(false, "a.test", "a.test", id), "a.test");
        assert_eq!(
            linked(true, "a.test", "a.test", id),
            "\x1b]8;;https://a.test\x1b\\a.test\x1b]8;;\x1b\\"
        );
    }

    #[test]
    fn bare_hosts_get_https() {
        assert_eq!(target_of("a.appz.page"), "https://a.appz.page");
        assert_eq!(target_of("http://a.test"), "http://a.test");
    }
}
