# Appz Deploy API Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement full `appz deploy` backend support in appz-dev: create/upload/continue flow, R2 storage, static serving worker, and queue-based orphan cleanup.

**Architecture:** v0 API extended with file-aware create/upload/continue; new static worker for serving; new cleanup worker consuming queue. Content-addressed storage (git-style) in R2; D1 blobs + deployment_blobs for existence checks and orphan detection.

**Tech Stack:** Cloudflare Workers (Hono), D1, R2, Queues, TypeScript

**Reference:** [2025-02-20-appz-deploy-api-design.md](./2025-02-20-appz-deploy-api-design.md)

---

## Phase 1: Storage Setup

### Task 1: D1 migration — blobs and deployment_blobs tables

**Files:**
- Create: `appz-dev/packages/db/db-d1/src/db/schema.ts` (add blobs, deployment_blobs)
- Create: `appz-dev/packages/db/db-d1/drizzle/0010_deploy_blobs.sql` (migration)

**Step 1: Add Drizzle schema for blobs and deployment_blobs**

In `packages/db/db-d1/src/db/schema.ts`, add `primaryKey` to imports from `drizzle-orm`, then append after the `deployment` table definition:

```typescript
export const blobs = sqliteTable(
  'blobs',
  {
    sha: text('sha').primaryKey(),
    size: integer('size').notNull(),
    firstSeenAt: integer('first_seen_at', { mode: 'timestamp_ms' }).notNull(),
    lastAccessedAt: integer('last_accessed_at', { mode: 'timestamp_ms' }),
  },
  (table) => [index('idx_blobs_last_accessed').on(table.lastAccessedAt)],
);

export const deploymentBlobs = sqliteTable(
  'deployment_blobs',
  {
    deploymentId: text('deployment_id')
      .notNull()
      .references(() => deployment.id, { onDelete: 'cascade' }),
    sha: text('sha')
      .notNull()
      .references(() => blobs.sha, { onDelete: 'cascade' }),
    path: text('path').notNull(),
    contentType: text('content_type').notNull(),
  },
  (table) => [
    primaryKey({ columns: [table.deploymentId, table.path] }),
    index('idx_deployment_blobs_sha').on(table.sha),
  ],
);
```

**Step 2: Generate migration**

Run: `cd appz-dev/packages/db/db-d1 && pnpm drizzle-kit generate`

Expected: New migration file `0010_*.sql` with CREATE TABLE for blobs and deployment_blobs.

**Step 3: Manually verify migration SQL**

Ensure the migration has:
- `CREATE TABLE blobs (sha, size, first_seen_at, last_accessed_at)`
- `CREATE TABLE deployment_blobs (deployment_id, sha, path, content_type)` with FKs

**Step 4: Apply migration locally**

Run: `cd appz-dev && pnpm wrangler d1 migrations apply DB_V0 --local`
Expected: Migration applied successfully.

**Step 5: Commit**

```bash
git add packages/db/db-d1/
git commit -m "feat(db): add blobs and deployment_blobs tables for deploy storage"
```

---

### Task 2: R2 bucket bindings and wrangler config

**Files:**
- Modify: `appz-dev/apps/workers/v0/wrangler.jsonc`
- Create: R2 buckets via wrangler (manual/script)

**Step 1: Add R2 bindings to v0 wrangler**

In `apps/workers/v0/wrangler.jsonc`, add to the top-level config (alongside existing r2_buckets if any):

```jsonc
"r2_buckets": [
  { "binding": "APPZ_DEPLOY_BLOBS", "bucket_name": "appz-deploy-blobs" },
  { "binding": "APPZ_DEPLOY_MANIFESTS", "bucket_name": "appz-deploy-manifests" }
],
```

Merge with existing `r2_buckets` (e.g. APPZ_TELEMETRY_BUCKET) into a single array.

**Step 2: Create R2 buckets (Cloudflare dashboard or wrangler)**

Run: `wrangler r2 bucket create appz-deploy-blobs` and `wrangler r2 bucket create appz-deploy-manifests`

Or create via Cloudflare dashboard. Buckets must exist before deploy.

