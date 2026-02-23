# Appz Toolbar Extension Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a browser extension that shows a developer toolbar on *.appz.dev deployments and integrates with the Appz dashboard, using WXT (same structure as repomix browser extension).

**Location:** The browser extension lives in `appz-dev/packages/browser/` (not appz-cli).

**Architecture:** Three content scripts—injector (all URLs), toolbar (*.appz.dev), appz-site (appz.dev)—plus background for API proxy. Toolbar displays env label, copy URL, Open in Appz. New API endpoint `/v0/extension/resolve` returns deployment metadata for authenticated users.

**Tech Stack:** WXT, TypeScript, Manifest V3, api.appz.dev

**Reference:** Design doc [2025-02-22-appz-toolbar-extension-design.md](./2025-02-22-appz-toolbar-extension-design.md)

---

## Phase 1: Scaffold & Core

### Task 1: WXT project scaffold

**Files:**
- Create: `appz-dev/packages/browser/package.json`
- Create: `appz-dev/packages/browser/wxt.config.ts`
- Create: `appz-dev/packages/browser/tsconfig.json`

**Step 1: Create package.json**

```json
{
  "name": "appz-toolbar",
  "private": true,
  "version": "0.1.0",
  "scripts": {
    "dev": "wxt",
    "dev:firefox": "wxt -b firefox",
    "build": "wxt build",
    "build:chrome": "wxt build -b chrome",
    "prepare": "wxt prepare"
  },
  "devDependencies": {
    "@types/chrome": "^0.1.36",
    "typescript": "^5.9.3",
    "wxt": "^0.20.14"
  }
}
```

**Step 2: Create wxt.config.ts**

```ts
import { defineConfig } from 'wxt';

export default defineConfig({
  manifest: {
    name: 'Appz Toolbar',
    version: '0.1.0',
    description: 'Developer toolbar for Appz deployments',
    permissions: ['storage', 'alarms', 'scripting'],
    host_permissions: [
      'https://appz.dev/*',
      'https://*.appz.dev/*',
      'https://api.appz.dev/*',
      'https://localhost/*',
    ],
    minimum_chrome_version: '88.0',
  },
});
```

**Step 3: Create tsconfig.json**

```json
{
  "extends": "./.wxt/tsconfig.json",
  "compilerOptions": {
    "strict": true
  }
}
```

**Step 4: Install deps**

Run: `cd appz-dev/packages/browser && pnpm install` (or from appz-dev root: `pnpm install`)

**Step 5: Verify scaffold**

Run: `pnpm run build`
Expected: Build succeeds; output in `.output/chrome-mv3` or similar

**Step 6: Commit**

```bash
git add packages/browser/  # in appz-dev repo
git commit -m "feat(browser): add WXT scaffold for Appz toolbar extension"
```

---

### Task 2: Background entrypoint

**Files:**
- Create: `appz-dev/packages/browser/entrypoints/background.ts`

**Step 1: Create background.ts**

```ts
export default defineBackground(() => {
  chrome.runtime.onMessage.addListener(
    (
      msg: { type: string; host?: string },
      _sender,
      sendResponse
    ) => {
      if (msg.type === 'resolve' && msg.host) {
        resolveDeployment(msg.host)
          .then(sendResponse)
          .catch((err) => sendResponse({ error: String(err) }));
        return true; // async response
      }
    }
  );
});

async function resolveDeployment(host: string): Promise<unknown> {
  const url = `https://api.appz.dev/v0/extension/resolve?host=${encodeURIComponent(host)}`;
  const res = await fetch(url, { credentials: 'include' });
  if (!res.ok) return { error: `HTTP ${res.status}` };
  return res.json();
}
```

**Step 2: Build and verify**

Run: `pnpm run build`
Expected: No errors; background.js in output

**Step 3: Commit**

```bash
git add packages/browser/entrypoints/background.ts
git commit -m "feat(browser): add background script with resolve handler"
```

---

### Task 3: Content script – toolbar injector

**Files:**
- Create: `appz-dev/packages/browser/entrypoints/content-toolbar-injector.ts`

**Step 1: Create content-toolbar-injector.ts**

```ts
const APPZ_TOOLBAR_COOKIE = '__appz_toolbar';
const APPZ_DEV_SUFFIX = '.appz.dev';
const APPZ_PREVIEW_SUFFIX = '.preview.appz.dev';

function isAppzDomain(host: string): boolean {
  return (
    host === 'appz.dev' ||
    host.endsWith(APPZ_DEV_SUFFIX) ||
    host.endsWith(APPZ_PREVIEW_SUFFIX)
  );
}

