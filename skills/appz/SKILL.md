---
name: appz
description: Detect, install, build, test, and deploy any project with the `appz` CLI (or its MCP server). Use when the user asks to "deploy my app", "push this live", "create a preview deployment", "build/test/lint this project" in a repo with no obvious toolchain, "set up deploy credentials", or "free up build output". Deploys to Vercel, Netlify, Cloudflare, Fly, Firebase, Deno Deploy, Azure Static, AWS S3/SST, Railway, Surge, GitHub Pages.
metadata:
  author: getappz
  version: "1.0.0"
---

# appz

`appz` is a zero-config front-end over `mise` and each hosting platform's own CLI. It detects the stack (96+ frameworks), runs cached install/build tasks, and deploys by driving the platform's CLI. It is not a cloud: it never talks to a platform API itself.

**Always deploy as preview** (the default) unless the user explicitly asks for production (`--prod`).

## Pick your interface

- **MCP available** (`appz mcp` is configured): use the tools `detect`, `doctor`, `run`, `deploy`. They return JSON.
- **Otherwise** run the CLI with `--json` (global flag, put it before the subcommand): `appz --json deploy`.

## Step 1: Understand the project

```bash
appz doctor      # toolchains, package manager, monorepo, config files, suggestions
appz init        # first time only: scaffolds appz.jsonc + mise.toml
```

MCP: `doctor` / `detect` with optional `dir`. Do not guess the build command; read `detect`'s resolved commands.

## Step 2: Lifecycle commands

```bash
appz install     # mise tools + dependencies
appz build       # mise run build — skips when inputs are unchanged
appz test | lint | format | dev
appz clean       # DRY RUN by default: lists build/cache output
appz clean --force        # actually deletes; add --deps to also remove node_modules/.venv
```

- Override a detected command with `-c "<cmd>"` or `buildCommand`/`installCommand`/`devCommand` in `appz.jsonc`.
- `appz clean` deletes only with `--force`. Show the dry-run listing to the user first; `--deps` is expensive to undo.
- MCP `run` takes `command: install|build|test|lint|format`; `structured: true` returns parsed diagnostics for a plain `cargo build` only.

## Step 3: Deploy

```bash
appz deploy --dry-run                 # readiness report: nothing changes
appz --json deploy                    # preview deploy
appz --json deploy --prod             # production (only if the user asked)
```

Flags: `--target <slug>`, `--project <name>`, `-e KEY=VALUE` (runtime env), `-b KEY=VALUE` (build env), `--force`, `-y`.

**Target resolution:** `--target` → `deploy.default` in `appz.jsonc` → detected from project files (e.g. `fly.toml`, `firebase.json`, `sst.config.ts`). Zero or several matches fail with an instruction. If the target is ambiguous, ask the user; do not pick one.

### Always dry-run first

`appz --json deploy --dry-run` reports, without side effects:

| Field | Meaning | What to do |
|-------|---------|------------|
| `cli_missing: true` | Platform CLI not installed | Tell the user; a real run installs it via mise (needs consent: `-y`, or MCP `allow_setup`) |
| `link_needed: true` | No platform project linked | Needs consent to create/link; pass `--project <name>` to choose the name |
| `auth: "not_authenticated"` | No token | See Credentials; you cannot log in interactively |
| `setup_files: [...]` | Config appz will write (`wrangler.jsonc`, `netlify.toml`, ...) | Mention it to the user; commit it if they want |

### Read the result

```json
{ "provider": "vercel", "url": "https://...", "is_preview": true, "status": "ready", "unchanged": false }
```

- `url` is **`null` when appz could not confirm one**. Report that honestly; never invent `https://<name>.vercel.app`.
- `status`: `ready` | `building` | `error` | `unknown` (`unknown` = CLI exited 0 but no URL confirmed).
- `unchanged: true` means nothing changed since the last deploy of this target: appz skipped and returned the previous `url`. Use `--force` only if the user wants a redeploy anyway.
- A platform with no preview support fails on a default deploy and says to pass `--prod`. Do not silently retry with `--prod`; ask.

### MCP `deploy` arguments

`target`, `dir`, `prod`, `dry_run`, `project`, `force`, and **`allow_setup`**. Without `allow_setup` it fails naming the step needed (CLI missing / not linked). Retry with `allow_setup: true` only after the user agrees. The tool never logs in.

## Credentials (`appz env`)

Agents cannot do browser login. Supply a token instead; the vault is encrypted and values are never printed.

```bash
appz env set VERCEL_TOKEN        # omit the value to be prompted (keeps it out of shell history)
appz env set NETLIFY_AUTH_TOKEN --global     # user-wide vault (~/.appz) instead of this project's
appz env ls                      # names only, never values
appz env unlock | lock | rm <NAME>
```

- Token variables: Vercel `VERCEL_TOKEN`, Netlify `NETLIFY_AUTH_TOKEN`, Cloudflare `CLOUDFLARE_API_TOKEN`, Fly `FLY_API_TOKEN`, Firebase `FIREBASE_TOKEN`, Deno Deploy `DENO_DEPLOY_TOKEN`, Railway `RAILWAY_TOKEN`, Surge `SURGE_TOKEN`, Azure `SWA_CLI_DEPLOYMENT_TOKEN`, AWS `AWS_ACCESS_KEY_ID`.
- A real environment variable beats the vault; `-e` beats both. `note: vault is locked` means run `appz env unlock` (the user must enter the passphrase).
- **Never** ask the user to paste a secret into chat, put one in `appz.jsonc`, or commit one. Tell them to run `appz env set <NAME>` themselves (`! appz env set VERCEL_TOKEN` in Claude Code).

## Config: `appz.jsonc`

```jsonc
{
  "deploy": {
    "default": "vercel",                 // used when --target is omitted
    "env": ["DATABASE_URL"],             // extra vault secrets to pass to the platform CLI
    "targets": { "fly": { "app": "my-app" } }   // per-platform keys
  }
}
```

Platforms with `required` keys (Fly `app`, Firebase `project`, Deno Deploy `project`, Railway `service`, Surge `domain`, AWS S3 `bucket`) stop and name the missing `deploy.targets.<slug>` key. Set it in `appz.jsonc`; do not guess a value.

## Rules

- Prefer `--dry-run` and the preview target; escalate to `--prod` only on explicit request.
- Do not install tools, create platform projects, or `--force` clean/deploy without the user's agreement.
- Surface errors verbatim; they name the exact next step (missing CLI, unlinked project, missing config key, locked vault).
- Do not hand-edit `.appz/` or the generated `[tasks]` in `mise.toml`; rerun `appz init` / `appz build`.
- For other agents' setup: `appz agents list|doctor`, and `appz skills` to discover/install project-relevant skills.
