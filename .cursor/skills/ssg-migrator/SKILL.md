---
name: ssg-migrator
description: Work with the ssg-migrator crate and its WASM plugin -- the self-contained React SPA migration tool. Use when adding transforms, modifying the Vfs trait, working on the analyzer/generator pipeline, or debugging WASM compilation.
---

# SSG Migrator

Crate locations:
- crates/ssg-migrator/ -- core migration library (compiles to both native and WASM)
- crates/plugins/ssg-migrator/ -- WASM plugin entry point

## Architecture

```text
ssg-migrator (library crate)
  |-- vfs.rs          -- Vfs trait (filesystem + git abstraction)
  |-- vfs_native.rs   -- NativeFs impl (std::fs + walkdir + git2) [native only]
  |-- types.rs        -- MigrationConfig, ProjectAnalysis, ComponentInfo, RouteInfo
  |-- analyzer.rs     -- analyze_project(&dyn Vfs, source_dir) -> ProjectAnalysis
  |-- transformer.rs  -- convert_to_astro(), transform_component_to_astro()
  |-- ast_transformer.rs -- transform_with_ast() [ast-grep native, regex fallback WASM]
  |-- common.rs       -- copy_public_assets(), filter_deps()
  |-- generator/      -- Astro project generation
  |   |-- mod.rs      -- generate_astro_project(&dyn Vfs, config, analysis)
  |   |-- config.rs   -- astro.config.mjs, package.json, tsconfig
  |   |-- components.rs -- component generation + copying
  |   |-- pages.rs    -- route -> Astro page generation
  |   |-- layout.rs   -- Layout.astro generation
  |   |-- files.rs    -- CSS/public asset copying
  |   |-- readme.rs   -- README generation
  |   |-- classifier.rs -- static safety analysis
  |-- nextjs/         -- Next.js project generation
  |   |-- mod.rs      -- generate_nextjs_project(&dyn Vfs, config, analysis, output_dir)
  |   |-- transform.rs -- client file transforms
  |   |-- convert.rs  -- convert_to_nextjs()
  |   |-- providers.rs -- React context providers
  |   |-- verify.rs   -- static export verification
  |   |-- pages.rs    -- App Router page creation
  |   |-- config.rs   -- package.json, next.config.ts, tsconfig
  |-- sync/           -- bidirectional sync between original and migrated projects
      |-- mod.rs      -- SyncManifest, write/read_manifest, convenience wrappers
      |-- forward.rs  -- sync_forward (original -> migrated)
      |-- backward.rs -- sync_backward (migrated -> original)

ssg-migrator-plugin (cdylib crate)
  |-- lib.rs          -- handshake, info, execute, handle_migrate, handle_convert
  |-- vfs_wasm.rs     -- WasmFs impl (PDK host functions)
```

## Self-Contained Plugin Design

The ssg-migrator plugin is fully self-contained:
- All Biome parsing (biome_js_parser, biome_js_syntax, biome_rowan) compiles directly into the WASM module
- The plugin calls ssg_migrator::analyze_project(), generate_astro_project(), etc. directly
- Filesystem and git I/O goes through the Vfs trait, implemented as WasmFs in the plugin
- WasmFs delegates to appz_pfs_* and appz_pgit_* host functions provided by the CLI
- No migration-specific host functions needed (the old appz_pmigrate_run / appz_pconvert_run were removed)

## Feature Flags

| Feature | Dependencies | Purpose |
|---------|-------------|---------|
| native (default) | walkdir, git2, ast-grep-core, ast-grep-language | Full native mode with NativeFs |
| (no features) | Biome parsers, regex, serde, miette, camino | WASM-compatible mode |

The native feature is enabled when:
- The app crate uses ssg-migrator (for any host-side code)
- Running cargo check -p ssg-migrator

The native feature is disabled when:
- Building the WASM plugin (ssg-migrator-plugin uses default-features = false)

## The Vfs Trait

