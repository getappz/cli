use appz_deploy::markdown::{
    ConvertError, HttpMarkdownConverter, MAX_HTML_BYTES, MarkdownConverter, add_markdown_siblings,
    markdown_sibling_path,
};
use appz_deploy::read_markdown_config;
use appz_deploy::static_cas::{ScannedTree, scan_output_dir, sha256_hex};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

/// A distinct page per `n`, so dedupe-by-content does not merge them.
fn page(n: &str) -> String {
    format!("<html><body><main><h1>Page {n}</h1><p>text</p></main></body></html>")
}

/// Stand-in for the hosted converter. Pages containing "FAIL" are refused.
fn fake(html: &[u8], page_url: Option<&str>) -> Result<String, ConvertError> {
    let text = std::str::from_utf8(html).map_err(|_| ConvertError::new("not utf-8"))?;
    if text.contains("FAIL") || !text.contains("<h1>") {
        return Err(ConvertError::new("unconvertible"));
    }
    Ok(format!("# Hello\n\nurl: {}\n", page_url.unwrap_or("-")))
}

fn site(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().unwrap();
    for (path, body) in files {
        let full = dir.path().join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, body).unwrap();
    }
    dir
}

fn paths(tree: &ScannedTree) -> Vec<String> {
    let mut p: Vec<String> = tree.files.iter().map(|f| f.path.clone()).collect();
    p.sort();
    p
}

fn read_md(t: &ScannedTree, path: &str) -> String {
    let e = t.files.iter().find(|f| f.path == path).unwrap();
    fs::read_to_string(&t.blobs_by_hash[&e.hash].path).unwrap()
}

fn config(dir: &Path, body: &str) -> appz_deploy::DeployResult<appz_deploy::MarkdownConfig> {
    fs::write(dir.join("appz.jsonc"), body).unwrap();
    read_markdown_config(dir)
}

#[test]
fn read_markdown_config_cases() {
    let dir = TempDir::new().unwrap();
    assert!(
        !read_markdown_config(dir.path()).unwrap().enabled,
        "absent file"
    );
    assert!(
        !config(dir.path(), "{ \"deploy\": {} }").unwrap().enabled,
        "absent key"
    );
    assert!(
        config(dir.path(), "{ \"markdown\": { \"enabled\": true } }")
            .unwrap()
            .enabled
    );
    assert!(!config(dir.path(), "{ \"markdown\": {} }").unwrap().enabled);
    let jsonc = "{\n // agents\n \"markdown\": { \"enabled\": true /* on */ }\n}";
    assert!(
        config(dir.path(), jsonc).unwrap().enabled,
        "JSONC with comments"
    );
    assert!(
        config(dir.path(), "{ \"markdown\": { \"enabled\": \"yes\" } }").is_err(),
        "wrong type"
    );
    assert!(config(dir.path(), "{ not json").is_err(), "malformed");
}

#[test]
fn sibling_path_mapping() {
    assert_eq!(
        markdown_sibling_path("/index.html").as_deref(),
        Some("/index.md")
    );
    assert_eq!(
        markdown_sibling_path("/about/index.html").as_deref(),
        Some("/about/index.md")
    );
    assert_eq!(
        markdown_sibling_path("/about.html").as_deref(),
        Some("/about.md")
    );
    assert_eq!(
        markdown_sibling_path("/a/b/c.html").as_deref(),
        Some("/a/b/c.md")
    );
    assert_eq!(markdown_sibling_path("/app.js"), None);
    assert_eq!(markdown_sibling_path("/readme.md"), None);
    assert_eq!(markdown_sibling_path("/noext"), None);
}

#[test]
fn enabled_adds_md_for_each_html() {
    let (a, b, c) = (page("a"), page("b"), page("c"));
    let dir = site(&[
        ("index.html", &a),
        ("about/index.html", &b),
        ("pricing.html", &c),
        ("app.js", "x"),
    ]);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let before = tree.files.len();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &fake).unwrap();
    assert_eq!(tree.files.len(), before + 3);
    assert_eq!((guard.generated, guard.skipped), (3, 0));
    assert!(guard.warning().is_none());
    let all = paths(&tree);
    for p in ["/index.md", "/about/index.md", "/pricing.md"] {
        assert!(all.contains(&p.to_string()), "{p} in {all:?}");
    }
    let entry = tree.files.iter().find(|f| f.path == "/index.md").unwrap();
    assert_eq!(entry.r#type, "text/markdown; charset=utf-8");
    let bytes = fs::read(&tree.blobs_by_hash[&entry.hash].path).unwrap();
    assert_eq!(sha256_hex(&bytes), entry.hash);
    assert_eq!(bytes.len(), entry.size);
    assert!(String::from_utf8(bytes).unwrap().contains("# Hello"));
}

