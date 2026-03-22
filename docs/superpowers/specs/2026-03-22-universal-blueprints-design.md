# Universal Blueprint System

**Date:** 2026-03-22
**Status:** Draft

## Overview

Replace the existing recipe system and WordPress-only blueprint system with a unified blueprint system that supports all frameworks. Blueprints define both project scaffolding (init-time setup) and operational tasks (build, dev, deploy). A GitHub-hosted registry (`appz-blueprints`) provides community blueprints for every supported framework.

## Goals

- Every `appz init` goes through the blueprint system — no separate FrameworkProvider or WordPressProvider
- Blueprints are the single format for scaffolding + operations, replacing recipes
- Users can reference blueprints via shorthand (`appz init nextjs/ecommerce`), flag (`--blueprint`), local file, or URL
- WordPress Playground blueprint JSON files remain supported via an on-the-fly converter
- Supports YAML, JSON, and JSONC file formats

## Non-Goals

- Blueprint versioning / migration (future work)
- Visual blueprint editor / GUI
- Blueprint generation from existing projects (except WordPress Playground compat)

---

## Blueprint File Format

The blueprint format extends the current recipe schema with scaffolding fields.

```yaml
# blueprint.yaml
meta:
  name: "Next.js E-commerce Starter"
  description: "Next.js with Tailwind, Prisma, and Stripe"
  author: "appz-community"
  framework: "nextjs"
  categories: ["ecommerce", "fullstack"]

config:
  db_provider: "postgresql"
  stripe_key: ""

tools:
  node: "20"
  prisma: "latest"

# Scaffolding steps — executed during init
setup:
  - desc: "Install Tailwind CSS"
    add_dependency: ["tailwindcss", "postcss", "autoprefixer"]
    dev: true

  - desc: "Install Prisma"
    add_dependency: ["prisma"]
    dev: true

  - desc: "Install runtime deps"
    add_dependency: ["@prisma/client", "stripe", "@stripe/stripe-js"]

  - desc: "Initialize Tailwind config"
    run_locally: "npx tailwindcss init -p"

  - desc: "Initialize Prisma"
    run_locally: "npx prisma init"

  - desc: "Create env file"
    write_file:
      path: ".env"
      content: |
        DATABASE_URL="{{db_provider}}://localhost:5432/myapp"
        STRIPE_SECRET_KEY="{{stripe_key}}"

  - desc: "Create Prisma schema"
    write_file:
      path: "prisma/schema.prisma"
      content: |
        generator client {
          provider = "prisma-client-js"
        }
        datasource db {
          provider = "{{db_provider}}"
          url      = env("DATABASE_URL")
        }

  - desc: "Set env var"
    set_env:
      NEXT_PUBLIC_STRIPE: "{{stripe_key}}"

# Operational tasks (same as current recipes)
tasks:
  build:
    - run_locally: "npm run build"
  dev:
    - run_locally: "npm run dev"
  db:migrate:
    - run_locally: "npx prisma migrate dev"

before:
  build:
    - db:migrate
```

### Top-Level Fields

| Field | Purpose | Inherited From |
|---|---|---|
| `meta` | Blueprint metadata (name, description, author, framework, categories) | New |
| `config` | Variable definitions for `{{var}}` substitution | Recipe |
| `tools` | Tool versions to install via mise | Recipe |
| `setup` | Ordered scaffolding steps, executed during `appz init` | New |
| `tasks` | Named operational tasks (build, deploy, dev, etc.) | Recipe |
| `before` | Pre-task hooks | Recipe |
| `after` | Post-task hooks | Recipe |
| `includes` | Include other blueprint/task files | Recipe |

### Setup Step Types

| Step Field | Description |
|---|---|
| `run_locally` | Execute a local command (existing) |
| `run` | Execute a remote command (existing) |
| `add_dependency` | Install packages via detected package manager. `dev: true` for dev deps. |
| `write_file` | Create/overwrite a file. Fields: `path`, `content`. Supports `{{var}}` substitution. |
| `set_env` | Append key-value pairs to `.env` file |
| `mkdir` | Create a directory |
| `cp` | Copy files. Fields: `src`, `dest` |
| `rm` | Remove a file or directory |
| `cd` | Change working directory for subsequent steps (existing) |
| `upload` / `download` | File transfer (existing) |
| `desc` | Human-readable step description (existing) |
| `once` | Run only once / idempotent (existing) |

### Package Manager Detection for `add_dependency`

- `yarn.lock` present -> yarn
- `pnpm-lock.yaml` present -> pnpm
- `bun.lockb` present -> bun
- `composer.json` present -> composer
- Default -> npm

---

## CLI Resolution & Source Detection

### Updated Detection Priority

1. `npm:` prefix -> NpmProvider
2. Known framework slug with `/blueprint` (e.g., `nextjs/ecommerce`) -> BlueprintProvider
3. Known framework slug alone (e.g., `nextjs`) -> BlueprintProvider (uses `default` blueprint)
4. Archive URLs (.zip, .tar.gz, etc.) -> RemoteArchiveProvider
5. Git URLs / `user/repo` (where first segment is NOT a known framework) -> GitProvider
6. Local paths (`./`, `../`, `/`) -> LocalProvider

