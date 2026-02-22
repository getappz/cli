# Appz Toolbar Browser Extension — Design Document

**Date:** 2025-02-22  
**Status:** Approved  
**Purpose:** Browser extension for Appz dev platform providing developer preview toolbar on deployments and quick dashboard access, built with WXT (Vercel toolbar parity + enhancements).

---

## 1. Overview

### Goals

1. **Developer preview toolbar** — Show toolbar on *.appz.dev deployments with env label, copy URL, and links
2. **Quick dashboard access** — "Open in Appz" and project-aware navigation from deployment pages
3. **Dashboard integration** — Dashboard (appz.dev) detects extension presence via meta tag
4. **Enhancements** — Copy deployment URL, environment label, optional environment switcher, last deploy time

### Out of Scope (v1)

- Pusher / WebSocket real-time
- Screenshot capture
- Proxy fetch (CORS bypass)
- Tab header capture
- Feedback widget

### Tech Stack

- **Framework:** WXT (same as repomix browser extension)
- **Runtime:** Chrome, Firefox, Edge (Manifest V3)
- **Language:** TypeScript

---

## 2. Architecture (Detailed)

### 2.1 Entrypoint Layout

**Location:** `appz-dev/packages/browser/`

```
packages/browser/
├── entrypoints/
│   ├── background.ts       # Service worker: API proxy, domain sync, message routing
│   ├── content-toolbar-injector.ts   # Runs on <all_urls>, document_start: decides whether to show toolbar
│   ├── content-toolbar.ts # Runs on *.appz.dev, *.preview.appz.dev: renders toolbar UI
│   ├── content-appz-site.ts          # Runs on appz.dev, *.appz.dev: meta tag for dashboard
│   └── popup.html + popup.ts        # Optional: extension icon popup (settings, link to dashboard)
├── components/           # Toolbar UI components (optional, or inline in content script)
├── public/
│   ├── images/           # Icons 16–128px
│   └── _locales/         # i18n (en first)
├── wxt.config.ts
├── package.json
└── tsconfig.json
```
(in appz-dev monorepo)

### 2.2 Content Script Execution Model

| Script | Matches | Run At | Purpose |
|--------|---------|--------|---------|
| `content-toolbar-injector` | `<all_urls>` | `document_start` | Check host; if Appz domain, set cookie/meta, signal toolbar to mount |
| `content-toolbar` | `*://*.appz.dev/*`, `*://*.preview.appz.dev/*`, custom alias domains | `document_end` | Render toolbar UI; only runs when injector has enabled it |
| `content-appz-site` | `*://appz.dev/*`, `*://*.appz.dev/*` | `document_end` | Inject `<meta name="appz-extension">` for dashboard detection |

**Rationale:** Injector runs early on all URLs to quickly decide; toolbar script only matches Appz domains to avoid unnecessary work. Appz-site runs on dashboard + deployment domains.

### 2.3 Message Flow

```
Page (deployment)                    Background                     API (api.appz.dev)
       |                                  |                                |
       |-- resolve(host) ---------------->|                                |
       |                                  |-- GET /v0/extension/resolve --->|
       |                                  |<-- { project, deployment } -----|
       |<-- resolve response -------------|                                |
       |                                                                   |
       |-- copyUrl(), openDashboard() (no BG needed)                       |
```

- **resolve:** Content script sends `host` to background; background calls `GET /v0/extension/resolve?host=...` with credentials (see Auth).
- **copyUrl:** Content script uses `navigator.clipboard.writeText` (no background).
- **openDashboard:** Content script opens `https://appz.dev/...` in new tab.

### 2.4 Storage

| Key | Storage | Purpose |
|-----|---------|---------|
| `domains` | `local` | Cache of user's deployment domains (from API) for injector |
| `lastSync` | `local` | Timestamp of last domain sync |
| `userSettings` | `sync` | Optional: toolbar position, collapsed state |

### 2.5 Permissions

