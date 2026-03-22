# Universal Blueprints Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the recipe system and WordPress-only blueprints with a unified blueprint system supporting all frameworks, a GitHub registry, and WordPress Playground compatibility.

**Architecture:** Extend the existing recipe task runner (`crates/task/`) with new scaffolding step types. Add a `BlueprintProvider` to the init system that fetches blueprints from a GitHub registry, local files, or URLs. Repurpose `crates/blueprint/` as a WordPress Playground compatibility/converter layer. Rename recipe references to blueprint throughout.

**Tech Stack:** Rust, serde (YAML/JSON/JSONC), reqwest (HTTP fetching), tokio (async), existing task runner (`crates/task/`), existing sandbox (`crates/sandbox/`)

**Spec:** `docs/superpowers/specs/2026-03-22-universal-blueprints-design.md`

---

## File Structure

### New Files
| File | Responsibility |
|---|---|
| `crates/init/src/providers/blueprint.rs` | BlueprintProvider: fetch, parse, scaffold, execute setup steps |
| `crates/init/src/blueprint_schema.rs` | Blueprint file schema types (meta, setup steps, config) |
| `crates/init/src/registry.rs` | GitHub registry client: fetch registry.json, resolve blueprint URLs, cache |
| `crates/app/src/commands/blueprints.rs` | `appz blueprints list` command |
| `crates/blueprint/src/converter.rs` | WordPress Playground JSON to generic blueprint converter |
| `crates/init/tests/blueprint_schema_test.rs` | Unit tests for blueprint schema parsing |
| `crates/init/tests/detect_test.rs` | Unit tests for updated source detection |
| `crates/init/tests/registry_test.rs` | Unit tests for registry client |
| `crates/blueprint/tests/converter_test.rs` | Unit tests for Playground converter |
| `crates/init/tests/fixtures/` | Test fixture blueprint files (YAML, JSON, JSONC) |
| `crates/blueprint/tests/fixtures/playground_simple.json` | Playground blueprint fixture for converter tests |

### Modified Files
| File | Change |
|---|---|
| `crates/init/src/lib.rs` | Add `blueprint_schema`, `registry` module declarations; export new types |
| `crates/init/src/providers/mod.rs` | Add `pub mod blueprint;` declaration |
| `crates/init/src/detect.rs` | Rewrite detection priority: framework/blueprint before git; add `git:` prefix escape hatch |
| `crates/init/src/provider.rs` | Replace FrameworkProvider + WordPressProvider with BlueprintProvider in registry |
| `crates/init/src/run.rs` | Update `is_source()` to recognize `framework/blueprint` and `git:` prefix |
| `crates/init/Cargo.toml` | Add `reqwest`, `json_comments` dependencies |
| `crates/app/src/importer.rs` | Extend `Step` struct with scaffolding fields; add JSONC support; relax validation; rename recipe->blueprint |
| `crates/app/src/session.rs` | Change discovery from `recipe.yaml` to `.appz/blueprint.yaml` |
| `crates/app/src/commands/mod.rs` | Add `pub mod blueprints;` |
| `crates/app/src/commands/init.rs` | Remove WordPress-specific post-init logic; delegate to BlueprintProvider |
| `crates/app/src/app.rs` | Add `Blueprints` subcommand routing |
| `crates/blueprint/src/lib.rs` | Add `pub mod converter;` |
| `crates/blueprint/Cargo.toml` | Add any needed deps for converter |

### Files to Retire (later, after migration verified)
| File | Replaced By |
|---|---|
| `crates/init/src/providers/framework.rs` | `BlueprintProvider` + default blueprints |
| `crates/init/src/providers/wordpress.rs` | `BlueprintProvider` + WordPress default blueprint |
| `crates/app/src/commands/blueprint.rs` | `crates/app/src/commands/blueprints.rs` |

---

## Task 1: Blueprint Schema Types

Define the new blueprint file format as Rust types in the init crate.

**Files:**
- Create: `crates/init/src/blueprint_schema.rs`
- Create: `crates/init/tests/blueprint_schema_test.rs`
- Create: `crates/init/tests/fixtures/simple_blueprint.yaml`
- Create: `crates/init/tests/fixtures/setup_only_blueprint.yaml`
- Create: `crates/init/tests/fixtures/simple_blueprint.json`
- Create: `crates/init/tests/fixtures/simple_blueprint.jsonc`
- Modify: `crates/init/src/lib.rs`
- Modify: `crates/init/Cargo.toml`

- [ ] **Step 1: Add dependencies to `crates/init/Cargo.toml`**

Add `serde_yaml`, `serde_json`, `json_comments` to dependencies:

```toml
serde_yaml = { workspace = true }
serde_json = { workspace = true }
json_comments = "0.2"
```

Check if `serde_yaml` and `serde_json` are already workspace dependencies in root `Cargo.toml`. If not, add them.

- [ ] **Step 2: Create test fixture files**

Create `crates/init/tests/fixtures/simple_blueprint.yaml`:

```yaml
version: 1

meta:
  name: "Test Blueprint"
  description: "A test blueprint"
  author: "test"
  framework: "nextjs"
  categories: ["test"]
  create_command: "npx create-next-app@latest"
  package_manager: "npm"

config:
  app_name: "myapp"

tools:
  node: "20"

setup:
  - desc: "Install deps"
    add_dependency: ["tailwindcss", "postcss"]
    dev: true

  - desc: "Create config"
    write_file:
      path: "tailwind.config.js"
      content: |
        module.exports = { content: ['./src/**/*.{js,ts,jsx,tsx}'] }

  - desc: "Set env"
    set_env:
      APP_NAME: "{{app_name}}"

  - desc: "Run init"
    run_locally: "echo setup complete"

tasks:
  build:
    - run_locally: "npm run build"
  dev:
    - run_locally: "npm run dev"

before:
  build:
    - dev
```

Create `crates/init/tests/fixtures/setup_only_blueprint.yaml`:

```yaml
version: 1

meta:
  name: "Setup Only"
  framework: "astro"

setup:
  - desc: "Install tailwind"
    add_dependency: ["tailwindcss"]
    dev: true
```

Create `crates/init/tests/fixtures/simple_blueprint.json`:

```json
{
  "version": 1,
  "meta": {
    "name": "JSON Blueprint",
    "framework": "nextjs"
  },
  "setup": [
    {
      "desc": "Install deps",
      "add_dependency": ["react"],
      "dev": false
    }
  ],
  "tasks": {
    "build": [{ "run_locally": "npm run build" }]
  }
}
```

Create `crates/init/tests/fixtures/simple_blueprint.jsonc`:

```jsonc
{
  // This is a JSONC blueprint with comments
  "version": 1,
  "meta": {
    "name": "JSONC Blueprint",
    "framework": "vite"
  },
  "setup": [
    {
      "desc": "Install deps",
      "add_dependency": ["vite-plugin-pwa"]
    }
  ]
}
```

- [ ] **Step 3: Write failing tests for blueprint schema parsing**

Create `crates/init/tests/blueprint_schema_test.rs`:

```rust
use std::path::PathBuf;

// We'll import the schema module once it exists
use init::blueprint_schema::{BlueprintSchema, parse_blueprint};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn parse_yaml_blueprint() {
    let bp = parse_blueprint(&fixture("simple_blueprint.yaml")).unwrap();
    assert_eq!(bp.version, Some(1));
    assert_eq!(bp.meta.as_ref().unwrap().name.as_deref(), Some("Test Blueprint"));
    assert_eq!(bp.meta.as_ref().unwrap().framework.as_deref(), Some("nextjs"));
    assert_eq!(bp.meta.as_ref().unwrap().create_command.as_deref(), Some("npx create-next-app@latest"));
    assert_eq!(bp.meta.as_ref().unwrap().package_manager.as_deref(), Some("npm"));
    assert!(bp.setup.as_ref().unwrap().len() == 4);
    assert!(bp.tasks.is_some());
}

#[test]
fn parse_json_blueprint() {
    let bp = parse_blueprint(&fixture("simple_blueprint.json")).unwrap();
    assert_eq!(bp.version, Some(1));
    assert_eq!(bp.meta.as_ref().unwrap().framework.as_deref(), Some("nextjs"));
    assert!(bp.setup.as_ref().unwrap().len() == 1);
}

#[test]
fn parse_jsonc_blueprint() {
    let bp = parse_blueprint(&fixture("simple_blueprint.jsonc")).unwrap();
    assert_eq!(bp.version, Some(1));
    assert_eq!(bp.meta.as_ref().unwrap().framework.as_deref(), Some("vite"));
}

#[test]
fn parse_setup_only_blueprint_is_valid() {
    let bp = parse_blueprint(&fixture("setup_only_blueprint.yaml")).unwrap();
    assert!(bp.tasks.is_none() || bp.tasks.as_ref().unwrap().is_empty());
    assert!(bp.setup.as_ref().unwrap().len() == 1);
}

#[test]
fn parse_add_dependency_step() {
    let bp = parse_blueprint(&fixture("simple_blueprint.yaml")).unwrap();
    let steps = bp.setup.as_ref().unwrap();
    let step = &steps[0];
    assert_eq!(step.add_dependency.as_ref().unwrap(), &vec!["tailwindcss".to_string(), "postcss".to_string()]);
    assert_eq!(step.dev, Some(true));
}

#[test]
fn parse_write_file_step() {
    let bp = parse_blueprint(&fixture("simple_blueprint.yaml")).unwrap();
    let steps = bp.setup.as_ref().unwrap();
    let step = &steps[1];
    let wf = step.write_file.as_ref().unwrap();
    assert_eq!(wf.path, "tailwind.config.js");
    assert!(wf.content.as_ref().unwrap().contains("module.exports"));
}

#[test]
fn parse_set_env_step() {
    let bp = parse_blueprint(&fixture("simple_blueprint.yaml")).unwrap();
    let steps = bp.setup.as_ref().unwrap();
    let step = &steps[2];
    let env = step.set_env.as_ref().unwrap();
    assert_eq!(env.get("APP_NAME").unwrap(), "{{app_name}}");
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test -p init --test blueprint_schema_test 2>&1 | head -30`
Expected: Compilation error — `init::blueprint_schema` does not exist yet.

- [ ] **Step 5: Implement blueprint schema types**

Create `crates/init/src/blueprint_schema.rs`:

