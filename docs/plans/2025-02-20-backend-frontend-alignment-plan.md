# Backend & Frontend Alignment Plan

> Align `appz-dev` (backend API + React frontend) with Phase 3 CLI changes (env, pull, logs, inspect).

**Reference:** `docs/plans/2025-02-20-vercel-alignment-implementation-plan.md`, CLI `crates/api` client shapes

---

## Summary of CLI Changes to Align

| CLI Feature       | API Endpoint                          | Status in Backend   |
|-------------------|----------------------------------------|--------------------|
| `appz env ls`     | `GET /v0/projects/:id/env`             | **Missing**        |
| `appz env add`    | `POST /v0/projects/:id/env`            | **Missing**        |
| `appz env rm`     | `DELETE /v0/projects/:id/env/:envId`   | **Missing**        |
| `appz env pull`   | Uses list with `decrypt=true`          | **Missing**        |
| `appz pull`       | `GET /v0/projects/:id` (exists)        | Exists             |
| `appz logs`       | `GET /v0/deployments/:id/logs`         | **Missing**        |
| `appz inspect`    | `GET /v0/deployments/:id`              | Exists             |

---

## Phase A: Backend API (appz-dev/apps/workers/v0)

### Task A1: Project env vars — schema and migration

**Files:**
- `packages/db/db-d1/src/db/schema.ts`
- New migration in `packages/db/db-d1/migrations/`

**Step 1: Add `project_env` table**

```sql
CREATE TABLE project_env (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  value_encrypted TEXT,  -- encrypted at rest; NULL when decrypted=false in list
  type TEXT NOT NULL DEFAULT 'plain',  -- 'plain' | 'secret'
  target TEXT NOT NULL,  -- 'production' | 'preview' | 'development'
  git_branch TEXT,
  created_at INTEGER,
  updated_at INTEGER,
  UNIQUE(project_id, key, target, git_branch)
);
CREATE INDEX idx_project_env_project ON project_env(project_id);
CREATE INDEX idx_project_env_target ON project_env(project_id, target);
```

**Step 2: Define Drizzle schema**

Add to `schema.ts`:

```ts
export const projectEnv = sqliteTable('project_env', {
  id: text('id').primaryKey(),
  projectId: text('project_id').notNull().references(() => project.id, { onDelete: 'cascade' }),
  key: text('key').notNull(),
  valueEncrypted: text('value_encrypted'),
  type: text('type').notNull().default('plain'),
  target: text('target').notNull(),
  gitBranch: text('git_branch'),
  createdAt: integer('created_at', { mode: 'timestamp_ms' }).default(sql`(unixepoch() * 1000)`).notNull(),
  updatedAt: integer('updated_at', { mode: 'timestamp_ms' }).default(sql`(unixepoch() * 1000)`).$onUpdate(() => new Date()).notNull(),
}, (table) => [
  index('idx_project_env_project').on(table.projectId),
  index('idx_project_env_target').on(table.projectId, table.target),
]);
```

**Step 3: Generate and apply migration**

```bash
cd packages/db/db-d1 && pnpm drizzle-kit generate && pnpm wrangler d1 migrations apply DB_V0
```

---

### Task A2: Project env vars — processor and handler

**Files:**
- Create: `apps/workers/v0/src/routers/projects/env.processor.ts`
- Create: `apps/workers/v0/src/routers/projects/env.handler.ts`
- Modify: `apps/workers/v0/src/routers/v0/v0.router.ts`
- Modify: `apps/workers/v0/src/routers/projects/project.handler.ts`

**Step 1: Implement env processor**

- `listEnv(dbd1, authWorker, userId, projectId, scope, { target?, decrypt })` — list env vars for project, filter by target, optionally decrypt values
- `addEnv(dbd1, authWorker, userId, projectId, scope, body, upsert?)` — insert or upsert env var
- `removeEnv(dbd1, authWorker, userId, projectId, envId, scope)` — delete by env id

Validate user has access to project (via team). Use industry-grade encryption (see **Encryption** section below) for all env values at rest.

**Step 2: Add env routes under projects**

Mount sub-routes:
- `GET /projects/:id/env` — query params: `target`, `decrypt`, `source`
- `POST /projects/:id/env` — body: `{ key, value, type?, target[], gitBranch? }`, query: `upsert=true` optional
- `DELETE /projects/:id/env/:envId`

**Step 3: Response shapes (match CLI models)**

```ts
// GET response
{ envs: Array<{ id, key, value?, type, target, gitBranch?, createdAt?, updatedAt? }> }

// POST: 201 No body or minimal
// DELETE: 204
```

---

### Task A3: Deployment logs endpoint

**Files:**
- Create: `apps/workers/v0/src/routers/deployments/logs.processor.ts`
- Modify: `apps/workers/v0/src/routers/deployments/deployment.handler.ts`

