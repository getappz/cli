# Appz Platform — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build Appz's own deployment platform by studying `appz-ref/vercel` and porting architecture to Rust. CLI and API target Appz infrastructure only. No Vercel CLI wrapper.

**Architecture:** Study Vercel packages (client, build-utils, static-build, frameworks, fs-detectors), port logic to Rust crates. Appz API handles deployment creation, file upload, storage. CLI uses new appz-client crate.

**Tech Stack:** Rust (appz-cli, new appz-build, appz-client crates), Appz API (existing), file storage (S3/R2 or similar).

**Reference:** [2025-02-20-vercel-alignment-design.md](./2025-02-20-vercel-alignment-design.md), `appz-ref/vercel/packages/*`

---

## Phase 0: Vercel Source Study (Documentation)

### Task 0: Document Vercel deploy flow and data structures

**Files:**
- Create: `docs/plans/vercel/vercel-source-study.md`

**Step 1: Trace deploy flow**

Read and document:
1. `packages/cli/src/commands/deploy/index.ts` — main deploy handler
2. `packages/cli/src/util/deploy/process-deployment.ts` — orchestration
3. `packages/client/src/create-deployment.ts` — API create + upload
4. `packages/client/src/upload.ts` — file upload logic
5. `packages/client/src/deploy.ts` — deploy API call

Document: request/response shapes, file tree format, upload endpoints.

**Step 2: Document Build Output format**

Read:
- `packages/static-build/src/utils/build-output-v3.ts`
- `packages/cli/test/fixtures/unit/commands/deploy/static-with-build-output/.vercel/output/`

Document: config.json schema, static/ structure, builds.json (if used).

**Step 3: Document framework detection**

Read:
- `packages/fs-detectors/src/` (detectFramework, etc.)
- `packages/frameworks/src/` (framework list)

Document: detection logic, framework config (buildCommand, outputDirectory, etc.).

**Step 4: Commit**

```bash
git add docs/plans/vercel/vercel-source-study.md
git commit -m "docs: Vercel source study for Appz platform port"
```

---

## Phase 1: Appz API — Deployment Creation & File Upload

### Task 1: API endpoint for deployment creation

**Files:**
- Modify: `crates/api/` (or backend service)
- Create endpoint: `POST /v0/deployments`

**Step 1: Define deployment creation payload**

From Vercel client, payload includes: projectId, name, target (preview|production), env, projectSettings, etc. Define minimal Appz schema:

```json
{
  "projectId": "proj_xxx",
  "name": "optional",
  "target": "preview",
  "meta": {}
}
```

**Step 2: Implement endpoint**

- Create deployment record (status: BUILDING or similar)
- Return `{ deploymentId, uploadToken, uploadUrls? }` for file upload

**Step 3: Add API client method**

In `crates/api/src/endpoints/deployments.rs`, add `create_deployment(&self, payload) -> Result<DeploymentCreateResponse>`.

**Step 4: Commit**

```bash
git add crates/api/
git commit -m "feat(api): deployment creation endpoint"
```

---

### Task 2: API endpoint for file upload

**Files:**
- Backend: file storage (S3 presigned URLs or direct upload)
- API: `POST /v0/deployments/:id/files` or presigned URL issuance

**Step 1: Design upload flow**

Vercel uses content-addressed blobs (SHA). API returns which SHAs are needed; client uploads only missing ones. Design Appz equivalent:
- Option A: Presigned URLs per file
- Option B: Multipart upload to single endpoint
- Option C: Resumable upload with SHA check

**Step 2: Implement upload endpoint or URL issuance**

Backend stores files in object storage, associates with deployment.

**Step 3: Implement continue endpoint**

`POST /v0/deployments/:id/continue` — mark files uploaded, trigger processing, return deployment URL when READY.

**Step 4: Commit**

```bash
git add <backend files>
git commit -m "feat(api): deployment file upload and continue"
```

---

### Task 3: appz-client crate — create deployment and upload

