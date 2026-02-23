# Appz Toolbar Ecosystem — Design & Technical Implementation Plan

**Date:** 2025-02-22  
**Status:** Design  
**References:**
- [Vercel Toolbar Scan](./2025-02-22-vercel-toolbar-scan.md) — API surface, UI, product context
- [Appz Toolbar Full Implementation](./2025-02-22-appz-toolbar-full-implementation.md) — Path A/B, hosted script
- [Appz Toolbar Extension Design](./2025-02-22-appz-toolbar-extension-design.md) — Extension architecture

---

## 1. Executive Summary

This document defines the **full design and technical plan** for the Appz toolbar ecosystem: browser extension (Path A), hosted script (Path B), backend APIs, data models, and implementation phases. The goal is Vercel toolbar parity for comments, menu, preferences, and integrations, with a phased rollout.

### 1.1 Scope

| Feature | Phase | Notes |
|---------|-------|-------|
| Extension toolbar (env, copy URL, Open in Appz) | Done | Current appz-dev |
| Comments (create, reply, inbox, markers) | P1 | Replicache sync, screenshot upload |
| Toolbar menu (search, navigate, preferences) | P1 | |
| User preferences (theme, position, shortcuts) | P1 | |
| Screenshot attachment | P1 | Extension + blob storage |
| Real-time (Pusher/WebSocket) | P2 | Presence, live updates |
| Feature flags, Draft mode | P2 | |
| Layout shifts, INP, Accessibility audit | P3 | |
| @appz/toolbar npm package | P2 | Path B enabler |
| JWE auth (Path B, custom domains) | P2 | |

---

## 2. System Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                              DEPLOYMENT PAGE (*.appz.dev)                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│  Path A: Extension                    │  Path B: Hosted Script (future)                  │
│  • toolbar-injector (all URLs)       │  • <script src="appz.dev/_appz/toolbar/feedback.js">│
│  • toolbar.content (*.appz.dev)      │  • Creates <appz-live-toolbar> + iframe             │
│  • postMessage, extension-clicked    │  • Same postMessage protocol                       │
└─────────────────────────────────────┴───────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│  api.appz.dev / appz.dev                                                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│  GET /v0/extension/resolve           │  Toolbar APIs (new)                                 │
│  GET /v0/toolbar/session             │  POST /v0/toolbar/validate (bootstrap)             │
│  POST /v0/replicache/pull            │  POST /v0/replicache/push                          │
│  GET /v0/toolbar/user-data           │  PUT /v0/toolbar/screenshot (→ R2)                 │
│  GET /v0/toolbar/sharing?hostname=   │  POST /v0/toolbar/recents                          │
│  GET /v0/toolbar/integrations        │  GET /v0/toolbar/team-members                      │
│  POST /v0/toolbar/pusher-auth        │  (optional)                                        │
└─────────────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│  Storage: D1 (metadata), R2 (screenshots), KV (user prefs), Replicache server state        │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.1 Room / Doc ID Format

All sync and real-time use a unified room identifier:

```
roomId = live_mode_1@prj_<projectId>@dpl_<deploymentId>
docID = roomId  (Replicache)
channel = private-vt-<roomId>  (Pusher)
```

---

## 3. API Specification

All APIs live under `api.appz.dev` or `appz.dev`; auth via cookie/session or JWE for Path B.

### 3.1 Bootstrap: Validate (unified init)

**Endpoint:** `POST /v0/toolbar/validate?hostname=&deploymentId=`  
**Purpose:** Single round-trip init — session + Replicache pull + initialData (users, threads, userData).

**Request:**
```json
{
  "skipRoomInit": "live_mode_1@prj_<id>@dpl_<id>",
  "page": "/",
  "existingTokens": ["jwt..."],
  "replicacheCookie": "{\"version\":0}",
  "appzAuthJWE": "jwe...",
  "tracking": { "sessionId": "...", "ua": "..." }
}
```

