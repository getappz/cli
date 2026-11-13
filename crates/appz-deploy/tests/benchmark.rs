use std::io::Write;
use std::time::Instant;
use tempfile::TempDir;

use appz_core::DetectorFilesystem;
use appz_deploy::format_env_file;
use appz_deploy::parse_env_file;
use appz_deploy::slugify_project_id;
use appz_deploy::static_cas::{guess_mime, is_immutable_asset, scan_output_dir};

fn create_sample_tree(count: usize) -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    for i in 0..count {
        let (sub, file, content) = match i % 6 {
            0 => (
                "pages",
                format!("page_{i}.html"),
                format!("<html><body>Page {i}</body></html>"),
            ),
            1 => (
                "assets",
                format!("bundle_{i}-a1b2c3d4.js"),
                format!("console.log('chunk {i}');\nlet x = {i};\n"),
            ),
            2 => (
                "assets",
                format!("style_{i}.css"),
                format!(".class_{i} {{ margin: {i}px; }}\n"),
            ),
            3 => (
                "static/images",
                format!("image_{i}.svg"),
                format!("<svg width=\"{i}\" height=\"{i}\"></svg>"),
            ),
            4 => (
                "fonts",
                format!("font_{i}.woff2"),
                format!("FONT_DATA_{i}_BYTES_PADDING_1234567890"),
            ),
            _ => (
                "data",
                format!("data_{i}.json"),
                format!("{{\"id\": {i}, \"status\": \"ok\"}}"),
            ),
        };
        let subdir = root.join(sub);
        std::fs::create_dir_all(&subdir).unwrap();
        let file_path = subdir.join(file);
        let mut f = std::fs::File::create(file_path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }
    dir
}

#[test]
fn bench_all() {
    println!("\n=================== BENCHMARK SUITE ===================");

    // 1. guess_mime (100,000 lookups)
    let paths = [
        "/assets/index.html",
        "/_astro/main.css",
        "/static/js/app-38a9bc12.js",
        "/images/logo.svg",
        "/images/banner.webp",
        "/fonts/inter.woff2",
        "/api/schema.json",
        "/wasm/engine.wasm",
        "/favicon.ico",
        "/videos/intro.mp4",
    ];
    let start = Instant::now();
    let iterations = 100_000;
    for i in 0..iterations {
        let p = paths[i % paths.len()];
        let _ = guess_mime(p);
    }
    let dur_mime = start.elapsed();
    println!("guess_mime (100,000 lookups): {:?}", dur_mime);

    // 2. is_immutable_asset (100,000 lookups)
    let test_paths = [
        "/_astro/client.073c9b0a.js",
        "/_next/static/chunks/framework.js",
        "/assets/vendor-b1x2c3D4.js",
        "/assets/style.css",
        "/static/image.png",
        "/static/image-1234abcd.png",
        "/pages/index.html",
        "/assets/main.chunk.css",
        "/static/bundle.hash.js",
        "/assets/logo-v2-xyz12345.svg",
    ];
    let start = Instant::now();
    for i in 0..iterations {
        let p = test_paths[i % test_paths.len()];
        let _ = is_immutable_asset(p);
    }
    let dur_immutable = start.elapsed();
    println!("is_immutable_asset (100,000 lookups): {:?}", dur_immutable);

    // 3. slugify_project_id (100,000 calls)
    let slugs = [
        "My Awesome App_v2.0!",
        "prj_already_prefixed_12345",
        "---special--characters---in---name---",
        "A Very Long Project Name That Definitely Exceeds Forty Characters In Total Length",
        "normal-slug-project",
    ];
    let start = Instant::now();
    for i in 0..iterations {
        let s = slugs[i % slugs.len()];
        let _ = slugify_project_id(s);
    }
    let dur_slug = start.elapsed();
    println!("slugify_project_id (100,000 calls): {:?}", dur_slug);

    // 4. format_env_file & parse_env_file (10,000 calls)
    let env_vars = vec![
        ("NODE_ENV".to_string(), "production".to_string()),
        ("API_URL".to_string(), "https://api.appz.dev/v0".to_string()),
        (
            "SECRET_KEY".to_string(),
            "foo=\"bar\"\\with\\quotes".to_string(),
        ),
        ("EMPTY_VAL".to_string(), "".to_string()),
        (
            "SPACED_VALUE".to_string(),
            "hello world from appz".to_string(),
        ),
    ];
    let start = Instant::now();
    let env_iters = 10_000;
    for _ in 0..env_iters {
        let formatted = format_env_file(&env_vars);
        let _parsed = parse_env_file(&formatted);
    }
    let dur_env = start.elapsed();
    println!("format & parse env (10,000 iterations): {:?}", dur_env);

    // 5. scan_output_dir (500 files directory tree)
    let sample = create_sample_tree(500);
    let sample_path = sample.path();

    // Warm-up run
    let _ = scan_output_dir(sample_path).unwrap();

    let start = Instant::now();
    let scan_iters = 10;
    for _ in 0..scan_iters {
        let scanned = scan_output_dir(sample_path).unwrap();
        assert_eq!(scanned.files.len(), 500);
    }
    let dur_scan = start.elapsed();
    println!("scan_output_dir 500 files (10 iterations): {:?}", dur_scan);
    println!(
        "scan_output_dir 500 files average per scan: {:?}",
        dur_scan / scan_iters
    );

    // 6. DetectorFilesystem probe caching (100,000 queries)
    let dfs = DetectorFilesystem::new(sample_path.to_path_buf());
    let probe_paths = [
        "pages/page_0.html",
        "assets/style_2.css",
        "package.json",
        "missing.txt",
        "data/data_5.json",
    ];
    let start = Instant::now();
    for i in 0..100_000 {
        let p = probe_paths[i % probe_paths.len()];
        let _ = dfs.has_path(p);
        let _ = dfs.is_file(p);
    }
    let dur_dfs = start.elapsed();
    println!("DetectorFilesystem cache queries (100,000): {:?}", dur_dfs);
    println!("=======================================================\n");
}
