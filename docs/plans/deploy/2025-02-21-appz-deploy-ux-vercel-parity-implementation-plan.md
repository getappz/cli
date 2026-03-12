# Appz Deploy UX Vercel Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Match Vercel deploy UI/UX using event-driven architecture: phased spinner messages, upload progress bar with size, deploy stamps, Inspect URL, and structured success output.

**Architecture:** Add `DeployEvent` stream in appz-client; refactor `deploy_prebuilt` to emit events via callback; deploy command maps events to phased UI (spinner + progress bar). Add `ui::format::bytes` and `ui::progress::bar_string` helpers.

**Tech Stack:** Rust (appz-cli, appz-client, ui crates), existing Appz API.

**Reference:** [2025-02-21-appz-deploy-ux-vercel-parity-design.md](./2025-02-21-appz-deploy-ux-vercel-parity-design.md)

---

## Task 1: Add bytes formatting helper (ui crate)

**Files:**
- Modify: `crates/ui/src/format.rs`
- Test: `crates/ui/src/format.rs` (existing module; add unit test if present) or manual test

**Step 1: Add `bytes` function**

Add to `crates/ui/src/format.rs`:

```rust
/// Format byte count to human-readable string (e.g. "1.2 MB", "500 KB").
/// Matches Vercel's bytes package: 1 decimal place.
pub fn bytes(n: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if n < KB {
        format!("{} B", n)
    } else if n < MB {
        format!("{:.1} KB", n as f64 / KB as f64)
    } else if n < GB {
        format!("{:.1} MB", n as f64 / MB as f64)
    } else {
        format!("{:.1} GB", n as f64 / GB as f64)
    }
}
```

**Step 2: Export from lib**

Ensure `format::bytes` is accessible (format is already `pub mod format`). Add to module doc if needed.

**Step 3: Run tests**

```bash
cd /home/avihs/workspace/appz-cli && CARGO_TARGET_DIR="$PWD/target" cargo test -p ui
```

Expected: PASS

**Step 4: Commit**

```bash
git add crates/ui/src/format.rs
git commit -m "feat(ui): add bytes() formatting for human-readable sizes"
```

---

## Task 2: Add progress bar string helper (ui crate)

**Files:**
- Modify: `crates/ui/src/progress.rs`

**Step 1: Add `bar_string` function**

Add to `crates/ui/src/progress.rs` (after the existing helpers):

```rust
const BAR_WIDTH: usize = 20;

/// Returns a progress bar string like "[=====-----]".
/// current/total determine fill; width chars total.
pub fn bar_string(current: u64, total: u64, width: usize) -> String {
    if total == 0 || current >= total {
        return "=".repeat(width);
    }
    let unit = total as f64 / width as f64;
    let pos = (current as f64 / unit) as usize;
    let pos = pos.min(width);
    format!("{}{}", "=".repeat(pos), "-".repeat(width - pos))
}
```

**Step 2: Run tests**

```bash
cd /home/avihs/workspace/appz-cli && CARGO_TARGET_DIR="$PWD/target" cargo test -p ui
```

**Step 3: Commit**

```bash
git add crates/ui/src/progress.rs
git commit -m "feat(ui): add bar_string() for upload progress display"
```

---

## Task 3: Define DeployEvent and deploy_prebuilt_stream in appz-client

**Files:**
- Modify: `crates/appz-client/src/deploy.rs`
- Modify: `crates/appz-client/src/lib.rs` (export DeployEvent if needed)

**Step 1: Add DeployEvent enum**

Add near the top of `crates/appz-client/src/deploy.rs` (after imports):

```rust
/// Events emitted during deployment (Vercel-aligned).
#[derive(Debug, Clone)]
pub enum DeployEvent {
    Preparing,
    FileCount {
        total: usize,
        missing: usize,
        total_bytes: u64,
    },
    FileUploaded {
        path: String,
        bytes: u64,
    },
    UploadProgress {
        uploaded_bytes: u64,
        total_bytes: u64,
    },
    Created {
        deployment_id: String,
        url: String,
        inspect_url: Option<String>,
        is_production: bool,
    },
    Processing,
    Ready {
        url: String,
        inspect_url: Option<String>,
        is_production: bool,
    },
    Error(String),
}
```

**Step 2: Add deploy_prebuilt_stream**

Refactor `deploy_prebuilt` logic to accept `on_event: impl FnMut(DeployEvent)`. Emit:

- `Preparing` at start (after building tree)
- `FileCount` when MissingFiles received (compute total_bytes from files_by_sha)
- `UploadProgress` after each `upload_file` completes (track running uploaded_bytes)
- `Created` when Created(Deployment) received (from create or continue)
- `Processing` before returning (or after Created if we infer)
- `Ready` when deployment is ready (same as Created for now — API returns READY)
- `Error` on any Err path

Keep existing `deploy_prebuilt(client, ctx)` as a thin wrapper that calls `deploy_prebuilt_stream` with a no-op callback and returns the result.

**Step 3: Implement**

