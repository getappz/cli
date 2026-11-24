use appz_deploy::markdown::{
    ConvertError, HttpMarkdownConverter, MAX_HTML_BYTES, MarkdownConverter, add_markdown_siblings,
    markdown_sibling_path,
};
use appz_deploy::read_markdown_config;
use appz_deploy::static_cas::{ScannedTree, scan_output_dir};
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
    assert_eq!(format!("b3:{}", blake3::hash(&bytes)), entry.hash);
    assert_eq!(bytes.len(), entry.size);
    assert!(String::from_utf8(bytes).unwrap().contains("# Hello"));
}

#[test]
fn page_url_is_passed_to_the_converter_when_origin_is_known() {
    let a = page("same");
    let dir = site(&[
        ("index.html", &a),
        ("about/index.html", &a),
        ("pricing.html", &a),
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
    assert!(head.starts_with("post /v0/markdown/convert "), "{head}");
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

// ---- batch converter over a local mock server ----

type ReqLog = Arc<Mutex<Vec<(String, String)>>>;

fn mock_batch_server(
    respond: impl Fn(&str, &str, usize) -> (u16, Vec<(&'static str, &'static str)>, String)
    + Send
    + Sync
    + 'static,
) -> (String, ReqLog) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let log: ReqLog = Arc::default();
    let seen = log.clone();
    let respond = Arc::new(respond);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut sock) = stream else { break };
            let seen = seen.clone();
            let respond = respond.clone();
            std::thread::spawn(move || {
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
                let body = String::from_utf8_lossy(
                    &buf[head_end.min(buf.len())..(head_end + len).min(buf.len())],
                )
                .into_owned();
                let index = {
                    let mut l = seen.lock().unwrap();
                    l.push((head.clone(), body.clone()));
                    l.len() - 1
                };
                let (status, headers, resp_body) = respond(&head, &body, index);
                let mut extra = String::new();
                for (k, v) in headers {
                    extra.push_str(&format!("{k}: {v}\r\n"));
                }
                let _ = write!(
                    sock,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{resp_body}",
                    resp_body.len()
                );
            });
        }
    });
    (base, log)
}

#[test]
fn batch_packs_by_count_maps_by_id_dedupes_hashes_and_caps_concurrency() {
    // 27 unique pages + 3 duplicates of page 0 -> 27 unique pages sent across 2 batches (25 + 2).
    let mut files = Vec::new();
    let mut contents = Vec::new();
    for i in 0..27 {
        contents.push((format!("p{i:02}.html"), page(&i.to_string())));
    }
    let dup = page("0");
    for d in 0..3 {
        contents.push((format!("dup{d}.html"), dup.clone()));
    }
    for (p, b) in &contents {
        files.push((p.as_str(), b.as_str()));
    }
    let dir = site(&files);

    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let (active_c, peak_c) = (active.clone(), peak.clone());

    let (base, log) = mock_batch_server(move |head, body, _| {
        assert!(
            head.to_lowercase()
                .starts_with("post /v0/markdown/convert-batch ")
        );
        assert!(head.to_lowercase().contains("authorization: bearer tok"));
        let cur = active_c.fetch_add(1, Ordering::SeqCst) + 1;
        peak_c.fetch_max(cur, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(25));

        let req: serde_json::Value = serde_json::from_str(body).unwrap();
        let pages = req["pages"].as_array().unwrap();
        // Return results in REVERSE order to verify ID-based (not positional) mapping.
        let results: Vec<serde_json::Value> = pages
            .iter()
            .rev()
            .map(|p| {
                let id = p["id"].as_str().unwrap();
                let html = p["html"].as_str().unwrap();
                let origin = p.get("origin").and_then(|o| o.as_str()).unwrap_or("-");
                serde_json::json!({
                    "id": id,
                    "status": 200,
                    "markdown": format!("# Converted {html} @ {origin}\n"),
                    "tokens": 5,
                    "mode": "minimal",
                })
            })
            .collect();
        active_c.fetch_sub(1, Ordering::SeqCst);
        let resp = serde_json::json!({ "results": results });
        (200, vec![], resp.to_string())
    });

    let c = HttpMarkdownConverter::new(&base, Some("tok"));
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &c).unwrap();
    assert_eq!((guard.generated, guard.skipped), (30, 0));
    assert!(guard.warning().is_none());
    assert!(
        peak.load(Ordering::SeqCst) <= 2,
        "peak={}",
        peak.load(Ordering::SeqCst)
    );

    let calls = log.lock().unwrap();
    assert_eq!(calls.len(), 2);
    let mut counts: Vec<usize> = calls
        .iter()
        .map(|(_, b)| {
            serde_json::from_str::<serde_json::Value>(b).unwrap()["pages"]
                .as_array()
                .unwrap()
                .len()
        })
        .collect();
    counts.sort();
    assert_eq!(counts, vec![2, 25]);
    assert!(read_md(&tree, "/p07.md").contains("Page 7"));
    assert!(read_md(&tree, "/dup2.md").contains("Page 0"));
}

