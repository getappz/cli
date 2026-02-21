# Hypermix Crate for Appz Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Port the TypeScript Hypermix tool (appz-ref/hypermix) into a new Rust crate `crates/hypermix` exposed as `appz hypermix`, enabling multi-source AI context building via Repomix orchestration, with init/add/run/uninstall workflows and token-aware output.

**Architecture:** A new `hypermix` crate loads a config file (`hypermix.config.json` or `.jsonc`), runs Repomix per mix (remote GitHub or local repomix config), updates ignore files (.gitignore, .cursorignore, .cursorignoreindex), and reports token counts. Uses sandbox for `npx repomix@latest` (same pattern as code-mix).

**Tech Stack:** Rust, sandbox crate, serde_json, jsonc, reqwest, tokio, miette, clap. Reference: appz-ref/hypermix.

---

## Reference Files

| Purpose | Path |
|---------|------|
| Config load | appz-ref/hypermix/src/load-config.ts |
| Main run flow | appz-ref/hypermix/src/mod.ts (lines 356-451) |
| Ignore file logic | appz-ref/hypermix/src/mod.ts `modifyIgnoreFile`, `updateIgnoreFiles` |
| Init | appz-ref/hypermix/src/init.ts |
| Add repo | appz-ref/hypermix/src/add-mix.ts |
| Constants/templates | appz-ref/hypermix/src/constants.ts |
| Repomix invocation | crates/code-mix/src/repomix.rs |

---

## Task 1: Create hypermix crate scaffold

**Files:**
- Create: `crates/hypermix/Cargo.toml`
- Create: `crates/hypermix/src/lib.rs`
- Modify: `Cargo.toml` (workspace members)

**Step 1: Add workspace member**

Add `crates/hypermix` to the `members` array in `Cargo.toml`:

```toml
"crates/hypermix",
```

**Step 2: Create Cargo.toml**

Create `crates/hypermix/Cargo.toml`:

```toml
[package]
name = "hypermix"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
sandbox = { path = "../sandbox" }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
miette = { workspace = true }
reqwest = { workspace = true }
json_comments = "0.2"
```

**Step 3: Create lib.rs**

Create `crates/hypermix/src/lib.rs`:

```rust
//! Multi-source AI context builder via Repomix orchestration.
//!
//! Loads hypermix.config.json, runs Repomix per mix (remote or local),
//! updates ignore files, and reports token counts.

mod config;
mod types;

pub use config::load_config;
pub use types::{HypermixConfig, MixConfig};
```

**Step 4: Create stub modules**

Create `crates/hypermix/src/types.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixConfig {
    pub remote: Option<String>,
    pub include: Option<Vec<String>>,
    pub ignore: Option<Vec<String>>,
    pub output: Option<String>,
    #[serde(alias = "repomixConfig")]
    pub repomix_config: Option<String>,
    #[serde(alias = "extraFlags")]
    pub extra_flags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypermixConfig {
    pub mixes: Vec<MixConfig>,
    pub silent: Option<bool>,
    #[serde(alias = "outputPath")]
    pub output_path: Option<String>,
}
```

Create `crates/hypermix/src/config.rs`:

```rust
use std::path::{Path, PathBuf};

use miette::IntoDiagnostic;

use crate::HypermixConfig;

const CONFIG_NAMES: &[&str] = &["hypermix.config.json", "hypermix.config.jsonc"];

/// Find and load hypermix config. Searches cwd then --config path.
pub fn load_config(config_path: Option<&Path>, cwd: &Path) -> miette::Result<(PathBuf, HypermixConfig)> {
    let path = match config_path {
        Some(p) => p.to_path_buf(),
        None => {
            let found = CONFIG_NAMES
                .iter()
                .map(|n| cwd.join(n))
                .find(|p| p.exists())
                .ok_or_else(|| miette::miette!(
                    "No config file found. Expected one of: {}",
                    CONFIG_NAMES.join(", ")
                ))?;
            found
        }
    };

    let content = std::fs::read_to_string(&path).into_diagnostic()?;
    let config = if path.extension().map_or(false, |e| e == "jsonc") {
        let stripped = json_comments::StripComments::new(content.as_bytes());
        serde_json::from_reader(stripped).into_diagnostic()?
    } else {
        serde_json::from_str(&content).into_diagnostic()?
    };

    Ok((path, config))
}
```