**Step 3: Add Queue producer binding**

In wrangler.jsonc, add:

```jsonc
"queues": {
  "producers": [{ "binding": "APPZ_DEPLOY_CLEANUP_QUEUE", "queue": "appz-deploy-cleanup" }]
}
```

**Step 4: Create the queue**

Run: `wrangler queues create appz-deploy-cleanup`
Expected: Queue created.

**Step 5: Add types to worker-configuration.d.ts**

In `apps/workers/v0/worker-configuration.d.ts` or equivalent env types, add:

```typescript
APPZ_DEPLOY_BLOBS: R2Bucket;
APPZ_DEPLOY_MANIFESTS: R2Bucket;
APPZ_DEPLOY_CLEANUP_QUEUE: Queue;
```

**Step 6: Commit**

```bash
git add apps/workers/v0/wrangler.jsonc
git commit -m "feat(v0): add R2 and Queue bindings for deploy"
```

---

## Phase 2: v0 Deployment API Handlers

### Task 3: Refactor POST /v0/deployments — accept files, return missing_files or created

**Files:**
- Modify: `appz-dev/apps/workers/v0/src/routers/deployments/deployment.handler.ts`
- Modify: `appz-dev/apps/workers/v0/src/routers/deployments/deployment.processor.ts`
- Create: `appz-dev/apps/workers/v0/src/routers/deployments/deploy-create.processor.ts` (optional, or inline in processor)

**Step 1: Add types for create request**

Create or extend types for:
```typescript
interface DeploymentCreateBody {
  projectId?: string;
  teamId?: string;
  type?: 'preview' | 'production';
  target?: string;
  name?: string;
  meta?: Record<string, unknown>;
  files?: Array<{ file: string; sha?: string; size?: number; mode?: number }>;
}
```

**Step 2: Implement createDeploymentWithFiles in processor**

In `deployment.processor.ts`, add function `createDeploymentWithFiles` that:
1. Validates projectId, teamId, user access
2. Creates deployment row (status: 'building')
3. If files provided: batch-query blobs (chunks of 500) for `sha IN (...)`
4. If all exist: build manifest, write to R2, insert blobs (INSERT OR IGNORE), insert deployment_blobs, update deployment status/url, return { deployment }
5. If any missing: return { missingFiles: true, deploymentId, missing: [...] }

**Step 3: Update POST handler to parse files and call new processor**

In `deployment.handler.ts`, change POST to:
- Parse body as DeploymentCreateBody
- Call createDeploymentWithFiles(db, r2Manifests, r2Blobs, authWorker, userId, body)
- If missingFiles: return 400 with `{ code: "missing_files", deploymentId, missing }`
- Else: return 201 with deployment JSON

**Step 4: Wire R2 and Queue into handler context**

Ensure `c.env.APPZ_DEPLOY_BLOBS`, `APPZ_DEPLOY_MANIFESTS` are passed to processor. May need to extend AppBindings.

**Step 5: Add helper for blob key**

```typescript
function blobKey(sha: string): string {
  return `objects/${sha.slice(0, 2)}/${sha.slice(2)}`;
}
```

**Step 6: Add helper for contentType from path**

```typescript
function contentTypeFromPath(path: string): string {
  const ext = path.split('.').pop() ?? '';
  const mime: Record<string, string> = {
    html: 'text/html', css: 'text/css', js: 'application/javascript',
    json: 'application/json', png: 'image/png', jpg: 'image/jpeg',
    svg: 'image/svg+xml', ico: 'image/x-icon', woff2: 'font/woff2',
  };
  return mime[ext] ?? 'application/octet-stream';
}
```

**Step 7: Test POST with files (manual)**

