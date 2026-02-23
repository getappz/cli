# Appz Deployment Management — Vercel Parity

> Design: Implement Vercel-style deployment management on Appz (filter, delete, redeploy, promote to production).

## Vercel Reference Summary

From [Vercel docs](https://vercel.com/docs/deployments/managing-deployments):

| Feature | Vercel | Appz Status |
|---------|--------|--------------|
| **Dashboard** | Project → Deployments tab | ✅ Exists (`AppzDeployments`, `ProjectDeployments`) |
| **Filter** | Branch, Date Range, All Environments, Status | ⚠️ Partial (type, search only) |
| **Delete** | ... menu → Delete | ✅ Exists (Trash icon + AlertDialog) |
| **Redeploy** | ... → Redeploy (with Build Cache option) | ❌ Missing |
| **Promote** | ... → Promote to production | ❌ Missing |
| **Retention policy** | Auto-delete after period | ❌ Future |
| **Deployment protection** | Auth, IP, password | ❌ Future (separate scope) |

---

## Current Appz Implementation

### Dashboard
- **AppzDeployments**: Recent deployments across all projects (limit 5 per project, 20 total)
- **ProjectDeployments**: Per-project list with type filter (all/production/preview) and text search

### API (`apps/workers/v0`)
- `GET /deployments?projectId=&teamId=&limit=&since=&until=` — list
- `GET /deployments/:id` — get one
- `DELETE /deployments/:id` — soft-delete + cleanup queue
- `POST /deployments` — create (with files)
- `POST /deployments/:id/continue` — upload more files
- No `branch`, `status` query params on list
- No redeploy or promote endpoints

### CLI (`appz-cli`)
- `appz deploy` — create deployment
- `appz remove <deployment>` — delete deployment
- No `appz redeploy` or `appz promote`

---

## Target Feature Set

### 1. Filter Enhancement (Dashboard + API)

**Vercel filters:** Branch, Date Range, Environment, Status.

**Appz additions:**

| Filter | API | Dashboard |
|--------|-----|-----------|
| Branch | `?branch=` (match `metadata.git_branch`) | Dropdown populated from deployments |
| Status | `?status=` (ready, error, building, canceled) | Dropdown |
| Date range | `since`, `until` (already exist) | Date picker / preset (Today, 7d, 30d) |
| Type | `?type=` (production, preview) | Existing pills |

**API changes:**
- `listDeployments` input: add `branch?: string`, `status?: string`, `type?: string`
- DB query: filter by `metadata` JSON (`git_branch`) and `status`
- `since`/`until` already passed to handler but not used in processor — wire them up

### 2. Redeploy

**Vercel behavior:** Create a new deployment from an existing one. Option to use build cache.

**Appz behavior:** Static deploys = no build. Redeploy = clone deployment (new deployment record, same manifest/blobs).

- **Backend:** `POST /deployments` with body `{ sourceDeploymentId, projectId, teamId, type? }`
  - Load manifest from source deployment's R2 key
  - Create new deployment, copy manifest to new key, reuse blob references
  - Return new deployment
- **Dashboard:** Add "Redeploy" to deployment card actions (ellipsis menu)
- **CLI:** `appz redeploy <deployment-id|url> [--target production|preview]`

### 3. Promote to Production

**Vercel behavior:** Promote a preview deployment to production. If source is preview, can create new production deployment with prod env vars.

**Appz behavior:** Promote = clone deployment as production (new deployment of type `production` with same blobs).

- **Backend:** `POST /deployments/:id/promote` — create new production deployment from this one
  - Only allowed for `status=ready` deployments
  - Creates production deployment with same manifest
  - Updates project’s production domain to point to new deployment (v0-static already resolves latest production by project)
- **Dashboard:** Add "Promote to production" to ellipsis menu (only for preview deployments)
- **CLI:** `appz promote <deployment-id|url>`

### 4. UX Parity: Ellipsis Menu

**Vercel:** Deployment row has `...` button with context menu: Redeploy, Promote, Delete.

**Appz:** Currently has only Trash icon. Add ellipsis (`MoreVertical`) with:
- Redeploy
- Promote to production (only if type=preview)
- Delete

---

## Implementation Plan

### Phase 1: Filter Enhancement
1. **Backend:** Extend `ListDeploymentsInput` with `branch`, `status`, `type`; apply filters in SQL.
2. Wire `since`/`until` in processor (currently passed from handler but not used).
3. **Frontend:** Add Branch dropdown, Status dropdown, Date range picker to `ProjectDeployments`.
4. **v0-api:** Extend `deployments.list()` params.

### Phase 2: Redeploy
1. **Backend:** Add `sourceDeploymentId` to `POST /deployments` body; new `cloneDeployment()` in processor.
2. **Frontend:** `useRedeployDeployment` hook + Redeploy action in menu.
3. **CLI:** `appz redeploy` command.

### Phase 3: Promote
1. **Backend:** `POST /deployments/:id/promote` route + `promoteDeployment()` processor.
2. **Frontend:** `usePromoteDeployment` hook + Promote action in menu.
3. **CLI:** `appz promote` command.

### Phase 4: UX Polish
1. Replace inline Trash with ellipsis menu (Redeploy, Promote, Delete).
2. Match Vercel copy and confirm dialogs.

---

## File Changes Summary

| Location | Change |
|----------|--------|
| `appz-dev/.../deployment.processor.ts` | `listDeployments` filters; `cloneDeployment()`; `promoteDeployment()` |
| `appz-dev/.../deployment.handler.ts` | Pass branch/status/type/since/until; POST promote route |
| `appz-dev/.../v0-api.ts` | Extend deployments.list params; add deployments.redeploy, deployments.promote |
| `appz-dev/.../ProjectDeployments.tsx` | Ellipsis menu; Branch/Status/Date filters; useRedeploy, usePromote |
| `appz-dev/.../use-deployments.ts` | useRedeployDeployment, usePromoteDeployment |
| `appz-cli/.../deployments.rs` | Add redeploy, promote API methods |
| `appz-cli/.../commands/` | Add `redeploy.rs`, `promote.rs` subcommands |

---

## API Spec Additions

### GET /deployments (extended params)

```
?projectId=xxx
&teamId=xxx
&limit=50
&since=1700000000000   # ms (already exist)
&until=1700100000000   # ms (already exist)
&branch=main           # NEW: filter by metadata.git_branch
&status=ready           # NEW: ready|error|building|canceled
&type=production        # NEW: production|preview
```

### POST /deployments (redeploy via sourceDeploymentId)

```json
{
  "sourceDeploymentId": "dpl_xxx",
  "projectId": "proj_xxx",
  "teamId": "team_xxx",
  "type": "preview"
}
```

Returns new deployment. No `files` needed; manifest copied from source.

### POST /deployments/:id/promote

Body: `{}`  
Returns new production deployment (clone of source with type=production).

---

## DB / R2 Notes

- **Clone:** Read manifest from `deployments/${sourceId}/manifest.json`, write to `deployments/${newId}/manifest.json`. Blobs are content-addressed; no copy needed.
- **D1:** New rows in `deployment`, `deployment_blobs`; blobs table unchanged (reference existing SHAs).
- `since`/`until`: Add to WHERE clause: `created_at >= ? AND created_at <= ?` (convert ms to ISO or SQLite date).

---

## Deferred

- **Deployment retention policy** — Cron to auto-delete old deployments
- **Deployment protection** — Auth, password, trusted IPs (separate security doc)
- **Build cache option** — N/A for static deploys
