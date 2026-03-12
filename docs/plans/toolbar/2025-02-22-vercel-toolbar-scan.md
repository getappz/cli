# @vercel/toolbar — Code Scan: Design & Tech Details

**Date:** 2025-02-22  
**Source:** npm `@vercel/toolbar@0.2.2` (extracted to `appz-ref/vercel-toolbar/`); hosted script from `https://vercel.live/_next-live/feedback/feedback.js` (saved as `appz-ref/vercel-toolbar/feedback.js`)  
**Scope:** Package structure, injection flow, hosted script internals, API surface, and localhost integration.

---

## 1. Overview

`@vercel/toolbar` is a **script-loading package** that injects the Vercel toolbar UI by loading an external script from `https://vercel.live` (or configurable hostname). It does **not** ship the actual toolbar UI; the real UI and logic live in the hosted script. The package focuses on:

- **Mount/unmount API** — inject or remove the external script
- **Config options** — pass context (owner, project, branch, deployment) via data attributes
- **Framework adapters** — Next.js, React, Nuxt, Vite
- **Localhost dev server** — Node.js server for branch sync and plugin commands when developing locally

---

## 1.1 Product & UX (from Vercel docs)

**Source:** [Vercel Toolbar docs](https://vercel.com/docs/vercel-toolbar)

### Features (toolbar capabilities)

| Feature | Description |
|--------|-------------|
| **Comments** | Leave feedback on deployments |
| **Menu navigation** | Navigate to dashboard pages; share deployments |
| **Feature Flags** | Read and set flags |
| **Draft Mode** | Preview unpublished content |
| **Edit Mode** | Edit content in real time |
| **Layout Shifts** | Inspect elements causing layout shifts |
| **Interaction Timing** | Inspect latency, INP |
| **Accessibility Audit** | WCAG 2.0 Level A/AA checks |
| **Open Graph** | View OG properties and link preview |

### Activation & visibility

- **Default:** Toolbar is "sleeping" until activated (click or Ctrl). Does not run tools or show comments until activated.
- **Auto-activate:** When visiting a link to a comment thread, flags overrides, etc.
- **Always Activate:** Requires [browser extension](https://vercel.com/docs/vercel-toolbar/browser-extension); set in Preferences.
- **Enabled by default** on preview deployments. Can disable at team, project, or session level.
- **Environment variables / headers** for automation and branch-specific control.

### Toolbar menu (Ctrl or click)

| Item | Purpose |
|------|---------|
| Search | Search toolbar, access dashboard |
| Quick branch access | Current branch, commit hash |
| Switch branches | Preview/production only |
| Layout shifts, Interaction timing, Accessibility audit | Per-tool panels |
| Open Graph | OG metadata and preview |
| Comments, View inbox | Feedback panel |
| Navigate to team/project/deployment | Dashboard links |
| Hide, Disable for session | Visibility |
| Preferences, Logout | Settings |

### Preferences (from menu)

| Setting | Maps to user-data |
|---------|-------------------|
| Notifications | `notificationPreference` |
| Theme | `theme` (system/light/dark) |
| Layout Shift Detection | `shifts.detect` |
| Interaction Timing | `inp.detect` |
| Accessibility Audit | `accessibility.detect` |
| Browser Extension | — |
| Always Activate | `userSettings.activate` |
| Start Hidden | `userSettings.show` |

### Sharing, repositioning

- **Share button** copies deployment URL (with relevant query params).
- **Reposition** by dragging; snaps to edges; persists per-user (`toolbarPosition`, `localStorage.vercel-toolbar-position`).

### Keyboard shortcuts

- **Default:** Ctrl = open menu; Ctrl+. = hide. Configurable via Preferences when extension installed.
- Per-tool shortcuts in Preferences → Configure → Keyboard Shortcuts.

### 1.2 Comments (from Vercel docs)

**Source:** [Comments Overview](https://vercel.com/docs/comments)

Comments allow teams and invited participants to give feedback on preview deployments through the toolbar. Threads can be linked to Slack.

| Aspect | Detail |
|--------|--------|
| **Availability** | All plans; enabled by default on preview deployments |
| **Requirement** | All users must have a Vercel account |
| **Placement** | Click anywhere on page or highlight text to place comment |
| **Activation** | Toolbar must be active; recommend extension + Always Activate for frequent use |
| **Notifications** | PR owners: email on new comment; participants: email on thread activity |
| **External users** | Pro/Enterprise: invite external collaborators to view and comment |
| **New deployment** | Popup modal bottom-right prompts refresh when PR has new preview |
| **Entry** | Toolbar menu → Comment, or comment bubble shortcut |

**Appz parity:** Implement comments via Replicache sync (§4.12); support `commentThread` placement (x/y, nodeId, selectionRange); notifications via email/optional Slack.

### 1.3 Browser Extension (from Vercel docs)

**Source:** [Toolbar Browser Extensions](https://vercel.com/docs/vercel-toolbar/browser-extension)

Supported: Chrome, Firefox, Opera, Edge, Chromium-based browsers.

| Capability | Description |
|------------|-------------|
| **Login detection** | Detects when user is logged in to Vercel |
| **Performance** | Operates faster, fewer network requests |
| **Preferences** | Remembers hide/activate preferences (Always Activate, Start Hidden) |
| **Screenshots** | Take screenshots and attach to comments (click-drag-release to select area) |
| **Quick access** | Click extension to hide/show toolbar; pin to browser bar |

**Extension-only preferences:** Always Activate, Start Hidden — require extension; stored per-user (`userData.userSettings`).

**Screenshot flow:** 1) Select Comment in menu; 2) Click-drag-release to select area; 3) Compose comment, send. Screenshot data stored in `comment.images` (see §4.12 comment shape).

### 1.3a Screenshot Upload API

**Endpoint:** `PUT https://blob.vercel-storage.com/<owner>/<projectId>/<deploymentId>/screenshot.png`

Uploads screenshot image for comment attachments. Path segments: `adsonmedia/prj_mQWND47C8q15UY7iZ8gvpaVW8xuW/dpl_BRVcgJ19YtZaxgtPY1sRghSXcANS/screenshot.png`.

**Payload:** Raw PNG bytes (binary) or base64-encoded image data.

**Response (200):**
| Field | Purpose |
|-------|---------|
| `url` | Public blob URL (redirects to actual storage) |
| `downloadUrl` | Same with `?download=1` for attachment |
| `pathname` | Storage path |
| `contentType` | `image/png` |
| `contentDisposition` | `inline; filename="screenshot.png"` |

**Note:** Actual returned URL uses hash suffix (e.g. `screenshot-nncWUooadwQ5BgQ7aYLeMhqv3ytfVU.png`). Stored in `comment.images[]` for Replicache.

**Appz parity:** Use R2 or existing blob storage; `PUT /v0/toolbar/screenshot` or direct R2 presigned upload with path `{owner}/{projectId}/{deploymentId}/screenshot-{id}.png`.

**Appz parity (extension):** Extension should detect login, persist preferences, support `extension-clicked` postMessage (§4.6), and provide screenshot capture for comments (`canProxy` for fetch-proxy).

### 1.4 UI Reference (screenshots)

**Toolbar menu (open):**
- **Main icon:** Dark pill-shaped button on right edge; draggable, snaps to edges.
- **Panel:** "Vercel Toolbar" header; search "What do you need?"; row of quick-action icons (comment, share, etc.); scrollable list of menu items (Comments, Feature flags, Edit Mode, Layout shifts, Interaction timing, Accessibility audit, Open Graph, View inbox, Navigate to team/project/deployment, Hide, Disable for session, Preferences, Logout).

**Comments UI:**
- **Markers:** Pink rounded rectangles with speech-bubble icon, anchored to page elements (text or node).
- **Comment dialog:** Floating panel with header (author, timestamp, options, refresh, close); "Add comment..." input; footer with Attach screenshot, Emoji, Send (paper airplane).

**Comments panel (on click comment bubble):**
- **Header:** "Need help?"
- **Input:** "What do you need?" — rich text field
- **Formatting toolbar:** Bold, italic, underline, strikethrough; bullet/numbered/checklist; code block, quote, link, image (paperclip)
- **Actions:** Add comment, Pin, Move, Duplicate, Share, Send to Slack; divider; Development, AI Help, Tasks, View full site
- **Selection overlay:** Pink highlight on page content when selecting area for screenshot/comment

**Comments inbox (View inbox):**
- **Layout:** Vertical panel on right, semi-transparent, rounded corners, shadow.
- **Header:** Back arrow, "Comments" title, plus (new), refresh, close (X).
- **Filter bar:** "X unresolved comments" with purple dot; sort dropdown ("Newest").
- **Thread list:** Scrollable; each thread has status dot (purple = unresolved, grey = resolved), timestamp (e.g. "3 minutes ago"), expandable content.
- **Expanded thread:** Avatar, comment text, "Reply" input, send (paper airplane).
- **Page markers:** Numbered bubbles (1, 2, …) on page link to threads.

**Reference images:** See workspace assets — menu open; comments markers + dialog; comments panel (Need help?); comments inbox.

---

## 2. Package Structure

```
vercel-toolbar/
├── package.json
├── README.md
├── feedback.js          # Downloaded hosted script (https://vercel.live/_next-live/feedback/feedback.js)
└── dist/
    ├── index.js / index.cjs      # Main entry: mount, unmount, isMounted
    ├── toolbar-object.js         # window.__vercel_toolbar accessor
    ├── injection-eb01a000.js     # Script injection + config
    ├── chunk-*.js / chunk-*.cjs  # Shared logic (config, commands registry, localhost server)
    ├── plugins/
    │   ├── next.cjs / next.js    # Next.js config plugin, Next Script component
    │   ├── vite.cjs / vite.js    # Vite plugin
    │   └── types.d.ts            # ToolbarPlugin, CommandInvocation, etc.
    ├── next/                     # Next.js components + localhost controller
    ├── react/                    # useCommand hook
    ├── nuxt/                     # Nuxt module
    ├── vite/                     # Vite integration
    └── commands/                 # createCommand, ToolbarCommandRegistry
```

**Exports:**

| Export | Path | Purpose |
|--------|------|---------|
| `mountVercelToolbar`, `unmountVercelToolbar`, `isVercelToolbarMounted` | `.` | Core injection API |
| `VercelToolbar`, `vercelToolbarWindowKey` | `./toolbar-object` | Toolbar object access |
| `createCommand`, `getInternalRegistry` | `./commands` | Custom commands (menu items) |
| `VercelToolbar` (component) | `./next` | Next.js `<Script>` wrapper |
| `LocalhostController` | `./next` | Localhost SSE + branch sync |
| `useCommand` | `./react` | React hook for custom commands |
| `withVercelToolbar` | `./plugins/next` | Next.js config plugin |
| `vercelToolbar` | `./plugins/vite` | Vite plugin |

---

## 3. Injection Flow

### 3.1 Core Logic (`chunk-MTBYBPJH.js` / `chunk-3KTWNYNM.cjs`)

```js
function mountVercelToolbar(options = {}) {
  if (!isMounted()) {
    configure(options);
    const script = document.createElement('script');
    script.src = getScriptUrl();  // https://vercel.live/_next-live/feedback/feedback.js
    script.setAttribute('data-explicit-opt-in', 'true');
    if (options.nonce) script.setAttribute('nonce', options.nonce);
    // Set data attributes from getDataAttributes()
    for (const [key, val] of Object.entries(getDataAttributes() ?? {}))
      if (val) script.setAttribute(key, val);
    (document.head || document.documentElement).appendChild(script);
  }
  return unmountVercelToolbar;
}
```

**Flow:**

1. `configure(options)` — merge options into internal config
2. `getScriptUrl()` — `https://vercel.live/_next-live/feedback/feedback.js` (or `NEXT_PUBLIC_VERCEL_TOOLBAR_HOST`)
3. `getDataAttributes()` — returns `{ "data-owner-id", "data-project-id", "data-branch" }` or `{ "data-deployment-id" }` depending on context
4. Script is injected; the **hosted script** creates the toolbar and exposes `window.__vercel_toolbar`

### 3.2 Configuration (`chunk-HV7HHOV3.js`)

| Option | Env Var | Description |
|--------|---------|-------------|
| `scriptHostname` | `NEXT_PUBLIC_VERCEL_TOOLBAR_HOST` | Base URL for toolbar script (default `https://vercel.live`) |
| `ownerId` | `NEXT_PUBLIC_VERCEL_TOOLBAR_OWNER_ID` | Vercel org/owner ID |
| `projectId` | `NEXT_PUBLIC_VERCEL_TOOLBAR_PROJECT_ID` | Vercel project ID |
| `branch` | `NEXT_PUBLIC_VERCEL_TOOLBAR_BRANCH` | Git branch (localhost) |
| `deploymentId` | `NEXT_DEPLOYMENT_ID` | Deployment ID (production) |
| `nonce` | — | CSP nonce for script tag |

**Data attributes logic:**

- If `ownerId` and `projectId` → `data-owner-id`, `data-project-id`, `data-branch` (branch from config or localhost)
- Else if `deploymentId` → `data-deployment-id` only

**Mounted check:** `document.querySelector('vercel-live-feedback') !== null`

---

## 4. Hosted Script (`feedback.js`) — Full Analysis

**URL:** `https://vercel.live/_next-live/feedback/feedback.js`  
**Downloaded:** `appz-ref/vercel-toolbar/feedback.js` (~74KB minified)

The hosted script is the **actual toolbar UI**. The npm package only injects this script; all rendering, auth, extension integration, and feature logic live here.

### 4.1 Entry & Gating

- **Entry:** IIFE runs on load when `document.body.attachShadow` exists and `document.currentScript` is present.
- **Origin check:** Skips init if script already loaded in parent frame (`i.rG`, `i.tW`).
- **Opt-in gating** (`i.pZ`): Toolbar runs only if:
  - Host is `vercel.com`, `beta.nextjs.org`, `localhost`, or `10.0.2.2`; **or**
  - Cookie `__vercel_toolbar=1` or `__vercel_toolbar=2`; **or**
  - Script has `data-explicit-opt-in="true"`; **or**
  - OPTIONS fetch to `/` returns no `x-robots-tag: noindex`.

### 4.2 Context from Script Tag

Reads attributes from `document.currentScript` (O.XZ):

| Attribute | Use |
|-----------|-----|
| `data-project-id` | With ownerId → full project context |
| `data-owner-id` | Vercel org ID |
| `data-branch` | Git branch (localhost) |
| `data-deployment-id` | Deployment ID (production fallback) |

**Auth options** (`ee()`): If `projectId` and `ownerId` → `{ projectId, ownerId, branch }`. Else → `{ hostname, deploymentId, vercelToolbarCode, path }`.

### 4.3 Auth Flow

- **JWE token:** Fetches `/.well-known/vercel/jwe` when `data-project-id` + `data-owner-id` set or on localhost. Adds `vercelAuthJWE` to auth payload.
- **init-reply:** Main iframe sends `init`; receives `init-reply` with `existingAuth` or triggers sign-in.
- **auth-popup-response:** Extension/browser auth popup response.

### 4.3.1 JWE Token (`.well-known/vercel/jwe`)

**Endpoint:** `GET /.well-known/vercel/jwe` — served from the **deployment origin** (e.g. `https://qwind-qy5rugdl7-adsonmedia.vercel.app/.well-known/vercel/jwe`), not vercel.live.

**Response:** JWE string (compact form). Example structure: `eyJlbmMiOiJBMjU2R0NNIiwiYWxnIjoiZGlyIn0..<iv>.<ciphertext>.<tag>` — `alg: dir`, `enc: A256GCM`.

**Purpose:** Proves the request originates from a Vercel deployment. The toolbar fetches this from the page's origin (same-origin, no CORS) and passes it as `vercelAuthJWE` in auth payloads (e.g. login/validate). Vercel injects this endpoint into deployments at build/serve time.

**Appz parity:** Serve `GET /.well-known/appz/jwe` from `*.appz.dev` preview deployments. Inject via Workers/Edge middleware or static `_redirects` / `vercel.json` equivalent. Return a signed JWE that appz.dev can verify to bind requests to the deployment.

### 4.4 DOM Structure

1. **Root:** `<vercel-live-feedback>` with `position:absolute; top:0; left:0; z-index:2147483647`.
2. **Shadow DOM:** `attachShadow({ mode: 'closed' })` — styles and UI isolated.
3. **Main iframe:** `/_next-live/feedback/feedback.html?dpl=dpl_...` — full toolbar UI (comments, menu, etc.).
4. **Instrument iframe:** `instrument.094a043ca7ca652d4806.js` — hidden iframe for:
   - `get-draft-status` → HEAD with `x-vercel-draft-status: 1`
   - `fetch-proxy` → CORS bypass (proxies fetch from page context)

### 4.4.1 Main iframe: `feedback.html`

**URL:** `https://vercel.live/_next-live/feedback/feedback.html?dpl=<deployment_id>`  
**Example:** [feedback.html?dpl=dpl_3cXdGEM1SGNEiETXUKpqNExzcqBW](https://vercel.live/_next-live/feedback/feedback.html?dpl=dpl_3cXdGEM1SGNEiETXUKpqNExzcqBW)

The `feedback.html` page hosts the **full toolbar UI** — menu, comments, intent overlays, etc. It is loaded as an iframe inside the shadow DOM.

| Aspect | Detail |
|--------|--------|
| **`dpl` param** | Deployment ID (e.g. `dpl_3cXdGEM1SGNEiETXUKpqNExzcqBW`). In the bundled `feedback.js`, this value is **hardcoded at build time**; the script does not dynamically substitute per-deployment. |
| **Origin** | Same as script: `vercel.live` (or `NEXT_PUBLIC_VERCEL_TOOLBAR_HOST`). Built via `new URL("/_next-live/feedback/feedback.html?dpl=...", scriptOrigin)`. |
| **Loading flow** | 1) Page creates iframe with `src=feedback.html?dpl=...`; 2) iframe loads and sends `ready`; 3) page replies with `preview-origin` (deployment origin) and `path`; 4) page sends `init` with authOptions; 5) iframe sends `init-reply` (or triggers sign-in). |
| **Standalone fetch** | Fetching the URL directly returns 500; it is intended to run inside the toolbar iframe context (e.g. from a Vercel deployment). |

