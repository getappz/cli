//! Benchmarks for prefilter: path discovery from include/ignore patterns.
//! Run with: cargo bench -p code-mix --bench prefilter

use code_mix::{discover_paths_from_patterns, PackOptions};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn workdir_with_files(dir: &std::path::Path, n: usize) {
    for i in 0..n {
        let p = dir.join(format!("src/file_{}.rs", i));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, format!("fn f_{}() {{}}\n", i)).unwrap();
    }
    std::fs::write(dir.join("Cargo.toml"), "[package]\n").unwrap();
}

fn bench_discover_empty_include(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    workdir_with_files(dir.path(), 10);
    let options = PackOptions {
        workdir: dir.path().to_path_buf(),
        include: vec![],
        ignore: vec![".git".into()],
        ..Default::default()
    };
    c.bench_function("discover_paths_empty_include_10_files", |b| {
        b.iter(|| black_box(discover_paths_from_patterns(black_box(dir.path()), black_box(&options))))
    });
}

fn bench_discover_with_include(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    workdir_with_files(dir.path(), 20);
    let options = PackOptions {
        workdir: dir.path().to_path_buf(),
        include: vec!["src/**".into()],
        ..Default::default()
    };
    c.bench_function("discover_paths_include_src_20_files", |b| {
        b.iter(|| black_box(discover_paths_from_patterns(black_box(dir.path()), black_box(&options))))
    });
}

fn bench_discover_many_files(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    workdir_with_files(dir.path(), 100);
    let options = PackOptions {
        workdir: dir.path().to_path_buf(),
        include: vec!["**/*.rs".into()],
        ..Default::default()
    };
    c.bench_function("discover_paths_100_rs_files", |b| {
        b.iter(|| black_box(discover_paths_from_patterns(black_box(dir.path()), black_box(&options))))
    });
}

criterion_group!(
    benches,
    bench_discover_empty_include,
    bench_discover_with_include,
    bench_discover_many_files,
);
criterion_main!(benches);