**Step 5: Verify build**

Run: `cd /home/avihs/workspace/appz-cli && CARGO_TARGET_DIR="$PWD/target" cargo build -p hypermix`
Expected: Build succeeds.

**Step 6: Commit**

```bash
git add Cargo.toml crates/hypermix/
git commit -m "feat(hypermix): add crate scaffold with config loader"
```

---

## Task 2: Implement Repomix runner in hypermix

**Files:**
- Create: `crates/hypermix/src/repomix.rs`
- Modify: `crates/hypermix/src/lib.rs`
- Test: `crates/hypermix/src/repomix.rs` (unit test in doc comment or inline)

**Step 1: Add repomix module**

Create `crates/hypermix/src/repomix.rs`:

```rust
//! Run Repomix for a single mix (remote or local) via sandbox.

use std::path::Path;

use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};
use sandbox::SandboxProvider;

use crate::MixConfig;

/// Result of running Repomix for one mix.
pub struct RepomixResult {
    pub success: bool,
    pub repo_name: String,
    pub output_path: std::path::PathBuf,
}

/// Run Repomix for one mix. Uses sandbox with Node 22.
pub async fn run_mix(
    workdir: &Path,
    mix: &MixConfig,
    output_dir: &Path,
) -> miette::Result<Option<RepomixResult>> {
    let config = SandboxConfig::new(workdir)
        .with_settings(SandboxSettings::default().with_tool("node", Some("22")));
    let sandbox = create_sandbox(config)
        .await
        .map_err(|e| miette::miette!("Failed to create sandbox: {}", e))?;

    let mut args: Vec<String> = vec!["npx repomix@latest".to_string()];

    if let Some(ref remote) = mix.remote {
        let url = if remote.starts_with("http") {
            remote.clone()
        } else {
            format!("https://github.com/{}", remote)
        };
        args.push("--remote".to_string());
        args.push(url);
    }

    let includes = mix.include.as_deref().unwrap_or(&["**/*"]);
    args.push("--include".to_string());
    args.push(includes.join(","));

    if let Some(ref ign) = mix.ignore {
        if !ign.is_empty() {
            args.push("-i".to_string());
            args.push(ign.join(","));
        }
    }

    if let Some(ref cfg) = mix.repomix_config {
        args.push("--config".to_string());
        args.push(cfg.clone());
    }

    args.push("--remove-empty-lines".to_string());
    args.push("--compress".to_string());
    args.push("--quiet".to_string());
    args.push("--parsable-style".to_string());

    if let Some(ref flags) = mix.extra_flags {
        args.extend(flags.clone());
    }

    let output_file = output_dir.join(
        mix.output
            .as_deref()
            .unwrap_or_else(|| {
                mix.remote
                    .as_ref()
                    .map(|r| format!("{}.xml", r.replace('/', "-")))
                    .unwrap_or_else(|| "codebase.xml".to_string())
            }),
    );
    std::fs::create_dir_all(output_dir).map_err(|e| miette::miette!("Failed to create output dir: {}", e))?;
    args.push("-o".to_string());
    args.push(output_file.display().to_string());

    let cmd = args.join(" ");
    let status = sandbox
        .exec_interactive(&cmd)
        .await
        .map_err(|e| miette::miette!("{}", e))?;

    if !status.success() {
        return Ok(None);
    }

    let repo_name = mix
        .remote
        .as_ref()
        .and_then(|r| r.split('/').last().map(String::from))
        .unwrap_or_else(|| "local".to_string());

    Ok(Some(RepomixResult {
        success: true,
        repo_name,
        output_path: output_file,
    }))
}
```