**Response:**
```json
{
  "token": "jwt",
  "session": { "userId", "roomId", "deploymentId", "projectId", "teamId", "deploymentUrl", "projectName", "ownerSlug", "flags", "branch", "branchStatus", ... },
  "pull": {
    "baseCookie": { "version": 0 },
    "pullResponse": { "lastMutationID", "cookie", "patch": [...] }
  },
  "initialData": {
    "users": [...],
    "threads": [...],
    "currentUser": {...},
    "userData": { "inp", "theme", "comments", "recentTools", "toolbarPosition", ... }
  },
  "authOrigin": "auto-logged-in",
  "hasRecommendedFlags": false
}
```

**Fallback:** If validate not implemented, client calls `session`, `user-data`, `replicache/pull` separately.

### 3.2 Session

**Endpoint:** `GET /v0/toolbar/session`  
**Auth:** Cookie.  
**Response:** `userId`, `roomId`, `roomKey`, `teamId`, `deploymentId`, `deploymentUrl`, `projectId`, `projectName`, `ownerId`, `ownerSlug`, `ownerName`, `author`, `flags`, `branch`, `branchStatus`, etc.

### 3.3 Replicache Pull

**Endpoint:** `POST /v0/replicache/pull?docID=<roomId>`  
**Request:** `{ profileID, clientID, cookie, lastMutationID, pullVersion, schemaVersion }`  
**Response:** `{ lastMutationID, cookie, patch: [{ op, key, value }] }`

**Keys:** `user-<id>`, `state`, `commentThread-<id>`, `control/partial-sync`.

### 3.4 Replicache Push

**Endpoint:** `POST /v0/replicache/push?docID=<roomId>`  
**Request:** `{ profileID, clientID, mutations: [{ id, name, args, timestamp }], pushVersion, schemaVersion }`  
**Response:** `{}`

**Mutations:** `createCommentThread`, `addComment`, `resolveThread`, `updateComment`, etc.

### 3.5 User Data (preferences)

**Endpoint:** `GET /v0/toolbar/user-data`  
**Response:** `inp`, `inbox`, `share`, `theme`, `shifts`, `shrink`, `comments`, `tipState`, `emojiUsage`, `recentTools`, `accessibility`, `toolbarPosition`, `requestMonitoring`.

**Write:** `PATCH /v0/toolbar/user-data` or per-field updates (TBD).

### 3.6 Sharing (hostname resolve)

**Endpoint:** `GET /v0/toolbar/sharing?hostname=<hostname>`  
**Response:** `alias`, `deployment`, `project`, `owner`.

### 3.7 Integrations

**Endpoint:** `GET /v0/toolbar/integrations`  
**Response:** `{ slack, slackBeta, jira, linear, ghIssues }`.

### 3.8 Team Members

**Endpoint:** `GET /v0/toolbar/team-members`  
**Response:** `[{ id, displayName, name, username, email }]`.

### 3.9 Recents

**Endpoint:** `POST /v0/toolbar/recents`  
**Payload:** `{ tools: ["comment","preferences",...] }` or similar.  
**Response:** `202 Accepted`.

### 3.10 Screenshot Upload

**Endpoint:** `PUT /v0/toolbar/screenshot` or direct R2 presigned URL.  
**Path:** `{owner}/{projectId}/{deploymentId}/screenshot-{id}.png`  
**Payload:** PNG bytes.  
**Response:** `{ url, downloadUrl, pathname, contentType }`.

### 3.11 Pusher Auth (P2)

**Endpoint:** `POST /v0/toolbar/pusher-auth`  
**Request:** `{ socketId, channels: ["private-vt-<roomId>"] }`  
**Response:** `{ "private-vt-<roomId>": { auth, channel_data } }`.

### 3.12 JWE (Path B, custom domains)

**Endpoint:** `GET /.well-known/appz/jwe` — served from deployment origin.  
**Response:** JWE string.

---

## 4. Data Models

### 4.1 Replicache Server State (per room)

| Key | Value Shape |
|-----|-------------|
| `user-<userId>` | `{ id, name, color, email, avatar, username, isExternal, notificationPreference }` |
| `state` | `{ userCount, lastUsedShortId }` |
| `commentThread-<id>` | See §4.2 |
| `control/partial-sync` | `"control/PARTIAL_SYNC_DONE"` |

