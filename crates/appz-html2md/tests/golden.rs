use appz_html2md::{ConvertError, MAX_HTML_BYTES, Mode, convert, estimate_tokens};
use url::Url;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn origin() -> Url {
    Url::parse("https://example.com/blog/post").unwrap()
}

#[test]
fn article_matches_golden() {
    let got = convert(&fixture("article.html"), &origin()).unwrap();
    assert_eq!(got.mode, Mode::Minimal);
    let golden = String::from_utf8(fixture("article.md")).unwrap();
    assert_eq!(got.markdown, golden);
    assert_eq!(got.tokens, estimate_tokens(&got.markdown));
}

#[test]
fn landing_falls_back_when_isolate_truncates() {
    let html = fixture("landing.html");
    let got = convert(&html, &origin()).unwrap();
    assert_eq!(got.mode, Mode::Fallback);
    // forced-minimal output is just the banner
    let forced =
        appz_html2md::convert_minimal_only(std::str::from_utf8(&html).unwrap(), origin().as_str());
    assert!(
        got.tokens > 4 * estimate_tokens(&forced),
        "{} vs {}",
        got.tokens,
        forced.len()
    );
    assert!(got.markdown.contains("Feature 40"));
    assert!(!got.markdown.contains("Footer links"));
}

#[test]
fn strips_svg_data_uris() {
    let html = b"<main><p>hi</p><img alt=\"logo\" src=\"data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=\"><img alt=\"pic\" src=\"/p.png\"></main>";
    let got = convert(html, &origin()).unwrap();
    assert!(!got.markdown.contains("data:"), "{}", got.markdown);
    assert!(got.markdown.contains("https://example.com/p.png"));
}

#[test]
fn strips_h1_and_sup_leaks() {
    let html = b"<main><p>x</p><a href=\"/a\"><h1>Big Title</h1></a> <a href=\"/b\">Note<sup>1</sup></a></main>";
    let got = convert(html, &origin()).unwrap();
    assert!(
        !got.markdown.contains("<h1>") && !got.markdown.contains("<sup>"),
        "{}",
        got.markdown
    );
    assert!(!got.markdown.contains("[# "), "{}", got.markdown);
    assert!(
        got.markdown.contains("[Big Title](https://example.com/a)"),
        "{}",
        got.markdown
    );
    assert!(
        got.markdown.contains("[Note1](https://example.com/b)")
            || got.markdown.contains("[Note](https://example.com/b)"),
        "{}",
        got.markdown
    );
}

#[test]
fn relative_links_resolve_against_origin() {
    let got = convert(&fixture("article.html"), &origin()).unwrap();
    assert!(
        got.markdown.contains("(https://example.com/docs/start)"),
        "{}",
        got.markdown
    );
    assert!(
        got.markdown.contains("(https://example.com/faq/)"),
        "{}",
        got.markdown
    );
    assert!(got.markdown.contains("(https://other.example/page)"));
}

#[test]
fn front_matter_and_json_ld() {
    let got = convert(&fixture("article.html"), &origin()).unwrap();
    assert!(got.markdown.starts_with("---\n"));
    assert!(got.markdown.contains("title: "));
    assert!(
        got.markdown
            .contains("How to ship a static site in five steps.")
    );
    assert!(!got.markdown.contains("OG description"));
    assert!(got.markdown.contains("https://example.com/img/cover.png"));
    assert!(got.markdown.trim_end().ends_with("```"));
    assert!(got.markdown.contains("```json\n{\"@context\""));
}

#[test]
fn deterministic() {
    let a = convert(&fixture("article.html"), &origin()).unwrap();
    let b = convert(&fixture("article.html"), &origin()).unwrap();
    assert_eq!(a.markdown, b.markdown);
    let c = convert(&fixture("landing.html"), &origin()).unwrap();
    let d = convert(&fixture("landing.html"), &origin()).unwrap();
    assert_eq!(c.markdown, d.markdown);
}

#[test]
fn rejects_over_cap() {
    let big = vec![b'a'; MAX_HTML_BYTES + 1];
    assert!(matches!(
        convert(&big, &origin()),
        Err(ConvertError::TooLarge)
    ));
}

#[test]
fn rejects_empty_result() {
    assert!(matches!(
        convert(b"<html><body></body></html>", &origin()),
        Err(ConvertError::Empty)
    ));
    assert!(matches!(convert(b"", &origin()), Err(ConvertError::Empty)));
}

#[test]
fn rejects_non_utf8() {
    assert!(matches!(
        convert(&[0xff, 0xfe, 0x3c, 0x70, 0x3e], &origin()),
        Err(ConvertError::NotUtf8)
    ));
}

#[test]
fn relative_mode_keeps_links_relative() {
    let got = appz_html2md::convert_relative(&fixture("article.html")).unwrap();
    assert!(got.markdown.contains("](/docs/start)"), "{}", got.markdown);
    assert!(
        got.markdown.contains("image: \"/img/cover.png\""),
        "{}",
        got.markdown
    );
    assert!(!got.markdown.contains("example.com"));
}
