# Appz Deploy API — Design Document

> Backend support for `appz deploy` with full CLI parity: scalable, cost-effective, content-addressed storage, safe deletion with orphan cleanup.

**Date:** 2025-02-20  
**Status:** Design approved

---

## Summary

Add API backend support for `appz deploy` to match the full deployment flow implemented in the CLI:

- **Workers:** v0 API + separate static-serving worker (Approach 3)
- **Storage:** Content-addressed (CAS) with git-style sharding; D1 for blob registry and reference tracking
- **Deletion:** Soft-delete deployment; queue job cleans orphaned blobs (shared blobs preserved)
- **URL patterns:** Vercel-aligned — production: `{project-slug}.appz.dev`, preview: `{project-slug}-{deployment-id}.preview.appz.dev`

---

## 1. Architecture Overview

### Workers

| Worker | Domain | Role |
|--------|--------|------|
| **appz-worker-v0** | `api.appz.dev` | Deployment API: create, upload, continue, list, get, delete |
| **appz-worker-v0-static** (new) | `*.preview.appz.dev`, `*.appz.dev` | Serves static deployment assets from R2 |
| **appz-worker-deploy-cleanup** (new) | — | Queue consumer: orphan blob deletion after deployment delete |

### Storage

- **D1 (DB_V0):** `deployment` (existing), `blobs` (new), `deployment_blobs` (new)
- **R2:** `appz-deploy-blobs` (objects), `appz-deploy-manifests` (manifests)
- **Cloudflare Queues:** `appz-deploy-cleanup` — delete events

### URL Patterns (Vercel-aligned)

| Target | Pattern | Example |
|--------|---------|---------|
| Production | `{project-slug}.appz.dev` | `myproject.appz.dev` |
| Preview | `{project-slug}-{deployment-id}.preview.appz.dev` | `myproject-abc123.preview.appz.dev` |
| Branch (future) | `{branch}-git-{project-slug}.preview.appz.dev` | — |

---

## 2. Storage Architecture

### 2.1 Content-Addressable Storage (R2)

**Blobs — `appz-deploy-blobs` bucket**

- Key: `objects/{sha[0:2]}/{sha[2:]}` (git-style sharding)
- Value: Raw file bytes
- Hash: SHA-1 via `x-now-digest` (CLI compatibility)
- Immutable; never modified after upload

**Manifests — `appz-deploy-manifests` bucket**

- Key: `deployments/{deployment_id}/manifest.json`
- Value: Path-keyed manifest for O(1) lookup when serving:

```json
{
  "id": "deploy_123",
  "timestamp": 1708185600,
  "target": "preview",
  "files": {
    "/index.html": {
      "hash": "abc123...",
      "size": 1024,
      "contentType": "text/html"
    },
    "/static/style.css": {
      "hash": "def456...",
      "size": 2048,
      "contentType": "text/css"
    }
  }
}
```

- Path format: Leading `/`, normalized
- `contentType` derived from extension for `Content-Type` header
- Mapping from CLI `{ file, sha, size, mode }` → `files["/" + file]: { hash, size, contentType }`

### 2.2 D1 Schema

**Existing:** `deployment` table (unchanged) — projectId, teamId, type, status, url, authorId, metadata, createdAt, updatedAt, deletedAt.

**New: `blobs` — blob registry**

```sql
CREATE TABLE blobs (
  sha TEXT PRIMARY KEY,
  size INTEGER NOT NULL,
  first_seen_at INTEGER NOT NULL,
  last_accessed_at INTEGER
);
CREATE INDEX idx_blobs_last_accessed ON blobs(last_accessed_at);
```

- Replaces naive `blob_index(sha)` — enables existence check without R2, plus analytics
- `last_accessed_at` optional for LRU cleanup (update on serve, or skip to avoid write amplification)

**New: `deployment_blobs` — many-to-many references**

```sql
CREATE TABLE deployment_blobs (
  deployment_id TEXT NOT NULL REFERENCES deployment(id) ON DELETE CASCADE,
  sha TEXT NOT NULL REFERENCES blobs(sha) ON DELETE CASCADE,
  path TEXT NOT NULL,
  content_type TEXT NOT NULL,
  PRIMARY KEY (deployment_id, path),
  FOREIGN KEY (deployment_id) REFERENCES deployment(id),
  FOREIGN KEY (sha) REFERENCES blobs(sha)
);
CREATE INDEX idx_deployment_blobs_sha ON deployment_blobs(sha);
```

- `path` per deployment enables “where is this blob used?”
- `content_type` stored for serving (reduces manifest parse on serve)
- Orphan detection: blobs with no active deployment reference

### 2.3 Reference Counting vs `metadata/refs.json`

D1 replaces a single `refs.json` file:

- No single-file write bottleneck
- Transactional integrity
- Efficient SQL for orphan queries
- Concurrent-safe

---

## 3. API Flow

### 3.1 POST /v0/deployments (create)

1. Auth + teamId scope (query or session)
2. Parse body: `{ projectId, target?, name?, files? }`
3. Validate project access via team membership
4. Create deployment row: `status: 'building'`, `type: preview|production`
5. If `files` provided:
   - Query `blobs` for existing SHAs: `SELECT sha FROM blobs WHERE sha IN (?, ...)` (batch ≤999)
   - If all SHAs exist: write manifest to R2, insert `deployment_blobs`, insert missing `blobs` (if any), set `status: 'ready'`, set `url`, return deployment (201)
   - If any missing: return `400` with `{ code: "missing_files", deploymentId, missing: [sha...] }`
6. If no `files`: return `missing_files` with deploymentId for client to upload then continue

**Scalability:** D1 query for existence; no R2 checks. Batch SHAs in chunks of 500–999.