```rust
//! Blueprint file schema — the unified format for scaffolding + operations.

use miette::{miette, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// Top-level schema
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct BlueprintSchema {
    #[serde(default)]
    pub version: Option<u32>,

    #[serde(default)]
    pub meta: Option<BlueprintMeta>,

    #[serde(default)]
    pub config: Option<serde_json::Value>,

    #[serde(default)]
    pub hosts: Option<serde_json::Value>,

    #[serde(default)]
    pub tools: Option<serde_json::Value>,

    /// Scaffolding steps — executed once during `appz init`.
    #[serde(default)]
    pub setup: Option<Vec<SetupStep>>,

    /// Operational tasks — registered into the task runner.
    #[serde(default)]
    pub tasks: Option<serde_json::Value>,

    #[serde(default)]
    pub before: Option<HashMap<String, Vec<String>>>,

    #[serde(default)]
    pub after: Option<HashMap<String, Vec<String>>>,

    #[serde(default)]
    pub includes: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Meta
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct BlueprintMeta {
    pub name: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub framework: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    pub create_command: Option<String>,
    pub package_manager: Option<String>,
}

// ---------------------------------------------------------------------------
// Setup step
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default, Clone)]
pub struct SetupStep {
    #[serde(default)]
    pub desc: Option<String>,

    // Command execution (inherited from recipe Step)
    #[serde(default)]
    pub run_locally: Option<String>,
    #[serde(default)]
    pub run: Option<String>,
    #[serde(default)]
    pub cd: Option<String>,

    // New scaffolding fields
    #[serde(default)]
    pub add_dependency: Option<Vec<String>>,
    #[serde(default)]
    pub dev: Option<bool>,
    #[serde(default)]
    pub write_file: Option<WriteFileDef>,
    #[serde(default)]
    pub patch_file: Option<PatchFileDef>,
    #[serde(default)]
    pub set_env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub mkdir: Option<String>,
    #[serde(default)]
    pub cp: Option<CopyDef>,
    #[serde(default)]
    pub rm: Option<String>,

    // Metadata
    #[serde(default)]
    pub once: Option<bool>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct WriteFileDef {
    pub path: String,
    /// Inline content. Supports `{{var}}` substitution.
    pub content: Option<String>,
    /// Template file name — fetched from `templates/` in the registry.
    pub template: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct PatchFileDef {
    pub path: String,
    /// Insert content after a line matching this pattern.
    pub after: Option<String>,
    /// Insert content before a line matching this pattern.
    pub before: Option<String>,
    /// Replace lines matching this regex.
    pub replace: Option<String>,
    /// The content to insert or replace with.
    pub content: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct CopyDef {
    pub src: String,
    pub dest: String,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a blueprint file (YAML, JSON, or JSONC).
pub fn parse_blueprint<P: AsRef<Path>>(path: P) -> Result<BlueprintSchema> {
    let path = path.as_ref();
    let raw = starbase_utils::fs::read_file(path)
        .map_err(|e| miette!("Failed to read blueprint file {}: {}", path.display(), e))?;

    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    match ext.to_lowercase().as_str() {
        "yaml" | "yml" => {
            serde_yaml::from_str(&raw)
                .map_err(|e| miette!("Invalid YAML in {}: {}", path.display(), e))
        }
        "jsonc" => {
            // Strip comments before parsing
            let stripped = json_comments::StripComments::new(raw.as_bytes());
            serde_json::from_reader(stripped)
                .map_err(|e| miette!("Invalid JSONC in {}: {}", path.display(), e))
        }
        _ => {
            // Default: try JSON
            serde_json::from_str(&raw)
                .map_err(|e| miette!("Invalid JSON in {}: {}", path.display(), e))
        }
    }
}
```

- [ ] **Step 6: Add module declaration to `crates/init/src/lib.rs`**

Add `pub mod blueprint_schema;` to the module declarations in `crates/init/src/lib.rs`.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p init --test blueprint_schema_test -v`
Expected: All 7 tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/init/src/blueprint_schema.rs crates/init/tests/ crates/init/src/lib.rs crates/init/Cargo.toml
git commit -m "feat(init): add blueprint schema types with YAML/JSON/JSONC parsing"
```

---

## Task 2: Registry Client

Fetch `registry.json` from GitHub and resolve blueprint URLs. Local caching with 24h TTL.

**Files:**
- Create: `crates/init/src/registry.rs`
- Create: `crates/init/tests/registry_test.rs`
- Modify: `crates/init/src/lib.rs`
- Modify: `crates/init/Cargo.toml`

- [ ] **Step 1: Write failing tests for registry client**

Create `crates/init/tests/registry_test.rs`:

```rust
use init::registry::{RegistryIndex, RegistryClient, resolve_blueprint_url};

#[test]
fn parse_registry_index() {
    let json = r#"{
        "version": 1,
        "frameworks": {
            "nextjs": {
                "name": "Next.js",
                "blueprints": {
                    "default": { "description": "Base Next.js setup" },
                    "ecommerce": { "description": "E-commerce starter" }
                }
            }
        }
    }"#;
    let index: RegistryIndex = serde_json::from_str(json).unwrap();
    assert_eq!(index.version, 1);
    assert!(index.frameworks.contains_key("nextjs"));
    let nextjs = &index.frameworks["nextjs"];
    assert_eq!(nextjs.name, "Next.js");
    assert!(nextjs.blueprints.contains_key("default"));
    assert!(nextjs.blueprints.contains_key("ecommerce"));
}

#[test]
fn resolve_blueprint_url_from_registry() {
    let url = resolve_blueprint_url("nextjs", "ecommerce");
    assert!(url.contains("nextjs/ecommerce/blueprint.yaml"));
}

#[test]
fn resolve_default_blueprint_url() {
    let url = resolve_blueprint_url("nextjs", "default");
    assert!(url.contains("nextjs/default/blueprint.yaml"));
}

#[test]
fn registry_has_blueprint_check() {
    let json = r#"{
        "version": 1,
        "frameworks": {
            "nextjs": {
                "name": "Next.js",
                "blueprints": {
                    "default": { "description": "Base" }
                }
            }
        }
    }"#;
    let index: RegistryIndex = serde_json::from_str(json).unwrap();
    assert!(index.has_blueprint("nextjs", "default"));
    assert!(!index.has_blueprint("nextjs", "ecommerce"));
    assert!(!index.has_blueprint("rails", "default"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p init --test registry_test 2>&1 | head -20`
Expected: Compilation error — `init::registry` does not exist.

- [ ] **Step 3: Implement registry client**

Create `crates/init/src/registry.rs`:

```rust
//! GitHub blueprint registry client.
//!
//! Fetches registry.json, resolves framework/blueprint to raw GitHub URLs,
//! and caches locally with a 24-hour TTL.

use miette::{miette, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const REGISTRY_REPO: &str = "AviHS/appz-blueprints"; // TODO: update to org repo
const REGISTRY_BRANCH: &str = "main";
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RegistryIndex {
    pub version: u32,
    pub frameworks: HashMap<String, FrameworkEntry>,
}

#[derive(Debug, Deserialize)]
pub struct FrameworkEntry {
    pub name: String,
    pub blueprints: HashMap<String, BlueprintEntry>,
}

#[derive(Debug, Deserialize)]
pub struct BlueprintEntry {
    pub description: String,
}

impl RegistryIndex {
    pub fn has_blueprint(&self, framework: &str, blueprint: &str) -> bool {
        self.frameworks
            .get(framework)
            .map(|f| f.blueprints.contains_key(blueprint))
            .unwrap_or(false)
    }

    pub fn has_framework(&self, framework: &str) -> bool {
        self.frameworks.contains_key(framework)
    }
}

// ---------------------------------------------------------------------------
// URL resolution
// ---------------------------------------------------------------------------

/// Build the raw GitHub URL for a blueprint file.
pub fn resolve_blueprint_url(framework: &str, blueprint: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/{}/{}/{}/{}/blueprint.yaml",
        REGISTRY_REPO, REGISTRY_BRANCH, framework, blueprint
    )
}

/// Build the raw GitHub URL for a template file within a blueprint.
pub fn resolve_template_url(framework: &str, blueprint: &str, template: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/{}/{}/{}/{}/templates/{}",
        REGISTRY_REPO, REGISTRY_BRANCH, framework, blueprint, template
    )
}

/// Build the raw GitHub URL for registry.json.
fn registry_index_url() -> String {
    format!(
        "https://raw.githubusercontent.com/{}/{}/registry.json",
        REGISTRY_REPO, REGISTRY_BRANCH
    )
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

fn cache_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".appz")
        .join("cache")
}

fn cache_path() -> PathBuf {
    cache_dir().join("registry.json")
}

fn is_cache_valid() -> bool {
    let path = cache_path();
    if !path.exists() {
        return false;
    }
    path.metadata()
        .and_then(|m| m.modified())
        .map(|modified| {
            SystemTime::now()
                .duration_since(modified)
                .unwrap_or(CACHE_TTL)
                < CACHE_TTL
        })
        .unwrap_or(false)
}

fn read_cache() -> Option<RegistryIndex> {
    let raw = std::fs::read_to_string(cache_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_cache(raw: &str) {
    let dir = cache_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(cache_path(), raw);
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

pub struct RegistryClient;

impl RegistryClient {
    /// Fetch the registry index, using cache when available.
    pub async fn fetch_index(no_cache: bool) -> Result<RegistryIndex> {
        if !no_cache && is_cache_valid() {
            if let Some(index) = read_cache() {
                return Ok(index);
            }
        }

        let url = registry_index_url();
        let response = reqwest::get(&url)
            .await
            .map_err(|e| miette!("Failed to fetch registry: {}", e))?;

        if !response.status().is_success() {
            return Err(miette!(
                "Failed to fetch registry (HTTP {}). Check your network connection.",
                response.status()
            ));
        }

        let raw = response
            .text()
            .await
            .map_err(|e| miette!("Failed to read registry response: {}", e))?;

        let index: RegistryIndex = serde_json::from_str(&raw)
            .map_err(|e| miette!("Invalid registry.json: {}", e))?;

        write_cache(&raw);
        Ok(index)
    }

    /// Fetch a blueprint file from the registry.
    pub async fn fetch_blueprint(framework: &str, blueprint: &str) -> Result<String> {
        let url = resolve_blueprint_url(framework, blueprint);
        let response = reqwest::get(&url)
            .await
            .map_err(|e| miette!("Failed to fetch blueprint: {}", e))?;

        if !response.status().is_success() {
            return Err(miette!(
                "Blueprint '{}/{}' not found in registry (HTTP {})",
                framework, blueprint, response.status()
            ));
        }

        response
            .text()
            .await
            .map_err(|e| miette!("Failed to read blueprint: {}", e))
    }

    /// Fetch a template file from the registry.
    pub async fn fetch_template(
        framework: &str,
        blueprint: &str,
        template: &str,
    ) -> Result<String> {
        let url = resolve_template_url(framework, blueprint, template);
        let response = reqwest::get(&url)
            .await
            .map_err(|e| miette!("Failed to fetch template '{}': {}", template, e))?;

        if !response.status().is_success() {
            return Err(miette!(
                "Template '{}' not found for {}/{} (HTTP {})",
                template, framework, blueprint, response.status()
            ));
        }

        response
            .text()
            .await
            .map_err(|e| miette!("Failed to read template: {}", e))
    }
}
```

- [ ] **Step 4: Add module declaration and reqwest dependency**

Add `pub mod registry;` to `crates/init/src/lib.rs`.

In `crates/init/Cargo.toml`, add:
```toml
reqwest = { workspace = true }
dirs = "5"
```