**Step 1: Define log source**

Options:
- **A:** Store build/runtime logs in D1 (new `deployment_log` table) when processing deployments
- **B:** Stream from Cloudflare Tail / Workers analytics (if available)
- **C:** Return empty array `{ logs: [] }` until logging pipeline exists

For MVP: implement `GET /deployments/:id/logs` returning `{ logs: [] }` with 200. Add processor that validates user has access to deployment, then returns stub.

**Step 2: Log entry shape (match CLI)**

```ts
{ logs: Array<{ id?, timestamp?, message?, level? }> }
```

**Step 3: Add route**

```ts
deploymentsRouter.get('/:id/logs', async (c) => {
  // Auth + scope check, get deployment, return logs
  return c.json({ logs: [] });
});
```

---

### Task A4: Project GET — ensure teamId in response

**Files:**
- `apps/workers/v0/src/routers/projects/project.processor.ts`

**Step 1: Verify `getProject` returns**

- `id`, `name`, `slug`, `teamId`, `createdAt`, `updatedAt` (and any existing fields)

The CLI `appz pull` uses this to update `.appz/project.json`. Ensure `teamId` is included in the JSON response.

---

## Phase B: Frontend (appz-dev/apps/app)

### Task B1: v0 API client — env and deployments

**Files:**
- `apps/app/src/lib/v0-api.ts`

**Step 1: Add env methods**

```ts
env: {
  list: (projectId: string, teamId: string, opts?: { target?: string }) =>
    v0Fetch<{ envs: EnvVar[] }>(`/projects/${projectId}/env?teamId=${teamId}${opts?.target ? `&target=${opts.target}` : ''}`),
  add: (projectId: string, teamId: string, body: AddEnvBody, upsert?: boolean) =>
    v0Fetch(`/projects/${projectId}/env${upsert ? '?upsert=true' : ''}`, { method: 'POST', body: JSON.stringify(body) }),
  remove: (projectId: string, teamId: string, envId: string) =>
    v0Fetch(`/projects/${projectId}/env/${envId}`, { method: 'DELETE' }),
},
deployments: {
  list: (projectId: string, teamId: string, opts?: { limit?: number }) =>
    v0Fetch<{ deployments: Deployment[]; pagination: { count: number } }>(
      `/deployments?projectId=${projectId}&teamId=${teamId}${opts?.limit ? `&limit=${opts.limit}` : ''}`
    ),
  get: (id: string) => v0Fetch<Deployment>(`/deployments/${id}`),
  logs: (id: string) => v0Fetch<{ logs: DeploymentLogEntry[] }>(`/deployments/${id}/logs`),
},
```

**Step 2: Add types**

```ts
interface EnvVar {
  id: string;
  key: string;
  value?: string;
  type?: string;
  target?: string | string[];
  gitBranch?: string;
  createdAt?: number;
  updatedAt?: number;
}

interface Deployment {
  id: string;
  projectId?: string;
  teamId?: string;
  status?: string;
  url?: string;
  createdAt: number;
  updatedAt: number;
  // ...extend as needed
}
```

---

### Task B2: ProjectEnv — wire to real API

**Files:**
- `apps/app/src/pages/appz/ProjectEnv.tsx`

**Step 1: Replace mock data**

- Use `useProject(projectId, teamId)` to get project
- Create `useProjectEnv(projectId, teamId)` that calls `v0Api.env.list`
- On mount: fetch env vars
- "Add Variable" form: call `v0Api.env.add` on submit
- Delete button: call `v0Api.env.remove` on confirm

**Step 2: Handle targets**

- Map API targets (`production`, `preview`, `development`) to UI labels
- When adding: collect selected targets, send as `target: ['production', 'preview']` etc.

**Step 3: Loading and error states**

- Show skeleton/spinner while loading
- Show toast on add/remove success or error

---

### Task B3: ProjectDeployments — wire to real API

**Files:**
- `apps/app/src/pages/appz/ProjectDeployments.tsx`

**Step 1: Replace mock data**

- Call `v0Api.deployments.list(project.id, teamId)` with project from `useProject`
- Map API deployment shape to UI: `status`, `type` (production/preview), `url`, `createdAt`

**Step 2: Link to deployment URL**

- Use `deployment.url` for "Visit" link
- Handle missing URL for Building/Error states

**Step 3: Optional — deployment detail / logs**

- Add route or modal for deployment detail
- "View logs" button → fetch `v0Api.deployments.logs(id)` and display in a drawer/modal

---

### Task B4: Deployment logs and inspect UI (optional)

**Files:**
- Create: `apps/app/src/pages/appz/DeploymentDetail.tsx` or extend ProjectDeployments

**Step 1: Deployment detail view**