**Step 2: Wire into lib.rs**

In `crates/hypermix/src/lib.rs`, add:

```rust
mod repomix;
pub use repomix::{run_mix, RepomixResult};
```

**Step 3: Verify build**

Run: `cd /home/avihs/workspace/appz-cli && CARGO_TARGET_DIR="$PWD/target" cargo build -p hypermix`
Expected: Build succeeds.

**Step 4: Commit**

```bash
git add crates/hypermix/src/repomix.rs crates/hypermix/src/lib.rs
git commit -m "feat(hypermix): add Repomix runner via sandbox"
```

---

## Task 3: Implement ignore file updates

**Files:**
- Create: `crates/hypermix/src/ignore.rs`
- Modify: `crates/hypermix/src/lib.rs`

**Step 1: Create ignore module**

Create `crates/hypermix/src/ignore.rs`:

```rust
//! Update .gitignore, .cursorignore, .cursorignoreindex for hypermix output.

use std::path::Path;

/// Append pattern to file if not already present. Create file if missing.
fn modify_ignore_file(
    file_path: &Path,
    pattern: &str,
    should_exist: bool,
    content_to_add: &str,
) -> miette::Result<()> {
    use std::io::Write;

    if !file_path.exists() {
        if should_exist {
            std::fs::write(file_path, content_to_add.trim())?;
        }
        return Ok(());
    }

    let content = std::fs::read_to_string(file_path)?;
    // Simple duplicate check: pattern already in file?
    let pattern_exists = content.contains(pattern);

    if (should_exist && !pattern_exists) || (!should_exist && pattern_exists) {
        let mut f = std::fs::OpenOptions::new().append(true).open(file_path)?;
        f.write_all(content_to_add.as_bytes())?;
    }

    Ok(())
}

/// Update .gitignore, .cursorignore, .cursorignoreindex for output path.
pub fn update_ignore_files(cwd: &Path, output_path: &Path) -> miette::Result<()> {
    let relative = pathdiff::diff_paths(output_path, cwd)
        .unwrap_or_else(|| output_path.to_path_buf());
    let normalized = relative.to_string_lossy().replace('\\', "/");
    let context_pattern = format!("{}/**/*.xml", normalized);

    modify_ignore_file(
        &cwd.join(".gitignore"),
        &context_pattern,
        true,
        &format!("\n# AI context files\n{}\n", context_pattern),
    )?;

    modify_ignore_file(
        &cwd.join(".cursorignoreindex"),
        &context_pattern,
        true,
        &format!("\n# AI context files\n{}\n", context_pattern),
    )?;

    let negated = format!("!{}", context_pattern);
    modify_ignore_file(
        &cwd.join(".cursorignore"),
        &negated,
        true,
        &format!("\n# Include AI context files (auto-generated by hypermix)\n!{}\n", context_pattern),
    )?;

    Ok(())
}
```

**Step 2: Add dependencies**

In `crates/hypermix/Cargo.toml`, add:

```toml
pathdiff = "0.2"
```

**Step 3: Wire into lib**

In `crates/hypermix/src/lib.rs`:

```rust
mod ignore;
pub use ignore::update_ignore_files;
```

**Step 4: Verify build**

Run: `cargo build -p hypermix`
Expected: Build succeeds.

**Step 5: Commit**

```bash
git add crates/hypermix/
git commit -m "feat(hypermix): add ignore file updates"
```

---

## Task 4: Implement token counting (chars/4 fallback)

**Files:**
- Create: `crates/hypermix/src/tokens.rs`
- Modify: `crates/hypermix/src/lib.rs`

**Step 1: Create tokens module**

Create `crates/hypermix/src/tokens.rs`:

