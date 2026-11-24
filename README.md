<div align="center">

<pre>
 █████╗ ██████╗ ██████╗ ███████╗
██╔══██╗██╔══██╗██╔══██╗╚══███╔╝
███████║██████╔╝██████╔╝  ███╔╝
██╔══██║██╔═══╝ ██╔═══╝  ███╔╝
██║  ██║██║     ██║     ███████╗
╚═╝  ╚═╝╚═╝     ╚═╝     ╚══════╝
</pre>

**Zero-config toolchains, mise, and deploy.**

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)

</div>

# appz

`appz` detects your stack (Node, Rust, Python, ...), wires up
[`mise`](https://mise.jdx.dev) with cached, incremental tasks instead of a
hand-rolled build cache, and drives install/build/dev/test/lint/format/deploy
through one command — no config to write by hand.

- **Zero-config detection** — reads your project, not a config file, to know
  how to build it (96+ frameworks, mise tool versions, output dirs).
- **mise-native, not another task runner** — generates `[tools]` + `[tasks]`
  with `sources`/`outputs`, so mise's own cache — not a second one — decides
  what to skip.
- **Unified deploy** — `appz deploy` drives each platform's own CLI (Vercel,
  Netlify, Cloudflare, ...) instead of competing with them.
- **Built for agents too** — `appz mcp` exposes detect/build/deploy as MCP
  tools, so an AI agent can drive the same pipeline a human does.

```
appz init      # detect toolchains, scaffold appz.jsonc + mise.toml
appz install   # mise + package manager install
appz build     # mise run build (skips when inputs are unchanged)
appz dev       # start the dev server
appz test      # run tests
appz lint      # run the linter
appz format    # run the formatter
appz backup    # snapshot local source and unpushed commits with Rustic
appz deploy    # deploy via the target platform's own CLI
appz build && appz deploy --prebuilt   # upload the existing build output, no rebuild
appz remove    # delete deployments by ID or URL (alias: rm; refused while an alias points at one)
appz promote <id-or-url>  # promote a ready appzdev deployment to production (--yes to skip the prompt)
appz ci        # write a GitHub Actions deploy workflow
appz doctor    # diagnose project stack, config, and suggestions
appz mcp       # stdio MCP server exposing detect/doctor/run to AI agents
appz activity  # organization audit events (--limit, --next, --action; owner or activity:read key)
appz usage     # resource usage vs. limits (80%+ flagged) and plan features
appz skills add  # install the appz agent skill into detected AI agents (--global for user-wide)
appz webhooks  # ls/add/inspect/rm/deliveries/test appz.dev webhooks; verify checks a signature locally
```

Hosted Appz projects have versioned URL redirects. `appz redirects add|rm|import`
stage a new version, and nothing changes live until `appz redirects promote <version>`.
Use `ls`, `versions` and `restore <version>` to inspect and roll back. `import`
takes a `.json` array or a `.csv` with `source,destination,status` columns and
reports every bad row by line number before uploading.

Cloud environment variables (appz.dev) are per environment: `production`,
`preview`, `development` or a custom one. `appz env pull [--environment X]`
writes decrypted values to a file (default environment: `development`, like
`vercel env pull`; needs the `env:decrypt` scope), `appz env push` (default: all three environments) uploads a
`.env` file, `appz env cloud ls` lists keys and metadata without values, and
`appz env cloud rm KEY` removes a key from every environment. Manage custom
environments with `appz env environments ls|add|rm`. `appz env run
[--environment X] -- <cmd>` runs a command with the values injected and
never writes or prints them; it exits with the child's exit code. The local
encrypted vault (`appz env set|ls|rm|...`) is unchanged.

To recover a hosted Appz project, run `appz projects --target appzdev restore <slug-or-id>`
within 30 days of removal. Restore before deploying again to the deleted slug,
because a new deploy can replace the deleted project.

