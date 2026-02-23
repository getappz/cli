# Appz Deployment URL — Vercel Parity

> Design: Generate deployment URLs following Vercel's structure for commit, branch, CLI, truncation, and anti-phishing.

## Vercel URL Reference

| Source | Pattern | Example |
|--------|---------|---------|
| **Commit** (Git) | `<project>-<unique-hash>-<scope-slug>.vercel.app` | `myapp-a1b2c3d4e-acme.vercel.app` |
| **Branch** (Git) | `<project>-git-<branch>-<scope-slug>.vercel.app` | `myapp-git-main-acme.vercel.app` |
| **CLI production** | `<project>-<scope-slug>.vercel.app` | `myapp-acme.vercel.app` |
| **CLI team/author** | `<project>-<author>-<scope-slug>.vercel.app` | `myapp-johndoe-acme.vercel.app` |

**Components:**
- `project-name` — project slug
- `unique-hash` — 9 alphanumeric chars (commit URL only)
- `scope-slug` — team/account slug (not name)
- `branch-name` — Git branch name
- `author-name` — deployer's username (for team CLI deploys)

**Rules:**
- **Truncation:** If > 63 chars before `.vercel.app`, truncate.
- **Anti-phishing:** If `project-name` resembles a domain (e.g. `www-company-com`), shorten (e.g. to `company`).

## Current Appz Behavior

| Type | Pattern | Example |
|------|---------|---------|
| Production | `<project-slug>.appz.dev` | `myapp.appz.dev` |
| Preview | `<project-slug>-<deployment-uuid>.preview.appz.dev` | `myapp-550e8400-e29b-41d4-a716-446655440000.preview.appz.dev` |

**Gaps vs Vercel:**
1. No scope-slug in production URL
2. Preview uses full UUID (36 chars) instead of 9-char hash
3. No branch URL format (`-git-<branch>-`)
4. No author-specific URL for team deploys
5. No truncation
6. No anti-phishing shortening

## Target Appz Behavior

### Production URLs

| Context | Pattern | Example |
|---------|---------|---------|
| With team | `<project>-<scope>.appz.dev` | `myapp-acme.appz.dev` |
| Personal scope | `<project>.appz.dev` | `myapp.appz.dev` |

### Preview URLs

| Context | Pattern | Example |
|---------|---------|---------|
| **Commit** (CLI, Git commit) | `<project>-<hash>-<scope>.appz.dev` | `myapp-a1b2c3d4e-acme.appz.dev` |
| **Branch** (Git integration) | `<project>-git-<branch>-<scope>.appz.dev` | `myapp-git-main-acme.appz.dev` |
| **CLI team/author** | `<project>-<author>-<scope>.appz.dev` | `myapp-johndoe-acme.appz.dev` |
| Fallback (no metadata) | `<project>-<hash>-<scope>.appz.dev` | `myapp-a1b2c3d4e-acme.appz.dev` |

**Hash generation:**
- 9 random alphanumeric chars (a-z, 0-9)
- Store in deployment metadata or derive from `deploymentId` (first 8–9 chars, base62)
- Must be unique per deployment; collisions handled by lookup (project+hash+scope)

### Truncation

- Subdomain (before `.appz.dev`) max 63 chars.
- Truncate from the right, preserving project-slug and scope as much as possible.
- Example: `very-long-project-name-a1b2c3d4e-very-long-team-slug` → truncate to fit.

### Anti-phishing

- If project slug looks like a domain (e.g. `www-company-com`, `subdomain-example-com`):
  - Replace with shortened form (e.g. `company`, `example`).
- Pattern: `www-*`, `*-*-*-com`, etc. → extract meaningful part.

## Implementation Plan

### 1. Backend (appz-dev)

**Files:**
- `apps/workers/v0/src/routers/deployments/deployment.processor.ts` — URL builder
- `apps/workers/v0-static/src/index.ts` — host parsing for new URL formats