**Appz parity:** Host an equivalent HTML/React app at `appz.dev/_next-live/feedback/feedback.html` (or `feedback.appz.dev/...`) with `?dpl=<appz_deployment_id>` for deployment-specific toolbar UI.

### 4.5 postMessage Types (Page ↔ iframe, origin: vercel.live)

| Type | Direction | Purpose |
|------|-----------|---------|
| `init` | page → iframe | Bootstrap with authOptions, origin, page |
| `init-reply` | iframe → page | Auth result, userSettings |
| `setup-popup` | page → iframe | Auth popup setup |
| `auth-popup-response` | iframe → page | Auth token from popup |
| `preview-origin` | page → iframe | Deployment origin and path |
| `ready` | iframe → page | Iframe loaded |
| `render-menu` | page → iframe | Open menu (popover), isMobile |
| `render-intent` | page → iframe | Show intent overlay (draft, flags, thread) |
| `menu-action` | iframe → page | sign-in, disable |
| `intent-action` | iframe → page | sign-in or dismiss |
| `intent-dismissed` | page → iframe | User dismissed overlay |
| `intent-skipped` | page → iframe | Intent shown too late (>3s) |
| `update-extension-settings` | page → iframe | position, etc. |
| `get-window-tracking-data` | iframe → page | Request; page replies with browser dimensions, pathname, hostname, is_mobile |