#[test]
fn page_url_is_passed_to_the_converter_when_origin_is_known() {
    let (a, b, c) = (page("a"), page("b"), page("c"));
    let dir = site(&[
        ("index.html", &a),
        ("about/index.html", &b),
        ("pricing.html", &c),
    ]);
    let mut with = scan_output_dir(dir.path()).unwrap();
    let _g1 =
        add_markdown_siblings(dir.path(), &mut with, Some("https://example.com/"), &fake).unwrap();
    assert!(read_md(&with, "/index.md").contains("url: https://example.com/\n"));
    assert!(read_md(&with, "/about/index.md").contains("url: https://example.com/about/\n"));
    assert!(read_md(&with, "/pricing.md").contains("url: https://example.com/pricing\n"));
    let mut without = scan_output_dir(dir.path()).unwrap();
    let _g2 = add_markdown_siblings(dir.path(), &mut without, None, &fake).unwrap();
    assert!(read_md(&without, "/index.md").contains("url: -\n"));
}

#[test]
fn existing_md_is_never_overwritten() {
    let (p, q) = (page("a"), page("b"));
    let dir = site(&[
        ("index.html", &p),
        ("index.md", "hand written"),
        ("a.html", &q),
    ]);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let hand_hash = tree
        .files
        .iter()
        .find(|f| f.path == "/index.md")
        .unwrap()
        .hash
        .clone();
    let _g = add_markdown_siblings(dir.path(), &mut tree, None, &fake).unwrap();
    let md: Vec<_> = tree
        .files
        .iter()
        .filter(|f| f.path == "/index.md")
        .collect();
    assert_eq!(md.len(), 1);
    assert_eq!(md[0].hash, hand_hash);
    assert!(paths(&tree).contains(&"/a.md".to_string()));
}

#[test]
fn conversion_failure_skips_that_page_only() {
    let good = page("good");
    let dir = site(&[
        ("good.html", &good),
        ("empty.html", "<html><body></body></html>"),
        ("refused.html", "<h1>FAIL</h1>"),
    ]);
    fs::write(dir.path().join("binary.html"), [0xff, 0xfe, 0x00, 0x80]).unwrap();
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &fake).unwrap();
    let all = paths(&tree);
    assert!(all.contains(&"/good.md".to_string()));
    for skipped in ["/empty.md", "/binary.md", "/refused.md"] {
        assert!(!all.contains(&skipped.to_string()), "{skipped}");
    }
    assert_eq!((guard.generated, guard.skipped), (1, 3));
}

#[test]
fn respects_ignore_rules() {
    let (k, s) = (page("k"), page("s"));
    let dir = site(&[
        ("keep.html", &k),
        ("secret.html", &s),
        (".appzignore", "secret.html\n"),
    ]);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let _g = add_markdown_siblings(dir.path(), &mut tree, None, &fake).unwrap();
    let all = paths(&tree);
    assert!(all.contains(&"/keep.md".to_string()));
    assert!(!all.contains(&"/secret.md".to_string()), "{all:?}");
}

#[test]
fn deterministic_hashes() {
    let (a, b) = (page("a"), page("b"));
    let dir = site(&[("index.html", &a), ("b/index.html", &b)]);
    let run = || {
        let mut t = scan_output_dir(dir.path()).unwrap();
        let g = add_markdown_siblings(dir.path(), &mut t, None, &fake).unwrap();
        let mut h: Vec<_> = t
            .files
            .iter()
            .map(|f| (f.path.clone(), f.hash.clone()))
            .collect();
        h.sort();
        drop(g);
        h
    };
    assert_eq!(run(), run());
}

#[test]
fn guard_keeps_blob_files_alive_until_dropped() {
    let a = page("a");
    let dir = site(&[("index.html", &a)]);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &fake).unwrap();
    let e = tree.files.iter().find(|f| f.path == "/index.md").unwrap();
    let path = tree.blobs_by_hash[&e.hash].path.clone();
    assert!(path.exists());
    drop(guard);
    assert!(!path.exists());
}

