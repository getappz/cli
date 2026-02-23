# Appz Alias & Custom Domains with Cloudflare SSL for SaaS

> Design: Vercel-parity alias commands and custom domains using Cloudflare SSL for SaaS.

## Overview

The `appz alias` command applies custom domains to deployments. With Cloudflare SSL for SaaS, users point their own domains (e.g. `www.customer.com`, `customer.com`) to appz.dev and get automatic SSL without managing certificates.

## Vercel Reference

| Command | Usage |
|---------|-------|
| `vercel alias set` | `alias set [deployment-url] [custom-domain]` |
| `vercel alias rm` | `alias rm [custom-domain]` |
| `vercel alias ls` | `alias ls [--limit N]` |

Options:
- `--yes` — skip confirmation when removing alias
- `--limit` — max aliases to return (default 20, max 100)

## DNS Setup for Users

Users configure their DNS as follows:

| User record | Type | Target |
|-------------|------|--------|
| `www.<userdomain>.com` | CNAME | `appz.dev` |
| `<userdomain>.com` (apex) | A | `<appz-provided-ip>` |

- **www / subdomains:** CNAME to `appz.dev` — traffic goes through Cloudflare edge.
- **Apex:** A record to an Appz-provided IP — required because CNAME at apex is often not supported; Cloudflare supports CNAME flattening on their own zones.

## Cloudflare SSL for SaaS

1. **Appz zone (appz.dev):** Enable Cloudflare for SaaS on the appz.dev zone.
2. **Custom hostnames:** For each customer domain (`www.customer.com`, `customer.com`), create a custom hostname via [Cloudflare Custom Hostnames API](https://developers.cloudflare.com/api/operations/custom-hostnames-for-a-zone-create-custom-hostname).
3. **Fallback origin:** Configure fallback origin (e.g. `proxy.appz.dev` or origin Worker) so traffic for custom hostnames is routed correctly.
4. **Validation:** HTTP or CNAME validation. User must point DNS as above before cert issuance.

### API Flow

```
POST https://api.cloudflare.com/client/v4/zones/{ZONE_ID}/custom_hostnames
{
  "hostname": "www.customer.com",
  "ssl": {
    "method": "http",
    "type": "dv"
  },
  "custom_metadata": {
    "deployment_id": "...",
    "project_id": "..."
  }
}
```

## Architecture

```
User DNS                    Cloudflare Edge              Appz Backend
─────────                   ───────────────              ────────────
www.customer.com (CNAME)  →  appz.dev zone             →  v0-static Worker
customer.com (A)         →  (custom hostname)          →  deployment routing
```

- v0-static (or edge Worker) parses `Host` header to resolve deployment.
- Custom hostname metadata maps hostname → deployment/project.

## CLI Implementation (Vercel Parity)

### alias set

```
appz alias set [deployment-url] [custom-domain]
appz alias [deployment-url] [custom-domain]   # set is default
```

- Omit protocol (`https://`).
- Resolve deployment from URL or ID.
- Call `POST /v0/deployments/{id}/aliases` with `{ "alias": "custom-domain" }`.
- Backend creates alias record + Cloudflare custom hostname.

### alias rm

```
appz alias rm [custom-domain] [--yes]
```

Already implemented. Backend deletes alias and removes Cloudflare custom hostname.

### alias ls

```
appz alias ls [--limit 100]
```

Add `--limit` support (API already supports it; default 20, max 100).

## API Contract

### Create Alias (Vercel-style)

```
POST /v0/deployments/{deploymentId}/aliases
Content-Type: application/json
Body: { "alias": "www.customer.com" }
Response: Alias
```

Alternative (appz-style, single aliases resource):

```
POST /v0/aliases
Body: { "deploymentId": "...", "alias": "www.customer.com" }
Response: Alias
```

Recommendation: Use deployment-scoped `POST /v0/deployments/{id}/aliases` for Vercel parity.

### Domains

- `GET /v0/domains` — list domains
- `POST /v0/projects/{id}/domains` — add domain to project
- `DELETE /v0/domains/{domain}` — remove domain

Adding a domain to a project registers it; aliasing assigns it to a deployment.

## Backend Tasks (appz-dev)

1. **Aliases router**
   - `POST /v0/deployments/:id/aliases` — create alias, call Cloudflare Custom Hostnames API.
   - `GET /v0/aliases` — list (existing).
   - `GET /v0/aliases/:id` — get (existing).
   - `DELETE /v0/aliases/:id` — delete, remove custom hostname.

2. **Cloudflare integration**
   - Cloudflare API token with `SSL and Certificates Write`, `Custom Hostnames Write`.
   - Service: create/delete custom hostnames, poll validation status.
   - Fallback origin for custom hostnames.

3. **Domains router**
   - `GET /v0/domains` — list.
   - `POST /v0/projects/:id/domains` — add domain to project.
   - `DELETE /v0/domains/:domain` — remove.

4. **v0-static routing**
   - Resolve deployment from `Host` for custom hostnames.
   - Use custom metadata or alias table for hostname → deployment mapping.

## Database

- `alias` table: `id`, `alias` (hostname), `deployment_id`, `project_id`, `team_id`, `cloudflare_hostname_id`, `created_at`, `updated_at`.
- `domain` table (if not exists): `id`, `name`, `project_id`, `team_id`, `created_at`, `updated_at`.

## Security

- Validate user has access to deployment and project before creating alias.
- Validate user owns or has verified the domain (e.g. DNS verification before creating custom hostname).
- Rate limit alias creation to avoid abuse.

## Files to Modify

| Location | Change |
|----------|--------|
| `appz-cli/crates/api/src/endpoints/aliases.rs` | Add `create(deployment_id, alias)` ✓ |
| `appz-cli/crates/app/src/commands/aliases/mod.rs` | Add Set subcommand, --limit to Ls ✓ |
| `appz-cli/crates/app/src/commands/aliases/set.rs` | New: alias set handler ✓ |
| `appz-cli/crates/app/src/commands/aliases/ls.rs` | Add --limit, pass to API ✓ |
| `appz-dev/apps/workers/v0/` | New aliases router, Cloudflare service ✓ |
| `appz-dev/apps/workers/v0-static/` | Custom hostname routing ✓ |

## Backend Deployment (appz-dev)

### Required secrets / vars

Set these for the v0 worker to enable Cloudflare SSL for SaaS:

```bash
# Zone ID for appz.dev (from Cloudflare dashboard)
wrangler secret put CLOUDFLARE_ZONE_ID

# API token with "SSL and Certificates Write" + "Custom Hostnames Write"
wrangler secret put CLOUDFLARE_API_TOKEN
```

### Fallback origin

In Cloudflare dashboard for appz.dev zone:

1. **SSL/TLS** → **Custom Hostnames** → **Fallback Origin**
2. Set fallback origin to the v0-static worker URL (e.g. `https://v0-static.appz.dev` or the Worker route that serves static assets).

Traffic for custom hostnames (e.g. www.customer.com) is proxied to this fallback. v0-static resolves the Host header via the `alias` table and serves the correct deployment.