**New `buildDeploymentUrl()` helper:**
```ts
function buildDeploymentUrl(params: {
  projectSlug: string;
  scopeSlug: string | null;
  type: 'production' | 'preview';
  uniqueHash?: string;      // 9 chars for commit-style
  branchName?: string;      // for -git-<branch>-
  authorSlug?: string;      // for team CLI
}): string
```

**Data flow:**
- `createDeployment` / `continueDeployment`: need project slug, team slug (from auth worker), metadata (git_branch, unique_hash)
- Generate 9-char hash if not in meta (e.g. from `crypto.randomUUID().replace(/-/g,'').slice(0,9)` or base62 of deploymentId)
- Lookup team slug via `AUTH_WORKER.getTeams()` → find team by id → slug

**v0-static routing:**
- Parse host: `{project}-{hash}-{scope}.preview.appz.dev` → deployment by (project, hash, scope) or by stored url
- Parse host: `{project}-git-{branch}-{scope}.preview.appz.dev` → deployment by branch alias
- Production: `{project}-{scope}.appz.dev` or `{project}.appz.dev` (personal)

**Schema:** Deployment stores `url`; may need `urlHash` (9 chars) for reverse lookup if we move away from embedding in URL.

### 2. CLI (appz-cli)

**Deploy meta:** Ensure CLI sends:
- `meta.git_branch` — from `git branch --show-current`
- `meta.git_commit` — from `git rev-parse HEAD`
- Backend generates `unique_hash` if not provided

**No URL building in CLI** — backend returns final URL.

### 3. Database / Resolution

**Option A:** Store full URL; v0-static parses host and does lookup by `url` column (current approach).
- Works if we keep URLs in DB.
- v0-static must support multiple host patterns.

**Option B:** Store `url_hash` (9 chars) in deployment; v0-static uses `project_slug + url_hash + scope_slug` to look up.
- Allows shorter URLs.
- Need index: `(project_id, url_hash)` or join project for slug.

**Recommendation:** Option A initially — store full URL, expand v0-static parsing. Simpler migration.

### 4. Migration

- **Phase 1:** Add new URL format; keep supporting old `{project}-{uuid}.preview.appz.dev` for existing deployments.
- **Phase 2:** New deployments get Vercel-style URLs.
- **Phase 3:** (Optional) backfill or redirect old URLs.

## File Changes Summary

| Location | Change |
|----------|--------|
| `appz-dev/.../deployment.processor.ts` | Add `buildDeploymentUrl()`, use team slug, 9-char hash, truncation, anti-phishing |
| `appz-dev/.../deployment.handler.ts` | Pass team slug to processor (from scope) |
| `appz-dev/.../v0-static/index.ts` | Parse `project-hash-scope`, `project-git-branch-scope` preview hosts |
| `appz-cli/.../deploy.rs` | Add `git_branch`, `git_commit` to meta when in git repo |
| `appz-dev` auth worker | Ensure `getTeams` returns slug (already does) |

## Truncation Algorithm

```ts
const MAX_SUBDOMAIN = 63;
const suffix = type === 'production' ? '.appz.dev' : '.preview.appz.dev';
const maxContentLen = MAX_SUBDOMAIN - suffix.length;

function truncateSubdomain(parts: string[], separator = '-'): string {
  let result = parts.join(separator);
  if (result.length <= maxContentLen) return result;
  // Truncate from right, keep at least project + separator + hash
  const minKeep = Math.min(parts[0].length + 1 + 9, result.length);
  return result.slice(0, maxContentLen);
}
```

## Anti-phishing Algorithm

```ts
function sanitizeProjectSlug(slug: string): string {
  // www-something -> something
  if (slug.startsWith('www-')) return slug.slice(4);
  // domain-com, subdomain-domain-com -> domain
  const parts = slug.split('-');
  if (parts.length >= 2 && parts[parts.length - 1] === 'com') {
    return parts[parts.length - 2];
  }
  return slug;
}
```
