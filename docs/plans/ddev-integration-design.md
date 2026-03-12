# DDEV Integration Design

Add DDEV support to appz-cli similar to mise: use DDEV for PHP/CMS project environments, supporting all DDEV project types (WordPress, Drupal, Laravel, TYPO3, Backdrop, CakePHP, Magento, Symfony, etc.).

## Reference

- [DDEV Documentation](https://docs.ddev.com/en/stable/)
- [DDEV CMS Quickstarts](https://docs.ddev.com/en/stable/users/quickstart/)
- [DDEV Installation](https://docs.ddev.com/en/stable/users/install/ddev-installation/)
- [DDEV Project Types](https://docs.ddev.com/en/stable/developers/project-types/)

## Current State

- **mise**: Used for Node, Hugo, Bun, etc. via `SandboxSettings::with_tool()` and `MiseManager`
- **DDEV (Jigsaw only)**: Hardcoded in `app/src/commands/dev.rs` for `framework.slug == "jigsaw"` — runs `ddev config --project-type=php`, `ddev start`
- **PHP frameworks**: WordPress, Sculpin, Spress, Kirby, Statamic use bare `php -S localhost:8000` or Composer commands
- **Init**: WordPress provider downloads from wordpress.org; no DDEV scaffolding

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                     DDEV Integration (parallel to mise)               │
├─────────────────────────────────────────────────────────────────────┤
│  Tool layer     │ mise: node, hugo, bun  │ ddev: PHP/CMS runtime     │
│  Init layer     │ Framework/create-command│ DDEV project types        │
│  Dev layer      │ npm run dev, hugo serve │ ddev start + ddev launch  │
│  Build layer    │ pnpm build, hugo -D     │ ddev exec composer install│
└─────────────────────────────────────────────────────────────────────┘
```

## Framework → DDEV Project Type Mapping

| Framework slug | DDEV --project-type | Docroot (if non-default) |
|----------------|---------------------|---------------------------|
| wordpress | wordpress | . |
| drupal, drupal9, drupal10 | drupal10 | web (or detected) |
| laravel | laravel | public |
| typo3 | typo3 | public |
| backdrop | backdrop | . |
| cakephp | cakephp | webroot |
| magento, magento2 | magento2 | . |
| symfony | symfony | public |
| codeigniter | codeigniter | public |
| jigsaw | php | build_local (or build) |
| sculpin, spress, kirby, statamic | php | varies |

## Phase 1: DDEV Tool & Helpers (Foundation)

### 1.1 Add DDEV to mise tools

- Add `ddev` as an installable tool via mise: `mise use -g ddev`
- Extend `mise_tools_for_execution` (or add `ddev_tools_for_execution`) so PHP frameworks can request DDEV
- Create `crates/app/src/ddev_helpers.rs`:
  - `is_ddev_available() -> bool`
  - `ddev_project_type_for_framework(slug: &str) -> Option<&str>`
  - `ddev_supported_frameworks() -> Vec<&str>`
  - `ensure_ddev_config(project_path, project_type, docroot?) -> Result`

### 1.2 Framework metadata

- Add optional `ddev_project_type` to `frameworks/data/php-frameworks.json` (and frameworks.json for Laravel/Symfony if present)
- Or: maintain a static map in Rust: `DDEV_PROJECT_TYPES: &[(&str, &str)]`

## Phase 2: Init Integration

### 2.1 WordPress init + DDEV (auto)

- After init for a DDEV-supported template (wordpress, drupal, etc.), automatically run `ddev config` when DDEV is available
- No flag needed — we know from the template which frameworks are DDEV-supported
- If DDEV is not installed, show a tip: "Install DDEV for local PHP development: ..."

### 2.2 DDEV-based init providers (future)

- `ddev wordpress` → `ddev config --project-type=wordpress` + download WordPress
- `ddev drupal` → `ddev config --project-type=drupal10` + composer create-project
- These could be alternative init sources (e.g. `appz init ddev:wordpress`)

## Phase 3: Dev Command Integration

### 3.1 Unify DDEV handling

- Replace Jigsaw-specific block with a generic DDEV branch for all `ddev_supported_frameworks()`
- Flow:
  1. Detect framework
  2. If framework in DDEV supported list:
     a. Check `command_exists("ddev")`; if missing, suggest install
     b. If no `.ddev/config.yaml`, run `ddev config --project-type=X`
     c. Run `ddev start`
     d. For dev server: use `ddev launch` (opens browser) or `ddev exec <framework_dev_cmd>` if framework needs a long-running process
  3. Else: use existing mise/sandbox flow

### 3.2 Dev server behavior

- **Option A**: `ddev launch` — opens browser; DDEV serves the site. No need to run `php -S` inside.
- **Option B**: `ddev exec php -S 0.0.0.0:8000` — run PHP built-in server inside container
- DDEV already serves the docroot via its web container; `ddev launch` is typically sufficient. For frameworks that need `npm run watch` + PHP, we may need `ddev exec` for the PHP part.

## Phase 4: Build Command Integration

- For PHP projects with DDEV: run install/build via `ddev exec`:
  - `ddev exec composer install`
  - `ddev exec vendor/bin/sculpin generate` (Sculpin)
- Sandbox `exec` could delegate to `ddev exec` when project has `.ddev` and framework is PHP.

## Implementation Order

1. **Phase 1.1–1.2**: DDEV helpers + framework mapping ✅ DONE
2. **Phase 3.1**: Extend dev command to use DDEV for all supported PHP frameworks ✅ DONE
3. **Phase 2.1**: Auto-add DDEV to init for supported frameworks ✅ DONE (no flag)
4. **Phase 4**: Build command DDEV support ✅ DONE (build.rs uses same generic DDEV flow)
5. **Phase 2.2**: Additional DDEV init sources (optional)

## DDEV Installation

DDEV can be installed:

- **Via mise**: `mise use -g ddev` (mise has a ddev recipe)
- **Standalone**: brew, chocolatey, apt, etc. — see [DDEV Installation](https://docs.ddev.com/en/stable/users/install/ddev-installation/)

Appz will **not** auto-install DDEV (unlike mise); we will check `which ddev` and inform the user to install if missing. Rationale: DDEV depends on Docker, which is a heavier dependency than Node/Python.

## Files to Create/Modify

| File | Change |
|------|--------|
| `crates/app/src/ddev_helpers.rs` | New: DDEV availability, project type mapping, config helper |
| `crates/app/src/sandbox_helpers.rs` | Extend: add `ddev_tools_for_execution` or integrate DDEV into mise_tools when PHP |
| `crates/app/src/commands/dev.rs` | Refactor: generic DDEV branch for all supported PHP frameworks |
| `crates/app/src/commands/init.rs` | Auto-add DDEV for supported templates |
| `crates/init/src/providers/wordpress.rs` | Optional post-init hook for DDEV config |
| `crates/frameworks/data/php-frameworks.json` | Add `ddevProjectType` per framework |
| `docs/plans/ddev-integration-design.md` | This document |

## Testing

- Manual: `appz init wordpress --name wp-test --ddev` → verify .ddev/config.yaml
- Manual: `appz dev` in WordPress project with .ddev → verify ddev start + launch
- CI: Add `ddev` to CI only for DDEV-related tests (optional; Docker-in-Docker may be needed)