### 4.2 Comment Thread

```ts
interface CommentThread {
  id: string;
  x: number;  // viewport 0–1
  y: number;
  page: string;
  nodeId: string;  // CSS selector
  shortId: number;
  subject: string;
  comments: Comment[];
  resolved: boolean;
  draftMode?: boolean;
  pageTitle?: string;
  userAgent?: string;
  screenWidth?: number;
  screenHeight?: number;
  devicePixelRatio?: number;
  deploymentUrl?: string;
  selectionRange?: { text, startOffset, endOffset, startContainerNodeId, endContainerNodeId, ... };
  followingUsers?: string[];
  hasReadUserMap?: Record<string, boolean>;
  frameworkContext?: string;
  lastScreenshotNumber?: number;
}

interface Comment {
  id: string;
  body: Block[];  // Slate/Plate-style
  text: string;
  href: string;
  images: ImageRef[];
  userId: string;
  pending?: boolean;
  timestamp: number;
  deployment: { id, ts, author };
  mentionedUsers?: string[];
  writeConfirmed?: boolean;
  leftOnLocalhost?: boolean;
}

interface ImageRef {
  id: string;   // blob path
  filename: string;
  type: string;
  size: number;
  width: number;
  height: number;
  number: number;
}
```

### 4.3 D1 Tables (toolbar)

```sql
-- User preferences (or KV keyed by userId)
CREATE TABLE toolbar_user_prefs (
  user_id TEXT PRIMARY KEY,
  team_id TEXT,
  data JSON NOT NULL,  -- full userData object
  updated_at INTEGER
);

-- Optional: comment_threads for persistence (if not Replicache-native)
-- Replicache can use in-memory + log; D1 for durability.
```

### 4.4 R2 Structure (screenshots)

```
bucket: appz-toolbar-screenshots
path: {owner}/{projectId}/{deploymentId}/screenshot-{nanoid}.png
```

---

## 5. UI Specification

### 5.1 Toolbar Button (floating)

- Pill-shaped, right edge, draggable
- Snaps left/right; stores position in localStorage
- States: sleeping, active, loading

### 5.2 Menu Panel (Ctrl / click)

- Header: "Appz Toolbar"
- Search: "What do you need?"
- Quick actions: comment, share, …
- List: Comments, Feature flags, Edit mode, Layout shifts, Interaction timing, Accessibility, Open Graph, View inbox; Navigate (team, project, deployment); Hide, Disable, Preferences, Logout

### 5.3 Comments Creation ("Need help?")

- Input: "What do you need?"
- Rich text toolbar: bold, italic, lists, code, quote, link, image
- Actions: Add comment, Pin, Move, Duplicate, Share, Send to Slack; Development, AI Help, Tasks, View full site
- Selection overlay: pink highlight on page

### 5.4 Comments Inbox

- Header: back, "Comments", plus, refresh, close
- Filter: "X unresolved"; sort (Newest)
- Thread list: status dot (purple/grey), timestamp, expandable
- Expanded: avatar, text, Reply input, send

### 5.5 Comment Markers

- Pink rounded rect with speech bubble; numbered (shortId)
- Anchored via `nodeId` (CSS selector) or `x,y` fallback

---

## 6. Implementation Phases

### Phase 1: Comments + Core APIs (8–12 weeks)

| Task | Deps | Deliverables |
|------|------|--------------|
| 1.1 D1 migrations | — | `toolbar_user_prefs`; `comment_threads` if needed |
| 1.2 R2 bucket + screenshot upload | — | `PUT /v0/toolbar/screenshot` → R2 |
| 1.3 Replicache server | 1.1 | Pull/Push handlers; in-memory or D1-backed |
| 1.4 Session API | resolve | `GET /v0/toolbar/session` |
| 1.5 Validate API | 1.3, 1.4 | `POST /v0/toolbar/validate` |
| 1.6 User data API | 1.1 | `GET /v0/toolbar/user-data` |
| 1.7 Sharing API | resolve | `GET /v0/toolbar/sharing?hostname=` |
| 1.8 Team members API | — | `GET /v0/toolbar/team-members` |
| 1.9 Recents API | 1.1 | `POST /v0/toolbar/recents` |
| 1.10 Toolbar UI (iframe) | — | Menu, comments panel, markers |
| 1.11 Extension: screenshot | — | `takeScreenshot`, upload, attach to comment |
| 1.12 createCommentThread mutation | 1.3 | Handle in push; store thread |

