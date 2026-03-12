# appz exec — Design Document

**Date:** 2025-02-21  
**Status:** Approved  
**Purpose:** Unified command execution for agents, MCP, and users with sandbox-by-default security.

---

## 1. Overview

`appz exec` runs arbitrary commands with sandbox isolation, cross-platform support, and consistent output for agents, MCP, and interactive use.

### Goals

- Single entry point for agents, MCP, and humans
- Sandbox by default; `--no-sandbox` opt-out for trusted use
- Cross-platform (Windows, macOS, Linux) via duct crate
- Capture mode (default) for agents/MCP; `--stream` for interactive
- Shared runner used by CLI and MCP (no subprocess overhead)

---

## 2. CLI Surface

```text
appz exec <command> [args...] [options]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `command` | Program to run (e.g. `npm`, `cargo`, `pytest`) |
| `args...` | Arguments (e.g. `run build`, `test`) |

### Options

| Option | Description |
|--------|-------------|
| `--no-sandbox` | Skip sandbox (trusted use) |
| `--stream` | Live output to terminal (default: capture) |
| `--json` | JSON output for agents/CI (stdout, stderr, exit_code) |
| `--cwd <path>` | Override working directory |
| `--shell` | Run via shell (duct_sh) for pipes, redirects, `$VAR` |
| `--timeout <seconds>` | Command timeout (default: none for CLI; 60s for MCP) |

### Examples

```bash
appz exec npm run build
appz exec cargo test --no-fail-fast
appz exec --stream pytest -v
appz exec --json --no-sandbox node -v
```

---

## 3. Security & Sandbox

### Sandbox by default

- Default: commands run inside appz sandbox (`create_sandbox(config)`), project root as cwd
- Same model as build/check/deploy: ScopedFs, mise tools, project directory only
- Opt-out: `--no-sandbox` runs in host environment (trusted use only)

### Execution (duct)

- Default: `duct::cmd!(program, arg1, arg2)` — no shell, execve-style
- `--shell`: use duct_sh for shell strings (pipes, redirects, `$VAR`)
- Parsing: `shell_words::split()` for `appz exec "npm run build"` → `["npm", "run", "build"]`

### Cross-platform

- duct supports Windows, macOS, Linux
- Shell fallback on Windows: `cmd /c` or `powershell -c` only when `--shell`

### Timeout

- Optional `--timeout N` (seconds)
- MCP tool: default 60s; CLI: no default unless specified

---

## 4. MCP & Output

### MCP tool: `exec`

- **Params:** `command` (string), `workdir` (optional), `shell` (optional), `timeout` (optional), `no_sandbox` (optional)
- **Flow:** Calls shared runner (library), returns `{ exit_code, stdout, stderr }`
- **Auth:** Not required (project-local)

### Output formats

- **Capture (default):** Buffered stdout, stderr, exit_code. CLI prints both; `--json` yields structured JSON.
- **Stream (`--stream`):** Passthrough to terminal. CLI only; MCP stays capture.

### Error handling

- Timeout → exit_code -1 or signal, message in stderr
- Sandbox failure → error with hint to use `--no-sandbox`
- Command not found → standard semantics

---

## 5. Architecture

### Shared runner

- Location: `crates/app/src/exec.rs` or `crates/exec/` crate
- Config: `ExecConfig { command, args, cwd, sandbox, stream, json, timeout }`
- Result: `ExecResult { exit_code, stdout, stderr }`
- Both CLI command and MCP tool call into this runner

### Dependencies

- `duct` — cross-platform command execution
- `duct_sh` — shell string execution (optional, when `--shell`)
- `shell_words` — already in workspace for parsing

---

## 6. Integration

- **Superpowers rule:** Update `.cursor/rules/17-appz-git-superpowers.mdc` or add new rule: agents should use `appz exec` instead of raw shell for command execution
- **verify/check:** Could optionally use `appz exec` under the hood for build/test (future refactor)