```rust
//! Token counting for generated XML files.
//! Uses chars/4 approximation (can upgrade to tiktoken-rs later).

use std::path::Path;

/// Count approximate tokens: chars / 4 (GPT-4 style).
pub fn count_tokens(content: &str) -> u64 {
    (content.len() as u64 + 3) / 4
}

/// Count tokens per file. Returns (filename -> tokens, total).
pub fn count_tokens_in_files(paths: &[impl AsRef<Path>]) -> miette::Result<(Vec<(String, u64)>, u64)> {
    let mut results = Vec::new();
    let mut total = 0u64;

    for p in paths {
        let path = p.as_ref();
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let tokens = count_tokens(&content);
            let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            results.push((name, tokens));
            total += tokens;
        }
    }

    Ok((results, total))
}
```

**Step 2: Wire into lib**

Add to lib.rs:

```rust
mod tokens;
pub use tokens::{count_tokens, count_tokens_in_files};
```

**Step 3: Verify build**

Run: `cargo build -p hypermix`
Expected: Build succeeds.

**Step 4: Commit**

```bash
git add crates/hypermix/src/tokens.rs crates/hypermix/src/lib.rs
git commit -m "feat(hypermix): add token counting (chars/4)"
```

---

## Task 5: Implement run command (core flow)

**Files:**
- Create: `crates/hypermix/src/run.rs`
- Modify: `crates/hypermix/src/lib.rs`

**Step 1: Create run module**

Create `crates/hypermix/src/run.rs`:

```rust
//! Main run flow: load config, run repomix per mix, update ignores, print tokens.

use std::path::Path;

use crate::config::load_config;
use crate::ignore::update_ignore_files;
use crate::repomix::run_mix;
use crate::tokens::count_tokens_in_files;
use crate::HypermixConfig;

pub async fn run(
    cwd: &Path,
    config_path: Option<&Path>,
) -> miette::Result<()> {
    let (_config_path, config) = load_config(config_path, cwd)?;
    let output_path = config
        .output_path
        .as_deref()
        .unwrap_or(".hypermix");
    let output_dir = cwd.join(output_path);

    let mut results = Vec::new();
    for mix in &config.mixes {
        if let Some(r) = run_mix(cwd, mix, &output_dir).await? {
            if r.output_path.exists() {
                results.push(r.output_path);
            }
        }
    }

    if results.is_empty() {
        return Err(miette::miette!(
            "No valid output files created. Check config and ensure repomix is available (npx repomix@latest --version)"
        ));
    }

    update_ignore_files(cwd, &output_dir)?;

    let (file_tokens, total) = count_tokens_in_files(&results)?;

    // Print table
    eprintln!("┌────────────────┬─────────────┐");
    eprintln!("│ File           │ Tokens      │");
    eprintln!("├────────────────┼─────────────┤");
    for (name, tokens) in &file_tokens {
        eprintln!("│ {:14} │ {:>11} │", name, format_number(*tokens));
    }
    eprintln!("└────────────────┴─────────────┘");
    eprintln!("Total Tokens: {}", format_number(total));

    let high = file_tokens.iter().any(|(_, t)| *t >= 60_000);
    if high {
        eprintln!("⚠️  One or more files exceed 60k tokens. Consider adding --compress to config.");
    }

    Ok(())
}

fn format_number(n: u64) -> String {
    n.to_string().chars().rev().collect::<Vec<_>>()
        .chunks(3)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(",")
        .chars()
        .rev()
        .collect()
}
```

**Step 2: Fix config module visibility**

In `crates/hypermix/src/lib.rs`, ensure `config` is accessible from `run`:

```rust
mod config;
mod ignore;
mod repomix;
mod tokens;
mod types;
mod run;

pub use config::load_config;  // if config is not pub, use crate::config in run
pub use run::run;
// ... rest
```

Actually, `load_config` is in config.rs. In run.rs we use `crate::config::load_config`. But config is `mod config` (private). We need to either make config pub or expose load_config. We already have `pub use config::load_config` so run can use `crate::load_config` or we need to fix the module. Let me use `crate::load_config` in run.rs.

**Step 3: Fix run.rs imports**

Use:

```rust
use crate::load_config;
```

**Step 4: Wire run into lib**

```rust
mod run;
pub use run::run;
```

**Step 5: Verify build**

Run: `cargo build -p hypermix`
Expected: Build succeeds.

**Step 6: Commit**

```bash
git add crates/hypermix/src/run.rs crates/hypermix/src/lib.rs
git commit -m "feat(hypermix): add run command"
```

---

## Task 6: Wire appz hypermix into app crate

**Files:**
- Modify: `crates/app/Cargo.toml`
- Modify: `crates/app/src/app.rs`
- Create: `crates/app/src/commands/hypermix.rs`
- Modify: `crates/app/src/commands/mod.rs`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/app/src/telemetry.rs`

**Step 1: Add hypermix dependency**

In `crates/app/Cargo.toml`, add:

```toml
hypermix = { path = "../hypermix", optional = true }
```

And in `[features]`:

```toml
hypermix = ["dep:hypermix"]
```

And add hypermix to default features if desired, or keep it opt-in.

**Step 2: Add Hypermix variant to Commands**

In `crates/app/src/app.rs`, add before `Code`:

```rust
/// Multi-source AI context builder (Repomix orchestration)
#[cfg(feature = "hypermix")]
Hypermix {
    #[command(subcommand)]
    command: crate::commands::hypermix::HypermixCommands,
},
```

**Step 3: Create hypermix commands module**

Create `crates/app/src/commands/hypermix.rs`:

```rust
//! appz hypermix: multi-source AI context builder.

use crate::session::AppzSession;
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand, Debug, Clone)]
pub enum HypermixCommands {
    /// Run hypermix (default: load config, run repomix per mix)
    Run {
        /// Config file path (default: hypermix.config.json or .jsonc in cwd)
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Override output directory
        #[arg(short, long)]
        output_path: Option<PathBuf>,
        /// Suppress non-error output
        #[arg(short, long)]
        silent: bool,
    },
}

pub async fn run(session: AppzSession, command: HypermixCommands) -> starbase::AppResult {
    match command {
        HypermixCommands::Run { config, .. } => {
            let cwd = session.working_dir.clone();
            hypermix::run(&cwd, config.as_deref().as_ref().map(|p| p.as_path()))
                .await
                .map_err(|e| miette::miette!("{}", e))?;
        }
    }
    Ok(None)
}
```

**Step 4: Register in commands/mod.rs**

Add:

```rust
#[cfg(feature = "hypermix")]
pub mod hypermix;
```

**Step 5: Add dispatch in main.rs**

In `crates/cli/src/main.rs`, add (before Code):

```rust
#[cfg(feature = "hypermix")]
Commands::Hypermix { command } => app::commands::hypermix::run(session, command).await,
```

**Step 6: Add to telemetry**

In `crates/app/src/telemetry.rs`, add:

```rust
#[cfg(feature = "hypermix")]
Commands::Hypermix { .. } => "hypermix",
```

**Step 7: Enable feature in workspace**

In root `Cargo.toml` or when building:

```bash
CARGO_TARGET_DIR="$PWD/target" cargo build --bin appz --features hypermix
```

Or add hypermix to app's default features temporarily for testing.

**Step 8: Verify**

Run: `cargo build --bin appz --features hypermix`
Expected: Build succeeds. Run `./target/debug/appz hypermix --help`.

**Step 9: Commit**

```bash
git add crates/app/ crates/cli/
git commit -m "feat(app): wire appz hypermix command"
```

---

## Task 7: Implement hypermix init

**Files:**
- Modify: `crates/hypermix/src/run.rs` → separate run logic
- Create: `crates/hypermix/src/init.rs`
- Modify: `crates/hypermix/src/lib.rs`
- Modify: `crates/app/src/commands/hypermix.rs`

**Step 1: Create init module**

Create `crates/hypermix/src/init.rs`:

```rust
//! Initialize hypermix: create config, repomix config, .hypermix dir, cursor rule.

