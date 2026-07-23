# appz

Zero-config project toolchain detection and lifecycle runner. `appz` detects
your stack (Node, Rust, Python, ...), wires up `mise`, and runs
install/build/dev/test/lint/format through one command, caching against
your project's inputs so repeat runs skip work that isn't stale.

```
appz init      # detect toolchains, scaffold appz.jsonc
appz install   # mise + package manager install
appz build     # run the detected (or overridden) build command
appz dev       # start the dev server
appz test      # run tests
appz lint      # run the linter
appz format    # run the formatter
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

## Layout

- `crates/appz` — the CLI binary
- `crates/appz-core` — toolchain detection, `mise.toml` generation, and
  the doctor/report logic `appz` runs on top of

## Build

```
cargo build --release
```