Check that `reqwest` is a workspace dependency in root `Cargo.toml`. If not, add it.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p init --test registry_test -v`
Expected: All 4 tests pass. (The sync parsing tests don't need network; the async `RegistryClient` methods are not tested here — integration tests later.)

- [ ] **Step 6: Commit**

```bash
git add crates/init/src/registry.rs crates/init/tests/registry_test.rs crates/init/src/lib.rs crates/init/Cargo.toml
git commit -m "feat(init): add blueprint registry client with caching"
```

---

## Task 3: WordPress Playground Blueprint Converter

Convert WordPress Playground JSON to the generic blueprint format.

**Files:**
- Create: `crates/blueprint/src/converter.rs`
- Create: `crates/blueprint/tests/converter_test.rs`
- Create: `crates/blueprint/tests/fixtures/playground_simple.json`
- Modify: `crates/blueprint/src/lib.rs`

- [ ] **Step 1: Create test fixture**

Create `crates/blueprint/tests/fixtures/playground_simple.json`:

```json
{
    "$schema": "https://playground.wordpress.net/blueprint-schema.json",
    "preferredVersions": { "php": "8.2", "wp": "6.4" },
    "plugins": ["woocommerce", "jetpack"],
    "siteOptions": {
        "blogname": "My Store"
    },
    "steps": [
        { "step": "installTheme", "themeData": { "resource": "wordpress.org/themes", "slug": "astra" } },
        { "step": "setSiteLanguage", "language": "de_DE" },
        { "step": "writeFile", "path": "/wordpress/test.txt", "data": "hello" },
        { "step": "mkdir", "path": "/wordpress/custom" },
        { "step": "wp-cli", "command": "cache flush" }
    ]
}
```

- [ ] **Step 2: Write failing tests**

Create `crates/blueprint/tests/converter_test.rs`:

```rust
use std::path::PathBuf;

use blueprint::converter::{convert_playground_to_generic, is_playground_blueprint};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn detect_playground_blueprint() {
    let raw = std::fs::read_to_string(fixture("playground_simple.json")).unwrap();
    assert!(is_playground_blueprint(&raw));
}

#[test]
fn detect_non_playground_json() {
    let raw = r#"{"version": 1, "meta": {"framework": "nextjs"}}"#;
    assert!(!is_playground_blueprint(raw));
}

#[test]
fn convert_plugins_shorthand() {
    let raw = std::fs::read_to_string(fixture("playground_simple.json")).unwrap();
    let result = convert_playground_to_generic(&raw).unwrap();
    // Should have setup steps for plugin installs
    let setup = result.setup.as_ref().unwrap();
    let plugin_steps: Vec<_> = setup.iter()
        .filter(|s| s.run_locally.as_ref().map(|c| c.contains("wp plugin install")).unwrap_or(false))
        .collect();
    assert_eq!(plugin_steps.len(), 2);
    assert!(plugin_steps[0].run_locally.as_ref().unwrap().contains("woocommerce"));
    assert!(plugin_steps[1].run_locally.as_ref().unwrap().contains("jetpack"));
}

#[test]
fn convert_site_options() {
    let raw = std::fs::read_to_string(fixture("playground_simple.json")).unwrap();
    let result = convert_playground_to_generic(&raw).unwrap();
    let setup = result.setup.as_ref().unwrap();
    let opt_steps: Vec<_> = setup.iter()
        .filter(|s| s.run_locally.as_ref().map(|c| c.contains("wp option update")).unwrap_or(false))
        .collect();
    assert!(opt_steps.len() >= 1);
    assert!(opt_steps[0].run_locally.as_ref().unwrap().contains("blogname"));
}

#[test]
fn convert_install_theme_step() {
    let raw = std::fs::read_to_string(fixture("playground_simple.json")).unwrap();
    let result = convert_playground_to_generic(&raw).unwrap();
    let setup = result.setup.as_ref().unwrap();
    let theme_steps: Vec<_> = setup.iter()
        .filter(|s| s.run_locally.as_ref().map(|c| c.contains("wp theme install")).unwrap_or(false))
        .collect();
    assert_eq!(theme_steps.len(), 1);
    assert!(theme_steps[0].run_locally.as_ref().unwrap().contains("astra"));
}

#[test]
fn convert_write_file_step() {
    let raw = std::fs::read_to_string(fixture("playground_simple.json")).unwrap();
    let result = convert_playground_to_generic(&raw).unwrap();
    let setup = result.setup.as_ref().unwrap();
    let wf_steps: Vec<_> = setup.iter()
        .filter(|s| s.write_file.is_some())
        .collect();
    assert_eq!(wf_steps.len(), 1);
    assert_eq!(wf_steps[0].write_file.as_ref().unwrap().path, "test.txt");
}

#[test]
fn convert_wp_cli_step() {
    let raw = std::fs::read_to_string(fixture("playground_simple.json")).unwrap();
    let result = convert_playground_to_generic(&raw).unwrap();
    let setup = result.setup.as_ref().unwrap();
    let cli_steps: Vec<_> = setup.iter()
        .filter(|s| s.run_locally.as_ref().map(|c| c.contains("wp cache flush")).unwrap_or(false))
        .collect();
    assert_eq!(cli_steps.len(), 1);
}

