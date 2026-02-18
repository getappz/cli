//! Benchmarks for config parsing and merging. Run with: cargo bench -p common

use common::user_config::{deep_merge_json, toml_to_json};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

const SAMPLE_TOML: &str = r#"
[check]
strict = true
aiProvider = "openai"
aiModel = "gpt-4o"

[check.aiSafety]
max_change_pct = 30.0
max_attempts = 5

[gen]
model = "gpt-4"
"#;

fn bench_toml_parse_and_convert(c: &mut Criterion) {
    c.bench_function("toml_parse_and_convert", |b| {
        b.iter(|| {
            let toml_val = toml::from_str::<toml::Value>(black_box(SAMPLE_TOML)).unwrap();
            let _ = toml_to_json(&toml_val);
        })
    });
}

fn bench_deep_merge_small(c: &mut Criterion) {
    let base = serde_json::json!({
        "a": 1,
        "b": {"c": 2, "d": 3},
        "e": "hello"
    });
    let overlay = serde_json::json!({
        "b": {"c": 99, "f": 100},
        "g": "new"
    });

    c.bench_function("deep_merge_small", |b| {
        b.iter(|| deep_merge_json(black_box(&base), black_box(&overlay)))
    });
}

fn bench_deep_merge_nested(c: &mut Criterion) {
    let base = serde_json::json!({
        "check": {
            "strict": true,
            "aiProvider": "openai",
            "aiSafety": {
                "max_change_pct": 30.0,
                "max_attempts": 5
            }
        }
    });
    let overlay = serde_json::json!({
        "check": {
            "aiModel": "gpt-4o",
            "aiSafety": {
                "max_change_pct": 25.0
            }
        }
    });

    c.bench_function("deep_merge_nested", |b| {
        b.iter(|| deep_merge_json(black_box(&base), black_box(&overlay)))
    });
}

criterion_group!(
    benches,
    bench_toml_parse_and_convert,
    bench_deep_merge_small,
    bench_deep_merge_nested
);
criterion_main!(benches);
