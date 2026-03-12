# appz exec Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `appz exec` CLI command and MCP `exec` tool for unified command execution with sandbox-by-default, duct-based cross-platform support, and capture/stream output modes.

**Architecture:** Shared exec runner in `crates/app/src/exec.rs` used by both CLI and MCP. Sandbox path uses existing `sandbox.exec()`; no-sandbox path uses duct. MCP already has `shell` tool; we add `exec` with more options (no_sandbox, separate stdout/stderr) and optionally refactor `shell` to delegate to exec runner.

**Tech Stack:** duct, duct_sh (optional), shell_words, existing sandbox crate.

---

## Task 1: Add workspace dependencies (duct, duct_sh)

**Files:**
- Modify: `crates/app/Cargo.toml`
- Modify: `Cargo.toml` (workspace) if duct is workspace-level

**Step 1: Add duct and duct_sh to workspace**

In `Cargo.toml` under `[workspace.dependencies]`, add:
```toml
duct = "0.1"
duct_sh = "0.1"
```
(Crates.io: duct 0.1.x, duct_sh 0.1.x)

**Step 2: Add to app crate**

In `crates/app/Cargo.toml` [dependencies]:
```toml
duct = { workspace = true }
duct_sh = { workspace = true }
```

**Step 3: Verify build**

Run: `cargo check -p app`
Expected: Compiles (may need to add duct to workspace first if not present).

**Step 4: Commit**

```bash
git add Cargo.toml crates/app/Cargo.toml
git commit -m "deps: add duct, duct_sh for appz exec"
```

---

## Task 2: Create exec module and ExecConfig/ExecResult types

**Files:**
- Create: `crates/app/src/exec.rs`

**Step 1: Create exec.rs with config and result structs**

```rust
//! Command execution for appz exec (CLI + MCP).
//! Sandbox-by-default; duct for cross-platform when --no-sandbox.

use miette::Result;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ExecConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: std::path::PathBuf,
    pub sandbox: bool,
    pub stream: bool,
    pub json: bool,
    pub shell: bool,
    pub timeout: Option<Duration>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}
```

**Step 2: Add pub mod exec to lib.rs**

In `crates/app/src/lib.rs`, add: `pub mod exec;`

**Step 3: Run cargo check**

Run: `cargo check -p app`
Expected: Compiles.

**Step 4: Commit**

```bash
git add crates/app/src/exec.rs crates/app/src/lib.rs
git commit -m "feat(exec): add ExecConfig and ExecResult types"
```

---

## Task 3: Implement run_exec (sandbox path)

**Files:**
- Modify: `crates/app/src/exec.rs`

**Step 1: Implement sandbox path**

Add to exec.rs:

```rust
use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};

pub async fn run_exec(config: ExecConfig) -> Result<ExecResult> {
    if config.sandbox {
        run_exec_sandbox(&config).await
    } else {
        run_exec_direct(&config).await
    }
}

async fn run_exec_sandbox(config: &ExecConfig) -> Result<ExecResult> {
    let sandbox_config = SandboxConfig::new(config.cwd.clone())
        .with_settings(SandboxSettings::default().quiet());
    let sandbox = create_sandbox(sandbox_config)
        .await
        .map_err(|e| miette::miette!("Sandbox creation failed: {}. Try --no-sandbox.", e))?;

    let cmd_str = build_command_string(&config.command, &config.args);
    let timeout = config.timeout;

    let result = if let Some(dur) = timeout {
        tokio::time::timeout(dur, sandbox.exec(&cmd_str)).await
    } else {
        Ok(sandbox.exec(&cmd_str).await)
    };

    match result {
        Ok(Ok(out)) => Ok(ExecResult {
            exit_code: out.exit_code().unwrap_or(1),
            stdout: out.stdout(),
            stderr: out.stderr(),
            timed_out: false,
        }),
        Ok(Err(e)) => Err(miette::miette!("Command failed: {}", e)),
        Err(_) => Ok(ExecResult {
            exit_code: 124,
            stdout: String::new(),
            stderr: format!("Command timed out after {:?}", timeout.unwrap_or_default()),
            timed_out: true,
        }),
    }
}

fn build_command_string(cmd: &str, args: &[String]) -> String {
    if args.is_empty() {
        cmd.to_string()
    } else {
        format!("{} {}", cmd, args.join(" "))
    }
}
```

