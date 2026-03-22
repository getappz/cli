//! Blueprint provider: initializes projects from universal blueprint definitions.
//!
//! Supports fetching blueprints from the GitHub registry, local files, or URLs,
//! detecting and converting WordPress Playground JSON, running base framework
//! scaffolding, executing setup steps, and saving the blueprint to disk.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use regex::Regex;
use tracing::{debug, info, warn};

use blueprint::converter::{
    convert_playground_to_generic, is_playground_blueprint, GenericBlueprint, GenericSetupStep,
};

use crate::blueprint_schema::{
    BlueprintMeta, BlueprintSchema, CopyDef, PatchFileDef, SetupStep, WriteFileDef,
};
use crate::config::InitContext;
use crate::detect::parse_framework_blueprint;
use crate::error::{InitError, InitResult};
use crate::output::InitOutput;
use crate::provider::InitProvider;
use crate::providers::framework::get_create_command;
use crate::registry::RegistryClient;
use crate::ui;

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

pub struct BlueprintProvider;

#[async_trait]
impl InitProvider for BlueprintProvider {
    fn name(&self) -> &str {
        "Blueprint"
    }

    fn slug(&self) -> &str {
        "blueprint"
    }

    async fn init(&self, ctx: &InitContext) -> InitResult<InitOutput> {
        // 1. Resolve framework + blueprint name, fetch the blueprint YAML
        let (framework_slug, blueprint_name, schema) = resolve_and_fetch(ctx).await?;

        debug!(
            framework = %framework_slug,
            blueprint = %blueprint_name,
            "Blueprint resolved"
        );

        // Dry-run: print what would happen and exit
        if ctx.options.dry_run {
            return print_dry_run(&schema, &framework_slug, &blueprint_name, ctx);
        }

        // 2. Base scaffolding (framework create command)
        run_base_scaffolding(ctx, &schema, &framework_slug).await?;

        // 3. Detect package manager
        let pm = detect_package_manager(ctx, &schema);
        info!(package_manager = %pm, "Detected package manager");

        // 4. Extract config variables
        let vars = extract_config_vars(&schema);

        // 5. Execute setup steps
        if let Some(steps) = &schema.setup {
            execute_setup_steps(ctx, steps, &framework_slug, &blueprint_name, &pm, &vars)
                .await?;
        }

        // 6. Save blueprint to .appz/blueprint.yaml
        save_blueprint(ctx, &schema)?;

        let project_path = ctx.project_path();
        let framework = frameworks::find_by_slug(&framework_slug).map(|f| f.name.to_string());

        Ok(InitOutput {
            project_path,
            framework,
            installed: false,
        })
    }
}

// ---------------------------------------------------------------------------
// Dry-run preview
// ---------------------------------------------------------------------------

