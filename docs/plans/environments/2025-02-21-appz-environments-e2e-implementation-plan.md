# Appz Environments E2E Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement Vercel-parity environments (Local, Preview, Production) for `appz pull` and `appz env pull`, with output file naming and E2E tests.

**Architecture:** Add `--environment` to `appz pull`; derive output filename from environment; reuse existing `pull_env` logic. E2E tests in `crates/cli/tests/environments_e2e.rs` using bin-based tempdir pattern.

**Tech Stack:** Rust (clap, miette), appz-cli structure

---

## Task 1: Add default filename helper

**Files:**
- Create: `crates/app/src/commands/env/filename.rs` (or add to `pull.rs` as private fn)

**Step 1: Add helper function**

```rust
/// Default output filename for env pull by target (Vercel parity).
/// - development → .env.local
/// - preview → .env.preview.local
/// - production → .env.production.local
pub fn default_env_filename(target: &str) -> String {
    match target {
        "development" => ".env.local".to_string(),
        "preview" => ".env.preview.local".to_string(),
        "production" => ".env.production.local".to_string(),
        _ => format!(".env.{}.local", target),
    }
}
```

**Step 2: Add unit test**

In same file or `crates/app/src/commands/env/mod.rs` test block:
- `default_env_filename("development")` → `.env.local`
- `default_env_filename("preview")` → `.env.preview.local`
- `default_env_filename("production")` → `.env.production.local`

**Step 3: Run tests**

```bash
cargo test -p app default_env_filename
```

**Step 4: Commit**

```bash
git add crates/app/src/commands/env/
git commit -m "feat(env): add default_env_filename helper for Vercel parity"
```

---

## Task 2: Add --environment to appz pull

**Files:**
- Modify: `crates/app/src/app.rs` — Add `environment`, `yes` to `Pull` variant
- Modify: `crates/app/src/commands/pull.rs` — Accept env/yes, pass to pull_env with derived filename
- Modify: `crates/cli/src/main.rs` — Pass `environment`, `yes` to `pull`

**Step 1: Update Pull variant in app.rs**

Change:
```rust
/// Pull project config and env from Appz (writes .appz/project.json, .env.local)
Pull,
```

To:
```rust
/// Pull project config and env from Appz (writes .appz/project.json, .env[.environment].local)
Pull {
    /// Target environment (development, preview, production) [default: development]
    #[arg(long, short = 'e', default_value = "development")]
    environment: String,
    /// Skip overwrite confirmation
    #[arg(long, short = 'y')]
    yes: bool,
},
```

**Step 2: Update main.rs dispatch**

Change:
```rust
Commands::Pull => app::commands::pull(session).await,
```
To:
```rust
Commands::Pull { environment, yes } => app::commands::pull(session, environment, yes).await,
```

**Step 3: Update pull.rs**

Change signature:
```rust
pub async fn pull(session: crate::session::AppzSession) -> AppResult
```
To:
```rust
pub async fn pull(
    session: crate::session::AppzSession,
    environment: String,
    yes: bool,
) -> AppResult
```

Use `crate::commands::env::default_env_filename` (or `pull::default_env_filename` if in pull.rs) to get filename. Call:
```rust
pull_env(session, default_env_filename(&environment), environment, yes).await?;
```

**Step 4: Build and run**

```bash
CARGO_TARGET_DIR="$PWD/target" cargo build -p appz
./target/debug/appz pull --help
./target/debug/appz pull --environment=preview --help
```

**Step 5: Commit**

```bash
git add crates/app/src/app.rs crates/app/src/commands/pull.rs crates/cli/src/main.rs
git commit -m "feat(pull): add --environment and --yes for Vercel parity"
```

---

## Task 3: Validate environment in pull

**Files:**
- Modify: `crates/app/src/commands/pull.rs`

**Step 1: Add validation**

At start of `pull`:
```rust
const VALID_ENVIRONMENTS: &[&str] = &["development", "preview", "production"];
if !VALID_ENVIRONMENTS.contains(&environment.as_str()) {
    return Err(miette!(
        "Environment must be one of: development, preview, production. Got: {}",
        environment
    ).into());
}
```

**Step 2: Test invalid env**

```bash
./target/debug/appz pull --environment=staging
```
Expected: error message about valid environments.

**Step 3: Commit**

```bash
git add crates/app/src/commands/pull.rs
git commit -m "fix(pull): validate --environment value"
```

---

## Task 4: Update env pull default filename by target

**Files:**
- Modify: `crates/app/src/commands/env/mod.rs` — Use `default_env_filename` when filename is default
- Modify: `crates/app/src/commands/env/pull.rs` — If filename is `.env.local` (default), consider using `default_env_filename(target)` for non-development targets for consistency

**Decision:** Keep `appz env pull` behavior: user can pass explicit filename. Default remains `.env.local`. When `--target` is preview/production, we could change default to `.env.{target}.local` for Vercel parity.

