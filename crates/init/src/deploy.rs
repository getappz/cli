//! Deploy blueprint merge — adds deploy tasks to an existing project's blueprint.

use std::path::Path;

use miette::{miette, Result};

use crate::blueprint_schema::{BlueprintSchema, parse_blueprint};
use crate::registry::RegistryClient;

/// Merge `source` JSON object keys into `target`, without overwriting existing keys.
fn merge_json_objects(target: &mut serde_json::Value, source: &serde_json::Value, overwrite: bool) {
    if let (Some(target_obj), Some(source_obj)) = (target.as_object_mut(), source.as_object()) {
        for (key, val) in source_obj {
            if overwrite || !target_obj.contains_key(key) {
                target_obj.insert(key.clone(), val.clone());
            }
        }
    }
}

/// Fetch a deploy blueprint and merge it into the project's existing blueprint.
pub async fn add_deploy_target(
    project_path: &Path,
    target: &str,
    no_cache: bool,
) -> Result<BlueprintSchema> {
    let appz_blueprint = project_path.join(".appz").join("blueprint.yaml");

    let mut project_bp = if appz_blueprint.exists() {
        parse_blueprint(&appz_blueprint)?
    } else {
        BlueprintSchema::default()
    };

    let client = RegistryClient::new();
    let raw = client.fetch_blueprint("deploy", target).await
        .map_err(|e| miette!("Deploy target '{}' not found: {}", target, e))?;

    let deploy_bp: BlueprintSchema = serde_yaml::from_str(&raw)
        .map_err(|e| miette!("Invalid deploy blueprint for '{}': {}", target, e))?;

    // Merge config and tools (don't overwrite existing keys)
    if let Some(source) = &deploy_bp.config {
        let target = project_bp.config.get_or_insert_with(|| serde_json::json!({}));
        merge_json_objects(target, source, false);
    }
    if let Some(source) = &deploy_bp.tools {
        let target = project_bp.tools.get_or_insert_with(|| serde_json::json!({}));
        merge_json_objects(target, source, false);
    }
    // Merge tasks (overwrite — deploy tasks replace existing deploy tasks)
    if let Some(source) = &deploy_bp.tasks {
        let target = project_bp.tasks.get_or_insert_with(|| serde_json::json!({}));
        merge_json_objects(target, source, true);
    }
    // Merge before hooks
    if let Some(deploy_before) = &deploy_bp.before {
        let project_before = project_bp.before.get_or_insert_with(Default::default);
        for (key, val) in deploy_before {
            project_before.entry(key.clone()).or_default().extend(val.clone());
        }
    }

    // Save updated blueprint
    let yaml = serde_yaml::to_string(&project_bp)
        .map_err(|e| miette!("Failed to serialize blueprint: {}", e))?;
    std::fs::create_dir_all(project_path.join(".appz"))
        .map_err(|e| miette!("Failed to create .appz dir: {}", e))?;
    std::fs::write(&appz_blueprint, yaml)
        .map_err(|e| miette!("Failed to save blueprint: {}", e))?;

    Ok(deploy_bp)
}

/// Execute setup steps from a deploy blueprint (install CLI tools, etc.)
pub async fn run_deploy_setup(
    project_path: &Path,
    deploy_bp: &BlueprintSchema,
) -> Result<()> {
    if let Some(steps) = &deploy_bp.setup {
        if steps.is_empty() {
            return Ok(());
        }
        println!("Running deploy target setup...");
        // Detect package manager once, outside the loop
        let pm = detect_pm(project_path);
        for step in steps {
            let desc = step.desc.as_deref().unwrap_or("(setup)");

            if let Some(deps) = &step.add_dependency {
                let is_dev = step.dev.unwrap_or(false);
                let cmd = pm_install_cmd(&pm, deps, is_dev);
                println!("  -> {}", desc);
                run_shell(project_path, &cmd, desc)?;
            }

            if let Some(cmd) = &step.run_locally {
                println!("  -> {}", desc);
                run_shell(project_path, cmd, desc)?;
            }
        }
        println!("Setup complete.");
    }
    Ok(())
}

fn run_shell(cwd: &Path, cmd: &str, desc: &str) -> Result<()> {
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(cwd)
        .status()
        .map_err(|e| miette!("Failed to run '{}': {}", cmd, e))?;
    if !status.success() {
        return Err(miette!("Setup step failed: {}", desc));
    }
    Ok(())
}

fn detect_pm(project_path: &Path) -> String {
    let lock_files: &[(&str, &str)] = &[
        ("yarn.lock", "yarn"), ("pnpm-lock.yaml", "pnpm"),
        ("bun.lockb", "bun"), ("package-lock.json", "npm"),
        ("composer.json", "composer"),
    ];
    for (file, pm) in lock_files {
        if project_path.join(file).exists() {
            return pm.to_string();
        }
    }
    "npm".to_string()
}

fn pm_install_cmd(pm: &str, deps: &[String], is_dev: bool) -> String {
    let pkgs = deps.join(" ");
    match pm {
        "yarn" => if is_dev { format!("yarn add -D {pkgs}") } else { format!("yarn add {pkgs}") },
        "pnpm" => if is_dev { format!("pnpm add -D {pkgs}") } else { format!("pnpm add {pkgs}") },
        "bun" => if is_dev { format!("bun add -d {pkgs}") } else { format!("bun add {pkgs}") },
        "composer" => if is_dev { format!("composer require --dev {pkgs}") } else { format!("composer require {pkgs}") },
        _ => if is_dev { format!("npm install --save-dev {pkgs}") } else { format!("npm install {pkgs}") },
    }
}