#[test]
fn batch_packs_by_serialized_size_and_isolates_oversized_escaped_page() {
    // 2 pages of ~1.9 MB each -> ~3.8 MB combined > 3.5 MB target -> must split into 2 batches.
    // Plus 1 page of ~1.9 MB full of `"` (escapes to ~3.8 MB > 3.5 MB target) -> isolated in its own batch.
    let big_plain = format!("<h1>plain</h1>{}", "a".repeat(1_850_000));
    let big_quotes = format!("<h1>q</h1>{}", "\"".repeat(1_850_000));
    let dir = site(&[
        ("a.html", &big_plain),
        ("b.html", &format!("<h1>b</h1>{}", "b".repeat(1_850_000))),
        ("q.html", &big_quotes),
    ]);

    let (base, log) = mock_batch_server(|_, body, _| {
        let req: serde_json::Value = serde_json::from_str(body).unwrap();
        let results: Vec<_> = req["pages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p["id"],
                    "status": 200,
                    "markdown": "# ok\n",
                    "tokens": 2,
                    "mode": "minimal",
                })
            })
            .collect();
        (
            200,
            vec![],
            serde_json::json!({ "results": results }).to_string(),
        )
    });

    let c = HttpMarkdownConverter::new(&base, None);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &c).unwrap();
    assert_eq!((guard.generated, guard.skipped), (3, 0));
    let calls = log.lock().unwrap();
    assert_eq!(
        calls.len(),
        3,
        "each ~1.9MB/escaped page isolated under 3.5MB"
    );
    for (_, b) in calls.iter() {
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(b).unwrap()["pages"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}

#[test]
fn batch_handles_partial_failures_and_retries_503_once() {
    let dir = site(&[
        ("a_ok.html", &page("ok")),
        ("b_422.html", &page("bad422")),
        ("c_503_recover.html", &page("retry_ok")),
        ("d_503_stay.html", &page("retry_fail")),
    ]);

    let (base, log) = mock_batch_server(|_, body, idx| {
        let req: serde_json::Value = serde_json::from_str(body).unwrap();
        let results: Vec<_> = req["pages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                let id = p["id"].as_str().unwrap();
                let html = p["html"].as_str().unwrap();
                if html.contains("bad422") {
                    serde_json::json!({ "id": id, "status": 422, "error": "unconvertible" })
                } else if html.contains("retry_ok") && idx == 0 {
                    serde_json::json!({ "id": id, "status": 503, "error": "timeout" })
                } else if html.contains("retry_fail") {
                    serde_json::json!({ "id": id, "status": 503, "error": "still down" })
                } else {
                    serde_json::json!({ "id": id, "status": 200, "markdown": "# Recovered\n", "tokens": 2, "mode": "minimal" })
                }
            })
            .collect();
        (
            200,
            vec![],
            serde_json::json!({ "results": results }).to_string(),
        )
    });

    let c = HttpMarkdownConverter::new(&base, None);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &c).unwrap();
    assert_eq!((guard.generated, guard.skipped), (2, 2));
    assert_eq!(log.lock().unwrap().len(), 2, "wave 1 + one 503 retry wave");
    let all = paths(&tree);
    assert!(all.contains(&"/a_ok.md".to_string()));
    assert!(all.contains(&"/c_503_recover.md".to_string()));
    assert!(!all.contains(&"/b_422.md".to_string()));
    assert!(!all.contains(&"/d_503_stay.md".to_string()));
}