Manage a hosted project's lifecycle with `appz projects pause|resume|keep <slug-or-id>`
(pause asks first unless `--yes`) and change settings with
`appz projects update <slug-or-id> --toolbar-mode previews --markdown true`
(team owners and admins; prints a before/after diff). Move a project to another
organization with `appz projects transfer <slug-or-id> --to <org-id>`, which
shows what moves and which keys are revoked; the destination owner finishes it
with `appz transfers accept <id>` (`transfers ls|inspect|cancel` also exist).
All of these accept `--json`.

Inspect a custom domain with `appz domains inspect <domain>` (project, verification, required DNS records,
serving status; `--json` for the raw record). Move it to another organization with
`appz domains move <domain> --to <org> --project <id> [--yes]` (`--project` is the destination project id) (owners of both organizations only).
Nameserver and registrar details are not shown because appz.dev is not a registrar.

To undo a bad production deploy on appz.dev, run `appz rollback` to repoint production at the
previous ready production deployment (shows `current -> target`, confirms unless `--yes`), or
`appz rollback <version-id|deployment-id|url>` for a specific one. A lone existing directory
argument (`appz rollback ./site`) is the project directory.

Set `APPZ_BACKUP_REPOSITORY` to a dedicated repository outside the project.
Store its password with `appz vault set APPZ_BACKUP_PASSWORD` and run
`appz vault unlock` before backing up, or set `APPZ_BACKUP_PASSWORD` for
unattended runs. Run `appz backup` to create a snapshot and `appz backup list`
to see snapshots. Run `appz backup prune` to apply retention to the entire repository.
Rustic is lock-free. Run retention when no other process is backing up to or
maintaining that repository.

Restore into an empty directory with `appz backup restore latest <destination>`.
The `worktree/` directory contains selected files at their project-relative paths.
If the backup includes local commits, locate `git/<digest>/local-commits.bundle`
under the destination. For a project with pushed history, clone the original
repository and enter that checkout. For a project that was never pushed, create
an empty checkout with `git init recovered-project` and enter it with
`cd recovered-project`. From the checkout, verify the bundle and fetch its HEAD
into a new branch.

```sh
git bundle verify <absolute-path-to-bundle>
git fetch <absolute-path-to-bundle> HEAD:refs/heads/recovered
git switch recovered
```

The bundle requires any pushed base commits listed by `git bundle verify`.
If those commits are lost, the incremental bundle alone cannot rebuild the history.
Working-tree environment files are excluded, but a bundle includes whatever
was committed. Copy restored working files into the checkout. For each changed
stage-zero blob, `git/index/<blob-id>/<project-relative-path>` holds the staged
bytes, even when they match the working copy. For a staged Git submodule entry
(gitlink), `git/gitlinks/<commit-id>/<project-relative-path>` contains the
submodule commit ID on a single line.

## Install

**Linux/macOS** (downloads a prebuilt binary and verifies it against the
release's `SHA256SUMS` when `sha256sum`/`shasum` is available, otherwise warns
and skips; builds from source instead if run from inside a clone):
```bash
curl -fsSL https://raw.githubusercontent.com/getappz/cli/main/install.sh | sh
```

**Windows, build from source** (no unsigned prebuilt binary to trip an AV
heuristic):
```powershell
git clone https://github.com/getappz/cli
cd cli
.\install.ps1
```

**Windows, Scoop** (prebuilt binary — not Authenticode-signed, so
Defender/SmartScreen false-positives are possible; verify with cosign/SLSA
instead, see "Verifying release binaries" below; report an issue if hit):
```powershell
scoop bucket add appz https://github.com/getappz/cli
scoop install appz
```

**npm:**
```bash
npm install -g @getappz/cli
```

**Docker** (pin a version — `latest` isn't tied to a specific verifiable
release):
```bash
docker run --rm ghcr.io/getappz/cli:v0.1.0 appz --version
```

**Any platform with Rust, no clone needed** (builds from source, no
checksum applicable):
```bash
cargo install --git https://github.com/getappz/cli appz
```

