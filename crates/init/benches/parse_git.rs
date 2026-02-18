//! Benchmark for git source parsing. Run with: cargo bench -p init

use init::sources::git::parse_git_source;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

const GIT_SOURCES: &[&str] = &[
    "https://github.com/owner/repo.git",
    "github.com/owner/repo",
    "owner/repo",
    "https://github.com/owner/repo/tree/main/src",
    "https://github.com/owner/repo#v1.0.0",
    "https://gitlab.com/group/project",
    "https://bitbucket.org/team/repo",
    "https://github.com/user/repo/tree/develop/packages/core",
];

fn bench_parse_git_single(c: &mut Criterion) {
    c.bench_function("parse_git_single", |b| {
        b.iter(|| parse_git_source(black_box("https://github.com/owner/repo.git")))
    });
}

fn bench_parse_git_variety(c: &mut Criterion) {
    c.bench_function("parse_git_variety", |b| {
        b.iter(|| {
            for &src in GIT_SOURCES {
                let _ = parse_git_source(black_box(src));
            }
        })
    });
}

criterion_group!(benches, bench_parse_git_single, bench_parse_git_variety);
criterion_main!(benches);
