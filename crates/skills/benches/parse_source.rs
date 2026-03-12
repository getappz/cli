//! Benchmark for skill source parsing. Run with: cargo bench -p skills

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use skills::source_parser::parse_source;

const SKILL_SOURCES: &[&str] = &[
    "owner/repo",
    "owner/repo@skill-name",
    "owner/repo:skill1,skill2",
    "https://github.com/owner/repo.git",
    "https://github.com/owner/repo/tree/main/skills/foo",
    "https://skills.sh/owner/repo",
    "https://skills.sh/owner/repo/skill-name",
    "./local/path",
    "/absolute/path",
    "https://example.com/skill.md",
];

fn bench_parse_source_single(c: &mut Criterion) {
    c.bench_function("parse_source_single", |b| {
        b.iter(|| parse_source(black_box("owner/repo")))
    });
}

fn bench_parse_source_github_at(c: &mut Criterion) {
    c.bench_function("parse_source_github_at", |b| {
        b.iter(|| parse_source(black_box("owner/repo@skill-name")))
    });
}

fn bench_parse_source_variety(c: &mut Criterion) {
    c.bench_function("parse_source_variety", |b| {
        b.iter(|| {
            for &src in SKILL_SOURCES {
                let _ = parse_source(black_box(src));
            }
        })
    });
}

criterion_group!(
    benches,
    bench_parse_source_single,
    bench_parse_source_github_at,
    bench_parse_source_variety
);
criterion_main!(benches);
