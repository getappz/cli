# Performance Optimizations Appendix

<!-- Supplements 06_performance_guidelines.md with proven patterns. -->

## Summary

| Optimization | Impact | Already in Guidelines |
|--------------|--------|-----------------------|
| Mimalloc as global allocator | Up to ~25% on alloc-heavy paths | Yes — [M-MIMALLOC-APPS](../02_application_guidelines.md#M-MIMALLOC-APPS) |
| Release profile (LTO, codegen-units=1, panic=abort) | Smaller, faster binaries | Partially — panic=abort in 08; LTO/codegen not |
| Bench profile with `debug = 1` | Profiling with symbols | Yes — [M-HOTPATH](../06_performance_guidelines.md#M-HOTPATH) |
| Regex caching with OnceLock | ~100–900× on hot parsing paths | **No** |
| Criterion benchmarks for hot paths | Measure before/after | Yes — [M-HOTPATH](../06_performance_guidelines.md#M-HOTPATH) |

---

## Regex Caching with OnceLock

**Problem**: Compiling `regex::Regex` on every call costs ~100–400 µs per pattern. On hot parsing paths this dominates runtime.

**Solution**: Compile once with `std::sync::OnceLock` and reuse. For `skills::parse_source`, this reduced latency from ~438 µs to ~498 ns (~880× faster).

### Pattern

```rust
use std::sync::OnceLock;

fn regex_for_tree_path() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)github\.com/([^/]+)/([^/]+)/tree/([^/]+)/(.+)").unwrap())
}
```

Use when:

- The regex pattern is known at compile time and does not depend on runtime input.
- The regex is used in a hot path (parsing, validation, matching).

---

## Release Profile Settings

For maximum release performance and smaller binaries:

```toml
[profile.release]
lto = true          # Link-time optimization; enables cross-crate inlining
codegen-units = 1   # Better optimization; slower compile
panic = "abort"     # Smaller binary; no unwinding
```

- **LTO**: Enables whole-program optimization across crates.
- **codegen-units = 1**: Gives the compiler more context for optimization.
- **panic = "abort"**: Drops unwind tables; follow [M-PANIC-IS-STOP](../08_universal_guidelines.md).

---

## Benchmarks for Hot Paths

1. Add Criterion (or divan) benchmarks for parsing, validation, and config loading.
2. Run benchmarks before and after changes to validate impact.
3. Use `[profile.bench]` with `debug = 1` when profiling with perf, VTune, or Superluminal.

Example benchmark layout: `crates/<crate>/benches/<operation>.rs` (e.g. `parse_source.rs`, `parse_git.rs`, `config.rs`).