fn print_dry_run(
    schema: &BlueprintSchema,
    framework_slug: &str,
    blueprint_name: &str,
    ctx: &InitContext,
) -> InitResult<InitOutput> {
    println!("\n{}", "Dry run — no changes will be made\n");

    // Blueprint info
    if let Some(meta) = &schema.meta {
        println!("Blueprint: {} ({})",
            meta.name.as_deref().unwrap_or(blueprint_name),
            format!("{}/{}", framework_slug, blueprint_name));
        if let Some(desc) = &meta.description {
            println!("  {}", desc);
        }
    }

    // Scaffolding
    let create_cmd = schema.meta.as_ref()
        .and_then(|m| m.create_command.as_deref())
        .or_else(|| get_create_command(framework_slug));
    if let Some(cmd) = create_cmd {
        println!("\nScaffolding:");
        println!("  run: {} .", cmd);
    }

    // Package manager
    let pm = detect_package_manager(ctx, schema);
    println!("\nPackage manager: {}", pm);

    // Config variables
    let vars = extract_config_vars(schema);
    if !vars.is_empty() {
        println!("\nConfig variables:");
        for (key, val) in &vars {
            println!("  {{{{{}}}}} = \"{}\"", key, val);
        }
    }

    // Setup steps
    if let Some(steps) = &schema.setup {
        println!("\nSetup steps ({}):", steps.len());
        for (i, step) in steps.iter().enumerate() {
            let desc = step.desc.as_deref().unwrap_or("(unnamed)");
            println!("\n  {}. {}", i + 1, desc);

            if let Some(dir) = &step.cd {
                println!("     cd {}", substitute_vars(dir, &vars));
            }
            if let Some(deps) = &step.add_dependency {
                let dev_flag = if step.dev.unwrap_or(false) { " (dev)" } else { "" };
                println!("     add_dependency{}: {}", dev_flag, deps.join(", "));
                let cmd = install_command(&pm, deps, step.dev.unwrap_or(false));
                println!("     -> {}", substitute_vars(&cmd, &vars));
            }
            if let Some(wf) = &step.write_file {
                let path = substitute_vars(&wf.path, &vars);
                if wf.template.is_some() {
                    println!("     write_file: {} (from template)", path);
                } else {
                    println!("     write_file: {}", path);
                }
            }
            if let Some(pf) = &step.patch_file {
                let path = substitute_vars(&pf.path, &vars);
                println!("     patch_file: {}", path);
            }
            if let Some(env) = &step.set_env {
                for (key, val) in env {
                    println!("     set_env: {}={}", key, substitute_vars(val, &vars));
                }
            }
            if let Some(cmd) = &step.run_locally {
                println!("     run: {}", substitute_vars(cmd, &vars));
            }
            if let Some(dir) = &step.mkdir {
                println!("     mkdir: {}", substitute_vars(dir, &vars));
            }
            if let Some(cp) = &step.cp {
                println!("     cp: {} -> {}", substitute_vars(&cp.src, &vars), substitute_vars(&cp.dest, &vars));
            }
            if let Some(rm) = &step.rm {
                println!("     rm: {}", substitute_vars(rm, &vars));
            }
        }
    }

    // Tasks
    if let Some(tasks) = &schema.tasks {
        if let Some(obj) = tasks.as_object() {
            println!("\nTasks ({}):", obj.len());
            for name in obj.keys() {
                println!("  - {}", name);
            }
        }
    }

    println!();

    Ok(InitOutput {
        project_path: ctx.options.project_path(),
        framework: frameworks::find_by_slug(framework_slug).map(|f| f.name.to_string()),
        installed: false,
    })
}

// ---------------------------------------------------------------------------
// Source resolution and fetching
// ---------------------------------------------------------------------------

/// Returns (framework_slug, blueprint_name, parsed BlueprintSchema).
async fn resolve_and_fetch(
    ctx: &InitContext,
) -> InitResult<(String, String, BlueprintSchema)> {
    // Priority 1: --blueprint flag with local file or URL
    if let Some(bp_flag) = &ctx.options.blueprint {
        if is_local_file(bp_flag) {
            return load_local_blueprint(bp_flag, &ctx.source);
        }
        if bp_flag.starts_with("http://") || bp_flag.starts_with("https://") {
            return fetch_url_blueprint(bp_flag, &ctx.source).await;
        }
        // Treat as blueprint name in the registry for the given framework
        let framework_slug = ctx.source.clone();
        let blueprint_name = bp_flag.clone();
        let schema = fetch_registry_blueprint(
            &framework_slug,
            &blueprint_name,
            ctx.options.no_cache,
        )
        .await?;
        return Ok((framework_slug, blueprint_name, schema));
    }

    // Priority 2: source contains "/" → framework/blueprint
    if let Some((fw, bp)) = parse_framework_blueprint(&ctx.source) {
        let schema = fetch_registry_blueprint(&fw, &bp, ctx.options.no_cache).await?;
        return Ok((fw, bp, schema));
    }

    // Priority 3: bare framework slug → framework/default
    let framework_slug = ctx.source.clone();
    let blueprint_name = "default".to_string();
    let schema =
        fetch_registry_blueprint(&framework_slug, &blueprint_name, ctx.options.no_cache).await?;
    Ok((framework_slug, blueprint_name, schema))
}

fn is_local_file(s: &str) -> bool {
    s.starts_with("./")
        || s.starts_with("../")
        || s.starts_with('/')
        || (s.len() >= 2 && s.chars().nth(1) == Some(':'))
}

