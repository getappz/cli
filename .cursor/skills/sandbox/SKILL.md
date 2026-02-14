---
name: sandbox
description: Work with the appz sandbox crate — a scoped execution environment for static site development. Use when creating, modifying, or debugging sandbox code, adding new providers, extending ScopedFs, working with MiseManager, or building features that use the sandbox API.
---

# Sandbox Crate

Crate location: `crates/sandbox/`

Scoped execution environment that confines all filesystem I/O and command execution to a project root directory. Uses [mise](https://mise.jdx.dev/) for tool version management (Node, Hugo, Bun, etc.).

## Architecture

```text
create_sandbox(config) -> Box<dyn SandboxProvider>
  │
  ├── SandboxProvider (trait, dyn-compatible)
  │     init · teardown · fs · exec · exec_interactive
  │     ensure_tool · exec_with_tool · exec_all
  │
  ├── SandboxProviderExt (extension trait, auto-impl, NOT dyn-compatible)
  │     write_files_progress · read_files_progress
  │     remove_files_progress · copy_progress
  │
  └── LocalProvider (only impl today)
        ├── ScopedFs       — path-safe file I/O + rayon batch ops
        ├── MiseManager    — mise CLI wrapper (install, exec, env)
        └── ui::{status, progress}  — spinners, progress bars (quiet-aware)
```

## Module map

| Module | File | Key types / functions |
|--------|------|----------------------|
| `config` | `src/config.rs` | `SandboxConfig`, `SandboxSettings`, `MiseToolSpec`, `ProviderKind` |
| `error` | `src/error.rs` | `SandboxError` (miette diagnostics), `SandboxResult<T>` |
| `provider` | `src/provider.rs` | `SandboxProvider` trait, `SandboxProviderExt` trait, `CommandOutput` |
| `local` | `src/local/mod.rs` | `LocalProvider` |
| `scoped_fs` | `src/scoped_fs.rs` | `ScopedFs`, `DirEntry` |
| `mise` | `src/mise.rs` | `MiseManager` |
| `json_ops` | `src/json_ops.rs` | `read_json`, `write_json`, `merge_json` |
| `toml_ops` | `src/toml_ops.rs` | `read_toml`, `write_toml` |

## Key design decisions

### Two traits: SandboxProvider + SandboxProviderExt

`SandboxProvider` is **dyn-compatible** (`Box<dyn SandboxProvider>`). Batch methods with generics live in `SandboxProviderExt` (auto-implemented via blanket impl). Always import both:

```rust
use sandbox::{SandboxProvider, SandboxProviderExt};
```

### Path security (ScopedFs)

`ScopedFs` is the security boundary. Every path is resolved relative to the sandbox root and validated before any I/O:

- Absolute paths → rejected (`PathEscape`)
- `..` traversal escaping root → rejected
- Symlinks → resolved via `canonicalize`, checked against root
- Root is canonicalized at construction time

### Parallelism strategy

- **CPU-bound batch file ops** (`read_files`, `write_files`, `remove_files`, `copy`): `rayon::par_iter`
- **Concurrent async commands** (`exec_all`): `futures::future::join_all` over tokio tasks
- **Single tool installs** (`install_tools`): single `mise use -g` call (mise parallelises internally)
- Each `_with_progress` method accepts `Option<F: Fn() + Send + Sync>` callback for progress tracking

### Error handling

All errors are `SandboxError` (miette `Diagnostic` + thiserror `Error`). Every variant has a `#[diagnostic(help = "...")]` for actionable CLI output. Key `From` impls:

- `std::io::Error` → `Io`
- `serde_json::Error` → `JsonError`
- `toml::{de,ser}::Error` → `TomlError`
- `glob::{PatternError,GlobError}` → `GlobError`
- `starbase_utils::fs::FsError` → `Other`

### UI behaviour

`LocalProvider` uses the workspace `ui` crate for spinners and status messages. All UI is gated on `SandboxSettings::quiet` — set `true` for tests and CI.

## Provider lifecycle (LocalProvider::init)

1. **Create project directory** if missing
2. **Check / install mise** (auto-install via brew/apk/pacman/curl on Unix, winget/scoop on Windows)
3. **Install tools** — `mise use -g node@22 bun@latest ...`
4. **Sync project tools** — `mise install` if `mise.toml` / `.tool-versions` exists
5. **Load mise environment** — `mise env --json` → HashMap
6. **Load dotenv** — optional `.env` file merged into env

After init: `fs()` returns `&ScopedFs`, `exec(cmd)` runs through mise.

## Common tasks

### Adding a new ScopedFs method

1. Add the method on `ScopedFs` in `src/scoped_fs.rs`
2. Always use `self.resolve()` or `self.resolve_existing()` for path safety
3. Use `starbase_utils::fs` for I/O (codebase convention)
4. For batch variants, add both `method()` and `method_with_progress()` using `rayon::par_iter`
5. Add unit tests in the `#[cfg(test)] mod tests` block at the bottom
6. If the method should have a progress-bar wrapper, add it to `SandboxProviderExt` in `src/provider.rs`

### Adding a new provider

1. Create `src/<provider>/mod.rs`
2. Implement `SandboxProvider` for your struct (all methods)
3. Add a variant to `ProviderKind` in `src/config.rs`
4. Wire it up in `create_sandbox()` in `src/lib.rs`
5. `SandboxProviderExt` is auto-implemented via the blanket impl

### Adding a new error variant

1. Add the variant to `SandboxError` in `src/error.rs`
2. Include `#[diagnostic(code(sandbox::your_code), help("..."))]`
3. Include `#[error("...")]` with a human-readable message
4. Add a `From` impl if there's a natural source error type

### Extending MiseManager

- All mise methods are sync (blocking `command::Command::exec`)
- Check `Self::is_available()` at the start of each method
- Scope commands to `self.project_path` via `cmd.cwd()`
- Use `cmd.set_error_on_nonzero(false)` and handle exit codes manually

## Usage examples

### Create and use a sandbox

```rust
use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};

let config = SandboxConfig::new("/tmp/my-site")
    .with_settings(
        SandboxSettings::default()
            .with_tool("node", Some("22"))
            .with_tool("hugo", Some("0.139"))
            .with_env("NODE_ENV", "production")
            .with_dotenv(".env"),
    );

let sandbox = create_sandbox(config).await?;
sandbox.fs().write_string("index.html", "<h1>Hello</h1>")?;
let out = sandbox.exec("node --version").await?;
```

### Batch file operations

```rust
use sandbox::SandboxProviderExt;

// Write many files with progress bar
let items: Vec<(String, String)> = pages.iter()
    .map(|p| (p.path.clone(), p.html.clone()))
    .collect();
sandbox.write_files_progress(&items, "Writing pages")?;

// Read many files in parallel
let results = sandbox.read_files_progress(&paths, "Reading templates");
```

### JSON / TOML operations

```rust
use sandbox::json_ops::{read_json_value, write_json_value, merge_json};
use sandbox::toml_ops::{read_toml, write_toml};
use serde_json::json;

let fs = sandbox.fs();
write_json_value(fs, "package.json", &json!({"name": "my-site"}))?;
merge_json(fs, "package.json", &json!({"version": "1.0.0"}))?;
```

### Quiet mode (tests / CI)

```rust
let config = SandboxConfig::new("/tmp/test-project")
    .with_settings(SandboxSettings::default().quiet());
```

## Dependencies

| Crate | Role |
|-------|------|
| `command` | Workspace crate for process execution |
| `ui` | Workspace crate for terminal UI (indicatif, owo-colors, inquire) |
| `common` | Workspace shared types |
| `starbase_utils` | Filesystem + JSON utilities (codebase convention) |
| `miette` + `thiserror` | Error handling with rich diagnostics |
| `rayon` | Thread-pool parallelism for batch file ops |
| `futures` | `join_all` for concurrent async commands |
| `tokio` | Async runtime |
| `async-trait` | Async methods in traits |
| `serde` + `serde_json` + `toml` | Serialisation |
| `glob` | File pattern matching |
| `which` | PATH lookup (mise detection) |
| `shell-words` | Shell argument parsing |

## Testing

```bash
cargo test -p sandbox          # 20 unit tests + doc-tests
cargo check -p sandbox         # type check only
cargo doc -p sandbox --no-deps # build docs
```

Tests use `tempfile::tempdir()` for isolated filesystem fixtures. All tests run without mise installed (they test ScopedFs, JSON/TOML ops directly).
