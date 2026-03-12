# Appz Platform — Port Vercel Architecture to Rust

> Design: Build Appz's **own platform** by studying and porting Vercel's architecture from `appz-ref/vercel` into Rust (appz-cli, Appz API, Appz backend). No Vercel CLI wrapper.

## Goal

Create an Appz deployment platform that mirrors Vercel's architecture and feature set. The appz-cli talks to the **Appz API** (not Vercel's). Deployments run on **Appz infrastructure**. Implementation is done by studying the Vercel source and porting the logic to Rust.

## Vercel Architecture Summary (from appz-ref/vercel)

### Key Packages

| Package | Purpose | Port Target |
|---------|---------|-------------|
| `@vercel/client` | Create deployment, upload files, deploy API calls | `appz-cli` + new `appz-client` crate |
| `@vercel/build-utils` | Files, glob, runCommand, package.json, cache | `appz-build` or `deployer` crate |
| `@vercel/static-build` | Framework build orchestration, run npm build | `appz-build` crate |
| `@vercel/frameworks` | Framework definitions (Gatsby, Next, Nuxt, etc.) | `appz-build` crate |
| `@vercel/fs-detectors` | Framework detection, monorepo detection | `appz-build` or `detectors` crate |
| `@vercel/static-config` | Read vercel.json, config merging | `appz-config` or existing config |
| `@vercel/routing-utils` | Routes, rewrites, redirects | `appz-build` / API |
| `packages/node`, `next`, `remix`, etc. | Runtime-specific build | `appz-build` (start with static) |
| `packages/cli` | CLI commands, deploy flow, env, logs | `appz-cli` (already exists, extend) |

### Build Output Format (Build Output API v3)

Vercel uses `.vercel/output/` with:

- `config.json` — version, routes, crons
- `static/` — static files
- `functions/` — serverless functions (optional)
- `builds.json` — build metadata (for legacy)

Appz can adopt the same or define `.appz/output/` with equivalent structure for interoperability.

### Deploy Flow (CLI → API)

1. **Link** — `.appz/project.json` or similar (projectId, orgId)
2. **Build** — Local: `appz build` runs framework build → produces `.appz/output/` or `dist/`
3. **Create deployment** — CLI calls Appz API `POST /v0/deployments` with metadata
4. **Upload files** — CLI uploads file tree (content-addressed, deduped) to Appz API
5. **Continue** — API processes deployment, assigns URL, returns status
6. **Poll** — CLI waits for `READY` (or streams build logs if remote build)

### API Surface (Vercel → Appz)

Vercel's REST API has: projects, deployments, env, domains, aliases, teams, logs, etc. Appz API already has a subset. The plan maps missing pieces.

---

## Appz Platform Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  appz-cli (Rust)                                                 │
│  deploy, build, env, pull, logs, inspect, project, domains, etc.  │
└───────────────────────────┬─────────────────────────────────────┘
                            │ HTTP (APPZ_TOKEN)
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│  Appz API (Rust / existing backend)                              │
│  /v0/projects, /v0/deployments, /v0/env, /v0/domains, etc.       │
└───────────────────────────┬─────────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        ▼                   ▼                   ▼
┌──────────────┐   ┌──────────────┐   ┌──────────────┐
│ File Storage │   │ Build Worker │   │ CDN/Edge     │
│ (S3/R2/etc)  │   │ (optional)   │   │ (serve)      │
└──────────────┘   └──────────────┘   └──────────────┘
```

### Build Pipeline (ported from Vercel)

1. **Framework detection** — `@vercel/fs-detectors` + `@vercel/frameworks` → scan `package.json`, config files
2. **Build** — Run framework build (npm/pnpm/yarn run build) via sandbox; output to `dist` or `.appz/output`
3. **Output** — Static files + optional `config.json` (routes, redirects)

### No Remote Build (Phase 1)

Start with **prebuilt only**: user runs `appz build` locally, then `appz deploy` uploads the output. No remote build workers initially.

---

## Phased Approach

### Phase 1: Core Deploy (prebuilt)

- Port file tree building from `@vercel/build-utils`
- Port upload flow from `@vercel/client` (create → upload → continue)
- Appz API: deployment creation, file upload, storage, URL assignment
- CLI: `appz deploy` uploads to Appz (no Vercel CLI)

### Phase 2: Build Pipeline

- Port framework detection from `@vercel/fs-detectors`, `@vercel/frameworks`
- Port static-build orchestration (run build, validate output)
- `appz build` produces standardized output
- Optional: Build Output API format (`.appz/output/config.json`)

### Phase 3: Env, Pull, Logs, Inspect

- Port env management (add/ls/rm/pull)
- Port pull (sync project config + env to local)
- Port logs streaming
- Port inspect (deployment details)

### Phase 4: Remote Build (optional)

- Build worker service
- Accept source upload, run build remotely, then deploy

### Phase 5: Full Parity

- Domains, aliases, teams (Appz may already have)
- Monorepo (project.json vs repo.json)
- Dev server emulation (lower priority)

---

## Files to Study (appz-ref/vercel)

| Component | Key Files |
|-----------|-----------|
| Deploy flow | `packages/cli/src/commands/deploy/`, `packages/cli/src/util/deploy/` |
| Client | `packages/client/src/create-deployment.ts`, `upload.ts`, `continue.ts` |
| Build utils | `packages/build-utils/src/` (glob, Files, runCommand, etc.) |
| Static build | `packages/static-build/src/index.ts` |
| Frameworks | `packages/frameworks/src/` |
| FS detectors | `packages/fs-detectors/src/` |
| Build output | `packages/static-build/src/utils/build-output-v2.ts`, `build-output-v3.ts` |
| Config | `packages/static-config/`, `packages/config/` |

---

## Design Decisions

- **Output format**: Adopt Vercel's Build Output v3 structure for `.appz/output` to ease future compatibility or migration tooling
- **File storage**: Appz API must provide upload URLs (presigned or direct). Content-addressed (SHA) for dedup
- **Auth**: `APPZ_TOKEN` (existing). No Vercel token
- **Config**: `appz.json` (existing). Map `vercel.json` concepts into `appz.json` where needed