fn load_local_blueprint(
    path: &str,
    source: &str,
) -> InitResult<(String, String, BlueprintSchema)> {
    let raw =
        std::fs::read_to_string(path).map_err(|e| InitError::NotFound(format!("{path}: {e}")))?;

    // Check for WordPress Playground JSON
    if path.ends_with(".json") && is_playground_blueprint(&raw) {
        let generic = convert_playground_to_generic(&raw)
            .map_err(|e| InitError::InvalidFormat(e.to_string()))?;
        let schema = generic_to_schema(generic);
        let fw = schema
            .meta
            .as_ref()
            .and_then(|m| m.framework.clone())
            .unwrap_or_else(|| "wordpress".to_string());
        return Ok((fw, "local".to_string(), schema));
    }

    let schema: BlueprintSchema = if path.ends_with(".yaml") || path.ends_with(".yml") {
        serde_yaml::from_str(&raw).map_err(|e| InitError::InvalidFormat(e.to_string()))?
    } else {
        serde_json::from_str(&raw).map_err(|e| InitError::InvalidFormat(e.to_string()))?
    };

    let fw = schema
        .meta
        .as_ref()
        .and_then(|m| m.framework.clone())
        .unwrap_or_else(|| extract_framework_from_source(source));

    Ok((fw, "local".to_string(), schema))
}

async fn fetch_url_blueprint(
    url: &str,
    source: &str,
) -> InitResult<(String, String, BlueprintSchema)> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| InitError::DownloadFailed(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(InitError::DownloadFailed(format!(
            "HTTP {} from {url}",
            resp.status()
        )));
    }

    let raw = resp
        .text()
        .await
        .map_err(|e| InitError::DownloadFailed(e.to_string()))?;

    // Check for Playground JSON
    if is_playground_blueprint(&raw) {
        let generic = convert_playground_to_generic(&raw)
            .map_err(|e| InitError::InvalidFormat(e.to_string()))?;
        let schema = generic_to_schema(generic);
        let fw = schema
            .meta
            .as_ref()
            .and_then(|m| m.framework.clone())
            .unwrap_or_else(|| "wordpress".to_string());
        return Ok((fw, "url".to_string(), schema));
    }

    // Try YAML first, fall back to JSON
    let schema: BlueprintSchema = if let Ok(s) = serde_yaml::from_str(&raw) {
        s
    } else {
        serde_json::from_str(&raw)
            .map_err(|e| InitError::InvalidFormat(e.to_string()))?
    };

    let fw = schema
        .meta
        .as_ref()
        .and_then(|m| m.framework.clone())
        .unwrap_or_else(|| extract_framework_from_source(source));

    Ok((fw, "url".to_string(), schema))
}

async fn fetch_registry_blueprint(
    framework: &str,
    blueprint: &str,
    no_cache: bool,
) -> InitResult<BlueprintSchema> {
    let client = RegistryClient::new();

    // Validate against registry index (best-effort — don't fail if index fetch fails)
    match client.fetch_index(no_cache).await {
        Ok(index) => {
            if !index.has_framework(framework) {
                return Err(InitError::NotFound(format!(
                    "Framework '{framework}' not found in blueprint registry"
                )));
            }
            if !index.has_blueprint(framework, blueprint) {
                return Err(InitError::NotFound(format!(
                    "Blueprint '{framework}/{blueprint}' not found in registry"
                )));
            }
        }
        Err(e) => {
            warn!("Could not fetch registry index (continuing anyway): {e}");
        }
    }

    let raw = client
        .fetch_blueprint(framework, blueprint)
        .await
        .map_err(|e| InitError::DownloadFailed(e.to_string()))?;

    let schema: BlueprintSchema =
        serde_yaml::from_str(&raw).map_err(|e| InitError::InvalidFormat(e.to_string()))?;

    Ok(schema)
}

fn extract_framework_from_source(source: &str) -> String {
    if let Some((fw, _)) = parse_framework_blueprint(source) {
        fw
    } else {
        source.to_string()
    }
}

// ---------------------------------------------------------------------------
// GenericBlueprint → BlueprintSchema conversion
// ---------------------------------------------------------------------------

fn generic_to_schema(generic: GenericBlueprint) -> BlueprintSchema {
    let meta = generic.meta.map(|m| BlueprintMeta {
        name: m.name,
        framework: m.framework,
        ..Default::default()
    });

    let setup = generic.setup.map(|steps| {
        steps
            .into_iter()
            .map(generic_step_to_schema)
            .collect()
    });

    BlueprintSchema {
        version: Some(1),
        meta,
        setup,
        ..Default::default()
    }
}

