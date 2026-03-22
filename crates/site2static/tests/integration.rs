use site2static::{MirrorConfig, SiteMirror, WebRoot};
use std::fs;
use std::thread;
use tempfile::TempDir;
use tiny_http::{Response, Server};
use url::Url;

fn serve_site(webroot: &std::path::Path) -> (String, thread::JoinHandle<()>) {
    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = format!("http://{}", server.server_addr().to_ip().unwrap());
    let webroot = webroot.to_path_buf();

    let handle = thread::spawn(move || {
        for _ in 0..50 {
            let request = match server.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(Some(r)) => r,
                _ => break,
            };
            let url_path = request.url().to_string();
            let file_path = if url_path == "/" || url_path.is_empty() {
                webroot.join("index.html")
            } else {
                let clean = url_path.trim_start_matches('/');
                let candidate = webroot.join(clean);
                if candidate.is_dir() {
                    candidate.join("index.html")
                } else if candidate.exists() {
                    candidate
                } else {
                    // Try as directory with index.html
                    let dir_candidate = webroot.join(clean).join("index.html");
                    if dir_candidate.exists() {
                        dir_candidate
                    } else {
                        candidate
                    }
                }
            };
            if file_path.exists() {
                let content = fs::read(&file_path).unwrap();
                let ct = match file_path.extension().and_then(|e| e.to_str()) {
                    Some("css") => "text/css",
                    Some("js") => "application/javascript",
                    Some("png") | Some("jpg") => "image/png",
                    _ => "text/html; charset=utf-8",
                };
                let resp = Response::from_data(content)
                    .with_header(tiny_http::Header::from_bytes("Content-Type", ct).unwrap());
                let _ = request.respond(resp);
            } else {
                let _ = request.respond(
                    Response::from_string("404 Not Found").with_status_code(404),
                );
            }
        }
    });

    (addr, handle)
}

#[test]
fn test_basic_mirror() {
    let site_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();

    // Create test site
    fs::write(
        site_dir.path().join("index.html"),
        r#"<html><head><link href="/style.css"></head><body><a href="/about/">About</a><img src="/logo.png"></body></html>"#,
    )
    .unwrap();

    fs::create_dir(site_dir.path().join("about")).unwrap();
    fs::write(
        site_dir.path().join("about/index.html"),
        r#"<html><body><a href="/">Home</a></body></html>"#,
    )
    .unwrap();

    fs::write(
        site_dir.path().join("style.css"),
        "body { color: red; }",
    )
    .unwrap();
    fs::write(site_dir.path().join("logo.png"), b"fake-png-data").unwrap();

    let (addr, _handle) = serve_site(site_dir.path());

    let config = MirrorConfig {
        origin: Url::parse(&addr).unwrap(),
        webroot: WebRoot::Direct(site_dir.path().to_path_buf()),
        output: output_dir.path().to_path_buf(),
        workers: 2,
        depth: None,
        force: true,
        exclude_patterns: vec![],
        include_patterns: vec![],
        copy_globs: vec![],
        search: None,
        on_progress: None,
    };

    let mirror = SiteMirror::new(config);
    let result = mirror.run().unwrap();

    // Verify pages were crawled
    assert!(
        result.pages_crawled >= 2,
        "Expected at least 2 pages, got {}",
        result.pages_crawled
    );

    // Verify output files exist
    assert!(
        output_dir.path().join("index.html").exists(),
        "index.html should exist"
    );
    assert!(
        output_dir.path().join("about/index.html").exists(),
        "about/index.html should exist"
    );

    // Verify assets were copied
    assert!(
        output_dir.path().join("style.css").exists(),
        "style.css should exist"
    );
    assert!(
        output_dir.path().join("logo.png").exists(),
        "logo.png should exist"
    );
}

#[test]
fn test_mirror_with_search_ui_injection() {
    let site_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();

    // Create test site with a WordPress search form
    fs::write(
        site_dir.path().join("index.html"),
        r#"<html><head><title>Test</title></head><body>
<main>
<h1>Welcome</h1>
<form class="search-form"><input type="search" name="s"><button>Search</button></form>
<a href="/about/">About</a>
</main>
</body></html>"#,
    )
    .unwrap();

    fs::create_dir(site_dir.path().join("about")).unwrap();
    fs::write(
        site_dir.path().join("about/index.html"),
        r#"<html><head><title>About</title></head><body><main><a href="/">Home</a></main></body></html>"#,
    )
    .unwrap();

    let (addr, _handle) = serve_site(site_dir.path());

    let config = MirrorConfig {
        origin: Url::parse(&addr).unwrap(),
        webroot: WebRoot::Direct(site_dir.path().to_path_buf()),
        output: output_dir.path().to_path_buf(),
        workers: 2,
        depth: None,
        force: true,
        exclude_patterns: vec![],
        include_patterns: vec![],
        copy_globs: vec![],
        search: Some(site2static::SearchMode::Full(site2static::SearchUiConfig::default())),
        on_progress: None,
    };

    let mirror = SiteMirror::new(config);
    let result = mirror.run();

    // If pagefind is not installed, the pre-check fails early
    match result {
        Ok(r) => assert!(r.pages_crawled >= 2),
        Err(site2static::MirrorError::SearchBinaryNotFound { .. }) => {
            // Expected in CI without pagefind installed — but HTML was never written
            // since pre-check happens before crawling. Skip assertions.
            return;
        }
        Err(e) => panic!("Unexpected error: {e}"),
    }

    // Verify search UI was injected
    let index_html = fs::read_to_string(output_dir.path().join("index.html")).unwrap();
    assert!(index_html.contains("pagefind-ui.js"), "should inject Pagefind JS");
    assert!(index_html.contains("pagefind-ui.css"), "should inject Pagefind CSS");
    assert!(index_html.contains("pagefind-replace-0"), "should replace WP search form");
    assert!(index_html.contains("s2s-search-overlay"), "should inject Cmd+K modal");
    assert!(index_html.contains("data-pagefind-body"), "should mark main content");

    // About page should also have search UI
    let about_html = fs::read_to_string(output_dir.path().join("about/index.html")).unwrap();
    assert!(about_html.contains("pagefind-ui.js"), "about page should have Pagefind JS");
    assert!(about_html.contains("s2s-search-overlay"), "about page should have modal");
}

#[test]
fn test_mirror_with_index_only_search() {
    let site_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();

    fs::write(
        site_dir.path().join("index.html"),
        r#"<html><head></head><body><form class="search-form"><input></form></body></html>"#,
    )
    .unwrap();

    let (addr, _handle) = serve_site(site_dir.path());

    let config = MirrorConfig {
        origin: Url::parse(&addr).unwrap(),
        webroot: WebRoot::Direct(site_dir.path().to_path_buf()),
        output: output_dir.path().to_path_buf(),
        workers: 2,
        depth: None,
        force: true,
        exclude_patterns: vec![],
        include_patterns: vec![],
        copy_globs: vec![],
        search: Some(site2static::SearchMode::IndexOnly),
        on_progress: None,
    };

    let mirror = SiteMirror::new(config);
    let result = mirror.run();

    match result {
        Ok(_) => {}
        Err(site2static::MirrorError::SearchBinaryNotFound { .. }) => return,
        Err(e) => panic!("Unexpected error: {e}"),
    }

    // IndexOnly: no UI injection
    let index_html = fs::read_to_string(output_dir.path().join("index.html")).unwrap();
    assert!(!index_html.contains("pagefind-ui.js"), "should NOT inject Pagefind JS in IndexOnly mode");
    assert!(!index_html.contains("s2s-search-overlay"), "should NOT inject modal in IndexOnly mode");
}