#[test]
fn oversized_html_is_never_sent_and_counts_as_skipped() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("big.html"),
        format!("<h1>x</h1>{}", "a".repeat(MAX_HTML_BYTES)),
    )
    .unwrap();
    fs::write(dir.path().join("ok.html"), page("ok")).unwrap();
    let calls = AtomicUsize::new(0);
    let counting = |h: &[u8], u: Option<&str>| {
        calls.fetch_add(1, Ordering::SeqCst);
        fake(h, u)
    };
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &counting).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!((guard.generated, guard.skipped), (1, 1));
    let all = paths(&tree);
    assert!(all.contains(&"/ok.md".to_string()) && !all.contains(&"/big.md".to_string()));
}

#[test]
fn identical_pages_are_converted_once() {
    let p = page("same");
    let dir = site(&[
        ("a.html", &p),
        ("b.html", &p),
        ("c/index.html", &p),
        ("d.html", &p),
        ("e.html", &p),
    ]);
    let calls = AtomicUsize::new(0);
    let counting = |h: &[u8], u: Option<&str>| {
        calls.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(30));
        fake(h, u)
    };
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &counting).unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "one conversion for five identical pages"
    );
    assert_eq!(guard.generated, 5);
    assert_eq!(
        paths(&tree).iter().filter(|p| p.ends_with(".md")).count(),
        5
    );
}

#[test]
fn at_most_four_conversions_run_in_parallel() {
    let pages: Vec<String> = (0..16).map(|i| page(&i.to_string())).collect();
    let files: Vec<(String, &str)> = pages
        .iter()
        .enumerate()
        .map(|(i, p)| (format!("p{i}.html"), p.as_str()))
        .collect();
    let refs: Vec<(&str, &str)> = files.iter().map(|(n, p)| (n.as_str(), *p)).collect();
    let dir = site(&refs);
    let (now, peak) = (AtomicUsize::new(0), AtomicUsize::new(0));
    let slow = |h: &[u8], u: Option<&str>| {
        let n = now.fetch_add(1, Ordering::SeqCst) + 1;
        peak.fetch_max(n, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(25));
        now.fetch_sub(1, Ordering::SeqCst);
        fake(h, u)
    };
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &slow).unwrap();
    assert_eq!(guard.generated, 16);
    assert!(
        peak.load(Ordering::SeqCst) <= 4,
        "peak {}",
        peak.load(Ordering::SeqCst)
    );
}

#[test]
fn warning_aggregates_skipped_pages_with_the_first_error() {
    let dir = site(&[
        ("a.html", "<h1>FAIL a</h1>"),
        ("b.html", "<h1>FAIL b</h1>"),
        ("c.html", "<h1>FAIL c</h1>"),
    ]);
    let failing = |_: &[u8], _: Option<&str>| -> Result<String, ConvertError> {
        Err(ConvertError::new("boom"))
    };
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &failing).unwrap();
    assert_eq!(guard.skipped, 3);
    let w = guard.warning().unwrap();
    assert!(
        w.contains("3 pages") && w.contains("first error: boom"),
        "{w}"
    );

    let one = site(&[("a.html", "<h1>FAIL</h1>")]);
    let mut t = scan_output_dir(one.path()).unwrap();
    let w = add_markdown_siblings(one.path(), &mut t, None, &failing)
        .unwrap()
        .warning()
        .unwrap();
    assert!(w.contains("1 page;"), "{w}");
}

#[test]
fn conversion_never_fails_the_deploy() {
    let (a, b) = (page("a"), page("b"));
    let dir = site(&[("a.html", &a), ("b.html", &b), ("app.js", "x")]);
    let failing = |_: &[u8], _: Option<&str>| -> Result<String, ConvertError> {
        Err(ConvertError::new("down"))
    };
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let before = paths(&tree);
    let guard =
        add_markdown_siblings(dir.path(), &mut tree, None, &failing).expect("never an error");
    assert_eq!(guard.generated, 0);
    assert_eq!(paths(&tree), before, "tree untouched when every page fails");
    // an unreachable hosted converter behaves the same
    let http = HttpMarkdownConverter::new("http://127.0.0.1:1", None);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &http).expect("never an error");
    assert_eq!((guard.generated, guard.skipped), (0, 2));
    assert!(guard.warning().unwrap().contains("unreachable"));
}

// ---- hosted converter over a local mock server ----

type Log = Arc<Mutex<Vec<String>>>;