**Step 1: Update env pull default**

In `env/mod.rs` `Pull`:
- Change default `filename` to be derived from target when not explicitly provided.
- Clap default_value for filename: keep `.env.local` for backward compatibility.
- Alternative: when target != development and user did not pass filename, use `default_env_filename(&target)`.

Simpler approach: keep `appz env pull` default as `.env.local`. Add doc: "Use `appz pull --environment=preview` to get `.env.preview.local`." So no change to env pull in Phase 1. Skip this task or simplify.

**Simplified:** Only add the helper; `appz env pull` keeps current behavior. `appz pull` uses the new filename logic. Mark Task 4 as: "Verify env pull --target with custom filename works" — manual smoke test only.

**Step 4 (revised): Smoke test env pull**

```bash
# With linked project + token
./target/debug/appz env pull --target=preview -y
# Should create/update .env.local (unchanged). Optional: in future, default filename could vary by target.
```

No code change. Commit: skip or "docs: note pull --environment for per-env files".

---

## Task 5: Create E2E test file

**Files:**
- Create: `crates/cli/tests/environments_e2e.rs`

**Step 1: Add test module**

```rust
//! E2E-style tests for environments (pull, env pull).
//! Run with: cargo test -p cli environments_e2e

use std::process::Command;
use tempfile::tempdir;

fn run_appz(args: &[&str], cwd: &std::path::Path) -> (String, String, i32) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_appz"));
    cmd.args(args).current_dir(cwd);
    let output = cmd.output().expect("failed to run appz");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}
```

**Step 2: Test pull requires link**

```rust
#[test]
fn test_pull_requires_linked_project() {
    let temp = tempdir().expect("tempdir");
    let (_out, stderr, code) = run_appz(&["pull", "-y"], temp.path());
    assert!(code != 0, "pull should fail when not linked");
    assert!(stderr.contains("link") || stderr.contains("Linked"), "stderr: {}", stderr);
}
```

**Step 3: Test pull help shows environment**

```rust
#[test]
fn test_pull_help_shows_environment() {
    let output = Command::new(env!("CARGO_BIN_EXE_appz"))
        .args(["pull", "--help"])
        .output()
        .expect("failed");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("environment") || stdout.contains("--environment"));
}
```

**Step 4: Run tests**

```bash
CARGO_TARGET_DIR="$PWD/target" cargo build -p appz
cargo test -p cli environments_e2e
```

**Step 5: Commit**

```bash
git add crates/cli/tests/environments_e2e.rs
git commit -m "test(cli): add environments E2E tests"
```

---

## Task 6: Add env pull E2E and invalid environment test

**Files:**
- Modify: `crates/cli/tests/environments_e2e.rs`

**Step 1: Test invalid environment**

```rust
#[test]
fn test_pull_rejects_invalid_environment() {
    let temp = tempdir().expect("tempdir");
    let (_out, stderr, code) = run_appz(&["pull", "--environment=staging", "-y"], temp.path());
    assert!(code != 0);
    assert!(stderr.contains("development") || stderr.contains("preview") || stderr.contains("production"));
}
```

**Step 2: Test env pull help**

```rust
#[test]
fn test_env_pull_help_shows_target() {
    let output = Command::new(env!("CARGO_BIN_EXE_appz"))
        .args(["env", "pull", "--help"])
        .output()
        .expect("failed");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("target") || stdout.contains("environment"));
}
```

**Step 3: Run tests**

```bash
cargo test -p cli environments_e2e
```

**Step 4: Commit**

```bash
git add crates/cli/tests/environments_e2e.rs
git commit -m "test(cli): add invalid env and env pull help E2E tests"
```

---

## Task 7: Telemetry for pull environment

**Files:**
- Modify: `crates/app/src/telemetry.rs` — Ensure `pull` records env if needed (check existing logic)

**Step 1:** If telemetry records pull, add environment to event. Otherwise skip.

**Step 2: Commit**

```bash
git add crates/app/src/telemetry.rs
git commit -m "chore(telemetry): record pull environment"
```

---

## Summary Checklist

- [ ] Task 1: default_env_filename helper
- [ ] Task 2: Pull --environment, --yes
- [ ] Task 3: Validate environment
- [ ] Task 4: (Optional) env pull default filename — skipped for Phase 1
- [ ] Task 5: E2E test file (pull requires link, pull help)
- [ ] Task 6: Invalid env test, env pull help test
- [ ] Task 7: Telemetry (if applicable)

---

## Execution Handoff

**Plan complete and saved to `docs/plans/environments/2025-02-21-appz-environments-e2e-implementation-plan.md`.**

Two execution options:

1. **Subagent-Driven (this session)** — Dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Parallel Session (separate)** — Open a new session with executing-plans, batch execution with checkpoints.

Which approach?