Defined in crates/ssg-migrator/src/vfs.rs. All migration modules accept &dyn Vfs:

```rust
pub trait Vfs {
    fn read_to_string(&self, path: &str) -> Result<String>;
    fn write_string(&self, path: &str, content: &str) -> Result<()>;
    fn exists(&self, path: &str) -> bool;
    fn is_file(&self, path: &str) -> bool;
    fn is_dir(&self, path: &str) -> bool;
    fn create_dir_all(&self, path: &str) -> Result<()>;
    fn remove_file(&self, path: &str) -> Result<()>;
    fn remove_dir_all(&self, path: &str) -> Result<()>;
    fn copy_file(&self, src: &str, dst: &str) -> Result<()>;
    fn copy_dir(&self, src: &str, dst: &str) -> Result<()>;
    fn walk_dir(&self, path: &str) -> Result<Vec<FsEntry>>;
    fn list_dir(&self, path: &str) -> Result<Vec<FsEntry>>;
    fn git_changed_files(&self, repo_path: &str) -> Result<Vec<String>>;
    fn git_staged_files(&self, repo_path: &str) -> Result<Vec<String>>;
    fn git_is_repo(&self, path: &str) -> bool;
}
```

Implementations:
- NativeFs (vfs_native.rs): std::fs + walkdir + git2 -- native only
- WasmFs (plugins/ssg-migrator/src/vfs_wasm.rs): PDK host functions -- WASM only

## Common Tasks

### Adding a new Vfs method

1. Add the method signature to trait Vfs in crates/ssg-migrator/src/vfs.rs
2. Implement in NativeFs (vfs_native.rs) using std::fs / walkdir / git2
3. Implement in WasmFs (plugins/ssg-migrator/src/vfs_wasm.rs) using the host_call! macro
4. If a new host function is needed, add it to the CLI side (see plugin-system skill)
5. Declare the host function in plugins/ssg-migrator/src/lib.rs extern block
6. Verify: cargo check -p ssg-migrator --features native && cargo check -p ssg-migrator --no-default-features && cargo check -p ssg-migrator-plugin --target wasm32-wasip1

### Adding a new transform

1. Add the transform logic in crates/ssg-migrator/src/transformer.rs (Astro) or nextjs/transform.rs (Next.js)
2. Use &dyn Vfs for any file I/O
3. Use biome_js_parser for AST parsing (always available)
4. For ast-grep patterns, gate behind #[cfg(feature = "native")] with a regex fallback
5. Wire the transform into the generator pipeline
6. Update parse_transforms() to recognize the new transform name

### Debugging WASM compilation

```bash
# Check that the library compiles without native features
cargo check -p ssg-migrator --no-default-features

# Check the plugin compiles for WASM target
cargo check -p ssg-migrator-plugin --target wasm32-wasip1

# Common issue: using std::fs directly (use Vfs instead)
# Common issue: using walkdir/git2 outside #[cfg(feature = "native")]
# Common issue: ast-grep C bindings (always gate behind native feature)
```

## Dependencies

| Crate | Always? | Role |
|-------|---------|------|
| biome_js_parser | yes | JS/TS/JSX/TSX parsing |
| biome_js_syntax | yes | AST node types |
| biome_rowan | yes | Red-green tree infrastructure |
| serde + serde_json | yes | Serialization |
| miette | yes | Error handling |
| regex | yes | Pattern matching (WASM fallback for ast-grep) |
| camino | yes | UTF-8 path manipulation |
| walkdir | native | Directory traversal |
| git2 | native | Git operations |
| ast-grep-core | native | AST pattern matching |
| ast-grep-language | native | Language bindings for ast-grep |

## Testing

```bash
cargo check -p ssg-migrator --features native      # native mode
cargo check -p ssg-migrator --no-default-features   # WASM mode
cargo check -p ssg-migrator-plugin                  # plugin (native target)
cargo check -p ssg-migrator-plugin --target wasm32-wasip1  # plugin (WASM target)
```