```json
{
  "permissions": ["storage", "alarms", "scripting"],
  "host_permissions": [
    "https://appz.dev/*",
    "https://*.appz.dev/*",
    "https://api.appz.dev/*",
    "https://localhost/*"
  ]
}
```

- `scripting`: Programmatic injection fallback (WXT may use manifest content_scripts only).
- `alarms`: Optional 60-min domain sync (like Vercel).

---

## 3. Domain Detection & Ownership

### 3.1 Appz Domain Patterns

- **Platform domains:** `*.appz.dev`, `*.preview.appz.dev` (legacy)
- **Custom domains:** Aliases resolved via API (e.g. `myapp.com` → deployment)
- **Dashboard:** `appz.dev`

### 3.2 When to Show Toolbar

| Condition | Show Toolbar |
|-----------|--------------|
| Host matches `*.appz.dev` or `*.preview.appz.dev` | Yes |
| Host in `storage.local.domains` (user's custom domains) | Yes |
| Host is `appz.dev` | No (dashboard has its own UI; meta only) |
| Other | No |

### 3.3 Domain Ownership Check (Phase 2)

- **API:** `GET /v0/extension/check-domain?domain=example.com`
- **Response:** `{ owned: boolean }`
- **Usage:** Background syncs user's domains (from `/v0/aliases` or dedicated endpoint) into `storage.local.domains` every 60 min. Injector consults cache first; optional live check for uncached custom domains.

### 3.4 Cookie Opt-in (Optional)

- Cookie `__appz_toolbar=1` allows manual enable on any domain (e.g. for testing). Injector checks cookie before domain cache.

---

## 4. Toolbar UI

### 4.1 Placement

- Fixed bar at **bottom** of viewport (Vercel-like), or configurable (top/bottom) via settings.
- Minimal height (~36px), semi-transparent background.

### 4.2 Elements (Left to Right)

| Element | Behavior |
|---------|----------|
| **Env label** | "Production" or "Preview" (from host heuristics or API) |
| **Project name** | If from API; else truncated host |
| **Copy URL** | Icon button; copies current page URL |
| **Open in Appz** | Link to dashboard (project/deployment path when available) |
| **Environment switcher** | Dropdown or toggle: switch to Production / Latest Preview (links to URLs from API) |

### 4.3 Styling

- Use CSS variables for theming; dark/light via `prefers-color-scheme` or user setting.
- Avoid conflicting with page styles: use shadow DOM or high-specificity class prefix (`appz-toolbar-*`).

### 4.4 Responsiveness

- On narrow viewports, collapse to icon-only or hamburger menu.

---

## 5. Dashboard Integration (appz.dev)

### 5.1 Meta Tag

On `appz.dev` and `*.appz.dev`:

```html
<meta name="appz-extension" content="1.0.0" data-version="1.0.0" />
```

Dashboard checks `document.querySelector('meta[name="appz-extension"]')` to know extension is installed.

### 5.2 Optional: PostMessage Handshake

Dashboard can `postMessage` to request extension capabilities (e.g. `{ type: 'appz-extension-ping' }`); content script replies `{ type: 'appz-extension-pong', version }`.

---

## 6. API Contracts

### 6.1 Resolve Deployment by Host (New)

**Endpoint:** `GET /v0/extension/resolve?host=myproject-team.appz.dev`

**Auth:** Session cookie (same as existing v0 routes). Extension sends `credentials: 'include'` from background fetch to `api.appz.dev` — requires cookie to be sent cross-origin. **Constraint:** User must have visited `appz.dev` in the same browser profile so cookies exist. Alternatively, use extension storage + popup auth flow (see 6.4).

**Response:**

```json
{
  "deploymentId": "uuid",
  "projectId": "uuid",
  "projectSlug": "myproject",
  "teamSlug": "team",
  "type": "production",
  "url": "https://myproject-team.appz.dev",
  "previewUrl": "https://myproject-abc123-team.appz.dev",
  "createdAt": 1730000000
}
```

**Errors:** `401` if unauthenticated; `404` if host not resolved.

### 6.2 Check Domain Owner (Phase 2)

**Endpoint:** `GET /v0/extension/check-domain?domain=example.com`

**Response:** `{ "owned": true }` or `{ "owned": false }`

### 6.3 List User Domains (Phase 2)

**Endpoint:** `GET /v0/extension/domains` (or reuse `/v0/aliases` with projection)

**Response:** `{ "domains": ["example.com", "*.appz.dev"] }` for current user's teams.

### 6.4 Extension Auth (If Needed)

If `credentials: 'include'` from the extension context does not send appz.dev cookies:

1. **Popup flow:** User clicks "Connect" in extension popup → opens `https://appz.dev/extension/connect` in new tab.
2. **Redirect:** After login, redirect to `https://appz.dev/extension/callback?token=...`.
3. **Storage:** Extension stores token in `storage.local`; background sends `Authorization: Bearer <token>` on API calls.
4. **Expiry:** Token with 7-day expiry; popup prompts re-connect when expired.

---

## 7. Host-to-Deployment Parsing (Client-Side Fallback)

When API is unavailable (e.g. user not logged in), use heuristics for `*.appz.dev`:

| Pattern | Example | Inferred Type |
|---------|---------|----------------|
| `{slug}.appz.dev` | `myproject.appz.dev` | Production |
| `{slug}-{scope}.appz.dev` | `myproject-team.appz.dev` | Production |
| `{slug}-{hash}-{scope}.appz.dev` | `myproject-abc123-team.appz.dev` | Preview |
| `{slug}-git-{branch}-{scope}.appz.dev` | `myproject-git-main-team.appz.dev` | Preview |

Dashboard link: `https://appz.dev` (no project path without API). With API: `https://appz.dev/{teamSlug}/{projectSlug}`.

---

## 8. Error Handling

| Scenario | Behavior |
|----------|----------|
| API 401 | Toolbar shows with env label + Copy + "Open Appz" (generic). No project/deployment info. |
| API 404 | Same as 401; host not in user's deployments. |
| API 5xx | Retry once; then fallback to client-side parsing. |
| No extension auth | Show toolbar with minimal info; "Sign in to Appz" link. |

---

## 9. Testing

- **Unit:** Host parsing, storage read/write.
- **Integration:** Mock API; verify toolbar renders, copy works, links correct.
- **Manual:** Load unpacked in Chrome; test on `*.appz.dev` and `appz.dev`.

---

## 10. Phasing

| Phase | Deliverables |
|-------|--------------|
| **1** | WXT scaffold; injector; toolbar UI (env, copy, open); appz-site meta; client-side host parsing; `GET /v0/extension/resolve` |
| **2** | Auth flow (if needed); `check-domain`, domain sync; environment switcher |
| **3** | Last deploy time; project name from API; optional settings popup |

---

## 11. File Manifest (Phase 1)

| File | Responsibility |
|------|----------------|
| `entrypoints/background.ts` | `resolve` message handler; fetch to api.appz.dev; domain sync (phase 2) |
| `entrypoints/content-toolbar-injector.ts` | Host check; cookie; postMessage to trigger toolbar |
| `entrypoints/content-toolbar.ts` | Render toolbar; copy URL; open dashboard; env label |
| `entrypoints/content-appz-site.ts` | Meta tag injection |
| `wxt.config.ts` | Matches, permissions, icons |
| `public/images/` | Icon set (16–128) |

---

## 12. Reference

- Vercel extension: `appz-ref/browser-ext/lahhiofdgnbcgmemekkmjnpifojdaelb/`
- Repomix WXT: `appz-ref/repomix/browser/`
- Appz deployment URLs: `appz-dev/apps/workers/v0/src/routers/deployments/deployment.processor.ts` (`buildDeploymentUrl`, `DOMAIN_SUFFIX`)