**Step 2: Add stub for run_exec_direct**

```rust
async fn run_exec_direct(config: &ExecConfig) -> Result<ExecResult> {
    let cmd_str = build_command_string(&config.command, &config.args);
    let parts = shell_words::split(&cmd_str)
        .unwrap_or_else(|_| vec![config.command.clone()]);
    let (program, program_args) = parts
        .split_first()
        .ok_or_else(|| miette::miette!("Empty command"))?;

    let expr = duct::cmd(program, program_args.iter().map(|s| s.as_str()));
    let expr = expr.dir(&config.cwd);
    let expr = expr.stdout_capture().stderr_capture();

    let result = if let Some(dur) = config.timeout {
        tokio::time::timeout(
            dur,
            tokio::task::spawn_blocking(move || expr.unchecked().run()),
        )
        .await
    } else {
        Ok(tokio::task::spawn_blocking(move || expr.unchecked().run()).await)
    };

    match result {
        Ok(Ok(output)) => Ok(ExecResult {
            exit_code: output.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            timed_out: false,
        }),
        Ok(Err(e)) => Err(miette::miette!("Command failed: {}", e)),
        Err(_) => Ok(ExecResult {
            exit_code: 124,
            stdout: String::new(),
            stderr: "Command timed out".to_string(),
            timed_out: true,
        }),
    }
}
```

**Step 3: Fix duct API if needed**

duct::cmd returns Expression. Check duct docs for run(), stdout_capture, stderr_capture. Adjust as needed.

**Step 4: cargo check**

Run: `cargo check -p app`
Expected: Compiles (fix any API mismatches).

**Step 5: Commit**

```bash
git add crates/app/src/exec.rs
git commit -m "feat(exec): implement run_exec with sandbox and direct paths"
```

---

## Task 4: Add ExecArgs and Exec command to CLI

**Files:**
- Create: `crates/app/src/args.rs` (add ExecArgs struct)
- Modify: `crates/app/src/app.rs` (add Exec variant to Commands)
- Create: `crates/app/src/commands/exec.rs`
- Modify: `crates/app/src/commands/mod.rs`

**Step 1: Add ExecArgs to args.rs**

Find an appropriate spot (e.g. after DevArgs) and add:

```rust
#[derive(Args, Debug, Clone)]
pub struct ExecArgs {
    /// Command to run (e.g. npm, cargo)
    pub command: String,
    /// Arguments for the command (e.g. run build, test). Use trailing args.
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
    /// Skip sandbox (run in host environment)
    #[arg(long)]
    pub no_sandbox: bool,
    /// Stream output to terminal
    #[arg(long)]
    pub stream: bool,
    /// Output as JSON (stdout, stderr, exit_code)
    #[arg(long)]
    pub json: bool,
    /// Working directory
    #[arg(long)]
    pub cwd: Option<std::path::PathBuf>,
    /// Use shell for pipes/redirects
    #[arg(long)]
    pub shell: bool,
    /// Timeout in seconds
    #[arg(long)]
    pub timeout: Option<u64>,
}
```

**Step 2: Add Exec(ExecArgs) to Commands enum in app.rs**

Add after Git variant:
```rust
/// Execute a command with sandbox (agents, MCP, users)
Exec(ExecArgs),
```

**Step 3: Create commands/exec.rs**

```rust
//! appz exec — Execute commands with sandbox-by-default.

use crate::exec::{run_exec, ExecConfig, ExecResult};
use crate::session::AppzSession;
use miette::Result;
use starbase::AppResult;

pub async fn exec(session: AppzSession, args: crate::args::ExecArgs) -> AppResult {
    let cwd = args
        .cwd
        .unwrap_or_else(|| session.working_dir.clone())
        .canonicalize()
        .map_err(|e| miette::miette!("Invalid cwd: {}", e))?;

    let config = ExecConfig {
        command: args.command,
        args: args.args,
        cwd,
        sandbox: !args.no_sandbox,
        stream: args.stream,
        json: args.json,
        shell: args.shell,
        timeout: args.timeout.map(std::time::Duration::from_secs),
    };

    let result = run_exec(config).await?;

    if args.json {
        println!("{}", serde_json::to_string(&result).unwrap());
    } else {
        if !result.stdout.is_empty() {
            print!("{}", result.stdout);
        }
        if !result.stderr.is_empty() {
            eprint!("{}", result.stderr);
        }
    }

    Ok(if result.exit_code == 0 {
        None
    } else {
        Some(result.exit_code)
    })
}
```