/// Serves every connection on a background thread (it ends with the test
/// process); `respond(request_head, request_index)` -> (status, json body).
fn mock_server(respond: impl Fn(&str, usize) -> (u16, String) + Send + 'static) -> (String, Log) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let log: Log = Arc::default();
    let seen = log.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut sock) = stream else { break };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let (head_end, len) = loop {
                let n = sock.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break (buf.len(), 0);
                }
                buf.extend_from_slice(&chunk[..n]);
                if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..p]).to_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length: "))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    break (p + 4, len);
                }
            };
            while buf.len() < head_end + len {
                let n = sock.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            let head = String::from_utf8_lossy(&buf[..head_end.min(buf.len())]).into_owned();
            let index = {
                let mut l = seen.lock().unwrap();
                l.push(head.clone());
                l.len() - 1
            };
            let (status, body) = respond(&head, index);
            let _ = write!(
                sock,
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (base, log)
}

const OK_BODY: &str = r##"{"markdown":"# Hosted\n","tokens":3,"mode":"minimal"}"##;

#[test]
fn http_converter_posts_html_with_auth_and_origin_header() {
    let (base, log) = mock_server(|_, _| (200, OK_BODY.into()));
    let c = HttpMarkdownConverter::new(&base, Some("tok123"));
    assert_eq!(
        c.convert(b"<h1>x</h1>", Some("https://example.com/a"))
            .unwrap(),
        "# Hosted\n"
    );
    let head = log.lock().unwrap()[0].to_lowercase();
    assert!(head.starts_with("post /api/v0/markdown/convert "), "{head}");
    assert!(head.contains("authorization: bearer tok123"));
    assert!(head.contains("content-type: text/html; charset=utf-8"));
    assert!(head.contains("x-page-origin: https://example.com/a"));

    c.convert(b"<h1>x</h1>", None).unwrap();
    assert!(
        !log.lock().unwrap()[1]
            .to_lowercase()
            .contains("x-page-origin")
    );
}

#[test]
fn http_403_says_the_feature_needs_enabling() {
    let (base, log) = mock_server(|_, _| (403, r#"{"error":"not entitled"}"#.into()));
    let c = HttpMarkdownConverter::new(&base, Some("t"));
    let err = c.convert(b"<h1>x</h1>", None).unwrap_err();
    assert_eq!(err.status, Some(403));
    assert!(
        err.message
            .contains("needs to be enabled for your organization"),
        "{err}"
    );
    assert_eq!(log.lock().unwrap().len(), 1, "403 is not retried");

    // and it reaches the deploy-level warning
    let p = page("a");
    let dir = site(&[("a.html", &p)]);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &c).unwrap();
    assert!(
        guard
            .warning()
            .unwrap()
            .contains("needs to be enabled for your organization")
    );
}

#[test]
fn http_4xx_is_not_retried() {
    for status in [401u16, 413, 422, 429] {
        let (base, log) = mock_server(move |_, _| (status, r#"{"error":"unconvertible"}"#.into()));
        let c = HttpMarkdownConverter::new(&base, None);
        let err = c.convert(b"<h1>x</h1>", None).unwrap_err();
        assert_eq!(err.status, Some(status));
        assert_eq!(
            log.lock().unwrap().len(),
            1,
            "HTTP {status} must not be retried"
        );
    }
}

#[test]
fn http_5xx_is_retried_exactly_once() {
    let (base, log) = mock_server(|_, i| {
        if i == 0 {
            (503, "{}".into())
        } else {
            (200, OK_BODY.into())
        }
    });
    let c = HttpMarkdownConverter::new(&base, None);
    assert_eq!(c.convert(b"<h1>x</h1>", None).unwrap(), "# Hosted\n");
    assert_eq!(log.lock().unwrap().len(), 2);

    let (base, log) = mock_server(|_, _| (503, "{}".into()));
    let c = HttpMarkdownConverter::new(&base, None);
    assert_eq!(
        c.convert(b"<h1>x</h1>", None).unwrap_err().status,
        Some(503)
    );
    assert_eq!(log.lock().unwrap().len(), 2, "one retry, no more");
}

#[test]
fn http_null_or_empty_markdown_is_an_error() {
    let (base, _) = mock_server(|_, _| {
        (
            200,
            r#"{"markdown":null,"tokens":0,"mode":"minimal"}"#.into(),
        )
    });
    let c = HttpMarkdownConverter::new(&base, None);
    assert!(c.convert(b"<h1>x</h1>", None).is_err());
}