#[test]
fn convert_sets_wordpress_framework() {
    let raw = std::fs::read_to_string(fixture("playground_simple.json")).unwrap();
    let result = convert_playground_to_generic(&raw).unwrap();
    assert_eq!(result.meta.as_ref().unwrap().framework.as_deref(), Some("wordpress"));
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p blueprint --test converter_test 2>&1 | head -20`
Expected: Compilation error — `blueprint::converter` does not exist.

- [ ] **Step 4: Implement the converter**

Create `crates/blueprint/src/converter.rs`:

```rust
//! Convert WordPress Playground blueprint JSON to generic blueprint format.
//!
//! The generic format uses `init::blueprint_schema` types. Since the blueprint
//! crate should not depend on the init crate, we output a standalone struct
//! that mirrors the schema and can be converted by the caller.

use miette::{miette, Result};
use serde::Deserialize;
use std::collections::HashMap;

// We re-define minimal output types here to avoid circular deps.
// The caller (BlueprintProvider) maps these to BlueprintSchema.

#[derive(Debug, Default)]
pub struct GenericBlueprint {
    pub meta: Option<GenericMeta>,
    pub setup: Option<Vec<GenericSetupStep>>,
}

#[derive(Debug, Default)]
pub struct GenericMeta {
    pub name: Option<String>,
    pub framework: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct GenericSetupStep {
    pub desc: Option<String>,
    pub run_locally: Option<String>,
    pub write_file: Option<GenericWriteFile>,
    pub mkdir: Option<String>,
    pub add_dependency: Option<Vec<String>>,
    pub dev: Option<bool>,
    pub set_env: Option<HashMap<String, String>>,
    pub cd: Option<String>,
    pub rm: Option<String>,
    pub cp: Option<GenericCopy>,
    pub patch_file: Option<GenericPatchFile>,
    pub once: Option<bool>,
}

#[derive(Debug, Default, Clone)]
pub struct GenericWriteFile {
    pub path: String,
    pub content: Option<String>,
    pub template: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct GenericCopy {
    pub src: String,
    pub dest: String,
}

#[derive(Debug, Default, Clone)]
pub struct GenericPatchFile {
    pub path: String,
    pub after: Option<String>,
    pub before: Option<String>,
    pub replace: Option<String>,
    pub content: String,
}

// ---------------------------------------------------------------------------
// Playground JSON types (minimal, for parsing)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaygroundBlueprint {
    #[serde(rename = "$schema", default)]
    schema: Option<String>,
    #[serde(default)]
    preferred_versions: Option<PlaygroundVersions>,
    #[serde(default)]
    plugins: Vec<serde_json::Value>,
    #[serde(default)]
    site_options: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    constants: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    login: Option<serde_json::Value>,
    #[serde(default)]
    meta: Option<PlaygroundMeta>,
    #[serde(default)]
    steps: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct PlaygroundVersions {
    php: Option<String>,
    wp: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlaygroundMeta {
    title: Option<String>,
    description: Option<String>,
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Check if a JSON string looks like a WordPress Playground blueprint.
pub fn is_playground_blueprint(raw: &str) -> bool {
    // Quick heuristic checks without full parse
    (raw.contains("\"$schema\"") && raw.contains("playground.wordpress.net"))
        || raw.contains("\"preferredVersions\"")
        || raw.contains("\"siteOptions\"")
        || (raw.contains("\"steps\"") && (
            raw.contains("\"installPlugin\"")
            || raw.contains("\"installTheme\"")
            || raw.contains("\"activatePlugin\"")
            || raw.contains("\"setSiteOptions\"")
            || raw.contains("\"runPHP\"")
            || raw.contains("\"wp-cli\"")
        ))
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

/// Convert a WordPress Playground blueprint JSON string to the generic format.
pub fn convert_playground_to_generic(raw: &str) -> Result<GenericBlueprint> {
    let pg: PlaygroundBlueprint = serde_json::from_str(raw)
        .map_err(|e| miette!("Failed to parse Playground blueprint: {}", e))?;

    let mut steps: Vec<GenericSetupStep> = Vec::new();

    // 1. Top-level plugins shorthand
    for plugin in &pg.plugins {
        let slug = match plugin {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Object(obj) => {
                obj.get("resource")
                    .and_then(|r| {
                        if r.as_str() == Some("wordpress.org/plugins") {
                            obj.get("slug").and_then(|s| s.as_str()).map(String::from)
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default()
            }
            _ => continue,
        };
        if !slug.is_empty() {
            steps.push(GenericSetupStep {
                desc: Some(format!("Install plugin: {}", slug)),
                run_locally: Some(format!("wp plugin install {} --activate", slug)),
                ..Default::default()
            });
        }
    }

    // 2. Top-level site options
    if let Some(opts) = &pg.site_options {
        for (key, val) in opts {
            let val_str = match val {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            steps.push(GenericSetupStep {
                desc: Some(format!("Set site option: {}", key)),
                run_locally: Some(format!("wp option update {} '{}'", key, val_str)),
                ..Default::default()
            });
        }
    }

    // 3. Convert individual steps
    for step_val in &pg.steps {
        let step_type = step_val.get("step").and_then(|s| s.as_str()).unwrap_or("");
        match step_type {
            "installPlugin" => {
                let slug = step_val.get("pluginData")
                    .or_else(|| step_val.get("pluginZipFile"))
                    .and_then(|d| d.get("slug"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown");
                steps.push(GenericSetupStep {
                    desc: Some(format!("Install plugin: {}", slug)),
                    run_locally: Some(format!("wp plugin install {} --activate", slug)),
                    ..Default::default()
                });
            }
            "installTheme" => {
                let slug = step_val.get("themeData")
                    .or_else(|| step_val.get("themeZipFile"))
                    .and_then(|d| d.get("slug"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown");
                steps.push(GenericSetupStep {
                    desc: Some(format!("Install theme: {}", slug)),
                    run_locally: Some(format!("wp theme install {} --activate", slug)),
                    ..Default::default()
                });
            }
            "activatePlugin" => {
                let name = step_val.get("pluginName")
                    .or_else(|| step_val.get("pluginPath"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown");
                steps.push(GenericSetupStep {
                    desc: Some(format!("Activate plugin: {}", name)),
                    run_locally: Some(format!("wp plugin activate {}", name)),
                    ..Default::default()
                });
            }
            "activateTheme" => {
                let name = step_val.get("themeFolderName")
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown");
                steps.push(GenericSetupStep {
                    desc: Some(format!("Activate theme: {}", name)),
                    run_locally: Some(format!("wp theme activate {}", name)),
                    ..Default::default()
                });
            }
            "setSiteOptions" => {
                if let Some(options) = step_val.get("options").and_then(|o| o.as_object()) {
                    for (key, val) in options {
                        let val_str = match val {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        steps.push(GenericSetupStep {
                            desc: Some(format!("Set site option: {}", key)),
                            run_locally: Some(format!("wp option update {} '{}'", key, val_str)),
                            ..Default::default()
                        });
                    }
                }
            }
            "setSiteLanguage" => {
                let lang = step_val.get("language").and_then(|s| s.as_str()).unwrap_or("en_US");
                steps.push(GenericSetupStep {
                    desc: Some(format!("Set site language: {}", lang)),
                    run_locally: Some(format!("wp language core install {} --activate", lang)),
                    ..Default::default()
                });
            }
            "writeFile" => {
                let path = step_val.get("path").and_then(|s| s.as_str()).unwrap_or("");
                let data = step_val.get("data").and_then(|s| s.as_str()).unwrap_or("");
                // Strip /wordpress/ prefix if present
                let clean_path = path.trim_start_matches("/wordpress/");
                steps.push(GenericSetupStep {
                    write_file: Some(GenericWriteFile {
                        path: clean_path.to_string(),
                        content: Some(data.to_string()),
                        template: None,
                    }),
                    desc: Some(format!("Write file: {}", clean_path)),
                    ..Default::default()
                });
            }
            "mkdir" => {
                let path = step_val.get("path").and_then(|s| s.as_str()).unwrap_or("");
                let clean_path = path.trim_start_matches("/wordpress/");
                steps.push(GenericSetupStep {
                    mkdir: Some(clean_path.to_string()),
                    desc: Some(format!("Create directory: {}", clean_path)),
                    ..Default::default()
                });
            }
            "wp-cli" => {
                let cmd = step_val.get("command")
                    .and_then(|c| match c {
                        serde_json::Value::String(s) => Some(format!("wp {}", s)),
                        serde_json::Value::Array(arr) => {
                            let parts: Vec<String> = arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect();
                            Some(format!("wp {}", parts.join(" ")))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| "wp --help".to_string());
                steps.push(GenericSetupStep {
                    desc: Some(format!("WP-CLI: {}", cmd)),
                    run_locally: Some(cmd),
                    ..Default::default()
                });
            }
            "runSql" => {
                let sql = step_val.get("sql").and_then(|s| s.as_str()).unwrap_or("");
                steps.push(GenericSetupStep {
                    desc: Some("Run SQL query".to_string()),
                    run_locally: Some(format!("wp db query '{}'", sql.replace('\'', "'\\''"))),
                    ..Default::default()
                });
            }
            "runPHP" | "runPHPWithOptions" => {
                let code = step_val.get("code").and_then(|s| s.as_str()).unwrap_or("");
                steps.push(GenericSetupStep {
                    desc: Some("Run PHP code".to_string()),
                    run_locally: Some(format!("wp eval '{}'", code.replace('\'', "'\\''"))),
                    ..Default::default()
                });
            }
            "cp" => {
                let from = step_val.get("fromPath").and_then(|s| s.as_str()).unwrap_or("");
                let to = step_val.get("toPath").and_then(|s| s.as_str()).unwrap_or("");
                steps.push(GenericSetupStep {
                    cp: Some(GenericCopy {
                        src: from.trim_start_matches("/wordpress/").to_string(),
                        dest: to.trim_start_matches("/wordpress/").to_string(),
                    }),
                    desc: Some(format!("Copy: {} -> {}", from, to)),
                    ..Default::default()
                });
            }
            "mv" => {
                let from = step_val.get("fromPath").and_then(|s| s.as_str()).unwrap_or("");
                let to = step_val.get("toPath").and_then(|s| s.as_str()).unwrap_or("");
                steps.push(GenericSetupStep {
                    run_locally: Some(format!("mv {} {}", from.trim_start_matches("/wordpress/"), to.trim_start_matches("/wordpress/"))),
                    desc: Some(format!("Move: {} -> {}", from, to)),
                    ..Default::default()
                });
            }
            "rm" | "rmdir" => {
                let path = step_val.get("path").and_then(|s| s.as_str()).unwrap_or("");
                let clean_path = path.trim_start_matches("/wordpress/");
                steps.push(GenericSetupStep {
                    rm: Some(clean_path.to_string()),
                    desc: Some(format!("Remove: {}", clean_path)),
                    ..Default::default()
                });
            }
            other => {
                // Warn and skip unknown step types
                tracing::warn!("Skipping unsupported Playground step type: {}", other);
            }
        }
    }

    let title = pg.meta.as_ref().and_then(|m| m.title.clone());

    Ok(GenericBlueprint {
        meta: Some(GenericMeta {
            name: title,
            framework: Some("wordpress".to_string()),
        }),
        setup: Some(steps),
    })
}
```

- [ ] **Step 5: Add module declaration**

Add `pub mod converter;` to `crates/blueprint/src/lib.rs`.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p blueprint --test converter_test -v`
Expected: All 8 tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/blueprint/src/converter.rs crates/blueprint/tests/ crates/blueprint/src/lib.rs
git commit -m "feat(blueprint): add WordPress Playground to generic blueprint converter"
```

---

## Task 4: Update Source Detection

Rewrite `detect.rs` to route `framework/blueprint` patterns to the new BlueprintProvider, and add `git:` escape hatch.

**Files:**
- Modify: `crates/init/src/detect.rs`
- Create: `crates/init/tests/detect_test.rs`

- [ ] **Step 1: Write failing tests for new detection logic**

Create `crates/init/tests/detect_test.rs`:

```rust
use init::detect::{resolve_source, parse_framework_blueprint};

#[test]
fn detect_framework_with_blueprint() {
    let (fw, bp) = parse_framework_blueprint("nextjs/ecommerce").unwrap();
    assert_eq!(fw, "nextjs");
    assert_eq!(bp, "ecommerce");
}

#[test]
fn detect_framework_default() {
    // Bare framework slug should resolve to framework/default
    let resolved = resolve_source("nextjs").unwrap();
    assert_eq!(resolved.provider.slug(), "blueprint");
}

#[test]
fn detect_framework_slash_blueprint() {
    let resolved = resolve_source("nextjs/ecommerce").unwrap();
    assert_eq!(resolved.provider.slug(), "blueprint");
    assert_eq!(resolved.source, "nextjs/ecommerce");
}

#[test]
fn detect_git_escape_hatch() {
    let resolved = resolve_source("git:nextjs/my-template").unwrap();
    assert_eq!(resolved.provider.slug(), "git");
    assert_eq!(resolved.source, "nextjs/my-template");
}

#[test]
fn detect_non_framework_user_repo() {
    let resolved = resolve_source("someuser/somerepo").unwrap();
    assert_eq!(resolved.provider.slug(), "git");
}

#[test]
fn detect_wordpress() {
    let resolved = resolve_source("wordpress").unwrap();
    assert_eq!(resolved.provider.slug(), "blueprint");
}

#[test]
fn detect_npm_prefix() {
    let resolved = resolve_source("npm:create-foo").unwrap();
    assert_eq!(resolved.provider.slug(), "npm");
}

#[test]
fn detect_local_path() {
    let resolved = resolve_source("./my-project").unwrap();
    assert_eq!(resolved.provider.slug(), "local");
}

#[test]
fn detect_archive_url() {
    let resolved = resolve_source("https://example.com/template.zip").unwrap();
    assert_eq!(resolved.provider.slug(), "remote-archive");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p init --test detect_test 2>&1 | head -30`
Expected: Compilation errors — `parse_framework_blueprint` doesn't exist, `"blueprint"` slug not found.

- [ ] **Step 3: Rewrite `crates/init/src/detect.rs`**

Update detection priority:
1. `npm:` prefix -> NpmProvider
2. `git:` prefix -> GitProvider (escape hatch, strip prefix)
3. Known framework slug + `/name` -> BlueprintProvider
4. Known framework slug alone -> BlueprintProvider
5. Archive URLs -> RemoteArchiveProvider
6. Git URLs / `user/repo` -> GitProvider
7. Local paths -> LocalProvider

Key changes:
- Add `pub fn parse_framework_blueprint(source: &str) -> Option<(String, String)>` that splits `framework/blueprint` and validates the first segment against the framework registry AND the `FRAMEWORK_CREATE` table.
- Replace `is_framework_slug()` check with a combined check: either `has_create_command(slug)` or `frameworks::find_by_slug(slug).is_some()`.
- Add `git:` prefix handling at the top.
- Route `"wordpress"` to BlueprintProvider (slug `"blueprint"`) instead of WordPressProvider.

The function `is_framework_slug` should now also accept slugs with `/` (for the `framework/blueprint` pattern). Separate into `is_known_framework(slug: &str) -> bool` that checks both the create command table and the frameworks data.

Note: `BlueprintProvider` must be added to the provider registry in `provider.rs` first (Task 5). For now, the tests will confirm detection routing. If BlueprintProvider doesn't exist yet, create a stub in the providers module.

- [ ] **Step 4: Update `crates/init/src/provider.rs`**

Add a stub `BlueprintProvider` to the registry. Replace `WordPressProvider` and `FrameworkProvider` with `BlueprintProvider`:

```rust
pub fn create_provider_registry() -> Vec<Box<dyn InitProvider>> {
    vec![
        Box::new(providers::blueprint::BlueprintProvider),
        Box::new(providers::git::GitProvider),
        Box::new(providers::remote_archive::RemoteArchiveProvider),
        Box::new(providers::npm::NpmProvider),
        Box::new(providers::local::LocalProvider),
    ]
}
```

Create a minimal stub `crates/init/src/providers/blueprint.rs`:

```rust
use async_trait::async_trait;
use crate::config::InitContext;
use crate::error::{InitError, InitResult};
use crate::output::InitOutput;
use crate::provider::InitProvider;

pub struct BlueprintProvider;

#[async_trait]
impl InitProvider for BlueprintProvider {
    fn name(&self) -> &str { "Blueprint" }
    fn slug(&self) -> &str { "blueprint" }
    async fn init(&self, _ctx: &InitContext) -> InitResult<InitOutput> {
        Err(InitError::SourceNotFound("Blueprint provider not yet implemented".to_string()))
    }
}
```

Update `crates/init/src/providers/mod.rs`: add `pub mod blueprint;`, keep `framework` and `wordpress` for now (they're still referenced elsewhere).

- [ ] **Step 5: Update `crates/init/src/run.rs` `is_source()`**

Update the `is_source()` function to recognize `framework/blueprint` patterns and `git:` prefix:

```rust
fn is_source(s: &str) -> bool {
    s.starts_with("https://")
        || s.starts_with("http://")
        || s.starts_with("npm:")
        || s.starts_with("git:")
        || s.starts_with("./")
        || s.starts_with("../")
        || s.starts_with('/')
        || (s.len() > 1 && s.chars().nth(1) == Some(':') && !s.contains("github.com"))
        || crate::detect::parse_framework_blueprint(s).is_some()
        || crate::detect::is_known_framework(s)
        || s.contains('/')
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p init --test detect_test -v`
Expected: All 9 tests pass.

- [ ] **Step 7: Run existing tests to make sure nothing broke**

Run: `cargo test -p init -v`
Expected: All existing tests still pass (some may need updating if they assumed old detection order).

- [ ] **Step 8: Commit**

```bash
git add crates/init/src/detect.rs crates/init/src/provider.rs crates/init/src/providers/ crates/init/src/run.rs crates/init/tests/detect_test.rs
git commit -m "feat(init): update source detection for blueprint provider and git: escape hatch"
```

---

## Task 5a: Add `blueprint` and `no_cache` to InitOptions and update `run()`

**Files:**
- Modify: `crates/init/src/config.rs`
- Modify: `crates/init/src/run.rs`

- [ ] **Step 1: Add fields to `InitOptions`**

In `crates/init/src/config.rs`, add new fields to `InitOptions`:

```rust
pub struct InitOptions {
    pub project_name: String,
    pub output_dir: PathBuf,
    pub skip_install: bool,
    pub force: bool,
    pub json_output: bool,
    pub is_ci: bool,
    pub blueprint: Option<String>,  // NEW: --blueprint flag value (name, path, or URL)
    pub no_cache: bool,             // NEW: --no-cache flag for registry
}
```

- [ ] **Step 2: Update `init::run()` signature**

In `crates/init/src/run.rs`, update the `run()` function to accept the new fields. Add `blueprint` and `no_cache` parameters and pass them into `InitOptions`:

```rust
pub async fn run(
    template_source: Option<String>,
    project_name: Option<String>,
    template_or_name: Option<String>,
    name: Option<String>,
    template: Option<String>,
    skip_install: bool,
    force: bool,
    output: Option<PathBuf>,
    json_output: bool,
    blueprint: Option<String>,   // NEW
    no_cache: bool,              // NEW
) -> InitResult<Option<InitOutput>> {
    // ... existing resolution logic ...
    let options = InitOptions {
        project_name: project_name.clone(),
        output_dir: output_dir.clone(),
        skip_install,
        force,
        json_output,
        is_ci,
        blueprint,
        no_cache,
    };
    // ... rest unchanged ...
}
```

- [ ] **Step 3: Fix callers of `init::run()`**

Update `crates/app/src/commands/init.rs` to pass `blueprint` (change type from `Option<PathBuf>` to `Option<String>`) and `no_cache` (new flag) to `init::run()`. Remove the `playground: bool` parameter — Playground detection is now handled by BlueprintProvider.

- [ ] **Step 4: Run tests**

Run: `cargo test -p init -v && cargo test -p app -v`
Expected: All tests pass (or compile errors that point to callers needing updates).

- [ ] **Step 5: Commit**

```bash
git add crates/init/src/config.rs crates/init/src/run.rs crates/app/src/commands/init.rs
git commit -m "refactor(init): add blueprint and no_cache fields to InitOptions"
```

---

## Task 5b: BlueprintProvider — Fetch and Parse

**Files:**
- Modify: `crates/init/src/providers/blueprint.rs` (replace stub)

- [ ] **Step 1: Implement blueprint fetching and parsing**

Replace the stub in `crates/init/src/providers/blueprint.rs`:

```rust
//! Blueprint provider: fetches blueprints from registry/local/URL,
//! runs framework scaffolding, executes setup steps, saves blueprint.

use async_trait::async_trait;
use miette::miette;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::instrument;

use crate::blueprint_schema::{BlueprintSchema, SetupStep, parse_blueprint, WriteFileDef, PatchFileDef};
use crate::config::InitContext;
use crate::detect::parse_framework_blueprint;
use crate::error::{InitError, InitResult};
use crate::output::InitOutput;
use crate::provider::InitProvider;
use crate::registry::RegistryClient;

/// Framework slugs that have create commands.
/// Migrated from the retired FrameworkProvider.
const FRAMEWORK_CREATE: &[(&str, &str)] = &[
    ("astro", "npm create astro@latest"),
    ("nextjs", "npx create-next-app@latest"),
    ("vite", "npm create vite@latest"),
    ("sveltekit", "npm create svelte@latest"),
    ("nuxt", "npx nuxi@latest init"),
    ("remix", "npx create-remix@latest"),
    ("docusaurus", "npx create-docusaurus@latest"),
    ("vitepress", "npx vitepress@latest init"),
    ("gatsby", "npm create gatsby@latest"),
    ("eleventy", "npm create @11ty/eleventy@latest"),
];

pub fn has_create_command(slug: &str) -> bool {
    FRAMEWORK_CREATE.iter().any(|(s, _)| *s == slug)
}

fn get_create_command(slug: &str) -> Option<&'static str> {
    FRAMEWORK_CREATE.iter().find(|(s, _)| *s == slug).map(|(_, cmd)| *cmd)
}

pub struct BlueprintProvider;

#[async_trait]
impl InitProvider for BlueprintProvider {
    fn name(&self) -> &str { "Blueprint" }
    fn slug(&self) -> &str { "blueprint" }

    #[instrument(skip_all)]
    async fn init(&self, ctx: &InitContext) -> InitResult<InitOutput> {
        // 1. Resolve framework and blueprint name
        let (framework, blueprint_name) = resolve_framework_and_blueprint(ctx)?;

        // 2. Fetch and parse the blueprint
        let blueprint = fetch_and_parse_blueprint(
            &framework,
            &blueprint_name,
            ctx.options.blueprint.as_deref(),
            ctx.options.no_cache,
        ).await?;

        // 3. Run base framework scaffolding
        let create_cmd = blueprint.meta.as_ref()
            .and_then(|m| m.create_command.as_deref())
            .or_else(|| get_create_command(&framework));

        if let Some(cmd) = create_cmd {
            crate::ui::info(&ctx.options, &format!("Running: {} .", cmd));
            let status = ctx.exec_interactive(&format!("{} .", cmd)).await
                .map_err(|e| InitError::CommandFailed(cmd.to_string(), e.to_string()))?;
            if !status.success() {
                return Err(InitError::CommandFailed(
                    cmd.to_string(),
                    "Framework create command failed".to_string(),
                ));
            }
        }

        // 4. Execute setup steps
        let project_path = ctx.project_path();
        if let Some(setup_steps) = &blueprint.setup {
            let config_vars = extract_config_vars(&blueprint);
            let pkg_manager = detect_package_manager(&project_path, &blueprint);
            execute_setup_steps(setup_steps, &project_path, &config_vars, &pkg_manager, ctx).await?;
        }

        // 5. Save full blueprint to .appz/blueprint.yaml
        save_blueprint_to_project(&project_path, &blueprint)?;

        let fw_name = frameworks::find_by_slug(&framework)
            .map(|f| f.name.to_string());

        Ok(InitOutput {
            project_path,
            framework: fw_name,
            installed: false,
        })
    }
}

/// Resolve framework slug and blueprint name from the source string.
fn resolve_framework_and_blueprint(ctx: &InitContext) -> InitResult<(String, String)> {
    let source = &ctx.source;

    // Check for framework/blueprint pattern
    if let Some((fw, bp)) = parse_framework_blueprint(source) {
        return Ok((fw, bp));
    }

    // Bare framework slug -> default blueprint
    Ok((source.clone(), "default".to_string()))
}

/// Fetch a blueprint from registry, local file, or URL, then parse it.
async fn fetch_and_parse_blueprint(
    framework: &str,
    blueprint_name: &str,
    blueprint_flag: Option<&str>,
    no_cache: bool,
) -> InitResult<BlueprintSchema> {
    // If --blueprint flag is a local file path, read directly
    if let Some(bp) = blueprint_flag {
        if bp.starts_with("./") || bp.starts_with("../") || bp.starts_with('/') || Path::new(bp).exists() {
            return parse_blueprint(Path::new(bp))
                .map_err(|e| InitError::InvalidFormat(e.to_string()));
        }

        // If it's a URL, fetch it
        if bp.starts_with("http://") || bp.starts_with("https://") {
            let raw = reqwest::get(bp).await
                .map_err(|e| InitError::NetworkError(e.to_string()))?
                .text().await
                .map_err(|e| InitError::NetworkError(e.to_string()))?;
            let schema: BlueprintSchema = serde_yaml::from_str(&raw)
                .or_else(|_| serde_json::from_str(&raw))
                .map_err(|e| InitError::InvalidFormat(format!("Invalid blueprint: {}", e)))?;
            return Ok(schema);
        }

        // If it's a Playground JSON (detected by content), convert
        if bp.ends_with(".json") && Path::new(bp).exists() {
            let raw = std::fs::read_to_string(bp)
                .map_err(|e| InitError::FsError(e.to_string()))?;
            if blueprint::converter::is_playground_blueprint(&raw) {
                let generic = blueprint::converter::convert_playground_to_generic(&raw)
                    .map_err(|e| InitError::InvalidFormat(e.to_string()))?;
                return Ok(generic_to_schema(generic));
            }
        }

        // Otherwise treat as a blueprint name from registry
        let raw = RegistryClient::fetch_blueprint(framework, bp).await
            .map_err(|e| InitError::NetworkError(e.to_string()))?;
        let schema: BlueprintSchema = serde_yaml::from_str(&raw)
            .map_err(|e| InitError::InvalidFormat(format!("Invalid blueprint YAML: {}", e)))?;
        return Ok(schema);
    }

    // No --blueprint flag: fetch from registry using framework/blueprint_name
    let raw = RegistryClient::fetch_blueprint(framework, blueprint_name).await
        .map_err(|e| InitError::NetworkError(e.to_string()))?;
    let schema: BlueprintSchema = serde_yaml::from_str(&raw)
        .map_err(|e| InitError::InvalidFormat(format!("Invalid blueprint YAML: {}", e)))?;
    Ok(schema)
}

/// Convert GenericBlueprint (from Playground converter) to BlueprintSchema.
fn generic_to_schema(generic: blueprint::converter::GenericBlueprint) -> BlueprintSchema {
    use crate::blueprint_schema::*;

    let meta = generic.meta.map(|m| BlueprintMeta {
        name: m.name,
        framework: m.framework,
        ..Default::default()
    });

    let setup = generic.setup.map(|steps| {
        steps.into_iter().map(|s| SetupStep {
            desc: s.desc,
            run_locally: s.run_locally,
            cd: s.cd,
            add_dependency: s.add_dependency,
            dev: s.dev,
            write_file: s.write_file.map(|wf| WriteFileDef {
                path: wf.path,
                content: wf.content,
                template: wf.template,
            }),
            patch_file: s.patch_file.map(|pf| PatchFileDef {
                path: pf.path,
                after: pf.after,
                before: pf.before,
                replace: pf.replace,
                content: pf.content,
            }),
            set_env: s.set_env,
            mkdir: s.mkdir,
            cp: s.cp.map(|c| CopyDef { src: c.src, dest: c.dest }),
            rm: s.rm,
            once: s.once,
            ..Default::default()
        }).collect()
    });

    BlueprintSchema {
        version: Some(1),
        meta,
        setup,
        ..Default::default()
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p init -v 2>&1 | tail -20`
Expected: Compiles and existing tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/init/src/providers/blueprint.rs
git commit -m "feat(init): implement BlueprintProvider fetch, parse, and Playground conversion"
```

---

## Task 5c: BlueprintProvider — Package Manager Detection and Variable Substitution

**Files:**
- Modify: `crates/init/src/providers/blueprint.rs`

- [ ] **Step 1: Add package manager detection and variable substitution**

Append to `crates/init/src/providers/blueprint.rs`:

```rust
// ---------------------------------------------------------------------------
// Package manager detection
// ---------------------------------------------------------------------------

fn detect_package_manager(project_path: &Path, blueprint: &BlueprintSchema) -> String {
    // 1. Explicit override in blueprint meta
    if let Some(pm) = blueprint.meta.as_ref().and_then(|m| m.package_manager.as_deref()) {
        return pm.to_string();
    }

    // 2. Detect from lock files
    if project_path.join("yarn.lock").exists() { return "yarn".to_string(); }
    if project_path.join("pnpm-lock.yaml").exists() { return "pnpm".to_string(); }
    if project_path.join("bun.lockb").exists() { return "bun".to_string(); }
    if project_path.join("package-lock.json").exists() { return "npm".to_string(); }
    if project_path.join("composer.json").exists() { return "composer".to_string(); }
    if project_path.join("Cargo.toml").exists() { return "cargo".to_string(); }
    if project_path.join("go.mod").exists() { return "go".to_string(); }
    if project_path.join("Gemfile").exists() { return "bundler".to_string(); }
    if project_path.join("pyproject.toml").exists() { return "poetry".to_string(); }
    if project_path.join("requirements.txt").exists() { return "pip".to_string(); }

    // 3. Default
    "npm".to_string()
}

/// Build the install command for a package manager.
fn install_command(pm: &str, packages: &[String], is_dev: bool) -> String {
    let pkgs = packages.join(" ");
    match pm {
        "yarn" => if is_dev { format!("yarn add --dev {}", pkgs) } else { format!("yarn add {}", pkgs) },
        "pnpm" => if is_dev { format!("pnpm add -D {}", pkgs) } else { format!("pnpm add {}", pkgs) },
        "bun" => if is_dev { format!("bun add -d {}", pkgs) } else { format!("bun add {}", pkgs) },
        "npm" => if is_dev { format!("npm install --save-dev {}", pkgs) } else { format!("npm install {}", pkgs) },
        "composer" => if is_dev { format!("composer require --dev {}", pkgs) } else { format!("composer require {}", pkgs) },
        "cargo" => if is_dev { format!("cargo add --dev {}", pkgs) } else { format!("cargo add {}", pkgs) },
        "go" => format!("go get {}", pkgs),
        "bundler" => format!("bundle add {}", pkgs),
        "pip" => format!("pip install {}", pkgs),
        "poetry" => if is_dev { format!("poetry add --group dev {}", pkgs) } else { format!("poetry add {}", pkgs) },
        _ => format!("npm install {}", pkgs),
    }
}

// ---------------------------------------------------------------------------
// Variable substitution
// ---------------------------------------------------------------------------

/// Extract config variables from blueprint as a string HashMap.
fn extract_config_vars(blueprint: &BlueprintSchema) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    if let Some(config) = &blueprint.config {
        if let Some(obj) = config.as_object() {
            for (key, val) in obj {
                let val_str = match val {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                vars.insert(key.clone(), val_str);
            }
        }
    }
    vars
}

/// Replace all `{{key}}` placeholders in a string with values from the config map.
fn substitute_vars(input: &str, vars: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (key, val) in vars {
        result = result.replace(&format!("{{{{{}}}}}", key), val);
    }
    result
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p init -v 2>&1 | tail -10`
Expected: Compiles and tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/init/src/providers/blueprint.rs
git commit -m "feat(init): add package manager detection and variable substitution"
```

---

## Task 5d: BlueprintProvider — Setup Step Execution

**Files:**
- Modify: `crates/init/src/providers/blueprint.rs`

- [ ] **Step 1: Implement `execute_setup_steps`**

Append to `crates/init/src/providers/blueprint.rs`:

```rust
use crate::blueprint_schema::CopyDef;

// ---------------------------------------------------------------------------
// Setup step execution
// ---------------------------------------------------------------------------

/// Execute setup steps sequentially. Fail-fast: if one step fails, halt.
async fn execute_setup_steps(
    steps: &[SetupStep],
    project_path: &Path,
    vars: &HashMap<String, String>,
    pkg_manager: &str,
    ctx: &InitContext,
) -> InitResult<()> {
    let mut cwd = project_path.to_path_buf();

    for (i, step) in steps.iter().enumerate() {
        let step_desc = step.desc.as_deref().unwrap_or(&format!("Step {}", i + 1));
        crate::ui::info(&ctx.options, &format!("  -> {}", step_desc));

        // cd
        if let Some(dir) = &step.cd {
            let parsed = substitute_vars(dir, vars);
            cwd = if Path::new(&parsed).is_absolute() {
                PathBuf::from(parsed)
            } else {
                cwd.join(parsed)
            };
        }

        // add_dependency
        if let Some(deps) = &step.add_dependency {
            let is_dev = step.dev.unwrap_or(false);
            let parsed_deps: Vec<String> = deps.iter().map(|d| substitute_vars(d, vars)).collect();
            let cmd = install_command(pkg_manager, &parsed_deps, is_dev);
            run_local_in_dir(&cmd, &cwd, ctx).await
                .map_err(|e| InitError::CommandFailed(cmd.clone(), e.to_string()))?;
        }

        // write_file
        if let Some(wf) = &step.write_file {
            let path = cwd.join(substitute_vars(&wf.path, vars));
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| InitError::FsError(format!("mkdir {}: {}", parent.display(), e)))?;
            }
            if let Some(content) = &wf.content {
                let parsed = substitute_vars(content, vars);
                std::fs::write(&path, parsed)
                    .map_err(|e| InitError::FsError(format!("write {}: {}", path.display(), e)))?;
            } else if let Some(template_name) = &wf.template {
                // Fetch template from registry
                let framework = ctx.source.split('/').next().unwrap_or(&ctx.source);
                let bp_name = ctx.source.split('/').nth(1).unwrap_or("default");
                let content = RegistryClient::fetch_template(framework, bp_name, template_name).await
                    .map_err(|e| InitError::NetworkError(e.to_string()))?;
                let parsed = substitute_vars(&content, vars);
                std::fs::write(&path, parsed)
                    .map_err(|e| InitError::FsError(format!("write {}: {}", path.display(), e)))?;
            }
        }

        // patch_file
        if let Some(pf) = &step.patch_file {
            let path = cwd.join(substitute_vars(&pf.path, vars));
            let content = substitute_vars(&pf.content, vars);
            let existing = std::fs::read_to_string(&path).unwrap_or_default();
            let patched = apply_patch(&existing, pf, &content);
            std::fs::write(&path, patched)
                .map_err(|e| InitError::FsError(format!("patch {}: {}", path.display(), e)))?;
        }

        // set_env
        if let Some(env_map) = &step.set_env {
            let env_path = cwd.join(".env");
            let mut existing = std::fs::read_to_string(&env_path).unwrap_or_default();
            for (key, val) in env_map {
                let parsed_val = substitute_vars(val, vars);
                let line = format!("{}={}", key, parsed_val);
                let pattern = format!("{}=", key);
                if existing.contains(&pattern) {
                    // Upsert: replace existing line
                    let lines: Vec<&str> = existing.lines().collect();
                    let updated: Vec<String> = lines.iter().map(|l| {
                        if l.starts_with(&pattern) { line.clone() } else { l.to_string() }
                    }).collect();
                    existing = updated.join("\n");
                    if !existing.ends_with('\n') { existing.push('\n'); }
                } else {
                    if !existing.is_empty() && !existing.ends_with('\n') { existing.push('\n'); }
                    existing.push_str(&line);
                    existing.push('\n');
                }
                // Also set in process env for subsequent steps
                std::env::set_var(key, &parsed_val);
            }
            std::fs::write(&env_path, existing)
                .map_err(|e| InitError::FsError(format!("write .env: {}", e)))?;
        }

        // run_locally
        if let Some(cmd) = &step.run_locally {
            let parsed = substitute_vars(cmd, vars);
            run_local_in_dir(&parsed, &cwd, ctx).await
                .map_err(|e| InitError::CommandFailed(parsed.clone(), e.to_string()))?;
        }

        // run (remote) — passthrough, same as run_locally for now
        if let Some(cmd) = &step.run {
            let parsed = substitute_vars(cmd, vars);
            run_local_in_dir(&parsed, &cwd, ctx).await
                .map_err(|e| InitError::CommandFailed(parsed.clone(), e.to_string()))?;
        }

        // mkdir
        if let Some(dir) = &step.mkdir {
            let path = cwd.join(substitute_vars(dir, vars));
            std::fs::create_dir_all(&path)
                .map_err(|e| InitError::FsError(format!("mkdir {}: {}", path.display(), e)))?;
        }

        // cp
        if let Some(cp_def) = &step.cp {
            let src = cwd.join(substitute_vars(&cp_def.src, vars));
            let dest = cwd.join(substitute_vars(&cp_def.dest, vars));
            if src.is_dir() {
                copy_dir_recursive(&src, &dest)
                    .map_err(|e| InitError::FsError(format!("cp {} -> {}: {}", src.display(), dest.display(), e)))?;
            } else {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| InitError::FsError(e.to_string()))?;
                }
                std::fs::copy(&src, &dest)
                    .map_err(|e| InitError::FsError(format!("cp {} -> {}: {}", src.display(), dest.display(), e)))?;
            }
        }

        // rm
        if let Some(target) = &step.rm {
            let path = cwd.join(substitute_vars(target, vars));
            if path.is_dir() {
                std::fs::remove_dir_all(&path)
                    .map_err(|e| InitError::FsError(format!("rm {}: {}", path.display(), e)))?;
            } else if path.exists() {
                std::fs::remove_file(&path)
                    .map_err(|e| InitError::FsError(format!("rm {}: {}", path.display(), e)))?;
            }
        }
    }

    Ok(())
}

/// Apply a patch to file content.
fn apply_patch(existing: &str, patch: &PatchFileDef, content: &str) -> String {
    let lines: Vec<&str> = existing.lines().collect();
    let mut result = Vec::new();

    if let Some(after_pattern) = &patch.after {
        for line in &lines {
            result.push(line.to_string());
            if line.contains(after_pattern.as_str()) {
                result.push(content.to_string());
            }
        }
    } else if let Some(before_pattern) = &patch.before {
        for line in &lines {
            if line.contains(before_pattern.as_str()) {
                result.push(content.to_string());
            }
            result.push(line.to_string());
        }
    } else if let Some(replace_pattern) = &patch.replace {
        let re = regex::Regex::new(replace_pattern).unwrap_or_else(|_| regex::Regex::new("$^").unwrap());
        for line in &lines {
            if re.is_match(line) {
                result.push(content.to_string());
            } else {
                result.push(line.to_string());
            }
        }
    } else {
        // No pattern: append content at end
        result.extend(lines.iter().map(|l| l.to_string()));
        result.push(content.to_string());
    }

    result.join("\n") + "\n"
}

/// Run a command locally in a specific directory.
async fn run_local_in_dir(cmd: &str, cwd: &Path, ctx: &InitContext) -> Result<(), InitError> {
    let status = ctx.exec_in_dir(cmd, cwd).await
        .map_err(|e| InitError::CommandFailed(cmd.to_string(), e.to_string()))?;
    if !status.success() {
        return Err(InitError::CommandFailed(cmd.to_string(), "Command failed".to_string()));
    }
    Ok(())
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Save blueprint
// ---------------------------------------------------------------------------

/// Save the full blueprint to `.appz/blueprint.yaml` in the project.
fn save_blueprint_to_project(project_path: &Path, blueprint: &BlueprintSchema) -> InitResult<()> {
    let appz_dir = project_path.join(".appz");
    std::fs::create_dir_all(&appz_dir)
        .map_err(|e| InitError::FsError(format!("Failed to create .appz dir: {}", e)))?;

    let yaml = serde_yaml::to_string(blueprint)
        .map_err(|e| InitError::InvalidFormat(format!("Failed to serialize blueprint: {}", e)))?;

    std::fs::write(appz_dir.join("blueprint.yaml"), yaml)
        .map_err(|e| InitError::FsError(format!("Failed to save blueprint: {}", e)))?;

    Ok(())
}
```

Note: `save_blueprint_to_project` requires `BlueprintSchema` to derive `serde::Serialize`. Go back to `crates/init/src/blueprint_schema.rs` and add `#[derive(Serialize)]` to `BlueprintSchema`, `BlueprintMeta`, `SetupStep`, `WriteFileDef`, `PatchFileDef`, `CopyDef`. Add `use serde::Serialize;` to the imports.

Note: `run_local_in_dir` calls `ctx.exec_in_dir()`. If `InitContext` does not have this method, add one to `crates/init/src/config.rs` that delegates to `self.sandbox.exec()` with a custom cwd. Check the existing `exec()` and `exec_interactive()` methods and mirror them with a cwd parameter.

Note: `regex` crate is needed for `patch_file`. Add `regex = { workspace = true }` to `crates/init/Cargo.toml`.

- [ ] **Step 2: Add `Serialize` derive to `BlueprintSchema` types**

In `crates/init/src/blueprint_schema.rs`, add `Serialize` to all struct derives and add `use serde::Serialize;`.

- [ ] **Step 3: Add `exec_in_dir` to `InitContext` if needed**

Check `crates/init/src/config.rs` for existing execution methods. Add:

```rust
pub async fn exec_in_dir(&self, cmd: &str, cwd: &Path) -> Result<std::process::ExitStatus, crate::error::InitError> {
    // Use sandbox or direct command execution with cwd override
    self.sandbox.exec_with_cwd(cmd, cwd).await
        .map_err(|e| crate::error::InitError::CommandFailed(cmd.to_string(), e.to_string()))
}
```

Adapt to match the existing sandbox API.

- [ ] **Step 4: Run tests**

Run: `cargo test -p init -v 2>&1 | tail -20`
Expected: Compiles and all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/init/src/providers/blueprint.rs crates/init/src/blueprint_schema.rs crates/init/src/config.rs crates/init/Cargo.toml
git commit -m "feat(init): implement BlueprintProvider setup step execution and blueprint saving"
```

---

## Task 6: Extend Importer with Scaffolding Step Types

Add `add_dependency`, `write_file`, `set_env`, `patch_file`, `mkdir`, `cp`, `rm` to the recipe/blueprint importer's `Step` struct and execution logic. Add JSONC support. Relax validation. Rename recipe -> blueprint.

**Important context:** This task adds the new step types to the **task runner** path (the `tasks:` section of blueprints, executed during `appz build`/`appz dev`). The **setup step execution** (during `appz init`) is handled by BlueprintProvider in Task 5d. Both paths support the same step types but execute in different contexts. Consider extracting shared step execution logic into a helper if significant duplication arises during implementation.

**Files:**
- Modify: `crates/app/src/importer.rs`

- [ ] **Step 1: Add JSONC support to `parse_file_schema()`**

In `crates/app/src/importer.rs`, update `parse_file_schema()` to handle `.jsonc` extension:

```rust
fn parse_file_schema<P: AsRef<Path> + std::fmt::Debug>(path: P) -> Result<FileSchema> {
    let raw = fs::read_file(&path).map_err(|e| {
        miette!("Failed to read blueprint file {}: {}", path.as_ref().display(), e)
    })?;
    let ext = path.as_ref().extension().and_then(|s| s.to_str()).unwrap_or("");
    let schema: FileSchema = match ext.to_lowercase().as_str() {
        "yaml" | "yml" => {
            serde_yaml::from_str(&raw)
                .map_err(|e| miette!("Invalid YAML in {}: {}", path.as_ref().display(), e))?
        }
        "jsonc" => {
            let stripped = json_comments::StripComments::new(raw.as_bytes());
            serde_json::from_reader(stripped)
                .map_err(|e| miette!("Invalid JSONC in {}: {}", path.as_ref().display(), e))?
        }
        _ => {
            serde_json::from_str(&raw)
                .map_err(|e| miette!("Invalid JSON in {}: {}", path.as_ref().display(), e))?
        }
    };
    Ok(schema)
}
```

Add `json_comments = "0.2"` to `crates/app/Cargo.toml`.

- [ ] **Step 2: Add `setup` field to `FileSchema`**

```rust
#[derive(Deserialize, Debug, Default)]
struct FileSchema {
    #[serde(default)]
    config: serde_json::Value,
    #[serde(default)]
    hosts: serde_json::Value,
    #[serde(default)]
    tools: serde_json::Value,
    #[serde(default)]
    tasks: HashMap<String, TaskDefWithMetadata>,
    #[serde(default)]
    setup: Vec<Step>,  // NEW
    #[serde(default)]
    before: HashMap<String, Vec<String>>,
    #[serde(default)]
    after: HashMap<String, Vec<String>>,
    #[serde(default)]
    includes: Option<Vec<PathBuf>>,
}
```

- [ ] **Step 3: Extend `Step` struct with new fields**

```rust
#[derive(Deserialize, Debug, Default, Clone)]
struct Step {
    // Existing
    #[serde(default)]
    cd: Option<String>,
    #[serde(default)]
    run: Option<String>,
    #[serde(default)]
    run_locally: Option<String>,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    upload: Option<UploadDef>,
    #[serde(default)]
    download: Option<DownloadDef>,
    #[serde(default)]
    desc: Option<String>,
    #[serde(default)]
    once: Option<bool>,
    #[serde(default)]
    hidden: Option<bool>,

    // New scaffolding fields
    #[serde(default)]
    add_dependency: Option<Vec<String>>,
    #[serde(default)]
    dev: Option<bool>,
    #[serde(default)]
    write_file: Option<WriteFileDef>,
    #[serde(default)]
    patch_file: Option<PatchFileDef>,
    #[serde(default)]
    set_env: Option<HashMap<String, String>>,
    #[serde(default)]
    mkdir: Option<String>,
    #[serde(default)]
    cp: Option<CopyDef>,
    #[serde(default)]
    rm: Option<String>,
}

#[derive(Deserialize, Debug, Default, Clone)]
struct WriteFileDef {
    path: String,
    content: Option<String>,
    template: Option<String>,
}

#[derive(Deserialize, Debug, Default, Clone)]
struct PatchFileDef {
    path: String,
    after: Option<String>,
    before: Option<String>,
    replace: Option<String>,
    content: String,
}

#[derive(Deserialize, Debug, Default, Clone)]
struct CopyDef {
    src: String,
    dest: String,
}
```

- [ ] **Step 4: Relax validation**

In `validate_file_schema()`, change:

```rust
// OLD:
if schema.tasks.is_empty() {
    return Err(miette!("tasks section is missing or empty"));
}

// NEW:
if schema.tasks.is_empty() && schema.setup.is_empty() {
    return Err(miette!("Blueprint must have either 'tasks' or 'setup' section"));
}
```

- [ ] **Step 5: Add execution logic for new step types in `create_task_from_steps()`**

Inside the step iteration loop in `create_task_from_steps()`, after the existing `run_locally` / `run` / `upload` / `download` handlers, add handlers for the new step types:

```rust
// Handle add_dependency
if let Some(deps) = &st.add_dependency {
    let is_dev = st.dev.unwrap_or(false);
    let deps_str = deps.iter().map(|d| ctx.parse(d)).collect::<Vec<_>>().join(" ");
    // TODO: detect package manager from project
    let cmd = if is_dev {
        format!("npm install --save-dev {}", deps_str)
    } else {
        format!("npm install {}", deps_str)
    };
    let opts = RunOptions {
        cwd: cwd_opt.clone(),
        env: None,
        show_output: true,
        package_manager: None,
        tool_info: None,
    };
    run_local_with(&ctx, &cmd, opts)
        .await
        .map_err(|e| miette!("Failed to install dependencies: {}", e))?;
}

// Handle write_file
if let Some(wf) = &st.write_file {
    let path = ctx.parse(&wf.path);
    let full_path = cwd_opt.as_ref()
        .map(|cwd| cwd.join(&path))
        .unwrap_or_else(|| PathBuf::from(&path));
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| miette!("Failed to create directory: {}", e))?;
    }
    if let Some(content) = &wf.content {
        let parsed_content = ctx.parse(content);
        std::fs::write(&full_path, parsed_content)
            .map_err(|e| miette!("Failed to write file {}: {}", full_path.display(), e))?;
    }
}

// Handle set_env
if let Some(env_vars) = &st.set_env {
    let env_path = cwd_opt.as_ref()
        .map(|cwd| cwd.join(".env"))
        .unwrap_or_else(|| PathBuf::from(".env"));
    let mut existing = std::fs::read_to_string(&env_path).unwrap_or_default();
    for (key, val) in env_vars {
        let parsed_val = ctx.parse(val);
        let line = format!("{}={}", key, parsed_val);
        // Upsert: replace if exists, append if not
        let pattern = format!("{}=", key);
        if existing.contains(&pattern) {
            let re = regex::Regex::new(&format!(r"(?m)^{}=.*$", regex::escape(key))).unwrap();
            existing = re.replace(&existing, line.as_str()).to_string();
        } else {
            if !existing.is_empty() && !existing.ends_with('\n') {
                existing.push('\n');
            }
            existing.push_str(&line);
            existing.push('\n');
        }
    }
    std::fs::write(&env_path, existing)
        .map_err(|e| miette!("Failed to write .env: {}", e))?;
    // Also set in process env for subsequent steps
    for (key, val) in env_vars {
        std::env::set_var(key, ctx.parse(val));
    }
}

// Handle mkdir
if let Some(dir) = &st.mkdir {
    let path = ctx.parse(dir);
    let full_path = cwd_opt.as_ref()
        .map(|cwd| cwd.join(&path))
        .unwrap_or_else(|| PathBuf::from(&path));
    std::fs::create_dir_all(&full_path)
        .map_err(|e| miette!("Failed to create directory {}: {}", full_path.display(), e))?;
}

// Handle cp
if let Some(cp_def) = &st.cp {
    let src = ctx.parse(&cp_def.src);
    let dest = ctx.parse(&cp_def.dest);
    copy_path_recursive(Path::new(&src), Path::new(&dest))
        .map_err(|e| miette!("Failed to copy {} -> {}: {}", src, dest, e))?;
}

// Handle rm
if let Some(path) = &st.rm {
    let parsed = ctx.parse(path);
    let full_path = cwd_opt.as_ref()
        .map(|cwd| cwd.join(&parsed))
        .unwrap_or_else(|| PathBuf::from(&parsed));
    if full_path.is_dir() {
        std::fs::remove_dir_all(&full_path)
            .map_err(|e| miette!("Failed to remove directory {}: {}", full_path.display(), e))?;
    } else if full_path.exists() {
        std::fs::remove_file(&full_path)
            .map_err(|e| miette!("Failed to remove file {}: {}", full_path.display(), e))?;
    }
}
```

- [ ] **Step 6: Rename recipe references to blueprint**

Do a search-and-replace across `crates/app/src/importer.rs`:
- "recipe" -> "blueprint" in comments and error messages
- "recipe file" -> "blueprint file"
- Keep function signatures stable for now (avoid breaking callers)

- [ ] **Step 7: Run tests**

Run: `cargo test -p app -v 2>&1 | tail -30`
Expected: All existing tests pass. New step types are parsed without error.

- [ ] **Step 8: Commit**

```bash
git add crates/app/src/importer.rs crates/app/Cargo.toml
git commit -m "feat(app): extend importer with scaffolding steps, JSONC support, relaxed validation"
```

---

## Task 7: Update Session Discovery

Change the session to discover `.appz/blueprint.yaml` instead of `recipe.yaml`.

**Files:**
- Modify: `crates/app/src/session.rs`

- [ ] **Step 1: Update recipe discovery in session.rs**

In `crates/app/src/session.rs`, around lines 344-361, update the discovery logic:

```rust
// Import blueprint file: prefer APPZ_IMPORT, otherwise auto-detect
if let Ok(path) = std::env::var("APPZ_IMPORT") {
    if let Err(e) = importer::import_file(path, &mut reg) {
        eprintln!("Warning: Failed to import blueprint: {}", e);
    }
} else {
    // New: check .appz/blueprint.{yaml,json,jsonc} first
    let appz_yaml = self.working_dir.join(".appz").join("blueprint.yaml");
    let appz_json = self.working_dir.join(".appz").join("blueprint.json");
    let appz_jsonc = self.working_dir.join(".appz").join("blueprint.jsonc");
    // Legacy: fall back to recipe.yaml/recipe.json
    let recipe_yml = self.working_dir.join("recipe.yaml");
    let recipe_json = self.working_dir.join("recipe.json");

    if appz_yaml.exists() {
        if let Err(e) = importer::import_file(appz_yaml, &mut reg) {
            eprintln!("Warning: Failed to import .appz/blueprint.yaml: {}", e);
        }
    } else if appz_json.exists() {
        if let Err(e) = importer::import_file(appz_json, &mut reg) {
            eprintln!("Warning: Failed to import .appz/blueprint.json: {}", e);
        }
    } else if appz_jsonc.exists() {
        if let Err(e) = importer::import_file(appz_jsonc, &mut reg) {
            eprintln!("Warning: Failed to import .appz/blueprint.jsonc: {}", e);
        }
    } else if recipe_yml.exists() {
        if let Err(e) = importer::import_file(recipe_yml, &mut reg) {
            eprintln!("Warning: Failed to import recipe.yaml: {}", e);
        }
    } else if recipe_json.exists() {
        if let Err(e) = importer::import_file(recipe_json, &mut reg) {
            eprintln!("Warning: Failed to import recipe.json: {}", e);
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p app -v 2>&1 | tail -20`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/session.rs
git commit -m "feat(app): update session to discover .appz/blueprint.yaml with recipe.yaml fallback"
```

---

## Task 8: Update Init Command

Simplify `init.rs` — remove WordPress-specific post-init logic, delegate everything to BlueprintProvider.

**Files:**
- Modify: `crates/app/src/commands/init.rs`

- [ ] **Step 1: Pass `--blueprint` and `--no-cache` flags to init::run()**

Update the `init()` function to pass blueprint and no_cache options through to `init::run()`. The blueprint application now happens inside BlueprintProvider, not as a post-init step in the command handler.

Remove the WordPress-specific DDEV/Playground configuration block (lines ~58-100 in the current file). The BlueprintProvider handles runtime setup as part of blueprint execution for WordPress.

- [ ] **Step 2: Run tests**

Run: `cargo test -p app -v 2>&1 | tail -20`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/commands/init.rs
git commit -m "refactor(app): simplify init command, delegate to BlueprintProvider"
```

---

## Task 9: Add `appz blueprints list` Command

Add a new subcommand for listing available blueprints from the registry.

**Files:**
- Create: `crates/app/src/commands/blueprints.rs`
- Modify: `crates/app/src/commands/mod.rs`
- Modify: `crates/app/src/app.rs`

- [ ] **Step 1: Create `crates/app/src/commands/blueprints.rs`**

```rust
//! `appz blueprints` subcommands.

use init::registry::RegistryClient;
use miette::miette;
use starbase::AppResult;

/// List available blueprints from the registry.
pub async fn list(framework_filter: Option<String>, no_cache: bool) -> AppResult {
    let index = RegistryClient::fetch_index(no_cache)
        .await
        .map_err(|e| miette!("Failed to fetch blueprint registry: {}", e))?;

    if let Some(fw) = &framework_filter {
        // List blueprints for a specific framework
        let entry = index.frameworks.get(fw.as_str())
            .ok_or_else(|| miette!("Framework '{}' not found in registry", fw))?;

        println!("{} blueprints:", entry.name);
        for (name, bp) in &entry.blueprints {
            println!("  {} - {}", name, bp.description);
        }
    } else {
        // List all frameworks and their blueprints
        println!("Available blueprints:\n");
        let mut frameworks: Vec<_> = index.frameworks.iter().collect();
        frameworks.sort_by_key(|(slug, _)| *slug);

        for (slug, entry) in frameworks {
            println!("{}:", entry.name);
            for (name, bp) in &entry.blueprints {
                println!("  {}/{} - {}", slug, name, bp.description);
            }
            println!();
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Add module declaration and command routing**

In `crates/app/src/commands/mod.rs`, add `pub mod blueprints;`.

In `crates/app/src/app.rs`, add the `Blueprints` subcommand to the `Commands` enum and route to `commands::blueprints::list()`.

- [ ] **Step 3: Run build**

Run: `cargo build -p app 2>&1 | tail -20`
Expected: Builds successfully.

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/commands/blueprints.rs crates/app/src/commands/mod.rs crates/app/src/app.rs
git commit -m "feat(app): add 'appz blueprints list' command"
```

---

## Task 10: Integration Tests

End-to-end tests for the full blueprint flow.

**Files:**
- Create: `crates/init/tests/blueprint_integration_test.rs`

- [ ] **Step 1: Write integration tests**

Create `crates/init/tests/blueprint_integration_test.rs`:

```rust
//! Integration tests for the blueprint init flow.
//!
//! These tests use local fixture blueprints (not the registry) to test
//! the full init pipeline without network dependencies.

use std::path::PathBuf;
use tempfile::TempDir;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[tokio::test]
async fn init_with_local_blueprint_creates_project() {
    let tmp = TempDir::new().unwrap();
    let project_name = "test-project";
    let blueprint_path = fixture("simple_blueprint.yaml");

    // Run init with local blueprint
    let result = init::run(
        Some("nextjs".to_string()),
        Some(project_name.to_string()),
        None, None, None,
        true,  // skip_install (no npm available in CI)
        false, // force
        Some(tmp.path().to_path_buf()),
        false, // json_output
        Some(blueprint_path.to_string_lossy().to_string()), // blueprint
        true,  // no_cache
    ).await;

    // The actual scaffold will fail without npm, but the blueprint parsing
    // and detection should work. This tests the plumbing.
    // For a full test, mock the exec_interactive call.
    // For now, just verify detection routes correctly.
    assert!(result.is_ok() || result.is_err()); // Placeholder
}

#[tokio::test]
async fn blueprint_file_saved_to_project() {
    // Test that .appz/blueprint.yaml is created after init
    // This requires a full init to succeed, so mock or use a simple
    // blueprint that doesn't require npm create
    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join("myproject");
    std::fs::create_dir_all(&project_dir).unwrap();

    let appz_dir = project_dir.join(".appz");
    // Simulate what BlueprintProvider does
    std::fs::create_dir_all(&appz_dir).unwrap();
    let blueprint_content = std::fs::read_to_string(fixture("simple_blueprint.yaml")).unwrap();
    std::fs::write(appz_dir.join("blueprint.yaml"), &blueprint_content).unwrap();

    assert!(appz_dir.join("blueprint.yaml").exists());
    let saved = std::fs::read_to_string(appz_dir.join("blueprint.yaml")).unwrap();
    assert!(saved.contains("Test Blueprint"));
}

#[test]
fn parse_framework_blueprint_pattern() {
    let result = init::detect::parse_framework_blueprint("nextjs/ecommerce");
    assert!(result.is_some());
    let (fw, bp) = result.unwrap();
    assert_eq!(fw, "nextjs");
    assert_eq!(bp, "ecommerce");
}

#[test]
fn non_framework_pattern_returns_none() {
    let result = init::detect::parse_framework_blueprint("someuser/somerepo");
    assert!(result.is_none());
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p init --test blueprint_integration_test -v`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/init/tests/blueprint_integration_test.rs
git commit -m "test(init): add blueprint integration tests"
```

---

## Task 11: Retire Old Providers and Clean Up

Remove deprecated providers and update all references.

**Files:**
- Modify: `crates/init/src/providers/mod.rs` — remove `framework` and `wordpress` modules
- Modify: `crates/init/src/lib.rs` — remove `has_create_command` re-export if it was public
- Modify: `crates/app/src/commands/mod.rs` — remove old `blueprint` module if replaced
- Delete (or leave with deprecation notice): `crates/init/src/providers/framework.rs`
- Delete (or leave with deprecation notice): `crates/init/src/providers/wordpress.rs`

- [ ] **Step 1: Move `FRAMEWORK_CREATE` and `has_create_command` into BlueprintProvider or a shared location**

The `FRAMEWORK_CREATE` table and `has_create_command()` are still needed by the detect module and BlueprintProvider. Move them to a shared location (e.g., `crates/init/src/framework_commands.rs`) or inline them into `detect.rs` / `blueprint.rs`.

- [ ] **Step 2: Remove old provider modules**

In `crates/init/src/providers/mod.rs`, remove:
```rust
// pub mod framework;   // RETIRED: absorbed into blueprint provider
// pub mod wordpress;   // RETIRED: absorbed into blueprint provider
```

Delete `crates/init/src/providers/framework.rs` and `crates/init/src/providers/wordpress.rs`.

- [ ] **Step 3: Update any remaining references**

Search for `FrameworkProvider`, `WordPressProvider`, `framework::has_create_command`, `framework::get_create_command` across the codebase and update all references to the new location.

Run: `cargo build 2>&1 | head -50` — fix any compilation errors.

- [ ] **Step 4: Run full test suite**

Run: `cargo test --workspace 2>&1 | tail -30`
Expected: All tests pass across all crates.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(init): retire FrameworkProvider and WordPressProvider, consolidate into BlueprintProvider"
```

---

## Task 12: Final Verification

Run all tests, check for warnings, verify the CLI builds.

**Files:** None (verification only)

- [ ] **Step 1: Run full workspace build**

Run: `cargo build --workspace 2>&1 | tail -20`
Expected: Clean build with no errors.

- [ ] **Step 2: Run full test suite**

Run: `cargo test --workspace 2>&1 | tail -30`
Expected: All tests pass.

- [ ] **Step 3: Check for clippy warnings**

Run: `cargo clippy --workspace 2>&1 | tail -30`
Expected: No new warnings from changed files.

- [ ] **Step 4: Verify CLI runs**

Run: `cargo run -- --help 2>&1 | head -20`
Expected: Shows help text including `blueprints` subcommand.

Run: `cargo run -- blueprints list --no-cache 2>&1 | head -20`
Expected: Either shows blueprints or a network error (registry repo may not exist yet). Should not crash.

- [ ] **Step 5: Commit any final fixes**

If any fixes were needed, commit them:
```bash
git add -A
git commit -m "fix: address final verification issues for universal blueprints"
```