**Files:**
- Create: `crates/appz-client/` (new crate)
- Create: `crates/appz-client/src/deploy.rs`
- Create: `crates/appz-client/src/upload.rs`
- Modify: `Cargo.toml` (workspace)

**Step 1: Create crate**

```bash
cargo new --lib crates/appz-client
```

Add deps: reqwest, tokio, serde, serde_json.

**Step 2: Port file tree building from build-utils**

Study `@vercel/build-utils`:
- `glob()` for file discovery
- `Files` map (path -> FileFsRef with mode, type)
- Content hashing for dedup

Implement in Rust: `build_file_tree(path: &Path, ignore: &[&str]) -> Result<HashMap<PathBuf, FileRef>>`.

**Step 3: Port upload flow from client**

Study `packages/client/src/upload.ts`, `create-deployment.ts`:
- Create deployment (POST)
- Build file tree
- Upload files (iterate, upload each)
- Call continue

Implement: `deploy_prebuilt(ctx: &DeployContext) -> Result<DeploymentOutput>`.

**Step 4: Add tests**

Unit test: file tree from fixture dir. Integration: mock API, run deploy flow.

**Step 5: Commit**

```bash
git add crates/appz-client/
git commit -m "feat: appz-client crate for deployment create and upload"
```

---

### Task 4: Wire appz deploy to appz-client (remove Vercel provider for Appz projects)

**Files:**
- Modify: `crates/deployer/src/providers/` — add `appz` provider OR
- Modify: `crates/app/src/commands/deploy.rs` — when linked to Appz project, use appz-client instead of deployer

**Step 1: Detect Appz project**

When `.appz/project.json` exists (or appz link state), use Appz platform deploy. Otherwise keep existing deployer for third-party (Netlify, etc.).

**Step 2: Call appz-client**

```rust
let output = appz_client::deploy_prebuilt(&ctx).await?;
```

**Step 3: Display result**

Same as current deploy: URL, status, etc.

**Step 4: Commit**

```bash
git add crates/app/src/commands/deploy.rs
git commit -m "feat(deploy): use appz-client for Appz-linked projects"
```

---

## Phase 2: Build Pipeline (Framework Detection + Local Build)

### Task 5: appz-build crate — framework detection

**Files:**
- Create: `crates/appz-build/`
- Create: `crates/appz-build/src/detect.rs`
- Create: `crates/appz-build/src/frameworks.rs`
- Modify: `Cargo.toml` (workspace)

**Step 1: Port fs-detectors**

Study `packages/fs-detectors/src/`:
- `detectFramework()` — scans package.json, config files
- Returns framework slug (gatsby, next, nuxt, etc.) + config

Implement in Rust: `detect_framework(project_root: &Path) -> Result<Option<DetectedFramework>>`.

**Step 2: Port framework definitions**

Study `packages/frameworks/src/`:
- Framework list with buildCommand, outputDirectory, devCommand
- Default values per framework

Implement: `Frameworks::get(slug) -> Option<FrameworkConfig>`.

**Step 3: Add tests**

Use fixtures from `packages/fs-detectors/test/fixtures/` or `packages/static-build/test/fixtures/` (copy a few).

**Step 4: Commit**

```bash
git add crates/appz-build/
git commit -m "feat: appz-build framework detection (ported from fs-detectors)"
```

---

### Task 6: appz-build — run build

**Files:**
- Modify: `crates/appz-build/src/build.rs`

**Step 1: Port build execution from static-build**

Study `packages/static-build/src/index.ts`:
- `runPackageJsonScript()` or `runShellScript()`
- Env vars, cwd, timeout
- Output dir validation

Implement: `run_build(project_root: &Path, framework: &FrameworkConfig, sandbox: &Sandbox) -> Result<()>`.

**Step 2: Integrate with sandbox**

Use existing `sandbox` crate for `exec` (npm run build, etc.).

**Step 3: Validate output**

Check output directory exists and is non-empty (port `validateDistDir` logic).

**Step 4: Commit**

