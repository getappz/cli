# Security Policy

## Reporting a Vulnerability

Please report security issues privately, not as a public GitHub issue:

- **GitHub**: [Create a private security advisory](https://github.com/getappz/cli/security/advisories/new)
- **Response time**: best-effort acknowledgment within a few days (small, single-maintainer project)

## What appz Does (and Doesn't Do)

appz is a **local CLI**. It reads your project's files to detect its
toolchain, writes generated config (`mise.toml`, `appz.jsonc`, optionally
`CLAUDE.md`), and shells out to `mise` and your package manager to install,
build, and run lifecycle commands. `appz deploy` shells out to the target
platform's own CLI (`vercel`, `netlify`, ...) instead of talking to any
platform's API directly.

**Does:**
- Read project files to detect its toolchain and lifecycle commands
- Write `mise.toml` (merges into an existing file, never overwrites tools/tasks
  you've already defined), `appz.jsonc`, and `.appz/` state
- Write `CLAUDE.md` only if it doesn't already exist
- Shell out to `mise`, your package manager, and (for `appz deploy`) the
  target platform's CLI — always via arg-vector execution, never through a
  shell, so detected commands can't be interpreted for shell metacharacters
- Pass auth tokens (`VERCEL_TOKEN`, `NETLIFY_AUTH_TOKEN`) to those child
  processes via environment, never as CLI arguments (keeps them out of
  process listings and shell history)

**Does NOT:**
- Make any network requests itself — no telemetry, no update check. Any
  network activity comes from the subprocesses it invokes (`mise`, your
  package manager, a platform's deploy CLI), not from appz's own code
- Read or transmit your project's source contents anywhere; detection reads
  config/manifest files (`package.json`, `Cargo.toml`, etc.) locally only
- Require elevated privileges

## The MCP Server (`appz mcp`)

`appz mcp` runs a stdio MCP server exposing `detect`/`doctor`/`run`/`deploy`
as tools, so an AI agent can drive the same pipeline a human does. This is a
real trust boundary worth being explicit about: the MCP server executes
build, install, and deploy commands **with the same privileges as whoever
runs it**, and does not sandbox those commands. Only connect it to agents you
trust with the same level of access you'd give a human running `appz` in
your terminal.

## Automated Checks

Every push and PR runs `cargo test`, `cargo clippy` (deny warnings), `cargo
fmt --check`, and `cargo audit`/`cargo deny` (dependency CVE and license
scanning).
