//! Benchmarks for cache: content hash, input key, store operations.
//! Run with: cargo bench -p code-mix --bench cache

use std::path::PathBuf;

use code_mix::{
    compute_content_hash, compute_input_key, get_cached_output, list_cached, PackOptions,
};
use code_mix::store::{self, PackMetadata};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tokio::runtime::Runtime;

fn default_options(workdir: PathBuf) -> PackOptions {
    PackOptions {
        workdir,
        ..Default::default()
    }
}

fn bench_compute_content_hash_small(c: &mut Criterion) {
    let content = b"fn main() { println!(\"hello\"); }";

    c.bench_function("compute_content_hash_small", |b| {
        b.iter(|| compute_content_hash(black_box(content)))
    });
}

fn bench_compute_content_hash_large(c: &mut Criterion) {
    let content = vec![0u8; 1024 * 100]; // 100 KB

    c.bench_function("compute_content_hash_large", |b| {
        b.iter(|| compute_content_hash(black_box(&content)))
    });
}

fn bench_compute_input_key(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let workdir = dir.path().to_path_buf();

    // Create a few small files
    for (i, name) in ["src/main.rs", "src/lib.rs", "Cargo.toml"].iter().enumerate() {
        let p = workdir.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, format!("// file {} content\nfn foo_{}() {{}}\n", i, i)).unwrap();
    }

    let paths: Vec<String> = vec![
        "src/main.rs".into(),
        "src/lib.rs".into(),
        "Cargo.toml".into(),
    ];
    let options = default_options(workdir.clone());

    c.bench_function("compute_input_key_3_files", |b| {
        b.to_async(&rt).iter(|| {
            compute_input_key(black_box(&workdir), black_box(&options), black_box(&paths))
        });
    });
}

fn bench_get_cached_output_miss(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let workdir = dir.path().to_path_buf();
    std::env::set_var("APPZ_STORE_DIR", dir.path().join("store"));

    let paths: Vec<String> = vec!["x.rs".into()];
    let p = workdir.join("x.rs");
    std::fs::write(&p, "fn x() {}").unwrap();
    let options = default_options(workdir.clone());

    c.bench_function("get_cached_output_miss", |b| {
        b.to_async(&rt).iter(|| {
            get_cached_output(black_box(&workdir), black_box(&options), black_box(&paths))
        });
    });
}

fn bench_list_cached_empty(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("APPZ_STORE_DIR", dir.path());
    store::ensure_store_dirs(dir.path()).unwrap();
    let _ = store::open_index(dir.path()).unwrap();

    c.bench_function("list_cached_empty", |b| {
        b.iter(|| black_box(list_cached(black_box(false))))
    });
}

fn bench_list_cached_with_entries(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("APPZ_STORE_DIR", dir.path());
    store::ensure_store_dirs(dir.path()).unwrap();
    let conn = store::open_index(dir.path()).unwrap();
    let meta = PackMetadata::default();
    for i in 0..20 {
        store::insert_index(
            &conn,
            &format!("input_key_{}", i),
            &format!("{:064x}", i),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            &meta,
        )
        .unwrap();
    }

    c.bench_function("list_cached_20_entries", |b| {
        b.iter(|| black_box(list_cached(black_box(false))))
    });
}

criterion_group!(
    benches,
    bench_compute_content_hash_small,
    bench_compute_content_hash_large,
    bench_compute_input_key,
    bench_get_cached_output_miss,
    bench_list_cached_empty,
    bench_list_cached_with_entries,
);
criterion_main!(benches);