### 4.6 Extension Integration

- **extension-clicked:** Page listens for this from extension origin; triggers `A()` callback (show toolbar / keybinding handler).
- **showToolbar:** `o8({ showToolbar: f })` — configures what happens when extension icon clicked.
- **Keybindings:** `activate: "ctrl"`, `hide: "meta+."` (configurable). Cmd/Ctrl+K opens command palette; sessionStorage `__vtkb-init-tool`, `__vtkb-hide-key`.

### 4.7 Position & Layout

- **Storage:** `localStorage.vercel-toolbar-position` = `{ x, y, offset }`.
- **CSS vars:** `--toolbar-x` (0 = left, 1 = right), `--toolbar-y` (0–1), `--toolbar-y-offset` (px).
- **Draggable:** Button can be dragged; snaps to edges; stores position for next load.
- **Responsive:** Mobile (<768px) uses bottom sheet / popover behavior.

### 4.8 Intent System

Intents control which overlay to show on first load:

| Intent | Trigger |
|--------|---------|
| `flagOverrides` | `__vercel_flags` or `vercelFlagOverrides` in URL/cookie |
| `threadLink` | `vercelThreadId` in URL |
| `draft` | `__vercel_draft=1` in URL or draft-status header |
| `flagRecommendations` | `hasRecommendedFlags` in init data |
| `toolbar` / `nonBlockingToolbar` | Normal toolbar vs deferred |
| `bisect` | A/B testing |
| `flagCookie` | `vercel-flag-overrides` cookie |