**Step 4: Register in commands/mod.rs**

Add `pub mod exec;` and `pub use exec::exec;`. Wire Exec variant in the match (find where Commands are dispatched).

**Step 5: Wire Exec in session/dispatch**

Find where Commands::Exec would be handled (likely in session.rs or a run loop). Add:
```rust
Commands::Exec(args) => exec(session, args).await,
```

**Step 6: cargo build**

Run: `cargo build`
Expected: Compiles.

**Step 7: Commit**

```bash
git add crates/app/src/args.rs crates/app/src/app.rs crates/app/src/commands/exec.rs crates/app/src/commands/mod.rs
git commit -m "feat(exec): add appz exec CLI command"
```

---

## Task 5: Add MCP exec tool

**Files:**
- Modify: `crates/mcp-server/src/tools.rs`
- Modify: `crates/mcp-server/Cargo.toml` (add app dependency if needed)

**Step 1: Add app dependency to mcp-server**

mcp-server currently uses sandbox directly. To use the exec runner, we need app. Check if mcp-server can depend on app (avoid circular). Alternatively, move the exec runner to a separate crate (e.g. `crates/exec`) that both app and mcp-server depend on.

**Simpler approach:** mcp-server already has shell. Add `exec` tool that calls `appz exec --json` via run_appz (subprocess). This avoids new deps and matches other MCP tools. Downside: subprocess overhead.

**Alternative:** Create `crates/exec` crate with the runner, have both app and mcp-server depend on it.

For minimal scope: have MCP exec tool invoke `appz exec --json` via run_appz. Add ExecParams, exec handler.

**Step 2: Add ExecParams and exec tool**

In tools.rs:
```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExecParams {
    pub command: String,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub shell: bool,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub no_sandbox: bool,
}

#[tool]
async fn exec(...) {
    let mut args = vec!["exec".to_string(), "--json".to_string()];
    args.push(params.command);
    if params.shell { args.push("--shell".to_string()); }
    if let Some(t) = params.timeout_ms { args.extend(["--timeout".to_string(), (t/1000).to_string()]); }
    if params.no_sandbox { args.push("--no-sandbox".to_string()); }
    let workdir = params.workdir.as_deref();
    let out = run_appz(&args, workdir).await?;
    ...
}
```

**Step 3: Register exec tool in tool_router**

Add exec to the tool list. Update get_info instructions.

**Step 4: cargo build**

Run: `cargo build -p mcp-server`
Expected: Compiles.

**Step 5: Commit**

```bash
git add crates/mcp-server/src/tools.rs
git commit -m "feat(mcp): add exec tool"
```

---

## Task 6: Update Superpowers rule

**Files:**
- Modify: `.cursor/rules/17-appz-git-superpowers.mdc` or create new rule

**Step 1: Add exec guidance**

Add section or new rule: When agents need to run arbitrary commands (npm, cargo, pytest, etc.), use `appz exec <cmd> [args]` instead of raw shell for sandbox-isolated, cross-platform execution.

**Step 2: Commit**

```bash
git add .cursor/rules/
git commit -m "docs: add appz exec to Superpowers rule"
```

---

## Task 7: Manual test

**Step 1: Test CLI**

```bash
appz exec npm --version
appz exec --json cargo --version
appz exec --no-sandbox node -e "console.log('ok')"
```

**Step 2: Test MCP exec tool** (if MCP client available)

Invoke exec tool with command "npm --version", verify JSON response.

---

## Execution Handoff

Plan complete and saved to `docs/plans/exec/2025-02-21-appz-exec-implementation.md`. Two execution options:

1. **Subagent-driven (this session)** — Dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Parallel session (separate)** — Open a new session with executing-plans for batch execution with checkpoints.

Which approach do you prefer?