#[test]
fn batch_403_and_429_stop_remaining_batches_and_emit_one_warning() {
    // 26 unique pages -> 2 batches; batch 0 returns 403 -> batch 1 must not be sent.
    let contents: Vec<(String, String)> = (0..26)
        .map(|i| (format!("p{i:02}.html"), page(&i.to_string())))
        .collect();
    let files: Vec<(&str, &str)> = contents
        .iter()
        .map(|(p, b)| (p.as_str(), b.as_str()))
        .collect();
    let dir = site(&files);

    let (base_403, log_403) =
        mock_batch_server(|_, _, _| (403, vec![], r#"{"error":"not entitled"}"#.into()));
    let c = HttpMarkdownConverter::new(&base_403, Some("tok"));
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &c).unwrap();
    assert_eq!((guard.generated, guard.skipped), (0, 26));
    assert!(log_403.lock().unwrap().len() <= 2);
    let w = guard.warning().unwrap();
    assert!(w.contains("26 pages") && w.contains("HTTP 403"), "{w}");

    // 429 with Retry-After: 0 waits and retries once; if still 429, stops cleanly with one warning.
    let (base_429, log_429) = mock_batch_server(|_, _, _| {
        (
            429,
            vec![("Retry-After", "0")],
            r#"{"error":"slow down"}"#.into(),
        )
    });
    let c = HttpMarkdownConverter::new(&base_429, Some("tok"));
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &c).unwrap();
    assert_eq!((guard.generated, guard.skipped), (0, 26));
    assert!(log_429.lock().unwrap().len() <= 4); // each in-flight batch may retry once
    let w = guard.warning().unwrap();
    assert!(w.contains("26 pages") && w.contains("HTTP 429"), "{w}");
}

#[test]
fn batch_404_falls_back_to_single_page_endpoint() {
    let (a, b) = (page("a"), page("b"));
    let dir = site(&[("a.html", &a), ("b.html", &b)]);
    let (base, log) = mock_batch_server(|head, _, _| {
        if head
            .to_lowercase()
            .starts_with("post /v0/markdown/convert-batch ")
        {
            (404, vec![], r#"{"error":"not found"}"#.into())
        } else {
            (200, vec![], OK_BODY.into())
        }
    });
    let c = HttpMarkdownConverter::new(&base, None);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &c).unwrap();
    assert_eq!((guard.generated, guard.skipped), (2, 0));
    let heads: Vec<String> = log
        .lock()
        .unwrap()
        .iter()
        .map(|(h, _)| h.lines().next().unwrap().to_lowercase())
        .collect();
    assert!(heads[0].starts_with("post /v0/markdown/convert-batch "));
    assert_eq!(
        heads
            .iter()
            .filter(|h| h.starts_with("post /v0/markdown/convert "))
            .count(),
        2
    );
}