Intents shown within ~3s of `performance.timeOrigin`; later intents are skipped.

### 4.9 Session Storage Keys

| Key | Purpose |
|-----|---------|
| `vercel-toolbar-activated` | User has activated toolbar this session |
| `vercel-live-feedback-optout` | User opted out |
| `vercel-live-feedback-hidden` | Toolbar hidden (keybinding) |
| `vercel-toolbar-intent` | Intent overlay acknowledged |
| `vercel-live-script-origin-override` | Override script origin for localhost/vercel-live-git-*.vercel.sh |
| `vt-init-retry-count` | Retry counter for init failures (max 3) |

### 4.10 initLiveFeedback (Instrument Iframe)

Called with:

- `data` — auth + settings
- `root` — shadow root
- `rootElement` — vercel-live-feedback element
- `liveOrigin` — vercel.live origin
- `canProxy`, `canProxyPusher` — extension capabilities
- `unmount` — cleanup
- `resetToolbar` — re-init
- `shouldBeHidden` — start hidden

### 4.11 Sub-Scripts

| Script | Role |
|--------|------|
| `feedback.js` | Main loader + UI shell |
| `feedback.html` | iframe content (full toolbar UI) |
| `instrument.094a043ca7ca652d4806.js` | Proxy fetch, draft status (hidden iframe) |