### Resolution Examples

```
appz init nextjs/ecommerce          -> BlueprintProvider (nextjs/ecommerce/blueprint.yaml)
appz init nextjs                    -> BlueprintProvider (nextjs/default/blueprint.yaml)
appz init nextjs --blueprint ecommerce  -> BlueprintProvider (nextjs/ecommerce/blueprint.yaml)
appz init nextjs --blueprint ./my.yaml  -> BlueprintProvider (local file)
appz init nextjs --blueprint https://..  -> BlueprintProvider (remote URL)
appz init wordpress                 -> BlueprintProvider (wordpress/default/blueprint.yaml)
appz init someuser/somerepo         -> GitProvider (not a known framework)
appz init npm:create-foo            -> NpmProvider
```

### Disambiguation: `framework/blueprint` vs `user/repo`

The first segment is checked against the known framework registry. If it matches a framework slug, route to BlueprintProvider. Otherwise, route to GitProvider.

---

## Blueprint Registry (GitHub)

### Repository Structure

Single monorepo: `appz-blueprints`

```
appz-blueprints/
  registry.json
  nextjs/
    default/
      blueprint.yaml
    ecommerce/
      blueprint.yaml
      templates/
        schema.prisma
    blog/
      blueprint.yaml
  astro/
    default/
      blueprint.yaml
    docs/
      blueprint.yaml
  wordpress/
    default/
      blueprint.yaml
    woocommerce/
      blueprint.yaml
  laravel/
    default/
      blueprint.yaml
    api/
      blueprint.yaml
  ...
```

### registry.json

Lightweight index fetched by the CLI to discover available blueprints:

```json
{
  "version": 1,
  "frameworks": {
    "nextjs": {
      "name": "Next.js",
      "blueprints": {
        "default": { "description": "Base Next.js setup" },
        "ecommerce": { "description": "Next.js + Tailwind + Prisma + Stripe" },
        "blog": { "description": "Next.js + MDX blog starter" }
      }
    },
    "astro": {
      "name": "Astro",
      "blueprints": {
        "default": { "description": "Base Astro setup" },
        "docs": { "description": "Astro Starlight documentation site" }
      }
    }
  }
}
```

### Fetching Flow

1. CLI fetches `registry.json` (cached locally with TTL)
2. Validates that framework and blueprint exist in registry
3. Fetches `{framework}/{blueprint}/blueprint.yaml` via GitHub raw URL
4. If blueprint references template files (`templates/`), fetches those too

### CLI Commands

```bash
appz blueprints list                # List all frameworks and blueprints
appz blueprints list nextjs         # List blueprints for a framework
```

---

## WordPress Playground Blueprint Compatibility

### Detection

A blueprint file is identified as WordPress Playground format if it is JSON and contains any of:
- `steps` array with WordPress-specific step types
- Top-level `plugins`, `siteOptions`, or `preferredVersions` fields
- `$schema` pointing to WordPress Playground

### Conversion Mapping

| Playground Step | Converted To |
|---|---|
| `installPlugin: {slug: "woo"}` | `run_locally: "wp plugin install woo --activate"` |
| `installTheme: {slug: "astra"}` | `run_locally: "wp theme install astra --activate"` |
| `activatePlugin` | `run_locally: "wp plugin activate X"` |
| `activateTheme` | `run_locally: "wp theme activate X"` |
| `setSiteOptions: {k: v}` | `run_locally: "wp option update k v"` |
| `defineWpConfigConsts` | `write_file` patching `wp-config.php` |
| `setSiteLanguage` | `run_locally: "wp language core install X --activate"` |
| `runPHP` / `runPHPWithOptions` | `run_locally: "wp eval 'code'"` or temp PHP file |
| `runSql` | `run_locally: "wp db query 'sql'"` |
| `writeFile` / `writeFiles` | `write_file` step |
| `mkdir`, `cp`, `mv`, `rm`, `rmdir` | Equivalent `run_locally` commands |
| `wp-cli` | `run_locally: "wp ..."` |
| Top-level `plugins` shorthand | Multiple `wp plugin install` steps |
| Top-level `constants` shorthand | `write_file` for `wp-config.php` |
| Top-level `login: true` | `run_locally: "wp user create ..."` or no-op |

### Implementation

The existing `crates/blueprint/` crate is repurposed as the Playground compatibility layer. Its current types stay for parsing Playground JSON. A new `convert_to_generic()` function outputs the new blueprint format.

Usage remains transparent:

```bash
appz init wordpress --blueprint ./playground-blueprint.json
# Detects Playground format, converts, executes through the same pipeline
```

---

## Execution Pipeline

Full flow for `appz init nextjs/ecommerce`:

1. **Parse source** -> BlueprintProvider("nextjs", "ecommerce")
2. **Fetch blueprint** from registry (or local file / URL)
3. **Detect format** -> if Playground JSON, convert to generic format
4. **Parse blueprint** YAML/JSON/JSONC
5. **Run base framework scaffolding** (`meta.framework` -> `npm create`, `composer create-project`, etc.)
6. **Execute `setup` steps** sequentially:
   - `add_dependency` -> detect package manager and install
   - `write_file` -> write to project directory with `{{var}}` substitution
   - `set_env` -> append to `.env` file
   - `run_locally` -> execute in project directory
   - `mkdir` / `cp` / `rm` -> filesystem operations
7. **Install tools** from `tools:` section (via mise)
8. **Save full blueprint** to `.appz/blueprint.yaml`
9. **Register `tasks`** from blueprint into task runner (for `appz dev/build/deploy`)

### Step Execution

Setup steps reuse the existing `Step` struct from the recipe importer, extended with new fields:

```rust
#[derive(Deserialize, Debug, Default, Clone)]
struct Step {
    // Existing fields
    cd: Option<String>,
    run_locally: Option<String>,
    run: Option<String>,
    host: Option<String>,
    upload: Option<UploadDef>,
    download: Option<DownloadDef>,
    desc: Option<String>,
    once: Option<bool>,
    hidden: Option<bool>,

    // New scaffolding fields
    add_dependency: Option<Vec<String>>,
    dev: Option<bool>,
    write_file: Option<WriteFileDef>,
    set_env: Option<HashMap<String, String>>,
    mkdir: Option<String>,
    cp: Option<CopyDef>,
    rm: Option<String>,
}
```

### Project File

The full blueprint is saved to `.appz/blueprint.yaml` in the project. On subsequent runs (`appz dev`, `appz build`), the session loads this file and registers only the `tasks` section — `setup` steps are skipped since they've already been executed.

---

## Crate Changes

| Crate | Change |
|---|---|
| `crates/init/` | Remove `FrameworkProvider`, `WordPressProvider`. Add `BlueprintProvider`. Update `detect.rs` resolution logic. |
| `crates/blueprint/` | Repurpose as Playground compatibility layer. Keep types for parsing Playground JSON. Add `convert_to_generic()`. |
| `crates/app/src/importer.rs` | Extend `Step` struct with new scaffolding fields. Add execution logic for each. Rename "recipe" references to "blueprint". |
| `crates/app/src/commands/` | Update `init.rs` to use BlueprintProvider. Add `blueprints.rs` command (`list`). Retire `blueprint.rs` (old WP-only commands). |
| `crates/app/src/session.rs` | Change recipe discovery to look for `.appz/blueprint.yaml` instead of `recipe.yaml`. |
| `crates/task/` | No changes — the task runner stays as-is. |

### New Modules

**`crates/init/src/providers/blueprint.rs`** — BlueprintProvider:
- Fetch from GitHub registry (with local cache)
- Fetch from local file or URL
- Detect Playground JSON and delegate to converter
- Parse YAML/JSON/JSONC
- Run base framework scaffold
- Execute `setup` steps via task runner
- Save full blueprint to `.appz/blueprint.yaml`

**`crates/app/src/registry.rs`** — Registry client:
- Fetch and cache `registry.json`
- Resolve `framework/blueprint` to a URL
- List available blueprints

### Files to Retire

- `crates/init/src/providers/wordpress.rs` — absorbed into default WordPress blueprint
- `crates/init/src/providers/framework.rs` — absorbed into BlueprintProvider
- `crates/app/src/commands/blueprint.rs` — replaced by new `blueprints.rs`
- `crates/app/src/recipe/` — renamed/merged into blueprint logic
- `recipes/` directory — migrated into `appz-blueprints` repo

---

## Testing Strategy

### Unit Tests

- **Blueprint parsing** — YAML, JSON, JSONC files parse correctly. Invalid files produce clear errors.
- **Source detection** — `nextjs/ecommerce` routes to BlueprintProvider, `nextjs` routes to BlueprintProvider with `default`, `someuser/repo` routes to GitProvider.
- **Playground converter** — each WordPress Playground step type converts to the correct generic steps. Full blueprint conversion round-trips correctly.
- **Step execution** — `add_dependency` calls the right package manager, `write_file` handles `{{var}}` substitution, `set_env` appends correctly.
- **Registry parsing** — `registry.json` deserializes correctly, framework/blueprint lookup works.

### Integration Tests

- `appz init nextjs` — fetches default blueprint, scaffolds project, setup steps run, `.appz/blueprint.yaml` saved.
- `appz init nextjs/ecommerce` — fetches named blueprint, full pipeline works.
- `appz init nextjs --blueprint ./local.yaml` — local blueprint file works.
- `appz init wordpress --blueprint ./playground.json` — Playground JSON detected, converted, executed.
- `appz blueprints list` — fetches registry, displays frameworks and blueprints.
- Task runner integration — after init, `appz build` / `appz dev` pick up tasks from `.appz/blueprint.yaml`.

### Test Fixtures

- Sample blueprints for 2-3 frameworks (nextjs, astro, wordpress)
- Sample WordPress Playground JSON for conversion testing
- Mock `registry.json`