### Phase 2: Real-time + Extensions

| Task | Deps | Deliverables |
|------|------|--------------|
| 2.1 Pusher auth | — | `POST /v0/toolbar/pusher-auth` |
| 2.2 Pusher integration | 2.1 | Subscribe to `private-vt-<roomId>`; presence |
| 2.3 @appz/toolbar npm | — | mountAppzToolbar, Next/Vite plugins |
| 2.4 JWE for Path B | — | `/.well-known/appz/jwe` on deployments |
| 2.5 Hosted script (Path B) | 2.3, 2.4 | feedback.js, toolbar.html |

### Phase 3: Advanced Tools

| Task | Deps | Deliverables |
|------|------|--------------|
| 3.1 Feature flags tab | session.flags | Flags explorer UI |
| 3.2 Layout shifts tool | — | CLS detection UI |
| 3.3 Interaction timing (INP) | — | INP panel |
| 3.4 Accessibility audit | — | WCAG checks |
| 3.5 Draft mode | — | Draft content preview |

---

## 7. Technical Decisions

### 7.1 Replicache vs Custom Sync

**Decision:** Use Replicache or a Replicache-compatible protocol.
- **Pros:** Offline-first, proven, Vercel parity.
- **Alternative:** Custom pull/push with simpler patch format; more control, less ecosystem.

### 7.2 Pusher vs Ably vs Custom

**Decision:** Pusher for P2 (matches Vercel). Alternative: Ably, or custom WebSocket.

### 7.3 Screenshot Storage

**Decision:** R2. Path: `{owner}/{projectId}/{deploymentId}/screenshot-{nanoid}.png`. Public URL via worker or R2 public bucket.

### 7.4 User Preferences

**Decision:** D1 `toolbar_user_prefs` or KV. KV is simpler for key-value; D1 if we need querying.

### 7.5 Auth for Path B

**Decision:** Phase 1: `deploymentId` + hostname; session via cookie if same-site. Phase 2: JWE from `/.well-known/appz/jwe` for custom domains.

---

## 8. File Structure (appz-dev)

```
appz-dev/
├── apps/workers/v0/src/routers/
│   ├── toolbar/
│   │   ├── toolbar.handler.ts      # session, user-data, sharing, recents, team-members, validate
│   │   ├── replicache.handler.ts  # pull, push
│   │   ├── screenshot.handler.ts  # upload → R2
│   │   └── pusher.handler.ts      # auth (P2)
│   └── extension/                 # existing
├── packages/browser/
│   ├── entrypoints/
│   │   ├── toolbar.content/      # enhanced: comments UI, screenshot capture
│   │   ├── toolbar-injector.content.ts
│   │   └── appz-site.content.ts
│   └── ...
├── packages/db/db-d1/
│   └── drizzle/
│       └── XXXX_toolbar_user_prefs.sql
└── apps/app/public/_appz/toolbar/  # or Worker route
    ├── feedback.js
    └── toolbar.html
```

---

## 9. Success Criteria

- [ ] User can create a comment on a deployment (click or text select)
- [ ] User can attach a screenshot (extension)
- [ ] Comments appear in inbox; filter by resolved
- [ ] User preferences persist (theme, position, recent tools)
- [ ] Session/bootstrap returns in one validate call when possible
- [ ] Extension and hosted script (Path B) share same API contracts

---

## 10. Open Questions

1. **Replicache server:** Use [replicache-replicache](https://github.com/rocicorp/replicache) server, or roll custom pull/push?
2. **Comment body format:** Slate, Plate, or custom JSON blocks?
3. **Notifications:** Email on new comment — use existing appz email infra?