- In the MissingFiles branch: before loop, emit `FileCount`; inside loop, after each upload, emit `UploadProgress`.
- When extracting output from Deployment, emit `Created` then `Ready` (or just `Ready` since we're done).
- inspect_url: Deployment model doesn't have it; use `None` for now.
- is_production: derive from `ctx.target == "production"` or deployment response.

**Step 4: Build**

```bash
cd /home/avihs/workspace/appz-cli && CARGO_TARGET_DIR="$PWD/target" cargo build -p appz-client
```

Expected: OK

**Step 5: Commit**

```bash
git add crates/appz-client/src/deploy.rs crates/appz-client/src/lib.rs
git commit -m "feat(appz-client): add DeployEvent and deploy_prebuilt_stream"
```

---

## Task 4: Wire deploy command to event stream and phased UI

**Files:**
- Modify: `crates/app/src/commands/deploy.rs`

**Step 1: Replace deploy_prebuilt call with deploy_prebuilt_stream**

In `deploy_to_appz`, instead of:

```rust
let output = appz_client::deploy_prebuilt(&client, &ctx).await?;
```

Use:

```rust
let output = appz_client::deploy_prebuilt_stream(&client, &ctx, |ev| {
    // handle event (see Step 2)
}).await?;
```

**Step 2: Implement event handler**

- **Preparing:** `sp.set_message("Deploying to Appz...")` if spinner exists
- **FileCount:** store total_bytes; if missing > 0, we'll show progress
- **UploadProgress:** `sp.set_message(format!("Uploading [{}] ({}/{})", ui::progress::bar_string(ev.uploaded_bytes, ev.total_bytes, 20), ui::format::bytes(ev.uploaded_bytes), ui::format::bytes(ev.total_bytes)))`. Throttle: when !is_tty, only update at 25% increments (track last printed percent)
- **Created:** stop spinner; print "Inspect: {url}" if inspect_url; print "Production/Preview: {url} [stamp]"
- **Processing:** `sp.set_message("Processing deployment...")`
- **Ready:** stop spinner; print final "Production/Preview: {url} [stamp]"; sp.finish_with_message("Deployed!")
- **Error:** stop spinner; return (propagate error)

**Step 3: Deploy stamp**

Use `ui::format::timestamp_age_short` or similar. For deploy we want "now" or elapsed since deploy — the Deployment has createdAt. Compute age from `chrono::Utc::now() - createdAt` or use a simple "[just now]" for immediate display. Add `ui::format::deploy_stamp(created_at: i64) -> String` if needed (e.g. "2m" or "just now").

**Step 4: TTY check**

Use `atty::is(Stream::Stderr)` or similar. If not TTY, throttle UploadProgress to 25% increments.

**Step 5: JSON mode**

When `json_output`, pass no-op callback (or a callback that does nothing). Existing JSON output at end unchanged.

**Step 6: Build and smoke test**

```bash
cd /home/avihs/workspace/appz-cli && CARGO_TARGET_DIR="$PWD/target" cargo build --bin appz
./target/debug/appz deploy --help
```

**Step 7: Commit**

```bash
git add crates/app/src/commands/deploy.rs
git commit -m "feat(deploy): Vercel-parity phased UI with event stream"
```

---

## Task 5: Add deploy_stamp helper if missing

**Files:**
- Modify: `crates/ui/src/format.rs`

**Step 1: Check if timestamp_age_short suffices**

`timestamp_age_short(ts)` gives "45s", "2m", "3h". For deploy we show age of deployment. If `createdAt` is in ms, convert: `timestamp_age_short(created_at / 1000)` or use `timestamp_auto`. Add a small helper:

```rust
/// Deploy stamp: relative age for display next to URL (e.g. "[2m]").
/// ts: Unix timestamp (seconds or milliseconds, auto-detected).
pub fn deploy_stamp(ts: i64) -> String {
    let s = timestamp_age_short(ts);
    format!("[{}]", s)
}
```

**Step 2: Use in deploy command**

When printing URL, append `ui::format::deploy_stamp(deployment.created_at)` (Deployment uses createdAt in ms from API).

**Step 3: Commit** (only if added)

```bash
git add crates/ui/src/format.rs crates/app/src/commands/deploy.rs
git commit -m "feat(ui): add deploy_stamp for URL display"
```

---

## Task 6: Polish and edge cases

**Files:**
- Modify: `crates/app/src/commands/deploy.rs`

**Step 1: No files to upload (all cached)**

When create returns Created immediately (no MissingFiles), emit Created/Ready without FileCount/UploadProgress. Handler should not panic.

**Step 2: Section title and blank lines**

Keep existing "Deploying to Appz" section title. Ensure blank line before/after as before.

**Step 3: Duration**

If we have duration, still print it in display_deploy_result. Ensure deploy_prebuilt_stream returns DeployOutput with duration if available.

**Step 4: Final verification**

```bash
cd /home/avihs/workspace/appz-cli && CARGO_TARGET_DIR="$PWD/target" cargo build --bin appz
```

**Step 5: Commit**

```bash
git add crates/app/src/commands/deploy.rs
git commit -m "fix(deploy): handle no-upload path and polish output"
```

---

## Execution Handoff

Plan complete and saved to `docs/plans/deploy/2025-02-21-appz-deploy-ux-vercel-parity-implementation-plan.md`.

**Two execution options:**

1. **Subagent-Driven (this session)** — Dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Parallel Session (separate)** — Open a new session with executing-plans, batch execution with checkpoints.

Which approach?
