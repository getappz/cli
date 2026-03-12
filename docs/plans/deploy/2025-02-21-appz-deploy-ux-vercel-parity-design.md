# Appz Deploy UI/UX — Vercel Parity (Event-Driven)

> Design: Full Vercel-parity deploy UX using an event-driven architecture. Reference: `appz-ref/vercel/packages/cli/src/util/deploy/process-deployment.ts`.

## Goal

Match the Vercel deploy command UI/UX in appz deploy: phased spinner messages, upload progress bar with size, deploy stamps, Inspect URL, and structured success output. Use an event stream so the CLI can drive each phase and future backend events can plug in.

## Reference: Vercel Deploy UX

| Phase | Vercel Shows |
|-------|--------------|
| Initial | Spinner: "Deploying {org}/{project}" |
| Upload | `Uploading [=====-----] (1.2 MB/2.5 MB)` — progress bar + human-sized bytes |
| Created | Inspect URL + Production/Preview URL with deploy stamp |
| Building | "Building..." / "Building: {log line}" |
| Ready | Final URL with success emoji, then "Completing..." |
| Checks | "Running Checks..." (when applicable) |
| Alias | "Aliased: https://..." (production) |
| After | Indications (tips/warnings), suggest-next-commands / guidance |

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  deploy command (deploy.rs)                                      │
│  Consumes DeployEvent stream → updates spinner/progress/print     │
└───────────────────────────────┬─────────────────────────────────┘
                                │ async stream / poll
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│  appz-client: deploy_prebuilt_stream()                           │
│  Emits DeployEvent at each phase                                │
└───────────────────────────────┬─────────────────────────────────┘
                                │ HTTP
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│  Appz API                                                        │
│  create → upload files → continue (no streaming yet)            │
└─────────────────────────────────────────────────────────────────┘
```

## Section 1: Event Stream API (appz-client)

### DeployEvent enum

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

### Flow (inferred from current API)

1. **Preparing** — building file tree, hashing
2. **FileCount** — after create returns MissingFiles; `total` = all files, `missing` = to upload, `total_bytes` = size of missing
3. **UploadProgress** — after each uploaded file; `(uploaded_bytes, total_bytes)`
4. **Created** — after create returns Created, or after continue returns Created
5. **Processing** — brief phase before Ready (backend processing)
6. **Ready** — deployment ready
7. **Error** — on failure

### API surface

```rust
/// Deploy prebuilt output with event stream.
/// Returns final DeployOutput on success, or Err on failure.
pub async fn deploy_prebuilt_stream(
    client: &Client,
    ctx: &DeployContext,
    mut on_event: impl FnMut(DeployEvent),
) -> Result<DeployOutput>;
```

Alternative: return `impl Stream<Item = DeployEvent>` and have caller drive. Callback is simpler and avoids stream ownership across async boundaries.

### Inspect URL

- Include in `DeployEvent::Created` and `Ready` when API returns it.
- Appz API `Deployment` model can add `inspectUrl` later; for now use `None`.

---

## Section 2: CLI Event → UI Mapping

| Event | CLI Action |
|-------|------------|
| `Preparing` | Spinner: "Deploying to Appz..." |
| `FileCount` | Store `total_bytes`; if missing > 0, prepare progress bar |
| `UploadProgress` | Update spinner: `Uploading [=====-----] (1.2 MB/2.5 MB)` |
| `FileUploaded` | Debug only (or skip) |
| `Created` | Stop spinner; print Inspect URL (if any); print Production/Preview URL + deploy stamp |
| `Processing` | Spinner: "Processing deployment..." |
| `Ready` | Stop spinner; print final URL + stamp; "Deployed!" |
| `Error` | Stop spinner; print error; return Err |

### Progress bar format

- Vercel: `[=====-----]` with `=` complete, `-` incomplete (20 chars).
- Human bytes: `1.2 MB/2.5 MB` (use `bytes` crate or custom formatter).
- Non-TTY: 25% increments to avoid log spam (same as Vercel).

### Deploy stamp

- Relative time next to URL: `[2m]` (from `ui::format::timestamp_age_short` or similar).
- Shown beside Inspect and deployment URL.

### Production vs Preview

- `Created` and `Ready` include `is_production: bool`.
- Print "Production: https://..." or "Preview: https://..." accordingly.

---

## Section 3: Supporting Changes

### 3.1 Bytes formatting (ui crate)

Add `ui::format::bytes(n: u64) -> String`:

- `0` → "0 B"
- `1024` → "1.0 KB"
- `1_500_000` → "1.4 MB"
- Match Vercel's `bytes` package: 1 decimal place, "B", "KB", "MB", "GB".

### 3.2 Progress bar string (ui crate)

Add `ui::progress::bar_string(current: u64, total: u64, width: usize) -> String`:

- Returns `[=====-----]` style string.
- Used when spinner message includes progress (spinner template + custom msg).

### 3.3 Spinner message update

`ui::progress::SpinnerHandle` already has `set_message()`. Use it to update:

- "Deploying to Appz..."
- "Uploading [=====-----] (1.2 MB/2.5 MB)"
- "Processing deployment..."
- "Deployed!"

### 3.4 Throttling (non-TTY)

When `stderr` is not a TTY, update progress only at 25% increments (0%, 25%, 50%, 75%, 100%) to avoid log spam.

---

## Section 4: Error Handling

- **Size limit exceeded:** Match Vercel's `size_limit_exceeded` handling; show human bytes.
- **Upload rate limit / too many files:** Suggest `--archive=tgz` if applicable (Appz may support archive later).
- **Network errors:** Clear message; suggest retry.
- **Auth errors:** Existing handling (Not logged in, etc.).

---

## Section 5: JSON / CI Mode

When `--json` or non-interactive (CI):

- Suppress all spinner and progress updates.
- Emit only the final JSON result.
- No deploy stamps, no phased messages.

---

## Section 6: Future Extensions

- **Backend streaming:** When Appz API supports deployment status stream, add events like `Building`, `ChecksRunning`, `AliasAssigned`.
- **Polling:** Optional `--no-wait` could skip Ready polling; show "Deployment created, processing..." with URL.
- **Indications:** Tips, notices, warnings (like Vercel) when API returns them.
- **Alias line:** "Aliased: https://custom-domain" when production alias is assigned.

---

## File Changes Summary

| Crate/File | Change |
|------------|--------|
| `appz-client/src/deploy.rs` | Add `deploy_prebuilt_stream` with callback; refactor `deploy_prebuilt` to call it (or keep both) |
| `appz-client/src/deploy.rs` | Define `DeployEvent` enum |
| `crates/app/src/commands/deploy.rs` | Switch to `deploy_prebuilt_stream`; map events to UI |
| `crates/ui/src/format.rs` | Add `bytes(n: u64) -> String` |
| `crates/ui/src/progress.rs` | Add `bar_string(current, total, width) -> String` (or inline in deploy) |

---

## Dependencies

- `bytes` crate (or `byte-unit`) for human formatting — or implement minimal helper in `ui::format`.
- No new crates strictly required; `format!` with manual KB/MB/GB logic is fine.

---

## Success Criteria

1. Upload shows progress bar with `(uploaded/total)` in human bytes.
2. Spinner phases: Deploying → Uploading [bar] → Processing → Deployed.
3. Deploy stamp next to URLs.
4. Inspect URL printed when API provides it.
5. Production vs Preview labels correct.
6. `--json` / CI: no interactive UI, JSON only.
7. Non-TTY: progress at 25% increments.
