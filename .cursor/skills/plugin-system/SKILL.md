---
name: plugin-system
description: How the appz WASM plugin system works end-to-end -- from CDN manifest to sandbox-isolated execution. Use when adding host functions, creating new plugins, debugging plugin loading/handshake, or working with the PDK.
---

# Plugin System

Crate locations:
- crates/plugin-manager/ -- download, verify, cache plugins
- crates/app/src/wasm/ -- WASM runtime, host functions, plugin runner
- crates/appz_pdk/ -- Plugin Development Kit (types + security)
- crates/plugins/ -- individual plugin crates (ssg-migrator, check, site)

## Architecture

```text
PluginManager (crates/plugin-manager/src/lib.rs)
  |-- PluginManifest     -- CDN manifest fetch + local cache (1h TTL)
  |-- EntitlementChecker -- subscription tier validation via API
  |-- PluginDownloader   -- WASM + signature download from CDN
  |-- PluginCache        -- local version cache (~/.appz/plugins/)
  |-- PluginSecurity     -- Ed25519 sig, header validation, handshake
  |-- PluginUpdateChecker -- periodic update hint (7d TTL)

PluginRunner (crates/app/src/wasm/plugin.rs)
  |-- load_verified_plugin() -- reads WASM, registers host functions, handshake
  |-- execute_command()      -- calls appz_plugin_execute in the WASM module
  |-- register_plugin_host_functions() -- ScopedFs, git, sandbox, AST, etc.
```

## Plugin Execution Flow

```text
1. User runs: appz migrate ...
2. Clap routes to Commands::External(["migrate", ...])
3. external::run() creates PluginManager
4. PluginManager::ensure_plugin("migrate"):
   a. Load manifest from CDN (or 1h cache)
   b. find_by_command("migrate") -> "ssg-migrator" plugin
   c. Check entitlement (tier: free/pro)
   d. Check local cache for matching version
   e. Download WASM + signature if not cached
   f. Verify Ed25519 signature
   g. Validate WASM header (magic bytes + plugin ID)
   h. Check CLI version compatibility
5. Create sandbox (ScopedFs + SandboxProvider)
6. PluginRunner::load_verified_plugin():
   a. Read WASM bytes
   b. Create Extism plugin with all host functions
   c. HMAC handshake challenge-response
   d. Call appz_plugin_info() -> PluginInfo
7. PluginRunner::execute_command():
   a. Build PluginExecuteInput (command, args, working_dir)
   b. Call appz_plugin_execute -> PluginExecuteOutput
   c. Print result or error
```

## Module Map

| Module | File | Key types / functions |
|--------|------|----------------------|
| manifest | plugin-manager/src/manifest.rs | PluginManifest, PluginEntry, CachedManifest |
| cache | plugin-manager/src/cache.rs | PluginCache -- get, list_versions, cleanup |
| downloader | plugin-manager/src/downloader.rs | PluginDownloader -- download WASM + sig |
| entitlements | plugin-manager/src/entitlements.rs | EntitlementChecker -- API-based tier check |
| security | plugin-manager/src/security.rs | PluginSecurity -- Ed25519, header, handshake |
| update_check | plugin-manager/src/update_check.rs | PluginUpdateChecker -- periodic hint |
| plugin | app/src/wasm/plugin.rs | PluginManager (legacy tasks), PluginRunner, PluginHostData |
| host_functions/* | app/src/wasm/host_functions/ | All registered host functions |
| types | app/src/wasm/types.rs | All host function input/output structs |
| stubs | app/src/wasm/host_functions/stubs.rs | Stubs for disabled features (check, site) |

## Host Functions

Host functions are registered in register_plugin_host_functions() in plugin.rs.

Categories:
- Plugin filesystem (appz_pfs_*): read, write, walk, exists, mkdir, copy, remove, list_dir, read_json, write_json
- Git (appz_pgit_*): changed_files, staged_files, is_repo
- Sandbox exec (appz_psandbox_*): exec, exec_with_tool, ensure_tool
- AST (appz_past_*): transform, parse_jsx
- Check (appz_pcheck_run): gated on check feature, stub when disabled
- Site (appz_psite_run): gated on site feature, stub when disabled

### Adding a new host function

1. Define input/output structs in crates/app/src/wasm/types.rs
2. Implement the function in crates/app/src/wasm/host_functions/<module>.rs using host_fn! macro
3. Add `pub mod <module>;` in host_functions/mod.rs
4. Import in plugin.rs and register in register_plugin_host_functions()
5. If feature-gated, add a stub in stubs.rs with `#[cfg(not(feature = "..."))]`
6. Declare the host function in the plugin's lib.rs inside `#[host_fn] extern "ExtismHost" { ... }`
7. Add the corresponding types to crates/appz_pdk/src/types.rs

## PDK Types (crates/appz_pdk/)

The PDK provides the plugin-side types and security utilities:

- PluginHandshakeChallenge / PluginHandshakeResponse -- HMAC handshake
- PluginInfo, PluginCommandDef, PluginArgDef -- plugin metadata
- PluginExecuteInput / PluginExecuteOutput -- command execution
- PluginFs* types -- filesystem host function I/O
- PluginGit* types -- git host function I/O
- security::compute_handshake() -- HMAC computation for handshake

## Plugin Crate Structure

Each plugin crate in crates/plugins/ follows this pattern:

```text
crates/plugins/<name>/
  |-- Cargo.toml          (crate-type = ["cdylib"])
  |-- src/
  |   |-- lib.rs          (exports: handshake, info, execute)
  |   |-- ...             (plugin-specific modules)
```

Required exports from every plugin:
- appz_plugin_handshake(Json<PluginHandshakeChallenge>) -> Json<PluginHandshakeResponse>
- appz_plugin_info() -> Json<PluginInfo>
- appz_plugin_execute(Json<PluginExecuteInput>) -> Json<PluginExecuteOutput>

## Plugin Build and Publish

Plugins are built with `cargo run -p plugin-build -- package` using config from scripts/plugins.toml. Build steps:
1. Cross-compile to wasm32-wasip1
2. Inject custom header section (magic bytes, plugin ID, min CLI version)
3. Sign with Ed25519 key
4. Upload WASM + signature to CDN

## Dev Plugin Override

For local development, set APPZ_DEV_PLUGIN_<COMMAND>=path/to/plugin.wasm to bypass CDN download and signature verification. Example:

```bash
APPZ_DEV_PLUGIN_MIGRATE=target/wasm32-wasip1/debug/ssg_migrator_plugin.wasm appz migrate ...
```

## Testing

```bash
# Type-check a plugin for WASM target
cargo check -p ssg-migrator-plugin --target wasm32-wasip1

# Build a plugin WASM binary
cargo build -p ssg-migrator-plugin --target wasm32-wasip1 --release

# Type-check the host-side plugin system
cargo check -p plugin-manager
cargo check -p app
```
