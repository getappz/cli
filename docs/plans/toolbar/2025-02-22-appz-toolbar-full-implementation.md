# Appz Toolbar — Full Implementation Plan (Vercel Parity + Hosted Script)

**Date:** 2025-02-22  
**Status:** Planning  
**Reference:** [Vercel Toolbar Scan](./2025-02-22-vercel-toolbar-scan.md), [Appz Toolbar Design](./2025-02-22-appz-toolbar-extension-design.md), **[Ecosystem Design](./2025-02-22-appz-toolbar-ecosystem-design.md)** (master plan)

---

## 1. Executive Summary

This document specifies the **full implementation** of the Appz toolbar with Vercel parity, including:

1. **Path A — Browser Extension** (existing, enhanced): Toolbar injected by extension on `*.appz.dev` and custom domains
2. **Path B — Hosted Script** (new): `feedback.js`-style script served from `appz.dev` for in-app injection (e.g. `@appz/toolbar` in Next.js)

Both paths share the same API contracts, toolbar UI design, and extension integration. Path B enables framework integrations (Next.js, React, Vite) similar to `@vercel/toolbar`.

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                           DEPLOYMENT PAGE (*.appz.dev)                           │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│  ┌─────────────────────┐                    ┌─────────────────────────────────┐ │
│  │ Path A: Extension    │                    │ Path B: Hosted Script            │ │
│  │                     │                    │                                 │ │
│  │ toolbar-injector    │     OR             │ <script src="https://appz.dev/  │ │
│  │   → postMessage     │                    │   _appz/toolbar/feedback.js"      │ │
│  │   → data-appz-      │                    │   data-project-id="..."           │ │
│  │     toolbar         │                    │   data-team-id="..."            │ │
│  │                     │                    │   data-deployment-id="...">      │ │
│  │ toolbar.content     │                    │                                 │ │
│  │   → resolves host   │                    │ Creates <appz-live-toolbar>      │ │
│  │   → renders bar     │                    │   → shadow DOM                   │ │
│  │   → shadow DOM      │                    │   → iframe (toolbar UI)          │ │
│  │                     │                    │   → postMessage protocol        │ │
│  └─────────────────────┘                    └─────────────────────────────────┘ │
│            │                                            │                        │
│            └──────────────────┬──────────────────────────┘                        │
│                               │                                                  │
│                               ▼                                                  │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │ Background / API                                                            │ │
│  │ • GET /v0/extension/resolve?host=...                                        │ │
│  │ • GET /.well-known/appz/jwe (optional auth token for hosted script)         │ │
│  │ • Extension: takeScreenshot, resolve, sync domains                          │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### 2.1 Path Selection

| Scenario | Path Used |
|----------|-----------|
| User has extension, visits `*.appz.dev` | **Path A** — extension injector + content script |
| User has extension, visits custom domain (myapp.com) | **Path A** — extension (if domain in cache) |
| Developer adds `@appz/toolbar` to Next.js app | **Path B** — script tag injects hosted feedback.js |
| Developer adds `mountAppzToolbar()` in React | **Path B** — same |
| User has extension, page also has Path B script | **Path A wins** — extension detects `data-appz-toolbar`, skips Path B; or Path B detects extension, uses extension capabilities |

**Coexistence:** When both are present, prefer extension (Path A) for deployments. Path B is for developer-opted-in apps that may run on any origin.

---

## 3. Hosted Script Infrastructure (Path B)

### 3.1 URL Scheme