fn generic_step_to_schema(step: GenericSetupStep) -> SetupStep {
    SetupStep {
        desc: step.desc,
        run_locally: step.run_locally,
        cd: step.cd,
        add_dependency: step.add_dependency.map(|d| {
            d.split_whitespace().map(String::from).collect()
        }),
        dev: step.dev,
        write_file: step.write_file.map(|w| WriteFileDef {
            path: w.path,
            content: w.content,
            template: w.template,
        }),
        patch_file: step.patch_file.map(|p| PatchFileDef {
            path: p.path,
            after: p.after,
            before: p.before,
            replace: p.replace,
            content: p.content.unwrap_or_default(),
        }),
        set_env: step.set_env,
        mkdir: step.mkdir,
        cp: step.cp.map(|c| CopyDef {
            src: c.src,
            dest: c.dest,
        }),
        rm: step.rm,
        once: step.once,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Base scaffolding
// ---------------------------------------------------------------------------

async fn run_base_scaffolding(
    ctx: &InitContext,
    schema: &BlueprintSchema,
    framework_slug: &str,
) -> InitResult<()> {
    // Check meta.create_command first
    let create_cmd = schema
        .meta
        .as_ref()
        .and_then(|m| m.create_command.as_deref())
        .map(String::from)
        .or_else(|| get_create_command(framework_slug).map(String::from));

    let Some(cmd) = create_cmd else {
        debug!("No create command for {framework_slug}, skipping base scaffolding");
        return Ok(());
    };

    ui::section_title(&ctx.options, "Running base scaffolding...");
    ui::info(&ctx.options, &format!("Running: {} .", cmd));

    let status = ctx.exec_interactive(&format!("{} .", cmd)).await?;
    if !status.success() {
        return Err(InitError::CommandFailed(
            cmd,
            "Base scaffolding command failed".to_string(),
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Package manager detection
// ---------------------------------------------------------------------------

fn detect_package_manager(ctx: &InitContext, schema: &BlueprintSchema) -> String {
    // 1) meta.package_manager
    if let Some(pm) = schema
        .meta
        .as_ref()
        .and_then(|m| m.package_manager.as_deref())
    {
        return pm.to_string();
    }

    // 2) Lock files in project dir
    let fs = ctx.fs();
    let lock_files: &[(&str, &str)] = &[
        ("yarn.lock", "yarn"),
        ("pnpm-lock.yaml", "pnpm"),
        ("bun.lockb", "bun"),
        ("package-lock.json", "npm"),
        ("composer.json", "composer"),
        ("Cargo.toml", "cargo"),
        ("go.mod", "go"),
        ("Gemfile", "bundler"),
        ("pyproject.toml", "poetry"),
        ("requirements.txt", "pip"),
    ];

    for (file, pm) in lock_files {
        if fs.exists(file) {
            return pm.to_string();
        }
    }

    // 3) Default
    "npm".to_string()
}

// ---------------------------------------------------------------------------
// Variable substitution
// ---------------------------------------------------------------------------

fn extract_config_vars(schema: &BlueprintSchema) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    if let Some(config) = &schema.config {
        if let Some(obj) = config.as_object() {
            for (k, v) in obj {
                let val = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                vars.insert(k.clone(), val);
            }
        }
    }
    vars
}

fn substitute_vars(input: &str, vars: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (key, val) in vars {
        result = result.replace(&format!("{{{{{}}}}}", key), val);
    }
    result
}

// ---------------------------------------------------------------------------
// Setup step execution
// ---------------------------------------------------------------------------

async fn execute_setup_steps(
    ctx: &InitContext,
    steps: &[SetupStep],
    framework_slug: &str,
    blueprint_name: &str,
    pm: &str,
    vars: &HashMap<String, String>,
) -> InitResult<()> {
    let mut cwd = PathBuf::from(".");

    ui::section_title(&ctx.options, "Running setup steps...");

    for (i, step) in steps.iter().enumerate() {
        let desc = step
            .desc
            .as_deref()
            .unwrap_or("(unnamed step)");
        debug!(step = i + 1, desc, "Executing setup step");

        // cd
        if let Some(dir) = &step.cd {
            let dir = substitute_vars(dir, vars);
            cwd = PathBuf::from(&dir);
            debug!(cwd = %cwd.display(), "Changed working directory");
            continue;
        }

        // mkdir
        if let Some(dir) = &step.mkdir {
            let dir = substitute_vars(dir, vars);
            ui::info(&ctx.options, &format!("  mkdir {dir}"));
            ctx.fs()
                .create_dir_all(&dir)
                .map_err(|e| InitError::FsError(e.to_string()))?;
            continue;
        }

        // write_file
        if let Some(wf) = &step.write_file {
            let path = substitute_vars(&wf.path, vars);
            ui::info(&ctx.options, &format!("  write {path}"));

            let content = if let Some(template) = &wf.template {
                // Fetch template from registry
                let client = RegistryClient::new();
                let bytes = client
                    .fetch_template(framework_slug, blueprint_name, template)
                    .await
                    .map_err(|e| InitError::DownloadFailed(e.to_string()))?;
                let raw = String::from_utf8_lossy(&bytes).to_string();
                substitute_vars(&raw, vars)
            } else {
                substitute_vars(wf.content.as_deref().unwrap_or(""), vars)
            };

            ctx.fs()
                .write_string(&path, &content)
                .map_err(|e| InitError::FsError(e.to_string()))?;
            continue;
        }

        // patch_file
        if let Some(pf) = &step.patch_file {
            let path = substitute_vars(&pf.path, vars);
            let content = substitute_vars(&pf.content, vars);
            ui::info(&ctx.options, &format!("  patch {path}"));

            let existing = ctx
                .fs()
                .read_to_string(&path)
                .map_err(|e| InitError::FsError(e.to_string()))?;

            let patched = apply_patch(&existing, pf, &content)?;
            ctx.fs()
                .write_string(&path, &patched)
                .map_err(|e| InitError::FsError(e.to_string()))?;
            continue;
        }

        // set_env
        if let Some(env_vars) = &step.set_env {
            ui::info(&ctx.options, "  set_env");
            let env_path = ".env";
            let mut existing = ctx
                .fs()
                .read_to_string(env_path)
                .unwrap_or_default();

            for (key, val) in env_vars {
                let key = substitute_vars(key, vars);
                let val = substitute_vars(val, vars);

                // Upsert in .env
                let pattern = format!("{}=", key);
                if let Some(pos) = existing.find(&pattern) {
                    // Replace existing line
                    let line_end = existing[pos..].find('\n').map(|i| pos + i).unwrap_or(existing.len());
                    existing.replace_range(pos..line_end, &format!("{}={}", key, val));
                } else {
                    if !existing.is_empty() && !existing.ends_with('\n') {
                        existing.push('\n');
                    }
                    existing.push_str(&format!("{}={}\n", key, val));
                }

                // Also set in process env for subsequent steps
                std::env::set_var(&key, &val);
            }

            ctx.fs()
                .write_string(env_path, &existing)
                .map_err(|e| InitError::FsError(e.to_string()))?;
            continue;
        }

        // add_dependency
        if let Some(deps) = &step.add_dependency {
            let is_dev = step.dev.unwrap_or(false);
            let deps_str = deps.iter().map(|d| substitute_vars(d, vars)).collect::<Vec<_>>();
            let install_cmd = install_command(pm, &deps_str, is_dev);
            ui::info(&ctx.options, &format!("  {install_cmd}"));
            run_local_in_dir(ctx, &cwd, &install_cmd).await?;
            continue;
        }

        // run_locally
        if let Some(cmd) = &step.run_locally {
            let cmd = substitute_vars(cmd, vars);
            ui::info(&ctx.options, &format!("  run: {cmd}"));
            run_local_in_dir(ctx, &cwd, &cmd).await?;
            continue;
        }

        // run (alias for run_locally)
        if let Some(cmd) = &step.run {
            let cmd = substitute_vars(cmd, vars);
            ui::info(&ctx.options, &format!("  run: {cmd}"));
            run_local_in_dir(ctx, &cwd, &cmd).await?;
            continue;
        }

        // cp
        if let Some(cp) = &step.cp {
            let src = substitute_vars(&cp.src, vars);
            let dest = substitute_vars(&cp.dest, vars);
            ui::info(&ctx.options, &format!("  cp {src} -> {dest}"));
            ctx.fs()
                .copy(&src, &dest)
                .map_err(|e| InitError::FsError(e.to_string()))?;
            continue;
        }

        // rm
        if let Some(target) = &step.rm {
            let target = substitute_vars(target, vars);
            ui::info(&ctx.options, &format!("  rm {target}"));
            let fs = ctx.fs();
            if fs.is_dir(&target) {
                fs.remove_dir_all(&target)
                    .map_err(|e| InitError::FsError(e.to_string()))?;
            } else if fs.is_file(&target) {
                fs.remove_file(&target)
                    .map_err(|e| InitError::FsError(e.to_string()))?;
            } else {
                debug!("rm target does not exist, skipping: {target}");
            }
            continue;
        }
    }

    ui::success(&ctx.options, "Setup steps completed");
    Ok(())
}

// ---------------------------------------------------------------------------
// Patch helpers
// ---------------------------------------------------------------------------

fn apply_patch(
    existing: &str,
    pf: &PatchFileDef,
    content: &str,
) -> InitResult<String> {
    if let Some(after) = &pf.after {
        // Insert content after the matched pattern
        let re = Regex::new(after)
            .map_err(|e| InitError::InvalidFormat(format!("Invalid regex in after: {e}")))?;
        if let Some(m) = re.find(existing) {
            let mut result = String::with_capacity(existing.len() + content.len());
            result.push_str(&existing[..m.end()]);
            result.push('\n');
            result.push_str(content);
            result.push_str(&existing[m.end()..]);
            return Ok(result);
        }
        warn!("patch_file: 'after' pattern not found, appending content");
        return Ok(format!("{existing}\n{content}"));
    }

    if let Some(before) = &pf.before {
        // Insert content before the matched pattern
        let re = Regex::new(before)
            .map_err(|e| InitError::InvalidFormat(format!("Invalid regex in before: {e}")))?;
        if let Some(m) = re.find(existing) {
            let mut result = String::with_capacity(existing.len() + content.len());
            result.push_str(&existing[..m.start()]);
            result.push_str(content);
            result.push('\n');
            result.push_str(&existing[m.start()..]);
            return Ok(result);
        }
        warn!("patch_file: 'before' pattern not found, prepending content");
        return Ok(format!("{content}\n{existing}"));
    }

    if let Some(replace) = &pf.replace {
        // Replace matched pattern with content
        let re = Regex::new(replace)
            .map_err(|e| InitError::InvalidFormat(format!("Invalid regex in replace: {e}")))?;
        return Ok(re.replace(existing, content).to_string());
    }

    // No pattern specified — append
    Ok(format!("{existing}\n{content}"))
}

// ---------------------------------------------------------------------------
// Command helpers
// ---------------------------------------------------------------------------

fn install_command(pm: &str, deps: &[String], is_dev: bool) -> String {
    let deps_str = deps.join(" ");
    match pm {
        "yarn" => {
            let dev_flag = if is_dev { " -D" } else { "" };
            format!("yarn add{dev_flag} {deps_str}")
        }
        "pnpm" => {
            let dev_flag = if is_dev { " -D" } else { "" };
            format!("pnpm add{dev_flag} {deps_str}")
        }
        "bun" => {
            let dev_flag = if is_dev { " -d" } else { "" };
            format!("bun add{dev_flag} {deps_str}")
        }
        "composer" => {
            let dev_flag = if is_dev { " --dev" } else { "" };
            format!("composer require{dev_flag} {deps_str}")
        }
        "cargo" => {
            // cargo add supports --dev
            let dev_flag = if is_dev { " --dev" } else { "" };
            format!("cargo add{dev_flag} {deps_str}")
        }
        "go" => format!("go get {deps_str}"),
        "bundler" => format!("bundle add {deps_str}"),
        "poetry" => {
            let group = if is_dev { " --group dev" } else { "" };
            format!("poetry add{group} {deps_str}")
        }
        "pip" => format!("pip install {deps_str}"),
        _ => {
            // npm (default)
            let dev_flag = if is_dev { " --save-dev" } else { "" };
            format!("npm install{dev_flag} {deps_str}")
        }
    }
}

async fn run_local_in_dir(
    ctx: &InitContext,
    cwd: &Path,
    cmd: &str,
) -> InitResult<()> {
    let full_cmd = if cwd == Path::new(".") {
        cmd.to_string()
    } else {
        format!("cd {} && {}", cwd.display(), cmd)
    };

    let status = ctx.exec_interactive(&full_cmd).await?;
    if !status.success() {
        return Err(InitError::CommandFailed(
            cmd.to_string(),
            "Setup step command failed".to_string(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Save blueprint
// ---------------------------------------------------------------------------

fn save_blueprint(ctx: &InitContext, schema: &BlueprintSchema) -> InitResult<()> {
    let yaml = serde_yaml::to_string(schema)
        .map_err(|e| InitError::Other(format!("Failed to serialize blueprint: {e}")))?;

    let fs = ctx.fs();
    fs.create_dir_all(".appz")
        .map_err(|e| InitError::FsError(e.to_string()))?;
    fs.write_string(".appz/blueprint.yaml", &yaml)
        .map_err(|e| InitError::FsError(e.to_string()))?;

    ui::success(&ctx.options, "Saved blueprint to .appz/blueprint.yaml");
    Ok(())
}
