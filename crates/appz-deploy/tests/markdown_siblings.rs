use appz_deploy::markdown::{add_markdown_siblings, markdown_sibling_path};
use appz_deploy::read_markdown_config;
use appz_deploy::static_cas::{ScannedTree, scan_output_dir, sha256_hex};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

const PAGE: &str = "<html><head><title>T</title></head><body><main><h1>Hello</h1><p>See <a href=\"/docs\">docs</a>.</p></main></body></html>";

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
    let dir = site(&[
        ("index.html", PAGE),
        ("about/index.html", PAGE),
        ("pricing.html", PAGE),
        ("app.js", "x"),
    ]);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let before = tree.files.len();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None).unwrap();
    assert_eq!(tree.files.len(), before + 3);
    let all = paths(&tree);
    for p in ["/index.md", "/about/index.md", "/pricing.md"] {
        assert!(all.contains(&p.to_string()), "{p} in {all:?}");
    }
    let entry = tree.files.iter().find(|f| f.path == "/index.md").unwrap();
    assert_eq!(entry.r#type, "text/markdown; charset=utf-8");
    let blob = &tree.blobs_by_hash[&entry.hash];
    let bytes = fs::read(&blob.path).unwrap();
    assert_eq!(sha256_hex(&bytes), entry.hash);
    assert_eq!(bytes.len(), entry.size);
    assert!(String::from_utf8(bytes).unwrap().contains("# Hello"));
    drop(guard);
}

#[test]
fn origin_resolves_links_and_none_keeps_them_relative() {
    let dir = site(&[("index.html", PAGE)]);
    let origin = url::Url::parse("https://example.com").unwrap();
    let mut with = scan_output_dir(dir.path()).unwrap();
    let _g1 = add_markdown_siblings(dir.path(), &mut with, Some(&origin)).unwrap();
    let mut without = scan_output_dir(dir.path()).unwrap();
    let _g2 = add_markdown_siblings(dir.path(), &mut without, None).unwrap();
    let read = |t: &ScannedTree| {
        let e = t.files.iter().find(|f| f.path == "/index.md").unwrap();
        fs::read_to_string(&t.blobs_by_hash[&e.hash].path).unwrap()
    };
    assert!(read(&with).contains("(https://example.com/docs)"));
    let rel = read(&without);
    assert!(rel.contains("(/docs)") && !rel.contains("http"), "{rel}");
}

#[test]
fn existing_md_is_never_overwritten() {
    let dir = site(&[
        ("index.html", PAGE),
        ("index.md", "hand written"),
        ("a.html", PAGE),
    ]);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let hand_hash = tree
        .files
        .iter()
        .find(|f| f.path == "/index.md")
        .unwrap()
        .hash
        .clone();
    let _g = add_markdown_siblings(dir.path(), &mut tree, None).unwrap();
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
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("good.html"), PAGE).unwrap();
    fs::write(dir.path().join("empty.html"), "<html><body></body></html>").unwrap();
    fs::write(dir.path().join("binary.html"), [0xff, 0xfe, 0x00, 0x80]).unwrap();
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let _g = add_markdown_siblings(dir.path(), &mut tree, None).unwrap();
    let all = paths(&tree);
    assert!(all.contains(&"/good.md".to_string()));
    assert!(!all.contains(&"/empty.md".to_string()));
    assert!(!all.contains(&"/binary.md".to_string()));
}

#[test]
fn respects_ignore_rules() {
    let dir = site(&[
        ("keep.html", PAGE),
        ("secret.html", PAGE),
        (".appzignore", "secret.html\n"),
    ]);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let _g = add_markdown_siblings(dir.path(), &mut tree, None).unwrap();
    let all = paths(&tree);
    assert!(all.contains(&"/keep.md".to_string()));
    assert!(!all.contains(&"/secret.md".to_string()), "{all:?}");
}

#[test]
fn deterministic_hashes() {
    let dir = site(&[("index.html", PAGE), ("b/index.html", PAGE)]);
    let run = || {
        let mut t = scan_output_dir(dir.path()).unwrap();
        let g = add_markdown_siblings(dir.path(), &mut t, None).unwrap();
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
    let dir = site(&[("index.html", PAGE)]);
    let mut tree = scan_output_dir(dir.path()).unwrap();
    let guard = add_markdown_siblings(dir.path(), &mut tree, None).unwrap();
    let e = tree.files.iter().find(|f| f.path == "/index.md").unwrap();
    let path = tree.blobs_by_hash[&e.hash].path.clone();
    assert!(path.exists());
    drop(guard);
    assert!(!path.exists());
}