use std::path::Path;

use crate::HypermixConfig;

const CURSOR_RULE_TEMPLATE: &str = r#"---
name: Hypermix Generated Files
description: This rule helps you understand the codebase generated by hypermix.
globs: {repomix_files_list}
---

# Understanding Your Hypermix Generated Files

The following files are generated by hypermix: {repomix_files_list}

They contain a structured representation of your code. Use them to understand
overall structure, find related code, and identify patterns."#;

/// Detect source folders (src, lib, etc.) excluding node_modules, .git, etc.
fn find_source_folders(cwd: &Path) -> Vec<String> {
    let exclude: std::collections::HashSet<_> = [
        "node_modules", ".git", "dist", "build", "target", ".hypermix", "test", "tests",
    ]
    .iter()
    .map(|s| *s)
    .collect();

    let mut folders = std::collections::HashSet::new();
    if let Ok(entries) = walkdir::WalkDir::new(cwd)
        .max_depth(3)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !exclude.contains(name.as_ref())
        })
    {
        for e in entries.flatten() {
            if e.file_type().is_file() {
                if let Ok(rel) = e.path().strip_prefix(cwd) {
                    if let Some(first) = rel.components().next() {
                        let name = first.as_os_str().to_string_lossy();
                        if !exclude.contains(name.as_ref()) {
                            folders.insert(name.into_owned());
                        }
                    }
                }
            }
        }
    }
    let mut v: Vec<_> = folders.into_iter().collect();
    v.sort();
    if v.is_empty() {
        v.push("src".to_string());
    }
    v
}

pub fn init(cwd: &Path) -> miette::Result<()> {
    // 1. Create hypermix.config.json
    let config_path = cwd.join("hypermix.config.json");
    let source_folders = find_source_folders(cwd);
    let includes: Vec<String> = source_folders.iter().map(|f| format!("{}/**/*", f)).collect();

    let config = HypermixConfig {
        mixes: vec![crate::MixConfig {
            remote: None,
            include: Some(includes),
            ignore: None,
            output: None,
            repomix_config: Some("repomix.config.json".to_string()),
            extra_flags: Some(vec!["--quiet".to_string()]),
        }],
        silent: Some(false),
        output_path: Some(".hypermix".to_string()),
    };

    let json = serde_json::to_string_pretty(&config)?;
    std::fs::write(&config_path, json)?;

    // 2. Create repomix.config.json
    let repomix_config = serde_json::json!({
        "output": { "include": source_folders.iter().map(|f| format!("{}/**/*", f)).collect::<Vec<_>>() }
    });
    std::fs::write(
        cwd.join("repomix.config.json"),
        serde_json::to_string_pretty(&repomix_config)?,
    )?;

    // 3. Create .hypermix dir
    std::fs::create_dir_all(cwd.join(".hypermix"))?;

    // 4. Create cursor rule
    let rule_dir = cwd.join(".cursor/rules/hypermix");
    std::fs::create_dir_all(&rule_dir)?;
    let output_file = ".hypermix/codebase.xml";
    let rule = CURSOR_RULE_TEMPLATE
        .replace("{repomix_files_list}", output_file);
    std::fs::write(rule_dir.join("cursor-rule.mdx"), rule)?;

    Ok(())
}
```

Add `walkdir` to hypermix Cargo.toml.

**Step 2: Fix MixConfig in types**

Ensure `MixConfig` has `repomix_config` field (we have it). Init creates one mix with `repomix_config` only.

**Step 3: Add init subcommand**

In `crates/app/src/commands/hypermix.rs`, add:

```rust
Init,
```

And in match:

```rust
HypermixCommands::Init => {
    hypermix::init(&session.working_dir).map_err(|e| miette::miette!("{}", e))?;
}
```

**Step 4: Export init from hypermix lib**

```rust
pub use init::init;
```

**Step 5: Verify build**

Run: `cargo build --bin appz --features hypermix`
Expected: Build succeeds.

**Step 6: Commit**

```bash
git add crates/hypermix/ crates/app/
git commit -m "feat(hypermix): add init command"
```

---

## Task 8: Implement hypermix add (GitHub repo)

**Files:**
- Create: `crates/hypermix/src/add.rs`
- Modify: `crates/hypermix/src/lib.rs`
- Modify: `crates/app/src/commands/hypermix.rs`

**Step 1: Add regex dependency**

In `crates/hypermix/Cargo.toml`, add: `regex = { workspace = true }`

**Step 2: Create add module**

Create `crates/hypermix/src/add.rs`:

```rust
//! Add a GitHub repo to hypermix config.

