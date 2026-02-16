# Cargo Dedupe

Identify and consolidate duplicate or redundant crates in Rust projects.

**Use the cargo-dedupe skill.** Read `.cursor/skills/cargo-dedupe/SKILL.md` for full guidelines (tools, workflow, consolidation patterns, and troubleshooting).

## Instructions

1. **Load the skill**: Read `.cursor/skills/cargo-dedupe/SKILL.md` and follow its workflow
2. **Discovery phase**:
   - Run `cargo tree --duplicates` to find duplicate versions
   - Run `cargo +nightly udeps` (or `cargo machete`) to find unused deps
   - Use `cargo tree -i <crate>` to trace what pulls in duplicates
3. **Consolidation phase**:
   - Add `[workspace.dependencies]` in root `Cargo.toml` (Rust 1.64+)
   - Unify versions across workspace members
   - Remove unused deps or move to `[dev-dependencies]`
4. **Validation phase**:
   - Run `cargo build --workspace --all-features`
   - Run `cargo test --workspace --all-features`
   - Run `cargo tree --duplicates` (should be empty or minimal)
   - Run `cargo deny check` if configured

## Quick Commands

```bash
cargo tree --duplicates
cargo +nightly udeps
cargo tree -i <crate>
cargo deny check bans
```

## Output

- Summary of duplicates found and their sources
- Recommended consolidation steps
- Updated `Cargo.toml` snippets if applicable
- Validation status after changes