Use curl or Postman:
- POST /v0/deployments with `{ projectId, teamId, files: [{ file: "index.html", sha: "abc", size: 10, mode: 33188 }] }`
- Expect 400 missing_files (blob abc doesn't exist yet)

**Step 8: Commit**

```bash
git add apps/workers/v0/src/routers/deployments/
git commit -m "feat(v0): POST deployments with files, return missing_files when blobs missing"
```

---

### Task 4: POST /v0/deployments/:id/files — upload blob

**Files:**
- Modify: `appz-dev/apps/workers/v0/src/routers/deployments/deployment.handler.ts`
- Modify: `appz-dev/apps/workers/v0/src/routers/deployments/deployment.processor.ts`

**Step 1: Add upload handler**

In `deployment.handler.ts`:
```typescript
deploymentsRouter.post('/:id/files', async (c) => {
  const user = c.var.session?.user;
  if (!user) return c.json({ message: 'Unauthorized' }, 401);
  const id = c.req.param('id');
  const digest = c.req.header('x-now-digest');  // SHA
  const sizeStr = c.req.header('x-now-size');
  if (!digest || !sizeStr) return c.json({ message: 'x-now-digest and x-now-size required' }, 400);
  const body = await c.req.arrayBuffer();
  if (body.byteLength !== parseInt(sizeStr, 10))
    return c.json({ message: 'Body size mismatch' }, 400);
  const result = await uploadDeploymentFile(
    c.var.dbd1, c.env.APPZ_DEPLOY_BLOBS, authWorkerLike(c.env), user.id,
    id, digest, parseInt(sizeStr, 10), new Uint8Array(body), getTeamScope(c),
  );
  if (result?.forbidden) return c.json({ error: 'forbidden' }, 403);
  if (!result?.ok) return c.json({ message: 'Not found or upload failed' }, 404);
  return new Response(null, { status: 200 });
});
```

**Step 2: Implement uploadDeploymentFile in processor**

```typescript
export async function uploadDeploymentFile(
  db, r2Blobs, authWorker, userId, deploymentId, sha, size, data, scope,
): Promise<{ ok: true } | { forbidden: true } | null> {
  const getResult = await getDeployment(db, authWorker, userId, deploymentId, scope);
  if (!(getResult && 'deployment' in getResult)) return getResult;
  const dep = getResult.deployment;
  if (dep.status !== 'building' && dep.status !== 'pending') return null;
  const key = `objects/${sha.slice(0, 2)}/${sha.slice(2)}`;
  await r2Blobs.put(key, data);
  const now = new Date();
  await db.prepare(
    'INSERT OR IGNORE INTO blobs (sha, size, first_seen_at) VALUES (?, ?, ?)'
  ).bind(sha, size, now.getTime()).run();
  return { ok: true };
}
```

**Step 3: Run local dev and test**

`cd appz-dev && pnpm exec wrangler dev` — POST to /v0/deployments/:id/files with x-now-digest, x-now-size, body.

**Step 4: Commit**

```bash
git add apps/workers/v0/src/routers/deployments/
git commit -m "feat(v0): POST deployments/:id/files upload blob to R2 and blobs table"
```

---

### Task 5: POST /v0/deployments/:id/continue — complete deployment

**Files:**
- Modify: `appz-dev/apps/workers/v0/src/routers/deployments/deployment.handler.ts`
- Modify: `appz-dev/apps/workers/v0/src/routers/deployments/deployment.processor.ts`

**Step 1: Add continue handler**

```typescript
deploymentsRouter.post('/:id/continue', async (c) => {
  const user = c.var.session?.user;
  if (!user) return c.json({ message: 'Unauthorized' }, 401);
  const id = c.req.param('id');
  let body: { files?: Array<{ file: string; sha?: string; size?: number; mode?: number }> };
  try { body = await c.req.json(); } catch { return c.json({ message: 'Invalid JSON' }, 400); }
  const result = await continueDeployment(
    c.var.dbd1, c.env.APPZ_DEPLOY_BLOBS, c.env.APPZ_DEPLOY_MANIFESTS,
    authWorkerLike(c.env), user.id, id, body.files ?? [], getTeamScope(c),
  );
  if (result?.forbidden) return c.json({ error: 'forbidden' }, 403);
  if (result?.missingFiles) return c.json({
    code: 'missing_files', deploymentId: id, missing: result.missing,
  }, 400);
  if (!(result && 'deployment' in result)) return c.json({ message: 'Not found' }, 404);
  return c.json(result.deployment, 200);
});
```

**Step 2: Implement continueDeployment**

- Get deployment, validate access, status in building/pending
- For each file: check blobs table for sha; if any missing, return { missingFiles: true, missing: [...] }
- Build manifest JSON (path -> { hash, size, contentType })
- Write manifest to R2: deployments/{id}/manifest.json
- Insert blobs (INSERT OR IGNORE)
- Insert deployment_blobs (deployment_id, sha, path, content_type)
- Update deployment: status='ready', url = buildUrl(project, deployment, type)
- Return deployment

**Step 3: Implement buildUrl**

For preview: `{projectSlug}-{deploymentId}.preview.appz.dev`
For production: `{projectSlug}.appz.dev`
(Need project slug from project table — join or fetch.)

**Step 4: Commit**

```bash
git add apps/workers/v0/src/routers/deployments/
git commit -m "feat(v0): POST deployments/:id/continue completes deployment with manifest"
```

---

### Task 6: DELETE /v0/deployments/:id — enqueue cleanup

**Files:**
- Modify: `appz-dev/apps/workers/v0/src/routers/deployments/deployment.handler.ts`
- Modify: `appz-dev/apps/workers/v0/src/routers/deployments/deployment.processor.ts`

**Step 1: Update deleteDeployment to enqueue**

After soft-delete (`UPDATE deployment SET deleted_at = ?`), add:
```typescript
await c.env.APPZ_DEPLOY_CLEANUP_QUEUE.send({ deploymentId: id });
```

**Step 2: Commit**

```bash
git add apps/workers/v0/src/routers/deployments/
git commit -m "feat(v0): DELETE deployments enqueues cleanup job"
```

---

## Phase 3: Static Serving Worker

### Task 7: Create appz-worker-v0-static worker

**Files:**
- Create: `appz-dev/apps/workers/v0-static/src/index.ts`
- Create: `appz-dev/apps/workers/v0-static/wrangler.jsonc`
- Create: `appz-dev/apps/workers/v0-static/package.json` (or use workspace root)

**Step 1: Create worker entry**

`apps/workers/v0-static/src/index.ts`:
```typescript
export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const host = url.hostname;
    const path = url.pathname || '/';
    const pathForLookup = path.endsWith('/') ? path + 'index.html' : path;

    // Parse host: {project}-{deploymentId}.preview.appz.dev or {project}.appz.dev
    let deploymentId: string;
    if (host.endsWith('.preview.appz.dev')) {
      const sub = host.replace('.preview.appz.dev', '');
      const lastDash = sub.lastIndexOf('-');
      deploymentId = lastDash >= 0 ? sub.slice(lastDash + 1) : sub;
    } else {
      // Production: resolve project slug -> deployment
      const projectSlug = host.replace('.appz.dev', '');
      deploymentId = await resolveProductionDeployment(env, projectSlug);
      if (!deploymentId) return new Response('Not Found', { status: 404 });
    }

    const manifest = await getManifest(env, deploymentId);
    if (!manifest) return new Response('Not Found', { status: 404 });
    const fileInfo = manifest.files[pathForLookup] ?? manifest.files['/index.html'];
    if (!fileInfo) return new Response('Not Found', { status: 404 });

    const sha = fileInfo.hash;
    const key = `objects/${sha.slice(0, 2)}/${sha.slice(2)}`;
    const object = await env.APPZ_DEPLOY_BLOBS.get(key);
    if (!object) return new Response('Not Found', { status: 404 });

    return new Response(object.body, {
      headers: {
        'Content-Type': fileInfo.contentType ?? 'application/octet-stream',
        'Cache-Control': 'public, max-age=31536000, immutable',
        'ETag': `"${sha}"`,
      },
    });
  },
};
```

**Step 2: Implement getManifest and resolveProductionDeployment**

- getManifest: optionally check KV `manifest:{deploymentId}`; else R2 `deployments/{id}/manifest.json`; cache in KV
- resolveProductionDeployment: D1 query `SELECT id FROM deployment WHERE projectId IN (SELECT id FROM project WHERE slug=?) AND type='production' AND deleted_at IS NULL AND status='ready' ORDER BY createdAt DESC LIMIT 1`

**Step 3: wrangler.jsonc for v0-static**

- Bind APPZ_DEPLOY_BLOBS, APPZ_DEPLOY_MANIFESTS, DB_V0, KV (optional)
- Routes: `*.preview.appz.dev`, `*.appz.dev` (exclude api.appz.dev)

**Step 4: Commit**

```bash
git add apps/workers/v0-static/
git commit -m "feat: add v0-static worker for deployment asset serving"
```

---

## Phase 4: Deploy Cleanup Worker

### Task 8: Create appz-worker-deploy-cleanup queue consumer

**Files:**
- Create: `appz-dev/apps/workers/deploy-cleanup/src/index.ts`
- Create: `appz-dev/apps/workers/deploy-cleanup/wrangler.jsonc`

**Step 1: Queue consumer**

```typescript
export default {
  async queue(batch: MessageBatch<{ deploymentId: string }>, env: Env): Promise<void> {
    for (const msg of batch.messages) {
      const { deploymentId } = msg.body;
      const db = env.DB_V0;
      const blobs = await db.prepare(
        'SELECT DISTINCT sha FROM deployment_blobs WHERE deployment_id = ?'
      ).bind(deploymentId).all();
      const orphaned: string[] = [];
      for (const row of blobs.results) {
        const ref = await db.prepare(
          'SELECT 1 FROM deployment_blobs db JOIN deployment d ON db.deployment_id = d.id WHERE db.sha = ? AND d.deleted_at IS NULL LIMIT 1'
        ).bind(row.sha).first();
        if (!ref) orphaned.push(row.sha);
      }
      for (const sha of orphaned) {
        const key = `objects/${sha.slice(0, 2)}/${sha.slice(2)}`;
        await env.APPZ_DEPLOY_BLOBS.delete(key);
        await db.prepare('DELETE FROM blobs WHERE sha = ?').bind(sha).run();
      }
      await env.APPZ_DEPLOY_MANIFESTS.delete(`deployments/${deploymentId}/manifest.json`);
      await db.prepare('DELETE FROM deployment_blobs WHERE deployment_id = ?').bind(deploymentId).run();
      msg.ack();
    }
  },
};
```

**Step 2: wrangler.jsonc**

- consumer for appz-deploy-cleanup queue
- Bind DB_V0, APPZ_DEPLOY_BLOBS, APPZ_DEPLOY_MANIFESTS

**Step 3: Commit**

```bash
git add apps/workers/deploy-cleanup/
git commit -m "feat: add deploy-cleanup worker for orphan blob deletion"
```

---

## Phase 5: Integration and Verification

### Task 9: End-to-end test with CLI

**Files:**
- appz-cli: `crates/appz-client`, `crates/api`
- appz-dev: ensure API base URL points to local/staging

**Step 1: Run appz-dev locally**

`cd appz-dev && pnpm exec wrangler dev` for v0; run v0-static and deploy-cleanup if separate processes.

**Step 2: Link project and deploy from appz-cli**

```bash
cd /path/to/test-project
appz link  # or manual .appz/project.json
appz build
appz deploy --provider appz
```

**Step 3: Verify**

- Deployment created
- Files uploaded
- Continue completes
- URL returns 200 with content
- Delete deployment; verify queue runs; orphan blobs removed

**Step 4: Commit**

```bash
git add .
git commit -m "chore: verify deploy flow end-to-end"
```

---

## Verification Checklist

- [ ] D1 migration applied
- [ ] R2 buckets exist and writable
- [ ] POST /v0/deployments with files returns 201 or missing_files
- [ ] POST /v0/deployments/:id/files uploads and inserts blobs
- [ ] POST /v0/deployments/:id/continue completes deployment
- [ ] DELETE enqueues cleanup
- [ ] v0-static serves at preview URL
- [ ] deploy-cleanup removes orphaned blobs
- [ ] CLI appz deploy works end-to-end