use std::path::Path;

use reqwest::blocking::Client;

use crate::config::load_config;
use crate::{HypermixConfig, MixConfig};

/// Validate GitHub repo and return owner/repo shorthand.
pub fn validate_github_repo(repo_input: &str) -> miette::Result<String> {
    let (shorthand, url) = if repo_input.starts_with("https://github.com/") {
        let m = regex::Regex::new(r"github\.com/([^/]+/[^/]+)")
            .unwrap()
            .captures(repo_input)
            .ok_or_else(|| miette::miette!("Invalid GitHub URL"))?;
        (m[1].to_string(), repo_input.to_string())
    } else if repo_input.contains('/') {
        (repo_input.to_string(), format!("https://github.com/{}", repo_input))
    } else {
        return Err(miette::miette!("Use owner/repo or full GitHub URL"));
    };

    let client = Client::new();
    let resp = client.head(&url).send().map_err(|e| miette::miette!("{}", e))?;
    if !resp.status().is_success() {
        return Err(miette::miette!("Repo {} not found or not accessible", shorthand));
    }

    Ok(shorthand)
}

/// Append mix to config and save.
pub fn add_repo(cwd: &Path, config_path: Option<&Path>, repo: &str) -> miette::Result<()> {
    let shorthand = validate_github_repo(repo)?;

    let (path, mut config) = load_config(config_path, cwd)?;
    if config.mixes.iter().any(|m| m.remote.as_deref() == Some(&shorthand)) {
        return Err(miette::miette!("Repository {} already in config", shorthand));
    }

    let new_mix = MixConfig {
        remote: Some(shorthand.clone()),
        include: Some(vec!["*.ts".to_string(), "*.js".to_string(), "*.md".to_string()]),
        ignore: None,
        output: Some(format!("{}.xml", shorthand.replace('/', "-"))),
        repomix_config: None,
        extra_flags: None,
    };
    config.mixes.push(new_mix);

    let json = serde_json::to_string_pretty(&config)?;
    std::fs::write(&path, json)?;

    Ok(())
}
```

**Step 3: Add Add subcommand**

In hypermix commands:

```rust
Add { repo: String },
```

Match arm:

```rust
HypermixCommands::Add { repo } => {
    hypermix::add_repo(&session.working_dir, None, &repo).map_err(|e| miette::miette!("{}", e))?;
    let _ = ui::status::success(&format!("Added {} to config", repo));
}
```

**Step 4: Export add from hypermix**

```rust
pub use add::{add_repo, validate_github_repo};
```

**Step 5: Verify build**

Run: `cargo build --bin appz --features hypermix`
Expected: Build succeeds.

**Step 6: Commit**

```bash
git add crates/hypermix/ crates/app/
git commit -m "feat(hypermix): add add command for GitHub repos"
```

---

## Task 9: Implement hypermix uninstall

**Files:**
- Create: `crates/hypermix/src/uninstall.rs`
- Modify: `crates/hypermix/src/lib.rs`
- Modify: `crates/app/src/commands/hypermix.rs`

**Step 1: Create uninstall module**

Create `crates/hypermix/src/uninstall.rs`:

```rust
//! Remove hypermix scripts and rules from project.

use std::path::Path;

