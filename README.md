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
  Netlify, ...) instead of competing with them.
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
appz deploy    # deploy via the target platform's own CLI
appz doctor    # diagnose project stack, config, and suggestions
appz mcp       # stdio MCP server exposing detect/doctor/run to AI agents
```

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
- uses: getappz/cli@v1
  with:
    command: deploy   # init/install/build/dev/test/lint/format/deploy/doctor
    # version: latest
    # working-directory: .
    # args: ""
```

`appz deploy` shells out to each target platform's own CLI (Vercel, Netlify,
...), so set that platform's native env vars/secrets (e.g. `VERCEL_TOKEN`)
directly on the step or job — the action doesn't take platform-specific
inputs.

## Layout

- `crates/appz` — the CLI binary
- `crates/appz-core` — toolchain detection, `mise.toml` generation, and the
  doctor/report logic `appz` runs on top of
- `crates/appz-deploy` — the `DeployProvider` trait + per-platform CLIs
- `crates/command` — shell-free process execution (arg-vector, timeouts)

## Build

```
cargo build --release
```
