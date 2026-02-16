---
name: cargo-dedupe
description: Identify and consolidate duplicate or redundant crates in Rust projects. Use when deduplicating dependencies, reducing workspace bloat, or when the user mentions duplicate crates, cargo tree --duplicates, cargo-deny, or dependency consolidation.
---

# Rust Dependency Consolidation & Management

## Overview

This skill covers identifying and consolidating duplicate or redundant crates in Rust projects, particularly in workspace configurations where multiple crates might be used to achieve the same result.

## Problem Statement

In Rust projects, especially workspaces with multiple crates, you may encounter:

- Different versions of the same dependency across workspace members
- Multiple crates providing similar functionality (e.g., multiple HTTP clients)
- Unused dependencies bloating your dependency tree
- Transitive dependencies pulling in duplicates
- Overlapping functionality across different crates

## Essential Tools

### 1. cargo tree (Built-in)

```bash
cargo tree                    # Full dependency tree
cargo tree --duplicates       # Only duplicate versions
cargo tree -p <package>       # Specific package
cargo tree -e features        # Show features
cargo tree -i <package>       # What depends on this
cargo tree --depth 1          # Direct dependencies only
```

### 2. cargo-deny (Critical for Workspaces)

```bash
cargo install cargo-deny
cargo deny init              # Create deny.toml
cargo deny check             # Check all policies
cargo deny check bans        # Check duplicates only
```

Example `deny.toml`:

```toml
[bans]
multiple-versions = "deny"
skip = [{ name = "windows-sys", version = "*" }]
```

### 3. cargo-udeps (Find Unused)

```bash
cargo install cargo-udeps
cargo +nightly udeps         # Requires nightly
```

### 4. cargo-machete (Faster Alternative)

```bash
cargo install cargo-machete
cargo machete               # Find unused
cargo machete --fix         # Remove automatically
```

## Step-by-Step Workflow

### Phase 1: Discovery

```bash
cargo tree --duplicates > duplicates.txt
cargo +nightly udeps > unused.txt
cargo tree > full-tree.txt
cargo tree -i <duplicate-crate-name>  # Track source
```

### Phase 2: Consolidation

**A. Workspace Dependencies (Rust 1.64+)**

```toml
# Root Cargo.toml
[workspace.dependencies]
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }

# Member Cargo.toml
[dependencies]
tokio = { workspace = true }
serde = { workspace = true, features = ["extra"] }  # Can add features
```

**B. Common Functional Duplicates**

| Category | Keep | Remove |
|----------|------|--------|
| HTTP Clients | reqwest | ureq, attohttpc |
| JSON | serde_json | alternatives |
| Async | tokio | async-std, smol |
| Errors | anyhow (apps), thiserror (libs) | — |
| CLI | clap v4+ | structopt, argh |
| Logging | tracing, or log if simple | — |

### Phase 3: Validation

```bash
cargo build --workspace --all-features
cargo test --workspace --all-features
cargo tree --duplicates  # Should be empty or minimal
cargo deny check
```

## Quick Commands Reference

```bash
# Discovery
cargo tree --duplicates
cargo +nightly udeps
cargo tree -i <crate>

# Validation
cargo build --workspace --all-features
cargo test --workspace
cargo deny check

# Maintenance
cargo outdated  # cargo install cargo-outdated
cargo update
```

## Troubleshooting

**Can't unify versions?**

```bash
cargo tree -i <conflicting-crate>  # Find what's pulling it
cargo update -p <parent-crate>      # Update parent
```

**False positive unused deps?**

- Check if used in tests/examples
- Proc macros may be flagged
- Move to `[dev-dependencies]` if appropriate

## CI Integration

```yaml
# .github/workflows/ci.yml
- uses: EmbarkStudios/cargo-deny-action@v1
```

## Checklist

- [ ] Run `cargo tree --duplicates`
- [ ] Run `cargo +nightly udeps`
- [ ] Identify functional duplicates
- [ ] Set up workspace dependencies
- [ ] Remove unused dependencies
- [ ] Install and configure cargo-deny
- [ ] Add CI checks
- [ ] Document dependency choices

## Resources

- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [cargo-deny Docs](https://embarkstudios.github.io/cargo-deny/)
- [Blessed.rs](https://blessed.rs/) – Curated crates
