---
name: cli-architecture
description: Understand the overall appz CLI architecture -- crate dependency graph, feature flags, command routing, session lifecycle, and directory layout. Use when working on command dispatch, adding new commands, modifying the session lifecycle, or understanding how the CLI binary is composed.
---

# CLI Architecture

The appz CLI is a Rust workspace with ~25 crates. The binary is built from crates/cli/ which depends on crates/app/ for all command logic.

## Crate Dependency Graph

```text
cli (binary)
 |-- app (core logic)
      |-- plugin-manager     -- on-demand WASM plugin download + verification
      |-- sandbox            -- scoped FS + tool management (mise)
      |-- frameworks         -- framework detection
      |-- init               -- project scaffolding
      |-- common             -- shared types/constants
      |-- ui                 -- terminal output (indicatif, owo-colors)
      |-- api                -- REST API client
      |-- command            -- process execution
      |-- task               -- task registry + dependency graph
      |-- env_var            -- thread-safe env var bag
      |
      |-- dev-server         -- (optional) local dev server
      |-- deployer           -- (optional) deployment providers
      |-- checker            -- (optional) code quality checker
      |-- site-builder       -- (optional) AI site builder
      |-- appz-studio        -- (optional) AI website gen
      |-- mcp-server         -- (optional) MCP protocol server
      |-- ssg-migrator       -- (optional, native only) SSG migration lib
```

## Feature Flag Strategy

The core CLI is kept small as an orchestration tool. Heavy modules are optional:

| Feature | Default? | What it enables |
|---------|----------|----------------|
| dev-server | yes | appz dev-server, appz preview |
| deploy | yes | appz deploy, deploy-init, deploy-list |
| mcp | yes | appz mcp-server |
| gen | yes | appz gen |
| self_update | yes | appz self-update |
| check | no | Host function for check plugin |
| site | no | Host function for site plugin |

Feature flags are defined in three levels:
1. Cargo.toml (root) -- forwarded to app crate
2. crates/cli/Cargo.toml -- forwarded to app crate
3. crates/app/Cargo.toml -- actual optional dependencies

The ssg-migrator plugin is fully self-contained (no host function needed). The check and site plugins still use host functions with stubs when their features are disabled.

## Command Routing

```text
User input: appz <command> [args...]
         |
         |-- Matches Commands enum variant?
         |    YES -> dispatch to app::commands::<handler>
         |
         |-- NO (external_subcommand)
              |-- Commands::External(args)
                   |-- external::run()
                        |-- Look up command in plugin manifest
                        |-- Download + verify WASM plugin
                        |-- Create sandbox
                        |-- Load plugin (handshake + info)
                        |-- Execute plugin command
```

The Commands enum in crates/app/src/app.rs defines all built-in commands. Unknown commands fall through to the External(Vec<String>) variant, which triggers the plugin system.

### Key files for command dispatch

| File | Role |
|------|------|
| crates/app/src/app.rs | Cli struct, Commands enum (clap derive) |
| crates/cli/src/main.rs | match session.cli.command dispatch |
| crates/app/src/session.rs | AppzSession lifecycle (startup, version check, execute) |
| crates/app/src/commands/external.rs | Plugin command handler |

### Adding a new built-in command

1. Add a variant to Commands in crates/app/src/app.rs
2. Create crates/app/src/commands/<name>.rs
3. Add `pub mod <name>;` in crates/app/src/commands/mod.rs
4. Add a match arm in crates/cli/src/main.rs
5. If the command needs a feature flag, gate with `#[cfg(feature = "...")]`

## Session Lifecycle

```text
main() -> Cli::try_parse() -> App::run(AppzSession::new(cli), |session| async { ... })
                                |
                                |-- session.startup()
                                |    |-- detect working dir, load project config
                                |
                                |-- session.execute()
                                |    |-- check_for_new_version() (24h cache, async, non-blocking)
                                |    |-- dispatch command
                                |
                                |-- session.shutdown()
```

Version checks run on every command execution but only hit the network once per 24 hours (cached at ~/.appz/cache/latest-version). CI and non-interactive terminals skip the check entirely.

## Directory Layout

```text
~/.appz/
  |-- auth.json           -- API authentication tokens
  |-- config.toml         -- user configuration
  |-- skills/             -- installed agent skills
  |-- cache/
  |   |-- latest-version  -- CLI version check cache (24h TTL)
  |   |-- plugin-updates.json -- plugin update hint state (7d TTL)
  |-- plugins/
      |-- manifest.json   -- CDN plugin manifest cache (1h TTL)
      |-- entitlements.json -- subscription tier cache (1h TTL)
      |-- <plugin-name>/
          |-- <version>/
              |-- plugin.wasm     -- WASM binary
              |-- plugin.wasm.sig -- Ed25519 signature
```

## Conventions

- **Filesystem:** `starbase_utils::fs` (re-export via `app::utils::fs` where convenient). Use for `read_file`, `write_file`, `read_dir`, `create_dir_all`, `remove_file`, `remove_dir_all`, `metadata`, `exists`. Use `std::fs` only for operations starbase_utils does not expose (e.g. executable bits via `PermissionsExt`).
- **JSON:** `starbase_utils::json` for file I/O (`read_file`, `write_file`) instead of manual `serde_json::from_str` + `fs::read_file`.
- **Config/cache paths:** `starbase_utils::dirs` (`home_dir`, `cache_dir`, `config_dir`, `data_dir`).
- **Error handling:** `miette::Result`, `thiserror` for library errors, `starbase::AppResult` for command handlers.
- **Tracing:** `tracing` crate, `#[tracing::instrument]` on command handlers.
- **Session:** `AppSession` with startup → analyze → execute → shutdown (aligned with Moon/Starbase phases).
- **Async:** tokio runtime, wrap blocking ops with `tokio::task::spawn_blocking`.
- **CLI parsing:** clap derive macros.
- **Session framework:** `starbase::App` + `starbase::AppSession` trait.