function shouldShowToolbar(host: string): boolean {
  if (!isAppzDomain(host)) return false;
  if (host === 'appz.dev') return false; // Dashboard only needs meta, no toolbar
  const cookie = document.cookie
    .split('; ')
    .find((c) => c.startsWith(`${APPZ_TOOLBAR_COOKIE}=`));
  if (cookie) return cookie.split('=')[1] === '1';
  return true; // Show on *.appz.dev by default
}

export default defineContentScript({
  matches: ['<all_urls>'],
  runAt: 'document_start',
  main() {
    const host = window.location.hostname;
    if (!shouldShowToolbar(host)) return;
    document.documentElement.setAttribute('data-appz-toolbar', '1');
    window.postMessage({ type: 'appz-toolbar-inject' }, '*');
  },
});
```

**Step 2: Add content script to wxt config**

In `wxt.config.ts`, content scripts are auto-discovered from `entrypoints/content-*.ts`. Verify filename matches WXT convention.

**Step 3: Build**

Run: `pnpm run build`
Expected: content-toolbar-injector.js in output

**Step 4: Commit**

```bash
git add packages/browser/entrypoints/content-toolbar-injector.ts
git commit -m "feat(browser): add toolbar injector content script"
```

---

### Task 4: Content script – toolbar UI

**Files:**
- Create: `appz-dev/packages/browser/entrypoints/content-toolbar.ts`
- Create: `appz-dev/packages/browser/entrypoints/toolbar.css`

**Step 1: Create content-toolbar.ts**

```ts

const DASHBOARD_URL = 'https://appz.dev';

function inferEnv(host: string): 'production' | 'preview' {
  const sub = host.replace('.appz.dev', '').replace('.preview.appz.dev', '');
  // Preview: slug-hash-scope or slug-git-branch-scope
  if (sub.includes('-git-') || /-[a-z0-9]{6,}-/.test(sub)) return 'preview';
  return 'production';
}

function createToolbar(host: string): HTMLElement {
  const env = inferEnv(host);
  const bar = document.createElement('div');
  bar.id = 'appz-toolbar';
  bar.className = 'appz-toolbar';
  bar.innerHTML = `
    <span class="appz-toolbar-env">${env}</span>
    <span class="appz-toolbar-host">${host}</span>
    <button class="appz-toolbar-copy" title="Copy URL">Copy</button>
    <a class="appz-toolbar-link" href="${DASHBOARD_URL}" target="_blank" rel="noopener">Open in Appz</a>
  `;

  bar.querySelector('.appz-toolbar-copy')?.addEventListener('click', () => {
    navigator.clipboard.writeText(window.location.href);
  });

  return bar;
}

function initToolbar() {
  if (document.getElementById('appz-toolbar')) return;
  if (!document.documentElement.hasAttribute('data-appz-toolbar')) return;

  const host = window.location.hostname;
  if (host === 'appz.dev') return;

  const bar = createToolbar(host);
  document.body.appendChild(bar);
}

function onMessage(e: MessageEvent) {
  if (e.data?.type === 'appz-toolbar-inject') initToolbar();
}