pub fn uninstall(cwd: &Path) -> miette::Result<()> {
    // Remove .cursor/rules/hypermix/
    let rule_dir = cwd.join(".cursor/rules/hypermix");
    if rule_dir.exists() {
        std::fs::remove_dir_all(&rule_dir)?;
    }

    // Remove hypermix script from package.json
    let pkg_path = cwd.join("package.json");
    if pkg_path.exists() {
        let content = std::fs::read_to_string(&pkg_path)?;
        if let Ok(mut pkg) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(scripts) = pkg.get_mut("scripts").and_then(|s| s.as_object_mut()) {
                scripts.remove("hypermix");
                std::fs::write(&pkg_path, serde_json::to_string_pretty(&pkg)?)?;
            }
        }
    }

    // Remove hypermix task from deno.json
    for name in &["deno.json", "deno.jsonc"] {
        let path = cwd.join(name);
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let mut deno: serde_json::Value = if path.extension().map_or(false, |e| e == "jsonc") {
                let stripped = json_comments::StripComments::new(content.as_bytes());
                serde_json::from_reader(stripped).map_err(|_| miette::miette!("Invalid JSONC"))?
            } else {
                serde_json::from_str(&content)?
            };
            if let Some(tasks) = deno.get_mut("tasks").and_then(|t| t.as_object_mut()) {
                tasks.remove("hypermix");
                std::fs::write(&path, serde_json::to_string_pretty(&deno)?)?;
            }
        }
    }

    Ok(())
}
```

**Step 2: Add Uninstall subcommand**

```rust
Uninstall,
```

```rust
HypermixCommands::Uninstall => {
    hypermix::uninstall(&session.working_dir).map_err(|e| miette::miette!("{}", e))?;
    let _ = ui::status::success("Hypermix uninstalled.");
}
```

**Step 3: Export from hypermix**

```rust
pub use uninstall::uninstall;
```

**Step 4: Verify build**

Run: `cargo build --bin appz --features hypermix`
Expected: Build succeeds.

**Step 5: Commit**

```bash
git add crates/hypermix/ crates/app/
git commit -m "feat(hypermix): add uninstall command"
```

---

## Task 10: Add hypermix to default features and polish

**Files:**
- Modify: `crates/app/Cargo.toml` (add hypermix to default)
- Modify: `Cargo.toml` (workspace features if needed)
- Modify: `crates/hypermix/src/config.rs` (handle outputPath override in run)
- Modify: `crates/app/src/commands/hypermix.rs` (pass --config, --output-path, --silent to run)

**Step 1: Add hypermix to app default features**

In `crates/app/Cargo.toml`:

```toml
default = ["self_update", "dev-server", "deploy", "mcp", "gen", "hypermix"]
```

**Step 2: Pass run flags through**

In hypermix run, accept overrides for output_path and silent. Thread from CLI.

**Step 3: Smoke test**

In a temp dir:
```bash
appz hypermix init
# Creates hypermix.config.json, repomix.config.json, .hypermix, .cursor/rules/hypermix/
appz hypermix add rust-lang/rust
# Appends to config (will fail repomix for huge repo, but add should work)
appz hypermix
# Runs repomix per mix
```

**Step 4: Commit**

```bash
git add crates/app/
git commit -m "feat: enable hypermix by default, pass CLI flags"
```

---

## Summary

| Task | Description |
|------|-------------|
| 1 | Crate scaffold + config loader |
| 2 | Repomix runner via sandbox |
| 3 | Ignore file updates |
| 4 | Token counting |
| 5 | Run command (core flow) |
| 6 | Wire appz hypermix into app |
| 7 | Init command |
| 8 | Add command (GitHub) |
| 9 | Uninstall command |
| 10 | Default feature, polish |

---

## Execution Handoff

Plan complete and saved to `docs/plans/2025-02-20-hypermix-crate-for-appz.md`.

**Two execution options:**

**1. Subagent-Driven (this session)** – I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Parallel Session (separate)** – Open a new session with superpowers:executing-plans, batch execution with checkpoints.

Which approach?