| Resource | URL | Purpose |
|----------|-----|---------|
| Main loader | `https://appz.dev/_appz/toolbar/feedback.js` | Entry script (like Vercel's feedback.js) |
| Toolbar UI iframe | `https://appz.dev/_appz/toolbar/toolbar.html?dpl=<deployment_id>` | Full toolbar UI (menu, env switcher, etc.). Vercel uses `feedback.html?dpl=dpl_...` — see [Vercel scan §4.4.1](./2025-02-22-vercel-toolbar-scan.md#441-main-iframe-feedbackhtml). |
| Auth token (optional) | `https://{deployment}/.well-known/appz/jwe` | JWE for deployment-scoped auth |

**Base URL:** Use `appz.dev` as the script origin (same as dashboard). Ensures cookies and auth flow align.

### 3.2 Serving the Script

**Option A — Static assets in dashboard app** (if appz.dev is Vite/React):

```
apps/app/public/_appz/toolbar/
├── feedback.js      # Bundled toolbar loader
└── toolbar.html     # Toolbar UI iframe (or SPA route)
```

**Option B — Dedicated Worker route** (if appz.dev is Workers/Pages):

Add route in appz.dev Worker:

```
GET /_appz/toolbar/feedback.js  → serve feedback.js bundle
GET /_appz/toolbar/toolbar.html → serve toolbar UI
```

**Option C — api.appz.dev** (reuse v0 Worker):

```
GET /v0/toolbar/feedback.js
GET /v0/toolbar/toolbar.html
```

Script origin would be `api.appz.dev`; ensure CORS and cookie scope are correct.

**Recommendation:** Use **appz.dev** as script origin (Option A or B) so the toolbar can share dashboard session. The v0 API already runs on api.appz.dev; script and API are separate concerns.

---

## 4. Hosted Script (`feedback.js`) — Implementation Spec

### 4.1 Entry & Gating

```js
// Pseudocode
if (!document.body.attachShadow || !document.currentScript) return;
if (alreadyLoadedInParentFrame()) return;

const shouldRun = 
  isAppzDomain(location.hostname) ||           // *.appz.dev
  /__appz_toolbar=[12]/.test(document.cookie) ||
  document.currentScript.getAttribute('data-explicit-opt-in') === 'true' ||
  (await fetch('/', { method: 'OPTIONS' }).then(r => !/noindex/i.test(r.headers.get('x-robots-tag') || '')));

if (!shouldRun) return;
init({ toolbarCode, ... });
```

### 4.2 Context from Script Tag

| Attribute | Use |
|-----------|-----|
| `data-project-id` | Appz project ID |
| `data-team-id` | Appz team/org ID |
| `data-deployment-id` | Deployment ID (production fallback) |
| `data-branch` | Git branch (localhost) |
| `data-explicit-opt-in` | Bypass domain check |

**Auth options builder:**

```ts
function getAuthOptions(script: HTMLScriptElement) {
  const projectId = script.getAttribute('data-project-id');
  const teamId = script.getAttribute('data-team-id');
  const branch = script.getAttribute('data-branch');
  const deploymentId = script.getAttribute('data-deployment-id');
  if (projectId && teamId) {
    return { projectId, teamId, branch: branch ?? undefined };
  }
  return {
    hostname: location.hostname,
    deploymentId: deploymentId ?? undefined,
    path: location.pathname + location.search,
  };
}
```

### 4.3 DOM Structure

1. **Root:** `<appz-live-toolbar>` with `position:absolute; top:0; left:0; z-index:2147483647`
2. **Shadow DOM:** `attachShadow({ mode: 'closed' })`
3. **Main iframe:** `https://appz.dev/_appz/toolbar/toolbar.html?deploymentId=...` — full toolbar UI

### 4.4 postMessage Protocol (Page ↔ iframe, origin: appz.dev)

| Type | Direction | Payload | Purpose |
|------|-----------|---------|---------|
| `init` | page → iframe | `{ authOptions, origin, page }` | Bootstrap |
| `init-reply` | iframe → page | `{ existingAuth?, userSettings? }` | Auth result |
| `preview-origin` | page → iframe | `{ previewOrigin, path }` | Deployment URL |
| `ready` | iframe → page | — | Iframe loaded |
| `render-menu` | page → iframe | `{ isMobile }` | Open popover menu |
| `menu-action` | iframe → page | `{ action: 'sign-in' \| 'disable' }` | User action |
| `update-extension-settings` | page → iframe | `{ position }` | Persist settings |
| `extension-clicked` | extension → page | — | Extension icon clicked (show toolbar) |

### 4.5 Extension Integration (Path B)

When the page has the hosted script and the user has the Appz extension:

- Page listens for `extension-clicked` from extension origin (`chrome-extension://...` or `moz-extension://...`)
- On `extension-clicked`, show toolbar / open menu / trigger keybinding handler
- Extension sends `extension-clicked` when user clicks the extension icon (background → content script → `postMessage` to page)

### 4.6 Position & Storage

- `localStorage.appz-toolbar-position` = `{ x, y, offset }`
- CSS vars: `--toolbar-x`, `--toolbar-y`, `--toolbar-y-offset`
- Draggable floating button (Vercel-style) or fixed bar (current Appz style) — configurable

### 4.7 Session Storage Keys

| Key | Purpose |
|-----|---------|
| `appz-toolbar-activated` | User activated toolbar this session |
| `appz-toolbar-hidden` | Toolbar hidden via keybinding |
| `appz-toolbar-optout` | User opted out |

---

## 5. @appz/toolbar npm Package (Path B Enabler)

New package (optional, Phase 3+) for framework integrations:

```
packages/toolbar/   (or publish as @appz/toolbar)
├── package.json
├── src/
│   ├── index.ts           # mountAppzToolbar, unmountAppzToolbar, isAppzToolbarMounted
│   ├── config.ts          # scriptHostname, data attributes
│   ├── next/
│   │   ├── VercelToolbar.tsx   # Next.js <Script> wrapper → AppzToolbar
│   │   └── plugin.ts           # withAppzToolbar()
│   ├── react/
│   │   └── useCommand.ts       # Optional: custom commands
│   └── vite/
│       └── plugin.ts
└── dist/
```

### 5.1 Core API

```ts
const SCRIPT_URL = 'https://appz.dev/_appz/toolbar/feedback.js';

function mountAppzToolbar(options?: {
  projectId?: string;
  teamId?: string;
  deploymentId?: string;
  branch?: string;
  nonce?: string;
}) {
  if (isAppzToolbarMounted()) return unmountAppzToolbar;
  const script = document.createElement('script');
  script.src = SCRIPT_URL;
  script.setAttribute('data-explicit-opt-in', 'true');
  if (options?.nonce) script.setAttribute('nonce', options.nonce);
  if (options?.projectId) script.setAttribute('data-project-id', options.projectId);
  if (options?.teamId) script.setAttribute('data-team-id', options.teamId);
  if (options?.deploymentId) script.setAttribute('data-deployment-id', options.deploymentId);
  if (options?.branch) script.setAttribute('data-branch', options.branch);
  document.head.appendChild(script);
  return unmountAppzToolbar;
}

function unmountAppzToolbar(): void {
  document.querySelector('appz-live-toolbar')?.remove();
}

function isAppzToolbarMounted(): boolean {
  return document.querySelector('appz-live-toolbar') !== null;
}
```

---

## 6. Toolbar UI (Shared by Path A & B)

### 6.1 Elements

| Element | Source | Behavior |
|---------|--------|----------|
| Env label | resolve / data attrs | "Production" or "Preview" |
| Project/host | resolve / data attrs | Truncated host or project name |
| Deployed at | resolve.createdAt | "5m ago", "2d ago" |
| Environment switcher | resolve.productionUrl, previewUrl | Dropdown |
| Copy URL | — | `navigator.clipboard.writeText` |
| Screenshot | Extension only (Path A) | `chrome.tabs.captureVisibleTab` |
| Open in Appz | resolve | Link to dashboard |

### 6.2 Styling (Unified)

- Shadow DOM for isolation
- CSS vars: `--toolbar-bg`, `--toolbar-fg`, `--toolbar-accent`
- `prefers-color-scheme` for dark/light
- Class prefix: `appz-toolbar-*`
- Min height 36px; fixed bottom or floating button (configurable)

### 6.3 Responsive

- &lt;768px: collapse to icon + hamburger, or bottom sheet

---

## 7. API Contracts (Existing + Extensions)

### 7.1 Resolve (Existing)

`GET /v0/extension/resolve?host=...`

Response includes: `deploymentId`, `projectId`, `projectSlug`, `teamSlug`, `type`, `url`, `productionUrl`, `previewUrl`, `createdAt`, `toolbarEnabled`.

### 7.2 JWE Auth (New, Optional for Path B)

`GET /.well-known/appz/jwe` on deployment origin.

- Returns JWE token for deployment-scoped auth when `data-project-id` + `data-team-id` are set
- Hosted script fetches this to authenticate toolbar iframe
- Implementation: Worker on deployment origin (or proxy from appz.dev) that issues short-lived JWE for the project

**Phase 2:** Can defer; initially Path B works with `deploymentId` and hostname fallback without JWE.

### 7.3 Toolbar Script Assets (New)

`GET /_appz/toolbar/feedback.js`  
`GET /_appz/toolbar/toolbar.html`

Served from appz.dev (or configured origin). Cache headers: `Cache-Control: public, max-age=3600`.

---

## 8. Extension Changes (Path A Enhancement)

### 8.1 Hosted Script Coexistence

When extension injector runs on a page that also has the hosted script (Path B):

- **Option 1:** Extension sets `data-appz-toolbar` early; hosted script sees it and skips init (extension wins)
- **Option 2:** Hosted script checks for `meta[name="appz-extension"]`; if present, delegates to extension for UI (extension provides toolbar DOM)
- **Option 3:** Both run; extension toolbar takes precedence visually; hosted script does nothing if extension toolbar is present

**Recommendation:** Option 1. Extension injector runs at `document_start`; if domain matches, it sets `data-appz-toolbar` and posts `appz-toolbar-inject`. Hosted script (loaded at `document_end`) checks `data-appz-toolbar` and skips if extension already claimed.

### 8.2 extension-clicked for Path B

When user has extension and visits a page with Path B script:

1. User clicks extension icon
2. Background broadcasts to content scripts
3. Content script (runs on all URLs) posts `extension-clicked` to `window`
4. Hosted script listens and shows toolbar / opens menu

**Implementation:** Add content script that matches `<all_urls>` at `document_end`, listens for `chrome.runtime.onMessage` with `action: 'extension-clicked'`, and posts `{ type: 'extension-clicked' }` to `window`.

### 8.3 Current Extension File Map

| File | Change |
|------|--------|
| `toolbar-injector.content.ts` | No change; sets `data-appz-toolbar` |
| `toolbar.content/index.ts` | Enhance: add `extension-clicked` listener for future Path B pages; keep current UI |
| `appz-site.content.ts` | Ensure `extension-clicked` can be sent to page when popup/icon clicked |
| `background.ts` | Add handler for `extension-clicked` from popup; broadcast to tabs |

---

## 9. Toolbar UI iframe (`toolbar.html`)

The hosted script loads an iframe with the full toolbar UI. This can be:

**Option A — Static HTML + inline JS**  
Single HTML file with embedded styles and script. Renders toolbar bar + popover menu. Communicates via postMessage.

**Option B — SPA route**  
Dashboard app route `/toolbar` (or `/_appz/toolbar`) that renders toolbar when loaded in iframe. Shares React components with dashboard.

**Option C — Dedicated micro-frontend**  
Small Vite/React app built specifically for toolbar, deployed to `/_appz/toolbar/toolbar.html` (or as static asset).

**Recommendation for Phase 1:** Option A — single HTML file with vanilla JS. Keeps scope small. Phase 2 can migrate to Option B/C if we need richer UI.

### 9.1 toolbar.html Structure

```html
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <style>/* toolbar styles */</style>
</head>
<body>
  <div id="appz-toolbar-root">
    <!-- Env label, Copy, Open in Appz, env switcher -->
    <!-- Popover menu (Sign in, Disable, etc.) -->
  </div>
  <script>
    // Listen for init postMessage
    // Fetch resolve data from api.appz.dev if needed
    // Render UI, post init-reply
  </script>
</body>
</html>
```

---

## 10. Implementation Phases

### Phase 1 — Extension Parity + Hosted Script Skeleton (4–6 days)

| Task | Owner | Deliverables |
|------|-------|--------------|
| 1.1 | Serve feedback.js | Route `GET /_appz/toolbar/feedback.js` from appz.dev (static or Worker) |
| 1.2 | feedback.js loader | Minimal loader: create `<appz-live-toolbar>`, shadow DOM, inject toolbar bar (no iframe yet) |
| 1.3 | Data attributes | Read `data-project-id`, `data-team-id`, `data-deployment-id` from script tag |
| 1.4 | Gating | Domain check (`.appz.dev`), cookie `__appz_toolbar=1`, `data-explicit-opt-in` |
| 1.5 | Extension | Add `extension-clicked` broadcast; content script posts to window |
| 1.6 | Unify toolbar UI | Ensure extension toolbar and hosted toolbar share same HTML/CSS structure |

### Phase 2 — Toolbar iframe + API Integration (3–4 days)

| Task | Owner | Deliverables |
|------|-------|--------------|
| 2.1 | toolbar.html | Full toolbar UI in iframe; postMessage init/init-reply |
| 2.2 | feedback.js | Load iframe, pass authOptions, handle init-reply |
| 2.3 | Resolve from iframe | Iframe fetches `api.appz.dev/v0/extension/resolve?host=...` (with credentials) when host is appz domain |
| 2.4 | Position/drag | localStorage, CSS vars, draggable (optional) |

### Phase 3 — @appz/toolbar Package + Framework Plugins (5–7 days)

| Task | Owner | Deliverables |
|------|-------|--------------|
| 3.1 | @appz/toolbar | npm package: mount/unmount, isMounted |
| 3.2 | Next.js | `withAppzToolbar()`, `<AppzToolbar />` component |
| 3.3 | React | `useAppzToolbar()`, `mountAppzToolbar` in useEffect |
| 3.4 | Vite | `appzToolbar()` plugin |
| 3.5 | Localhost dev server | Optional: Node server for branch sync (like Vercel) |

### Phase 4 — JWE Auth + Advanced Features (3–5 days)

| Task | Owner | Deliverables |
|------|-------|--------------|
| 4.1 | /.well-known/appz/jwe | Deployment-scoped JWE for Path B auth |
| 4.2 | Intent overlays | Optional: draft mode, feature flags (Vercel-style intents) |
| 4.3 | Commands API | Optional: createCommand, plugin system |

---

## 11. File Manifest

### 11.1 New Files

| Path | Purpose |
|------|---------|
| `apps/app/public/_appz/toolbar/feedback.js` | Hosted loader script (or build output) |
| `apps/app/public/_appz/toolbar/toolbar.html` | Toolbar UI iframe |
| `packages/toolbar/` | @appz/toolbar npm package (Phase 3) |

### 11.2 Modified Files

| Path | Changes |
|------|---------|
| `packages/browser/entrypoints/background.ts` | Handle `extension-clicked`, broadcast to tabs |
| `packages/browser/entrypoints/toolbar-injector.content.ts` | Ensure early `data-appz-toolbar` for Path B skip |
| `packages/browser/entrypoints/appz-site.content.ts` | Post `extension-clicked` on icon click |
| `apps/workers/v0/` or appz.dev Worker | Route for `/_appz/toolbar/*` if not static |

---

## 12. Testing

| Scenario | Verification |
|----------|--------------|
| Extension on *.appz.dev | Toolbar shows; Copy, Open in Appz, env switcher work |
| Extension on custom domain | Toolbar shows when domain in cache |
| Hosted script on *.appz.dev | Toolbar shows when script injected with data attrs |
| Hosted script + extension | Extension wins; or both coexist, extension provides capabilities |
| No auth (401) | Toolbar shows minimal (env, Copy, Open Appz generic) |
| toolbarEnabled: false | No toolbar |

---

## 13. Reference

- Vercel feedback.js: `appz-ref/vercel-toolbar/feedback.js`
- Vercel scan: `docs/plans/toolbar/2025-02-22-vercel-toolbar-scan.md`
- Appz extension design: `docs/plans/toolbar/2025-02-22-appz-toolbar-extension-design.md`
- Extension handler: `appz-dev/apps/workers/v0/src/routers/extension/extension.handler.ts`