**Uninstall** (curl install only — for the others: `scoop uninstall appz`,
`npm uninstall -g @getappz/cli`, `cargo uninstall appz`, or remove the
Docker image):
```bash
curl -fsSL https://raw.githubusercontent.com/getappz/cli/main/install.sh | sh -s -- --uninstall
```

## Verifying release binaries

The curl installer verifies SHA-256 checksums against the release's
`SHA256SUMS` when `sha256sum`/`shasum` is available — enough to catch a
corrupted download, not a substituted one. Scoop verifies its manifest's own
`hash` field (sourced from the same `SHA256SUMS`); npm, Docker, and
`cargo install` don't consume `SHA256SUMS` at all. For higher-assurance
environments, verify the cryptographic signature and build provenance
before running the binary.

### cosign (signing identity)

Every release archive (`.tar.gz`/`.zip`) is signed in CI using
[cosign](https://docs.sigstore.dev/cosign/overview/) keyless signing via the
GitHub OIDC token — the certificate is issued by Fulcio and bound to this
repo's `release.yml` workflow, so verifiers pin to the workflow identity
instead of a long-lived key.

```bash
VERSION=v0.x.x
FILE=appz-x86_64-unknown-linux-gnu.tar.gz
curl -fL -o "$FILE" "https://github.com/getappz/cli/releases/download/${VERSION}/${FILE}"
curl -fL -o "${FILE}.cosign.bundle" "https://github.com/getappz/cli/releases/download/${VERSION}/${FILE}.cosign.bundle"

cosign verify-blob \
  --bundle "${FILE}.cosign.bundle" \
  --certificate-identity-regexp "^https://github\.com/getappz/cli/\.github/workflows/release\.yml@refs/tags/${VERSION}\$" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  "$FILE"
```

### SLSA L3 provenance (build process)

cosign proves *this repo's CI signed it*; SLSA provenance proves *how it was
built* — which commit, workflow, and inputs. Releases include a
`<tag>.intoto.jsonl` attestation generated by the
[SLSA GitHub generator](https://github.com/slsa-framework/slsa-github-generator).
Verify with [`slsa-verifier`](https://github.com/slsa-framework/slsa-verifier):

```bash
curl -fL -o "${VERSION}.intoto.jsonl" "https://github.com/getappz/cli/releases/download/${VERSION}/${VERSION}.intoto.jsonl"

slsa-verifier verify-artifact \
  --provenance-path "${VERSION}.intoto.jsonl" \
  --source-uri github.com/getappz/cli \
  --source-tag "${VERSION}" \
  "$FILE"
```

## Deploying to appz.dev

`appz deploy --target appzdev` automatically fingerprints static assets for
preview and production. For example, `css/site.css` gets an immutable URL
such as `css/site-a3b128db29.css`. The CLI rewrites local asset references
in uploaded HTML and stylesheets. Module entry points, module preloads, and
integrity-checked resources keep their original URLs. Original asset paths
also remain available for runtime imports and existing links. The filename uses
the first 10 hex characters of the BLAKE3 digest of the uploaded bytes.
Deploy manifests use the full digest prefixed with `b3:`.

`appz open [ID_OR_URL] [DIR]` opens the production site (or the given
deployment) in your browser; `--dashboard` opens the project's (or that
deployment's) page on the my.appz.dev dashboard instead (set
`APPZ_DASHBOARD_URL` to change the base). With
`--json` or no terminal it prints `{url, kind}` / the URL instead. Other
providers open the last production URL recorded by `appz deploy --prod`.

Manage a hosted project's aliases with `appz alias --target appzdev`:
`appz alias set <domain>` (or the shorthand `appz alias <domain>`) points a
domain at the project, `appz alias ls` lists serving hosts and the deployments
they point at, and `appz alias rm <host>` removes one after a confirmation
(`--yes` skips it). The production host cannot be removed. Use `--json` for
machine-readable output.

## Deploying to GitHub Pages, S3, and Render

These targets publish to production; use `--prod`. A preview request fails
without publishing.

- `appz deploy --target github-pages --prod` uses the `gh-pages` CLI
  (npm package `gh-pages`) and your Git credentials. Enable branch publishing
  from `gh-pages` in the repository's Pages settings. URLs respect the build
  output's `CNAME`, user/org sites, and project sites. The push does not confirm
  completion of GitHub's asynchronous Pages build.
  The bundled framework presets supply the build command and output directory.
  For example, appz runs `zola build` and publishes `public/` for a Zola site.
  To publish an existing build from any static site generator, set
  `deploy.targets.github-pages.outputDir` in `appz.jsonc` to the directory
  containing only the files you want to publish. Relative paths resolve from
  the project root. Run your usual build first, then run the deploy command.
- `appz deploy --target aws-s3 --prod` uses AWS CLI credentials (including
  profiles/roles), honors the configured region, and syncs the build output
  with `--delete`, skipping symlinks. It refuses output with no readable files,
  including empty directory trees. The reported URL is a
  bucket REST endpoint, not confirmation of a public website.
- `appz deploy --target render --prod` uses the
  [Render CLI](https://render.com/docs/cli) to deploy an existing service's
  connected source and waits for completion. Push your source to its connected
  repository first. Authenticate with `render login` or `RENDER_API_KEY`;
  the service URL comes from Render metadata.

Example `appz.jsonc` configuration:

```json
{
  "deploy": {
    "targets": {
      "github-pages": { "outputDir": "public" },
      "aws-s3": { "bucket": "my-site", "region": "eu-west-1", "prefix": "site" },
      "render": { "service": "srv-your-service-id" }
    }
  }
}
```

If AWS credentials are stored in the appz vault, add `AWS_SECRET_ACCESS_KEY`
and, for temporary credentials, `AWS_SESSION_TOKEN` to `deploy.env`.

The Rust `DeployProvider` API lists deployments for Vercel, Netlify, Cloudflare
Workers, and Render, and rolls back Vercel, Netlify, and single-version
Cloudflare deployments. Other combinations return `Unsupported`; no capability
flags promise an operation that isn't implemented. Render's CLI supports
redeploying commits but has no deployment-ID artifact rollback command.

## MCP server

`appz mcp` speaks the [Model Context Protocol](https://modelcontextprotocol.io)
over stdio so AI agents can drive appz directly. Tools:

- `detect` — JSON toolchain report for a directory
- `doctor` — JSON diagnosis + suggestions
- `run` — execute a lifecycle command (`install`/`build`/`test`/`lint`/`format`)
  and return its output

Point an MCP client at `appz mcp` (e.g. `{"command": "appz", "args": ["mcp"]}`).

## GitHub Action

```yaml
- uses: getappz/cli@v0.1.0
  with:
    command: deploy   # init/install/build/dev/test/lint/format/deploy/doctor
    # version: latest
    # working-directory: .
    # args: ""
```

`appz deploy` shells out to each target platform's own CLI (Vercel, Netlify,
Cloudflare, ...), so set that platform's native env vars/secrets (e.g.
`VERCEL_TOKEN`, `CLOUDFLARE_API_TOKEN`) directly on the step or job — the
action doesn't take platform-specific inputs.

### Scaffold the workflow

`appz ci init` writes `.github/workflows/appz-deploy.yml` for the target in
`appz.jsonc` (or the one detected), pinned to the checked-out branch. With
`--yes` (or a confirmation at a terminal) it also stores the provider token as
a repository secret through `gh`. Values come from the environment or the appz
vault and reach `gh` on stdin — never the command line, logs, or the workflow.

```bash
appz ci init --yes          # write workflow + set VERCEL_TOKEN (etc.)
appz ci init --no-secrets   # workflow only; set secrets yourself
```

## Configuration

`appz` reads `appz.jsonc` (JSON with comments) in the project root. The
`vite` section is read by the `@getappz/vite` plugin (the `appz` CLI itself
does not read it):

```jsonc
{
  "vite": {
    // Deploy target for `appz deploy --target <slug>` after the vite build.
    // One of: vercel, netlify, cloudflare, github-pages, fly, deno-deploy,
    // aws-s3, aws-sst, railway, surge, firebase, azure-static. Omit to let the plugin detect it from
    // the installed vite provider plugin.
    "target": "vercel",
    // Run `appz deploy --target <target> --yes` after `vite build`.
    // appz installs the platform CLI via mise when it's missing.
    "deploy_after_build": false,
    // Vite provider plugin names the plugin expects to find in
    // vite.config.*. When none match, appz warns instead of failing.
    "provider_plugins": ["vite-plugin-vercel"]
  }
}
```

Field | Meaning
---|---
`target` | Deploy slug. If set, `appz deploy` is pinned to it; otherwise the target is detected from the installed vite provider plugin.
`deploy_after_build` | When true, `appz deploy` runs after `vite build` (default: false).
`provider_plugins` | Provider plugin names to look for (default: `[]`, no plugin check).

The plugin detects the target from provider plugin names (`vercel` → `vite-plugin-vercel`, `netlify` → `@netlify/vite-plugin`, `cloudflare` → `vite-plugin-cloudflare-pages`) and warns when the expected plugin is missing. No Universal Deploy dependency is used — `appz deploy` drives each platform's own CLI directly.
### Deploy config (`appz.jsonc`)

```jsonc
{
  "deploy": {
    "default": "vercel",
    "env": ["SENTRY_AUTH_TOKEN"],            // vault secret NAMES to pass through
    "environments": {                        // literal, non-secret values
      "production": { "API_URL": "https://api.example.com" },
      "preview":    { "API_URL": "https://staging.example.com" }
    },
    "hooks": {                               // argv arrays, never shell strings
      "before_deploy": ["npm", "run", "lint"],
      "after_deploy":  ["node", "scripts/notify.js"]
    },
    "targets": { "vercel": {}, "netlify": {} }
  }
}
```

`env` (a list of vault secret names) and `environments` (literal values per
`production` / `preview`) coexist. Precedence, lowest to highest:
`environments` < vault secrets < the real process environment < `-e`.
Hooks are spawned directly (no shell) in the project directory with the
resolved deployment environment and a 15-minute timeout. Hook output goes to
stderr, or is captured with `--json` to keep the result parseable;
`before_deploy` runs after the build and aborts the deploy on failure, and
`after_deploy` failing is reported even though the deploy already went out.
`appz deploy --all` uploads each local build before preparing the next target,
so a build or hook can safely reuse the same output directory. Without prompts,
Render's remote-source deploy can overlap a local deploy, and dry-run probes
can run in parallel. After hooks run in target order once all uploads finish.
A hook that changes source triggers a fresh deploy next time. A failed after
hook clears that target's success cache so the next deploy retries it.
With `--all --json`, the command prints one JSON array with a
`{target, output?, error?}` entry per target; it exits non-zero if any target or
hook failed.

## devcontainer Feature

```json
"features": {
  "ghcr.io/getappz/cli/appz:1": {
    "version": "latest",
    "autoInstall": false
  }
}
```

`autoInstall` runs `appz install` once, after the workspace is created, via the
Feature's own `postCreateCommand` — if your own `devcontainer.json` also sets
`postCreateCommand`, both run (the Feature's first), it doesn't override
yours.

## Layout

- `crates/appz` — the CLI binary
- `crates/appz-core` — toolchain detection, `mise.toml` generation, and the
  doctor/report logic `appz` runs on top of
- `crates/appz-deploy` — the `DeployProvider` trait + per-platform CLIs
- `crates/command` — shell-free process execution (arg-vector, timeouts)

## Build

```
cargo build --release -p appz
```
