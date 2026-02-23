# Appz Environments — Vercel Parity Design

> **Status:** Draft  
> **Date:** 2025-02-21  
> **Reference:** Vercel docs (Local, Preview, Production, Custom Environments)

## Goal

Implement end-to-end environment handling in Appz to match Vercel's UX: three default environments (Local/Preview/Production), env var management per environment, and pull/deploy flows that respect target environment.

## Current State

| Capability                 | Appz                            | Vercel                           |
|---------------------------|----------------------------------|----------------------------------|
| Default environments      | production, preview, development | Same + custom (Pro/Enterprise)   |
| `pull` command            | Hardcoded development, .env.local | `--environment` → `.env.{env}.local` |
| `env pull`                | `--target` (dev/preview/prod)    | `--environment`                   |
| `deploy --target`         | preview, production, staging     | preview, production, custom      |
| Backend env targets       | production, preview, development | Same                             |

## Design

### 1. Environment Model

**Default environments (no backend change):**

- **development** — Local development; pulled vars for `.env.local`
- **preview** — Preview deployments (non-prod branch)
- **production** — Production deployments

**Output file naming (Vercel parity):**

- `appz pull` (default env=development) → `.env.local`
- `appz pull --environment=development` → `.env.local`
- `appz pull --environment=preview` → `.env.preview.local`
- `appz pull --environment=production` → `.env.production.local`

For development, `.env.local` stays the default; for preview/production we use `.env.{env}.local` to avoid overwriting local dev vars.

### 2. CLI Changes

**`appz pull`**

- Add `--environment` (alias `-e`), default `development`
- Add `--yes` (existing in env pull)
- Pass environment to `pull_env` and derive output filename
- Valid values: `development`, `preview`, `production`

**`appz env pull`**

- Keep current `--target` (Vercel uses `--environment`; we retain `--target` for consistency with `env add/rm`)
- Output filename: if target is `development`, use `.env.local`; else `.env.{target}.local`
- Optional: add `--environment` as alias for `--target` for Vercel compatibility

**`appz deploy`**

- Already supports `--target` (preview/production/staging)
- Backend currently only stores production/preview/development for env vars
- No change for Phase 1; staging maps to preview on backend for env lookup

### 3. Data Flow

```
appz pull [--environment=development|preview|production]
  → Fetch project
  → pull_env(session, filename_for(env), env, yes)
  → Write .appz/project.json + .env{.env_suffix}.local

appz env pull [--target=development|preview|production] [filename]
  → pull_env(session, filename ?? default_for(target), target, yes)
  → Write file
```

### 4. E2E Test Strategy

**Scope (Phase 1):**

1. **CLI validation** — Help text, flag parsing, invalid target
2. **Unlinked project** — `appz pull` fails when not linked (like open_e2e)
3. **Output file selection** — When env=preview, verify `.env.preview.local` is used (needs mock/stub)
4. **Optional integration** — Full flow against dev API when `APPZ_TOKEN` set (documented, CI-optional)

**Approach:**

- Reuse `crates/cli/tests/` pattern (bin-based, tempdir)
- New file: `crates/cli/tests/environments_e2e.rs`
- Tests that need API: skip when `APPZ_TOKEN` unset (or use `#[ignore]` with opt-in)

### 5. Custom Environments (Future)

Vercel custom environments (staging, QA) require:

- Pro/Enterprise plans
- API: `POST /v9/projects/:id/custom-environments`
- Backend: `custom_environment` table, project limits

Out of scope for this design; document as future work.

## Architecture

- **CLI:** Add `--environment` to `Pull` variant in `app.rs`; update `pull.rs` to accept env and derive filename.
- **Shared logic:** `env/pull.rs` already takes `filename` and `target`; `pull.rs` will pass `default_filename(environment)` and `environment`.
- **Backend:** No changes; already supports production/preview/development.

## Error Handling

- Invalid `--environment`: clear error: "Environment must be one of: development, preview, production"
- Unlinked project: existing "Project not linked. Run 'appz link'..." message

## Success Criteria

1. `appz pull --environment=preview` writes `.env.preview.local`
2. `appz pull` (default) writes `.env.local` (development)
3. `appz env pull --target=production` can write `.env.production.local` when filename inferred
4. E2E tests cover unlinked case, help, and (optionally) full flow with API
