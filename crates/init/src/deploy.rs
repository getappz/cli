//! Deploy blueprint merge — adds deploy tasks to an existing project's blueprint.

use std::path::Path;

use miette::{miette, Result};

use crate::blueprint_schema::{BlueprintSchema, SetupStep, parse_blueprint};
use crate::registry::RegistryClient;

/// Fetch a deploy blueprint and merge it into the project's existing blueprint.
pub async fn add_deploy_target(
    project_path: &Path,
    target: &str,
    no_cache: bool,
) -> Result<BlueprintSchema> {
    let appz_blueprint = project_path.join(".appz").join("blueprint.yaml");

    // Load existing project blueprint
    let mut project_bp = if appz_blueprint.exists() {
        parse_blueprint(&appz_blueprint)?
    } else {
        BlueprintSchema::default()
    };

    // Fetch deploy blueprint from registry
    let client = RegistryClient::new();
    let raw = client.fetch_blueprint("deploy", target).await
        .map_err(|e| miette!("Deploy target '{}' not found: {}", target, e))?;

    let deploy_bp: BlueprintSchema = serde_yaml::from_str(&raw)
        .map_err(|e| miette!("Invalid deploy blueprint for '{}': {}", target, e))?;

    // Merge config
    if let Some(deploy_config) = &deploy_bp.config {
        if let Some(deploy_obj) = deploy_config.as_object() {
            let project_config = project_bp.config.get_or_insert_with(|| serde_json::json!({}));
            if let Some(project_obj) = project_config.as_object_mut() {
                for (key, val) in deploy_obj {
                    if !project_obj.contains_key(key) {
                        project_obj.insert(key.clone(), val.clone());
                    }
                }
            }
        }
    }

    // Merge tools
    if let Some(deploy_tools) = &deploy_bp.tools {
        if let Some(deploy_obj) = deploy_tools.as_object() {
            let project_tools = project_bp.tools.get_or_insert_with(|| serde_json::json!({}));
            if let Some(project_obj) = project_tools.as_object_mut() {
                for (key, val) in deploy_obj {
                    if !project_obj.contains_key(key) {
                        project_obj.insert(key.clone(), val.clone());
                    }
                }
            }
        }
    }

    // Merge tasks
    if let Some(deploy_tasks) = &deploy_bp.tasks {
        if let Some(deploy_obj) = deploy_tasks.as_object() {
            let project_tasks = project_bp.tasks.get_or_insert_with(|| serde_json::json!({}));
            if let Some(project_obj) = project_tasks.as_object_mut() {
                for (key, val) in deploy_obj {
                    project_obj.insert(key.clone(), val.clone());
                }
            }
        }
    }

    // Merge before hooks
    if let Some(deploy_before) = &deploy_bp.before {
        let project_before = project_bp.before.get_or_insert_with(Default::default);
        for (key, val) in deploy_before {
            project_before.entry(key.clone()).or_insert_with(Vec::new).extend(val.clone());
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
        for step in steps {
            let desc = step.desc.as_deref().unwrap_or("(setup)");

            if let Some(deps) = &step.add_dependency {
                let is_dev = step.dev.unwrap_or(false);
                let dev_flag = if is_dev { " --save-dev" } else { "" };
                let cmd = format!("npm install{} {}", dev_flag, deps.join(" "));
                println!("  -> {}", desc);
                let status = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .current_dir(project_path)
                    .status()
                    .map_err(|e| miette!("Failed to run '{}': {}", cmd, e))?;
                if !status.success() {
                    return Err(miette!("Setup step failed: {}", desc));
                }
            }

            if let Some(cmd) = &step.run_locally {
                println!("  -> {}", desc);
                let status = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .current_dir(project_path)
                    .status()
                    .map_err(|e| miette!("Failed to run '{}': {}", cmd, e))?;
                if !status.success() {
                    return Err(miette!("Setup step failed: {}", desc));
                }
            }
        }
        println!("Setup complete.");
    }
    Ok(())
}