```bash
git add crates/appz-build/src/build.rs
git commit -m "feat(appz-build): run framework build via sandbox"
```

---

### Task 7: appz build produces standardized output

**Files:**
- Modify: `crates/app/src/commands/build.rs`
- Modify: `crates/appz-build/`

**Step 1: Optionally produce .appz/output**

After build, copy/structured output to `.appz/output/`:
- `static/` — built static files
- `config.json` — routes, version (minimal)

Port from `build-output-v3` if needed.

**Step 2: appz deploy reads .appz/output or dist**

When deploying, if `.appz/output` exists, use it. Else use `outputDirectory` from appz.json (dist, build, etc.).

**Step 3: Commit**

```bash
git add crates/appz-build/ crates/app/src/commands/build.rs
git commit -m "feat: standardized build output .appz/output"
```

---

## Phase 3: Env, Pull, Logs, Inspect

### Task 8: API and CLI for env vars

**Files:**
- API: `GET/POST/DELETE /v0/projects/:id/env`
- CLI: `crates/app/src/commands/env/`
- Modify: `crates/app/src/app.rs`

**Step 1: API endpoints**

Study Vercel env: key, value, environments (production, preview, development). Implement CRUD.

**Step 2: CLI env commands**

Port from `packages/cli/src/commands/env/`:
- `appz env ls`
- `appz env add KEY value production`
- `appz env rm KEY preview`
- `appz env pull` — write to .env.local

**Step 3: Commit**

```bash
git add crates/api/ crates/app/src/commands/env/
git commit -m "feat: env vars API and CLI (ported from Vercel)"
```

---

### Task 9: appz pull

**Files:**
- Create: `crates/app/src/commands/pull.rs`
- API: `GET /v0/projects/:id` with config + env

**Step 1: Port pull logic**

Study `packages/cli/src/commands/pull/`:
- Fetch project config
- Fetch env vars
- Write to local (vercel.json or equivalent, .env.local)

Implement: `appz pull` → fetch from Appz API, write appz.json + .env.local.

**Step 2: Commit**

```bash
git add crates/app/src/commands/pull.rs
git commit -m "feat: appz pull syncs project config and env"
```

---

### Task 10: appz logs and inspect

**Files:**
- API: `GET /v0/deployments/:id/logs` (streaming or paginated)
- API: `GET /v0/deployments/:id` (details)
- CLI: `crates/app/src/commands/logs.rs`, `inspect.rs`

**Step 1: Port logs**

Study `packages/cli/src/commands/logs/`:
- Fetch logs for deployment
- Optional --follow for streaming

**Step 2: Port inspect**

Study `packages/cli/src/commands/inspect/`:
- Fetch deployment details
- Optional --logs

**Step 3: Commit**

```bash
git add crates/app/src/commands/logs.rs crates/app/src/commands/inspect.rs
git commit -m "feat: appz logs and inspect"
```

---

## Phase 4: Deployer Refactor — Appz as Native Provider

### Task 11: Remove Vercel provider dependency for Appz deploys

**Files:**
- Modify: `crates/deployer/` — optionally remove or keep Vercel provider for users who want to deploy to Vercel
- Modify: `crates/app/` — default to Appz when linked

**Step 1: Decision**

- If keeping multi-provider: Appz is one provider, uses appz-client. Vercel remains for users deploying to vercel.com.
- If Appz-only: Remove Vercel provider, all deploy goes through Appz.

Document choice. Likely: keep Vercel provider for flexibility; Appz is default when linked.

**Step 2: Commit**

```bash
git add crates/deployer/ crates/app/
git commit -m "refactor: Appz native deploy, Vercel provider optional"
```

---

## Execution Handoff

Plan complete and saved to `docs/plans/vercel/2025-02-20-vercel-alignment-implementation-plan.md`.

**Execution options:**

1. **Subagent-Driven (this session)** — Dispatch subagent per task, review between tasks.
2. **Parallel Session** — Open new session, load `superpowers:executing-plans`, batch execution.

Which approach do you prefer?
