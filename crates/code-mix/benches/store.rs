//! Benchmarks for store: index ops, get_content_hash, get_entries_for_workdir.
//! Run with: cargo bench -p code-mix --bench store

use code_mix::store::{
    ensure_store_dirs, get_content_hash, get_entries_for_workdir, insert_index, list_entries,
    open_index, PackMetadata,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_open_index_fresh(c: &mut Criterion) {
    c.bench_function("open_index_fresh", |b| {
        b.iter(|| {
            let dir = tempfile::tempdir().unwrap();
            std::env::set_var("APPZ_STORE_DIR", dir.path());
            ensure_store_dirs(dir.path()).unwrap();
            black_box(open_index(black_box(dir.path())).unwrap());
        });
    });
}

fn bench_get_content_hash_hit(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("APPZ_STORE_DIR", dir.path());
    ensure_store_dirs(dir.path()).unwrap();
    let conn = open_index(dir.path()).unwrap();
    insert_index(
        &conn,
        "test_input_key",
        "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234",
        0,
        &PackMetadata::default(),
    )
    .unwrap();

    c.bench_function("get_content_hash_hit", |b| {
        b.iter(|| {
            black_box(get_content_hash(black_box(&conn), black_box("test_input_key")).unwrap());
        });
    });
}

fn bench_get_content_hash_miss(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("APPZ_STORE_DIR", dir.path());
    ensure_store_dirs(dir.path()).unwrap();
    let conn = open_index(dir.path()).unwrap();

    c.bench_function("get_content_hash_miss", |b| {
        b.iter(|| {
            black_box(get_content_hash(black_box(&conn), black_box("nonexistent_key")).unwrap());
        });
    });
}

fn bench_insert_index(c: &mut Criterion) {
    c.bench_function("insert_index", |b| {
        b.iter(|| {
            let dir = tempfile::tempdir().unwrap();
            std::env::set_var("APPZ_STORE_DIR", dir.path());
            ensure_store_dirs(dir.path()).unwrap();
            let conn = open_index(dir.path()).unwrap();
            insert_index(
                &conn,
                "bench_key",
                "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234",
                0,
                &PackMetadata::default(),
            )
            .unwrap();
        });
    });
}

fn bench_get_entries_for_workdir(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("APPZ_STORE_DIR", dir.path());
    let workdir = "/tmp/bench_workdir";
    ensure_store_dirs(dir.path()).unwrap();
    let conn = open_index(dir.path()).unwrap();
    let meta = PackMetadata {
        workdir: Some(workdir.into()),
        ..Default::default()
    };
    for i in 0..50 {
        insert_index(&conn, &format!("key_{}", i), &format!("{:064x}", i), i as i64, &meta)
            .unwrap();
    }

    c.bench_function("get_entries_for_workdir_50", |b| {
        b.iter(|| black_box(get_entries_for_workdir(black_box(&conn), black_box(workdir)).unwrap()));
    });
}

fn bench_list_entries(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("APPZ_STORE_DIR", dir.path());
    ensure_store_dirs(dir.path()).unwrap();
    let conn = open_index(dir.path()).unwrap();
    for i in 0..50 {
        insert_index(
            &conn,
            &format!("key_{}", i),
            &format!("{:064x}", i),
            i as i64,
            &PackMetadata::default(),
        )
        .unwrap();
    }

    c.bench_function("list_entries_50", |b| {
        b.iter(|| black_box(list_entries(black_box(&conn)).unwrap()));
    });
}

criterion_group!(
    benches,
    bench_open_index_fresh,
    bench_get_content_hash_hit,
    bench_get_content_hash_miss,
    bench_insert_index,
    bench_get_entries_for_workdir,
    bench_list_entries,
);
criterion_main!(benches);