#[test]
fn batch_fallback_keeps_pages_converted_before_a_fatal_status() {
    let dir = site(&[
        ("a.html", &page("a")),
        ("b.html", &page("b")),
        ("c.html", &page("boom")),
        ("d.html", &page("d")),
    ]);
    let (base, log) = mock_batch_server(|head, body, _| {
        if head
            .to_lowercase()
            .starts_with("post /v0/markdown/convert-batch ")
        {
            (404, vec![], r#"{"error":"not found"}"#.into())
        } else if body.contains("boom") {
            (429, vec![], r#"{"error":"slow down"}"#.into())
        } else {
            (200, vec![], OK_BODY.into())
        }
    });
    let c = HttpMarkdownConverter::new(&base, None);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &c).unwrap();
    assert_eq!((guard.generated, guard.skipped), (2, 2));
    let all = paths(&tree);
    assert!(all.contains(&"/a.md".to_string()) && all.contains(&"/b.md".to_string()));
    assert!(guard.warning().unwrap().contains("HTTP 429"));
    // batch attempt + a + b + boom; `d` is never sent after the fatal 429.
    assert_eq!(log.lock().unwrap().len(), 4);
}

#[test]
fn batch_5xx_other_than_503_is_not_retried() {
    let dir = site(&[("a.html", &page("a"))]);
    let (base, log) = mock_batch_server(|_, _, _| (504, vec![], r#"{"error":"gateway"}"#.into()));
    let c = HttpMarkdownConverter::new(&base, None);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &c).unwrap();
    assert_eq!((guard.generated, guard.skipped), (0, 1));
    assert_eq!(log.lock().unwrap().len(), 1);
}

#[test]
fn batch_whole_503_is_retried_once_in_call() {
    let dir = site(&[("a.html", &page("a"))]);
    let (base, log) = mock_batch_server(|_, body, idx| {
        if idx == 0 {
            return (503, vec![], r#"{"error":"busy"}"#.into());
        }
        let req: serde_json::Value = serde_json::from_str(body).unwrap();
        let id = req["pages"][0]["id"].clone();
        let r = serde_json::json!({"results":[{"id":id,"status":200,"markdown":"# ok\n"}]});
        (200, vec![], r.to_string())
    });
    let c = HttpMarkdownConverter::new(&base, None);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &c).unwrap();
    assert_eq!((guard.generated, guard.skipped), (1, 0));
    assert_eq!(log.lock().unwrap().len(), 2);
}

/// 429 once per distinct first page (i.e. once per batch), then success.
fn rate_limit_each_batch_once(retry_after: &'static str) -> (String, ReqLog) {
    let seen = Mutex::new(std::collections::HashSet::<String>::new());
    mock_batch_server(move |_, body, _| {
        let req: serde_json::Value = serde_json::from_str(body).unwrap();
        let first = req["pages"][0]["html"].as_str().unwrap().to_owned();
        if seen.lock().unwrap().insert(first) {
            return (
                429,
                vec![("Retry-After", retry_after)],
                r#"{"error":"slow"}"#.into(),
            );
        }
        let results: Vec<_> = req["pages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| serde_json::json!({"id": p["id"], "status": 200, "markdown": "# ok\n"}))
            .collect();
        (
            200,
            vec![],
            serde_json::json!({ "results": results }).to_string(),
        )
    })
}

fn many_pages_site(n: usize) -> TempDir {
    let contents: Vec<(String, String)> = (0..n)
        .map(|i| (format!("p{i:03}.html"), page(&i.to_string())))
        .collect();
    let files: Vec<(&str, &str)> = contents
        .iter()
        .map(|(p, b)| (p.as_str(), b.as_str()))
        .collect();
    site(&files)
}

#[test]
fn batch_honors_a_later_rate_limit_window() {
    // 51 pages -> 3 batches; each gets one 429 (Retry-After: 0) then succeeds.
    let dir = many_pages_site(51);
    let (base, _log) = rate_limit_each_batch_once("0");
    let c = HttpMarkdownConverter::new(&base, None);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &c).unwrap();
    assert_eq!((guard.generated, guard.skipped), (51, 0));
}

#[test]
fn batch_retry_after_http_date_is_honored() {
    let dir = many_pages_site(3);
    let (base, log) = rate_limit_each_batch_once("Wed, 21 Oct 2015 07:28:00 GMT");
    let c = HttpMarkdownConverter::new(&base, None);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &c).unwrap();
    assert_eq!((guard.generated, guard.skipped), (3, 0));
    assert_eq!(log.lock().unwrap().len(), 2);
}

#[test]
fn batch_retry_after_beyond_the_cap_is_not_retried_early() {
    let dir = many_pages_site(3);
    let (base, log) = rate_limit_each_batch_once("300");
    let c = HttpMarkdownConverter::new(&base, None);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &c).unwrap();
    assert_eq!((guard.generated, guard.skipped), (0, 3));
    assert_eq!(log.lock().unwrap().len(), 1);
    assert!(guard.warning().unwrap().contains("HTTP 429"));
}

#[test]
fn batch_page_too_large_once_encoded_is_skipped_locally() {
    // 2.05 MB of quotes is under MAX_HTML_BYTES but JSON-escapes past ~4 MB.
    let huge = format!("<h1>q</h1>{}", "\"".repeat(2_050_000));
    assert!(huge.len() < MAX_HTML_BYTES);
    let dir = site(&[("q.html", &huge), ("ok.html", &page("ok"))]);
    let (base, log) = mock_batch_server(|_, body, _| {
        let req: serde_json::Value = serde_json::from_str(body).unwrap();
        let results: Vec<_> = req["pages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| serde_json::json!({"id": p["id"], "status": 200, "markdown": "# ok\n"}))
            .collect();
        (
            200,
            vec![],
            serde_json::json!({ "results": results }).to_string(),
        )
    });
    let c = HttpMarkdownConverter::new(&base, None);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None, &c).unwrap();
    assert_eq!((guard.generated, guard.skipped), (1, 1));
    assert!(!paths(&tree).contains(&"/q.md".to_string()));
    assert_eq!(log.lock().unwrap().len(), 1, "only the small page is sent");
}
