//! Benchmarks for search: line mapping, search_packed (internal logic + grep).
//! Run with: cargo bench -p code-mix --bench search
//!
//! Note: search_packed invokes ripgrep; repomix run itself is excluded per design.

use code_grep::SearchRequest;
use code_mix::search_packed;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Minimal pack content with ## File: headers for benchmarking line mapping + grep.
fn sample_pack_content() -> String {
    let mut s = String::with_capacity(2000);
    for i in 0..20 {
        s.push_str(&format!("## File: src/file_{}.rs\n", i));
        s.push_str("fn main() {\n");
        s.push_str("    let x = 42;\n");
        s.push_str("    println!(\"{}\", x);\n");
        s.push_str("}\n\n");
    }
    s
}

fn bench_search_packed(c: &mut Criterion) {
    let pack = sample_pack_content();
    let dir = tempfile::tempdir().unwrap();
    let pack_path = dir.path().join("pack.md");
    std::fs::write(&pack_path, &pack).unwrap();

    let req = SearchRequest {
        query: "println".into(),
        is_regex: Some(false),
        file_glob: None,
        max_results: Some(50),
    };

    c.bench_function("search_packed_20_files", |b| {
        b.iter(|| {
            black_box(
                search_packed(black_box(&req), black_box(&pack_path)).unwrap(),
            );
        });
    });
}

fn bench_search_packed_large(c: &mut Criterion) {
    let mut pack = String::with_capacity(50_000);
    for i in 0..100 {
        pack.push_str(&format!("## File: src/module_{}/file.rs\n", i));
        pack.push_str("fn foo() { let x = 1; }\n");
        pack.push_str("fn bar() { let y = 2; }\n");
        pack.push_str("fn baz() { println!(\"hi\"); }\n\n");
    }

    let dir = tempfile::tempdir().unwrap();
    let pack_path = dir.path().join("pack.md");
    std::fs::write(&pack_path, &pack).unwrap();

    let req = SearchRequest {
        query: "println".into(),
        is_regex: Some(false),
        file_glob: None,
        max_results: Some(100),
    };

    c.bench_function("search_packed_100_files", |b| {
        b.iter(|| {
            black_box(
                search_packed(black_box(&req), black_box(&pack_path)).unwrap(),
            );
        });
    });
}

criterion_group!(benches, bench_search_packed, bench_search_packed_large);
criterion_main!(benches);
