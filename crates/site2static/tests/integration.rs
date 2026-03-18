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