### 4.12 Replicache Sync API (feedback.html backend)

`feedback.html` uses [Replicache](https://replicache.com) for real-time sync of comments, users, and toolbar state.

**Pull endpoint:**
```
POST https://vercel.live/api/replicache/pull?docID=live_mode_1@prj_<projectId>@dpl_<deploymentId>
```

**Query params:**
| Param | Example | Meaning |
|-------|---------|---------|
| `docID` | `live_mode_1@prj_mQWND47C8q15UY7iZ8gvpaVW8xuW@dpl_BRVcgJ19YtZaxgtPY1sRghSXcANS` | `live_mode_1@prj_<projectId>@dpl_<deploymentId>` |

**Pull request payload:**
```json
{
  "profileID": "p2d4e7d58119f4fc08c956ac761107716",
  "clientID": "a74ff7ed-248d-4200-9de9-a2a06b6769af",
  "cookie": { "version": 7 },
  "lastMutationID": 0,
  "pullVersion": 0,
  "schemaVersion": ""
}
```

| Field | Purpose |
|-------|---------|
| `profileID` | User/session identifier |
| `clientID` | Replicache client instance ID |
| `cookie` | Server cursor for incremental sync |
| `lastMutationID` | Last applied mutation (client → server) |
| `pullVersion` / `schemaVersion` | Sync versioning |

**Pull response (patch format):**
```json
{
  "lastMutationID": 0,
  "cookie": { "version": 8 },
  "patch": [
    { "op": "put", "key": "user-<userId>", "value": { "id", "name", "color", "email", "avatar", "username", ... } },
    { "op": "put", "key": "state", "value": { "userCount", "lastUsedShortId" } },
    { "op": "put", "key": "commentThread-<id>", "value": { /* comment thread */ } },
    { "op": "put", "key": "control/partial-sync", "value": "control/PARTIAL_SYNC_DONE" }
  ]
}
```

**Comment thread shape (from patch):**
| Field | Type | Example |
|-------|------|---------|
| `id` | string | `C3raRGXeWXo5` |
| `x`, `y` | number | `0.198`, `0.308` (viewport-relative) |
| `page` | string | `"/"` |
| `nodeId` | string | CSS selector for anchor element |
| `shortId` | number | Display number (e.g. 2) |
| `subject` | string | Thread subject |
| `comments` | array | `{ id, body, text, userId, timestamp, deployment, ... }` |
| `resolved` | boolean | Thread resolution |
| `pageTitle`, `deploymentUrl`, `screenWidth`, `screenHeight`, `devicePixelRatio` | — | Info for screenshots/context |
| `followingUsers`, `hasReadUserMap` | — | Presence/read state |

**Push endpoint:**
```
POST https://vercel.live/api/replicache/push?docID=live_mode_1@prj_<projectId>@dpl_<deploymentId>
```

**Push request payload:**
| Field | Purpose |
|-------|---------|
| `profileID` | User/session |
| `clientID` | Replicache client ID |
| `mutations` | Array of `{ id, name, args, timestamp }` |
| `pushVersion` / `schemaVersion` | Sync versioning |

**Example mutation — `createCommentThread`:**
```json
{
  "id": 1,
  "name": "createCommentThread",
  "args": {
    "shortId": 0,
    "id": "8ZOzd-_oTGDT",
    "nodeId": "body>main>section:nth-of-type(4)>div:nth-of-type(2)>...>h3",
    "x": 0.096, "y": 0.515,
    "page": "/",
    "pageTitle": "...",
    "userAgent": "...",
    "screenWidth": 934, "screenHeight": 732, "devicePixelRatio": 1.25,
    "deploymentUrl": "qwind-qy5rugdl7-adsonmedia.vercel.app",
    "draftMode": false,
    "frameworkContext": "...",
    "firstComment": {
      "id": "60qoD1B3fuilPvYW8Fasi",
      "href": "https://...",
      "deployment": { "id", "ts", "author" },
      "body": [{ "type": "paragraph", "children": [{ "text": "..." }] }],
      "text": "teasing the toolbar",
      "images": [{ "id": "/3/.../screenshot-....png", "filename", "type", "size", "width", "height", "number" }],
      "leftOnLocalhost": false
    }
  },
  "timestamp": 416026.9
}
```

**Push response:** `{}` (empty object; 200 OK). Mutations are processed server-side; subsequent pull returns the new state.

### 4.12a Pusher Auth API

**Endpoint:** `POST https://vercel.live/api/pusher/auth`

Authorizes Pusher channel subscriptions for real-time updates (presence, new comments, etc.). Required for private channels (`private-vt-<roomId>`).

**Request payload:**
| Field | Type | Example |
|-------|------|---------|
| `socketId` | string | Pusher socket ID |
| `channels` | string[] | `["private-vt-live_mode_1@prj_<id>@dpl_<id>"]` |

**Response:** Object keyed by channel name; each value has:
| Field | Purpose |
|-------|---------|
| `auth` | Auth signature for Pusher |
| `channel_data` | JSON string: `{"user_id":"<userId>"}` (for presence) |

**Channel naming:** `private-vt-<roomId>` — `roomId` = `live_mode_1@prj_<projectId>@dpl_<deploymentId>` (same as Replicache docID).

**Appz parity:** If using Pusher/Ably/Socket for real-time comments: implement `POST /v0/toolbar/pusher-auth` (or equivalent) that signs channel subscriptions for the room.

### 4.13 User Data API (preferences / settings)

**Endpoint:** `GET https://vercel.live/api/feedback/user-data`

Returns user preferences and toolbar settings (authenticated; likely cookie/session).

**Response shape:**
| Field | Type | Purpose |
|-------|------|---------|
| `inp` | `{ detect, silent }` | INP (Interaction to Next Paint) monitoring |
| `inbox` | boolean | Inbox feature enabled |
| `share` | boolean | Share feature enabled |
| `theme` | string | `"system"`, `"light"`, `"dark"` |
| `shifts` | `{ detect, filter }` | Layout shift detection/filter |
| `shrink` | `{ auto, snapPoint }` | Toolbar shrink behavior (e.g. `snapPoint: 0.5`) |
| `comments` | boolean | Comments feature enabled |
| `tipState` | object | Per-tip/per-feature "last shown" timestamps, archived state |
| `emojiUsage` | `{ recents }` | Emoji picker recents |
| `recentTools` | string[] | Recently used tools (e.g. `["comment","preferences","branch-switcher","og","shifts","inp","inbox"]`) |
| `accessibility` | `{ detect }` | Accessibility detection |
| `toolbarPosition` | string | `"bottom"` or `"top"` |
| `requestMonitoring` | `{ monitor }` | Request monitoring toggle |

**Appz parity:** Implement `GET /api/feedback/user-data` (or `GET /v0/toolbar/user-data`) returning similar user preferences; persist per-user in D1/KV.

### 4.14 Toolbar Session API (bootstrap context)

**Endpoint:** `GET https://vercel.live/toolbar/session`

Returns session/context data for the current toolbar session — deployment, project, team, flags. Likely called after init when `feedback.html` has deployment context (from `dpl` or resolved from hostname).

**Response shape:**
| Field | Type | Purpose |
|-------|------|---------|
| `userId` | string | Current user ID |
| `roomId` | string | Sync room: `live_mode_1@prj_<projectId>@dpl_<deploymentId>` (matches Replicache docID) |
| `roomKey` | string | `deploymentId` |
| `teamId` | string | Team/org ID |
| `deploymentId`, `deploymentUrl` | string | Deployment reference |
| `projectId`, `projectName` | string | Project |
| `ownerId`, `ownerSlug`, `ownerName`, `ownerPlan` | string | Org/team context |
| `author` | string | Deployment author (username) |
| `rootDirectory` | string \| null | Monorepo root |
| `deploymentTs`, `deploymentTarget` | number, string | Timestamp, target (`preview`/`production`) |
| `isExternal`, `isMicrofrontend`, `isAdmin` | boolean | Access flags |
| `flags` | object | Feature flags: `branchSwitcher`, `accessibilityAuditAutoRun`, `flagsTab`, `distributedTracing`, `openInV0`, etc. |
| `experiments` | object | A/B experiments |
| `isLocalhost` | boolean | Dev vs production |
| `hasFlagsSecret` | boolean | Flags override cookie present |
| `branch`, `branchStatus` | string, object | Git branch; `readyState`, `detailsUrl`, `commitMessage` |

**Appz parity:** Implement `GET /toolbar/session` (or `GET /v0/toolbar/session`) that returns equivalent context from resolved deployment — project, team, deployment URL, feature flags. Use existing `/v0/extension/resolve` data where applicable.

### 4.15 Flags Explorer State API

**Endpoint:** `GET https://vercel.live/toolbar/flags-explorer/state`

Returns state for the flags explorer tab in the toolbar (used when `flags.flagsTab` is true in session).

**Response shape:**
| Field | Type | Purpose |
|-------|------|---------|
| `count` | number | Number of flags (e.g. `0`) |
| `threshold` | number | Threshold for flag list (e.g. `150`) |
| `overrideStatus` | string | `"DEFAULT"` \| `"OVERRIDE"` \| etc. — whether flags are overridden |

### 4.16 Sharing API (hostname → deployment resolve)

**Endpoint:** `GET https://vercel.live/api/feedback/sharing?hostname=<deployment_hostname>`

Resolves a deployment hostname (e.g. `qwind-qy5rugdl7-adsonmedia.vercel.app`) to alias, deployment, project, and owner context. Used for share links and toolbar bootstrap when context comes from hostname only.

**Query params:**
| Param | Example |
|-------|---------|
| `hostname` | `qwind-qy5rugdl7-adsonmedia.vercel.app` |

**Response shape:**
| Section | Fields |
|---------|--------|
| `alias` | `uid`, `alias`, `userRequests`, `canUpdate` |
| `deployment` | `id`, `url`, `isPreview`, `meta` |
| `project` | `id`, `name`, `passwordProtectionEnabled`, `vercelAuthEnabled`, `trustedIpsEnabled`, `hasContributors`, `frameworkLogo` |
| `owner` | `slug`, `plan`, `canReadShareableLink` |

**Appz parity:** Extend `/v0/extension/resolve` or add `GET /v0/toolbar/sharing?hostname=<hostname>` to resolve `*.appz.dev` hostnames to deployment/project/team. Return `deploymentId`, `projectId`, `ownerId`, `canShare`, etc.

### 4.17 Integrations API

**Endpoint:** `GET https://vercel.live/api/feedback/integrations`

Returns which integrations are enabled for the team/project (Slack, Jira, Linear, GitHub Issues). Used for toolbar UI (e.g. "Share to Slack", export to Jira).

**Response shape:**
| Field | Type | Purpose |
|-------|------|---------|
| `slack` | boolean | Slack integration enabled |
| `slackBeta` | boolean | Slack beta features |
| `jira` | boolean | Jira integration |
| `linear` | boolean | Linear integration |
| `ghIssues` | boolean | GitHub Issues integration |

### 4.17a Recents API

**Endpoint:** `POST https://vercel.live/api/recents`

Updates recently used tools/items (e.g. `recentTools` in user-data). Returns `202 Accepted` — fire-and-forget; no response body expected.

**Appz parity:** `POST /v0/toolbar/recents` to persist recent tools for `userData.recentTools` sync.

### 4.17b Slack Emojis API

**Endpoint:** `GET https://vercel.live/api/slack/emojis`

Returns custom Slack emojis for the team (used in comments/feedback). Response: array; may be empty `[]` if none configured.

### 4.18 Team Members API

**Endpoint:** `GET https://vercel.live/api/feedback/team-members`

Returns team members for the current project/team. Used for @mentions in comments, assignees, etc.

**Response:** Array of objects:
| Field | Type |
|-------|------|
| `id` | string |
| `displayName` | string |
| `name` | string |
| `username` | string |
| `email` | string |

### 4.19 Login / Validate API (bootstrap)

**Endpoint:** `POST https://vercel.live/login/validate?hostname=<hostname>&deploymentId=<deploymentId>`

Unified bootstrap endpoint: validates auth, returns session, Replicache pull response, and initial data (users, threads, userData) in one call. Replaces or supplements separate calls to session, user-data, and replicache/pull.

**Query params:**
| Param | Example |
|-------|---------|
| `hostname` | `qwind-qy5rugdl7-adsonmedia.vercel.app` |
| `deploymentId` | `dpl_BRVcgJ19YtZaxgtPY1sRghSXcANS` |

**Request payload:**
| Field | Type | Purpose |
|-------|------|---------|
| `skipRoomInit` | string | Room ID: `live_mode_1@prj_<id>@dpl_<id>` |
| `page` | string | Current path (e.g. `"/"`) |
| `existingTokens` | string[] | JWT auth tokens |
| `replicacheCookie` | string | JSON: `{"version":N}` — Replicache cursor |
| `vercelAuthJWE` | string | JWE from `/.well-known/vercel/jwe` |
| `tracking` | object | `sessionId`, `ua` |

**Response shape:**
| Section | Content |
|---------|---------|
| `token` | JWT — auth token for subsequent requests |
| `session` | Same as [§4.14](#414-toolbar-session-api-bootstrap-context) — userId, roomId, deployment, project, owner, flags, etc. |
| `pull` | `baseCookie` + `pullResponse` (Replicache patch, same format as [§4.12](#412-replicache-sync-api-feedbackhtml-backend)) |
| `initialData` | `users`, `threads`, `currentUser`, `userData` (same as user-data API §4.13) |
| `authOrigin` | string | `"auto-logged-in"` \| etc. |
| `hasRecommendedFlags` | boolean | Flag recommendations for intent |

**Appz parity:** Implement a single `POST /v0/toolbar/validate` (or similar) that accepts hostname/deploymentId + auth, returns session + initial Replicache pull + user preferences in one response to reduce round-trips on toolbar init.

---

## 5. Toolbar Object (Package API)

From `toolbar-object.d.ts`:

```ts
type VercelToolbar = BaseVercelToolbar | AuthenticatedVercelToolbar;

interface BaseVercelToolbar {
  unmount: () => void;
  isAuthenticated: false;
}

interface AuthenticatedVercelToolbar extends Omit<BaseVercelToolbar, 'isAuthenticated'> {
  isAuthenticated: true;
  setLocalhostEventSource?: (eventSource: LocalhostEventSource) => void;
}

interface LocalhostEventSource {
  version: string;
  subscribe: <K extends keyof ServerEvents>(type: K, callback: (payload: ServerEvents[K]) => void) => () => void;
  fetchLocal: <T, U>(path: string, body?: U) => Promise<{ result: T } | { error: string }>;
}

type ServerEvents = {
  'branch-change': string | undefined;
  'commands-ready': ToolbarPluginCommand[];
  'command-reply-stream': CommandReplyStreamEvent;
};
```

- **Unauthenticated:** Only `unmount()`.
- **Authenticated:** Same plus `setLocalhostEventSource` for localhost dev.
- **Localhost:** Toolbar connects to local dev server via `EventSource` and `fetch`; branch changes trigger reload; plugins provide commands.

---

## 6. Commands API (`commands/`)

Custom menu items in the toolbar:

```ts
interface ToolbarCommandConfig {
  label: string;
  badge?: string;
  badgeColor?: 'amber' | 'blue' | 'pink' | 'purple' | 'red' | 'teal' | 'green' | 'gray';
  aliases?: string[];
  href?: string;
  onSelect?: () => void | boolean | Promise<void | boolean>;
  visible?: boolean;
}

createCommand(config): VercelToolbarCommand;
getInternalRegistry(useTopWindow?): ToolbarCommandRegistry | undefined;
```

Commands are stored in `window.__vtcr` (internal registry). React: `useCommand(config)`.

---

## 7. Localhost Dev Server

Runs a Node.js HTTP server (default port **43214**) when using the Next.js or Vite plugin in development.

### Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/events` | SSE stream for `branch-change`, `commands-ready`, `command-reply-stream` |
| GET | `/branch` | Current git branch (`git branch --show-current`) |
| GET | `/microfrontend-config` | Microfrontends config (from `microfrontends.json`) |
| GET | `/commands` | List of plugin commands |
| POST | `/command-invoke` | Invoke a plugin command (body: `CommandInvocation`) |
| POST | `/command-stop` | Abort running command by `replyCommentId` |

### Project discovery

1. **Repo:** `.vercel/repo.json` → `orgId`, `projectId` from matching `directory`
2. **Single project:** `find-up` `.vercel/project.json` → `orgId`, `projectId`
3. **Branch:** `git branch --show-current`
4. **Watcher:** `chokidar` on `.git/HEAD` → emits `branch-change` → page reload

### Plugin system

```ts
interface ToolbarPlugin {
  name: string;
  onInit?: (options: PluginInitOptions) => Promise<void> | void;
  getCommands: () => Promise<ToolbarPluginCommand[]> | ToolbarPluginCommand[];
  onCommand: (invocation: CommandInvocation, reply: CommandReply) => Promise<void>;
}
```

Plugins register commands; comments can trigger them. Reply supports streaming via `reply.update()` and `reply.finalize()`.

---

## 8. Framework Integrations

### Next.js

- **Plugin:** `withVercelToolbar({ devServerPort?, enableInProduction?, plugins? })` — wraps `next.config.js`
- **Component:** `<VercelToolbar />` — uses `next/script` to load the toolbar script
- **LocalhostController:** Connects to dev server, subscribes to `branch-change`, exposes `setLocalhostEventSource` when authenticated

### React

- **Hook:** `useCommand(config)` — registers a command, returns `VercelToolbarCommand`
- Requires `mountVercelToolbar()` to be called separately

### Vite

- **Plugin:** `vercelToolbar({ devServerPort?, plugins? })` — adds dev server and toolbar injection in dev

### Nuxt

- **Module:** `mode: 'auto' | 'manual' | 'disabled'` — controls when toolbar is shown (auto = dev only)

---

## 9. Dependencies

From `package.json`:

- `@tinyhttp/app` — localhost HTTP server
- `@vercel/microfrontends` — config discovery
- `chokidar` — file watching
- `execa` — git / Node subprocess
- `fast-glob` — globbing
- `find-up` — `.vercel` lookup
- `get-port` — free port
- `jsonc-parser` — JSONC for microfrontends config
- `strip-ansi` — log output

---

## 10. Appz Toolbar Comparison

| Aspect | @vercel/toolbar | Appz Toolbar |
|--------|-----------------|--------------|
| **Architecture** | Script loader → hosted script from vercel.live | Self-contained content script |
| **UI location** | External (`vercel.live/_next-live/feedback/feedback.js`) | In extension (`toolbar.content/`) |
| **Context** | ownerId, projectId, branch, deploymentId | host → API resolve → project/deployment |
| **Localhost** | Node dev server (SSE, branch sync, plugins) | Cookie opt-in |
| **Extension** | Separate Vercel extension (toolbar-injector) | Same WXT package (toolbar-injector + toolbar) |
| **Commands** | Plugin system, comments → commands | Copy, Screenshot, Open in Appz, env switcher |
| **Auth** | Vercel session / extension auth | Appz session (`credentials: 'include'`) |

---

## 11. Takeaways for Appz

1. **Self-contained vs hosted:** Appz ships the toolbar in the extension; Vercel loads it from a CDN. Appz avoids a hosted dependency.
2. **Context passing:** Vercel uses data attributes; Appz uses `resolve` API and message passing.
3. **Localhost:** Vercel’s Node dev server is for branch sync and plugins. Appz can add a similar flow later if needed.
4. **Commands:** Vercel’s `createCommand` / plugin model is extensible. Appz could introduce a simple command registry for future features.
5. **Toolbar object:** `window.__vercel_toolbar` gives the hosted script a way to receive localhost events. Appz could expose `window.__appz_toolbar` for future dashboard/extension handshake.

---

## 12. Appz-Ref Toolbar References

**Location:** `~/workspace/appz-ref/`

### Contents

| Path | Purpose |
|------|---------|
| `vercel/` | Full Vercel source (CLI, client, packages) |
| `vercel-toolbar/` | Extracted `@vercel/toolbar@0.2.2` + `feedback.js` (hosted script) |
| `byob-starter/` | Rocicorp minimal Replicache BYOB skeleton |
| `browser-ext/<id>/` | Vercel extension unpacked (Chrome extension ID directory) |

### BYOB Starter (`appz-ref/byob-starter/`)

- **Origin:** `rocicorp/byob-starter` (cloned)
- **Structure:** `client/`, `server/`, `shared/` monorepo
- **Server:** Express in `server/src/main.ts` — static serve + health only. **No pull/push handlers wired.**
- **Dependencies:** `replicache-transaction`, `pusher`, `express` in server; `replicache`, `replicache-react`, `pusher-js` in client
- **Status:** Skeleton; pull/push must be implemented per Replicache BYOB tutorial or a full reference.

### Cloudflare BYOB (external reference)

**Repo:** [KaiSpencer/replicache-byob-cloudflare](https://github.com/KaiSpencer/replicache-byob-cloudflare)

Full Replicache sync server on Cloudflare — Hono on Workers, D1, Drizzle, Pusher, SST Ion.

| Component | Implementation |
|-----------|----------------|
| **Framework** | Hono on Cloudflare Workers |
| **DB** | D1 + Drizzle ORM |
| **Routes** | `POST /api/replicache/pull`, `POST /api/replicache/push` |
| **Tables** | `replicache_server` (global version), `replicache_client` (lastMutationID per client), `message` (domain) |
| **Pull** | Batch read from D1; return `PatchOperation[]` (put/del); cookie = currentVersion |
| **Push** | Switch on `mutation.name` (e.g. `createMessage`); update version; `sendPoke()` via Pusher |
| **Poke** | Pusher trigger to `default` channel; clients re-pull on `poke` |

**Appz adaptation:** Use this repo as the Cloudflare implementation pattern. Replace `message` domain with `commentThread` / comment-related tables; add auth (team/project/deployment scope).