export default defineContentScript({
  matches: ['*://*.appz.dev/*', '*://*.preview.appz.dev/*'],
  runAt: 'document_end',
  css: ['toolbar.css'],
  main() {
    window.addEventListener('message', onMessage);
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', initToolbar);
    } else {
      initToolbar();
    }
  },
});
```

**Step 2: Create toolbar.css**

Create `appz-dev/packages/browser/entrypoints/toolbar.css` (WXT injects it via `css` in defineContentScript):

```css
.appz-toolbar {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  height: 36px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 16px;
  background: rgba(0, 0, 0, 0.85);
  color: #fff;
  font-size: 12px;
  z-index: 2147483647;
}
.appz-toolbar-env { font-weight: 600; text-transform: capitalize; }
.appz-toolbar-host { opacity: 0.8; }
.appz-toolbar-copy {
  background: #333;
  border: 1px solid #555;
  color: #fff;
  padding: 4px 8px;
  border-radius: 4px;
  cursor: pointer;
}
.appz-toolbar-link { color: #0cf; text-decoration: none; }
```

**Step 3: Build and test**

Run: `pnpm run build`
Load unpacked from `appz-dev/packages/browser/.output/chrome-mv3`, visit `https://some-project.appz.dev` (or localhost mock). Verify toolbar appears.

**Step 4: Commit**

```bash
git add packages/browser/entrypoints/content-toolbar.ts packages/browser/entrypoints/toolbar.css
git commit -m "feat(browser): add toolbar UI with env, copy, open link"
```

---

### Task 5: Content script – appz-site meta tag

**Files:**
- Create: `appz-dev/packages/browser/entrypoints/content-appz-site.ts`

**Step 1: Create content-appz-site.ts**

```ts
export default defineContentScript({
  matches: ['*://appz.dev/*', '*://*.appz.dev/*'],
  runAt: 'document_end',
  main() {
    const meta = document.createElement('meta');
    meta.name = 'appz-extension';
    meta.content = '1.0.0';
    meta.setAttribute('data-version', '1.0.0');
    document.head.appendChild(meta);
  },
});
```

**Step 2: Build**

Run: `pnpm run build`

**Step 3: Commit**

```bash
git add packages/browser/entrypoints/content-appz-site.ts
git commit -m "feat(browser): inject appz-extension meta on appz.dev"
```

---

### Task 6: API endpoint – resolve

**Files:**
- Modify: `appz-dev/apps/workers/v0/src/app.ts` (or router) – add extension router
- Create: `appz-dev/apps/workers/v0/src/routers/extension/extension.handler.ts`

**Reference:** @appz-dev structure (v0 router, deployment processor, aliases)

**Step 1: Create extension.handler.ts**

```ts
import { Hono } from 'hono';
import type { AppBindings } from '@/lib/context';
import { getDeployment } from '@/routers/deployments/deployment.processor';
// Resolve host (e.g. myproject-team.appz.dev) to deployment.
// Uses same auth as other v0 routes.
```

Implement `GET /resolve?host=...`:
- Parse host; if `*.appz.dev`, extract subdomain.
- For custom domains, use alias resolution (see v0-static).
- Call getDeployment or alias lookup; return `{ deploymentId, projectId, projectSlug, teamSlug, type, url, previewUrl?, createdAt }`.
- 401 if no session; 404 if not found.

**Step 2: Mount extension router**

In v0 router: `v0Router.route('/extension', extensionRouter)`.

**Step 3: Test API**

Run appz-dev locally; `curl -b "session=..." "http://localhost:8080/v0/extension/resolve?host=myproject.appz.dev"`.

**Step 4: Commit**

```bash
git add apps/workers/v0/src/routers/extension/
git commit -m "feat(v0): add /v0/extension/resolve endpoint"
```

---

### Task 7: Icons and manifest polish

**Files:**
- Create: `appz-dev/packages/browser/public/images/icon.svg` (or PNG set)
- Modify: `appz-dev/packages/browser/wxt.config.ts` – add icons to manifest

**Step 1: Add icons**

Generate or copy icon 16, 32, 48, 128. Add to `wxt.config.ts`:

```ts
manifest: {
  icons: {
    16: 'images/icon-16.png',
    32: 'images/icon-32.png',
    48: 'images/icon-48.png',
    128: 'images/icon-128.png',
  },
  // ...
},
```

**Step 2: Commit**

```bash
git add packages/browser/public/images/ packages/browser/wxt.config.ts
git commit -m "chore(browser): add extension icons"
```

---

## Phase 2: Auth & Enhancements (Optional)

### Task 8: Extension auth flow (if cookies not sent)

**Context:** If `credentials: 'include'` from background does not send appz.dev cookies, implement token-based auth.

**Files:**
- Create: `appz-dev/apps/app/src/pages/extension/ConnectPage.tsx`
- Create: `appz-dev/apps/app/src/pages/extension/CallbackPage.tsx`
- Modify: `appz-dev/packages/browser/entrypoints/background.ts` – use stored token

**Steps:** User visits appz.dev/extension/connect → approves → redirect to callback with token → extension stores token. Background sends `Authorization: Bearer <token>`.

---

### Task 9: Environment switcher

**Files:**
- Modify: `appz-dev/packages/browser/entrypoints/content-toolbar.ts`

Add dropdown: "Production" → production URL, "Preview" → previewUrl from resolve response. Call background resolve on load; populate switcher from response.

---

### Task 10: Domain sync (alarm)

**Files:**
- Modify: `browser/entrypoints/background.ts`

On install: `chrome.alarms.create('sync-domains', { periodInMinutes: 60 })`. On alarm: fetch user domains from `/v0/extension/domains` or `/v0/aliases`; save to `storage.local.domains`. Injector checks cache for custom domains.

---

## Testing Checklist

- [ ] Load unpacked in Chrome; visit https://project.appz.dev
- [ ] Toolbar appears at bottom; env shows Production/Preview
- [ ] Copy button copies current URL
- [ ] "Open in Appz" opens appz.dev in new tab
- [ ] On appz.dev, meta tag present: `document.querySelector('meta[name="appz-extension"]')`
- [ ] API resolve returns deployment when authenticated

---

## Execution Handoff

Plan complete and saved to `docs/plans/toolbar/2025-02-22-appz-toolbar-extension-implementation.md`.

**Two execution options:**

1. **Subagent-Driven (this session)** – Dispatch a fresh subagent per task, review between tasks, fast iteration.

2. **Parallel Session (separate)** – Open a new session with executing-plans for batch execution with checkpoints.

**Which approach?**