- Show deployment id, status, url, createdAt
- "View logs" fetches `/deployments/:id/logs` and displays in a pre/code block or table

**Step 2: Routing**

- Add route like `/appz/:team/project/:projectId/deployments/:deploymentId` or open in modal from ProjectDeployments row

---

## Verification Checklist

- [ ] `GET /v0/projects/:id/env` returns `{ envs: [...] }` with target filter
- [ ] `POST /v0/projects/:id/env` creates env var
- [ ] `DELETE /v0/projects/:id/env/:envId` removes env var
- [ ] `GET /v0/deployments/:id/logs` returns `{ logs: [...] }` (can be empty)
- [ ] `GET /v0/projects/:id` includes `teamId` in response
- [ ] ProjectEnv page loads real env vars
- [ ] ProjectEnv add/remove work end-to-end
- [ ] ProjectDeployments loads real deployments
- [ ] CLI `appz env ls`, `add`, `rm`, `pull` work against backend
- [ ] CLI `appz logs`, `appz inspect` work against backend

---

## Execution Order

1. **Task A1** — Schema and migration (blocking for A2)
2. **Task A2** — Env processor and routes
3. **Task A3** — Deployment logs (stub OK)
4. **Task A4** — Verify project response
5. **Task B1** — v0-api client
6. **Task B2** — ProjectEnv wiring
7. **Task B3** — ProjectDeployments wiring
8. **Task B4** — Optional logs UI

---

## Encryption (Industry-Grade)

Env values must be encrypted at rest. Use the following approach:

### Algorithm: AES-256-GCM

- **Authenticated encryption** — provides confidentiality and integrity (no separate HMAC)
- Web Crypto API: `crypto.subtle.encrypt` / `decrypt` with `AES-GCM`
- 256-bit key, 96-bit (12-byte) random IV per encryption
- Tag length: 128 bits (default for GCM)

### Key Management

1. **Master key:** Store in `wrangler secret` (e.g. `ENV_ENCRYPTION_KEY`) — 32 bytes (256 bits), base64 or hex
2. **Key derivation:** Optional — use HKDF to derive per-project keys from master + projectId for key isolation
3. **Rotation:** Support key version prefix in ciphertext to allow rotation without re-encrypting all values at once

### Storage Format

Store in D1 as `value_encrypted`:

```
{version}:{iv_base64}:{ciphertext_base64}
```

- `version`: `v1` (for future key rotation)
- `iv`: 12-byte IV, base64url
- `ciphertext`: AES-GCM output (ciphertext + 16-byte auth tag), base64url

### Implementation (Node/Workers)

```ts
const ALG = 'AES-GCM';
const KEY_LEN = 256;
const IV_LEN = 12;

async function encrypt(plaintext: string, keyBase64: string): Promise<string> {
  const keyData = decodeBase64(keyBase64);
  const iv = crypto.getRandomValues(new Uint8Array(IV_LEN));
  const key = await crypto.subtle.importKey('raw', keyData, { name: ALG, length: KEY_LEN }, false, ['encrypt']);
  const enc = await crypto.subtle.encrypt(
    { name: ALG, iv, tagLength: 128 },
    key,
    new TextEncoder().encode(plaintext)
  );
  return `v1:${toBase64Url(iv)}:${toBase64Url(new Uint8Array(enc))}`;
}

async function decrypt(cipherBlob: string, keyBase64: string): Promise<string> {
  const [ver, ivB64, ctB64] = cipherBlob.split(':');
  if (ver !== 'v1') throw new Error('Unsupported encryption version');
  const key = await crypto.subtle.importKey('raw', decodeBase64(keyBase64), { name: ALG, length: KEY_LEN }, false, ['decrypt']);
  const dec = await crypto.subtle.decrypt(
    { name: ALG, iv: decodeBase64Url(ivB64), tagLength: 128 },
    key,
    decodeBase64Url(ctB64)
  );
  return new TextDecoder().decode(dec);
}
```

### Implementation & Key Rotation

Implementation: `appz-dev/apps/workers/v0/src/utils/env-encryption.ts`  
Full docs (setup, rotation, recovery): `appz-dev/docs/env-encryption.md`

### Security Requirements

- Never log or expose plaintext values (mask in API responses when `decrypt=false`)
- Use TLS for all API traffic (HTTPS)
- Restrict `decrypt=true` to authenticated requests with project access
- Generate master key with `openssl rand -base64 32` and store via `wrangler secret put ENV_ENCRYPTION_KEY`

---

## Notes

- **teamId:** All v0 project/deployment routes require `teamId` in query (or from session). The frontend obtains it from `session.activeTeamId`; the CLI uses `--scope` or `appz switch`.
- **CORS:** Ensure v0 routes allow frontend origin when using `credentials: 'include'`.