### 3.2 POST /v0/deployments/:id/files (upload)

1. Auth + scope
2. Headers: `x-now-digest` (SHA), `x-now-size` (bytes); body = raw binary
3. Validate deployment exists, team access, not deleted, status in `building` or `pending`
4. Validate body length against `x-now-size`
5. Write blob to R2: `objects/{sha[0:2]}/{sha[2:]}`
6. `INSERT OR IGNORE INTO blobs(sha, size, first_seen_at) VALUES (?, ?, ?)`
7. Return 200 (no manifest change yet; continue inserts `deployment_blobs`)

### 3.3 POST /v0/deployments/:id/continue

1. Auth + scope
2. Parse body: `{ files: [{ file, sha, size?, mode }] }`
3. Validate deployment exists, team access, not deleted
4. For each SHA: check `blobs` table; if any missing, return `missing_files`
5. Write manifest to R2 at `deployments/{id}/manifest.json`
6. Insert `blobs` (INSERT OR IGNORE) for new SHAs
7. Insert `deployment_blobs(deployment_id, sha, path, content_type)` per file
8. Update deployment: `status: 'ready'`, `url` = production or preview pattern
9. Return deployment (200)

### 3.4 DELETE /v0/deployments/:id

1. Auth + scope
2. Soft-delete: `UPDATE deployment SET deleted_at = ? WHERE id = ?`
3. Enqueue `{ deploymentId }` to `appz-deploy-cleanup` queue
4. Return 200

### 3.5 GET /v0/deployments, GET /v0/deployments/:id

Unchanged; filter by `deleted_at IS NULL`.

---

## 4. Static Serving (v0-static worker)

### Flow

1. Parse host: `{project-slug}-{deployment-id}.preview.appz.dev` or `{project-slug}.appz.dev`
2. Resolve deployment: production → latest production deployment; preview → by deployment-id
3. Fetch manifest from R2 (with KV cache: `manifest:{deploymentId}`, TTL 3600s)
4. Look up path: `manifest.files[path]` or `manifest.files['/index.html']`
5. Fetch blob from R2: `objects/{sha[0:2]}/{sha[2:]}`
6. Return with `Content-Type`, `Cache-Control: public, max-age=31536000, immutable`, `ETag: "{sha}"`

### KV cache (optional)

- `active:{projectId}:production` → deploymentId (TTL 60s)
- `manifest:{deploymentId}` → manifest JSON (TTL 3600s)

Reduces R2 + D1 reads on repeat requests.

### Production resolution

- For `{project-slug}.appz.dev`: query `deployment` where `projectId` (by slug), `type='production'`, `deleted_at IS NULL`, `status='ready'`, order by `createdAt DESC` limit 1
- Cache in KV for 60s

---

## 5. Queue + Blob Cleanup

### 5.1 Delete Queue Consumer

On message `{ deploymentId }`:

1. Load deployment_blobs for this deployment (already soft-deleted)
2. For each unique `sha` in deployment_blobs:
   - Orphan check: `SELECT 1 FROM deployment_blobs db JOIN deployment d ON db.deployment_id = d.id WHERE db.sha = ? AND d.deleted_at IS NULL LIMIT 1`
   - If no rows: blob is orphaned
3. Delete orphaned blobs from R2: `objects/{sha[0:2]}/{sha[2:]}`
4. Delete from `blobs`: `DELETE FROM blobs WHERE sha IN (orphaned_list)`
5. Delete manifest from R2: `deployments/{deploymentId}/manifest.json`
6. Delete `deployment_blobs` rows for this deployment (cascade may handle)

### 5.2 Batch Orphan Query

```sql
SELECT b.sha FROM blobs b
WHERE NOT EXISTS (
  SELECT 1 FROM deployment_blobs db
  JOIN deployment d ON db.deployment_id = d.id
  WHERE db.sha = b.sha AND d.deleted_at IS NULL
);
```

### 5.3 Retention (optional)

Per-target retention:

- Production: keep last N (e.g. 100)
- Preview: keep last N (e.g. 10)

Soft-delete oldest beyond N; queue processes as above.

---

## 6. Error Handling

- **401:** Unauthorized
- **403:** Forbidden (wrong team/project)
- **404:** Deployment not found or deleted
- **400:** Invalid body; `missing_files` with `deploymentId` and `missing` array
- **413:** Payload too large (file size limits)
- **500:** R2/D1 failure; retry with backoff

---

## 7. Verification Checklist

- [ ] POST /v0/deployments with files → 201 or missing_files
- [ ] POST /v0/deployments/:id/files uploads blob + inserts blobs
- [ ] POST /v0/deployments/:id/continue completes deployment
- [ ] DELETE soft-deletes and enqueues cleanup
- [ ] Queue deletes only orphaned blobs
- [ ] Static worker serves at preview/production URLs
- [ ] CLI `appz deploy` works end-to-end

---

## 8. Execution Order

1. D1 migration: `blobs`, `deployment_blobs`
2. R2 buckets: `appz-deploy-blobs`, `appz-deploy-manifests`
3. v0 worker: create, upload, continue handlers; queue producer
4. v0-static worker: routing, manifest + blob fetch, KV cache
5. deploy-cleanup worker: queue consumer, orphan deletion
6. Wrangler: routes for `*.preview.appz.dev`, `*.appz.dev`

---

## Appendix: Deployment Metadata (manifest)

```json
{
  "id": "deploy_123",
  "timestamp": 1708185600,
  "target": "preview",
  "git_commit": "abc123",
  "git_branch": "main",
  "author": "user@example.com",
  "build_duration_ms": 5000,
  "files": { ... }
}
```

Optional fields in manifest for analytics; CLI can pass via `meta` in create request.
