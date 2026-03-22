//! `appz blueprints` subcommands.

use clap::Subcommand;
use init::blueprint_schema::{BlueprintMeta, BlueprintSchema, SetupStep};
use init::deploy::{add_deploy_target, run_deploy_setup};
use init::registry::RegistryClient;
use miette::miette;
use starbase::AppResult;
use std::path::Path;

#[derive(Subcommand, Debug, Clone)]
pub enum BlueprintsCommand {
    /// List available blueprints from the registry
    #[command(alias = "ls")]
    List {
        /// Filter by framework slug (e.g. nextjs, wordpress)
        framework: Option<String>,
        /// Skip local cache and fetch fresh data from the registry
        #[arg(long)]
        no_cache: bool,
    },
    /// Add a deploy target to the current project
    Add {
        /// Deploy target (e.g. vercel, netlify, aws-s3)
        target: String,
        /// Skip local cache
        #[arg(long)]
        no_cache: bool,
        /// Skip running setup steps (just merge tasks)
        #[arg(long)]
        skip_setup: bool,
    },
    /// Generate a blueprint from the current project
    Gen {
        /// Output file (default: .appz/blueprint.yaml)
        #[arg(short, long)]
        output: Option<String>,
        /// Overwrite existing blueprint
        #[arg(long)]
        force: bool,
    },
}

pub async fn run(command: BlueprintsCommand) -> AppResult {
    match command {
        BlueprintsCommand::List { framework, no_cache } => list(framework, no_cache).await,
        BlueprintsCommand::Add { target, no_cache, skip_setup } => add(target, no_cache, skip_setup).await,
        BlueprintsCommand::Gen { output, force } => gen(output, force).await,
    }
}

/// List available blueprints from the registry.
async fn list(framework_filter: Option<String>, no_cache: bool) -> AppResult {
    let client = RegistryClient::new();
    let index = client
        .fetch_index(no_cache)
        .await
        .map_err(|e| miette!("Failed to fetch blueprint registry: {}", e))?;

    if let Some(fw) = &framework_filter {
        let entry = index
            .frameworks
            .get(fw.as_str())
            .ok_or_else(|| miette!("Framework '{}' not found in registry", fw))?;

        println!(
            "\n{}",
            ui::theme::style_accent_bold(&format!("{} blueprints", entry.name))
        );
        println!();

        let mut blueprints: Vec<_> = entry.blueprints.iter().collect();
        blueprints.sort_by_key(|(name, _)| if *name == "default" { "" } else { name.as_str() });

        for (name, bp) in &blueprints {
            let cmd = format!("appz init {}/{}", fw, name);
            println!(
                "  {}  {}",
                ui::theme::style_accent_bold(&format!("{:<12}", name)),
                bp.description,
            );
            println!(
                "  {}  {}",
                " ".repeat(12),
                ui::theme::style_muted_italic(&cmd),
            );
        }
        println!();
    } else {
        println!(
            "\n{}",
            ui::theme::style_accent_bold("Available blueprints")
        );
        println!(
            "{}",
            ui::theme::style_muted_italic("Use `appz init <framework>/<blueprint>` to create a project")
        );
        println!();

        let mut frameworks: Vec<_> = index.frameworks.iter().collect();
        frameworks.sort_by_key(|(slug, _)| slug.as_str());

        for (slug, entry) in &frameworks {
            let count = entry.blueprints.len();
            let count_label = if count == 1 {
                "1 blueprint".to_string()
            } else {
                format!("{} blueprints", count)
            };
            println!(
                "  {}  {}",
                ui::theme::style_accent_bold(&format!("{:<14}", entry.name)),
                ui::theme::style_muted_italic(&count_label),
            );

            let mut blueprints: Vec<_> = entry.blueprints.iter().collect();
            blueprints.sort_by_key(|(name, _)| if *name == "default" { "" } else { name.as_str() });

            for (name, bp) in &blueprints {
                println!(
                    "    {:<24} {}",
                    format!("{}/{}", slug, name),
                    ui::theme::style_muted_italic(&bp.description),
                );
            }
            println!();
        }

        // Deploy targets
        if !index.deploy.is_empty() {
            println!(
                "  {}",
                ui::theme::style_accent_bold("Deploy targets"),
            );
            println!(
                "    {}",
                ui::theme::style_muted_italic("Use `appz blueprints add <target>` to add to your project"),
            );
            println!();

            let mut targets: Vec<_> = index.deploy.iter().collect();
            targets.sort_by_key(|(name, _)| name.as_str());

            for (name, entry) in &targets {
                println!(
                    "    {:<24} {}",
                    name,
                    ui::theme::style_muted_italic(&entry.description),
                );
            }
            println!();
        }
    }

    Ok(None)
}

/// Add a deploy target to the current project.
async fn add(target: String, no_cache: bool, skip_setup: bool) -> AppResult {
    let cwd = std::env::current_dir()
        .map_err(|e| miette!("Failed to get current directory: {}", e))?;

    println!("Adding deploy target: {}", ui::theme::style_accent_bold(&target));

    let deploy_bp = add_deploy_target(&cwd, &target, no_cache)
        .await
        .map_err(|e| miette!("{}", e))?;

    if !skip_setup {
        run_deploy_setup(&cwd, &deploy_bp)
            .await
            .map_err(|e| miette!("{}", e))?;
    }

    // Show what was added
    if let Some(tasks) = &deploy_bp.tasks {
        if let Some(obj) = tasks.as_object() {
            println!("\nAdded tasks:");
            for name in obj.keys() {
                println!("  - {}", name);
            }
        }
    }

    println!(
        "\n{}",
        ui::theme::style_muted_italic("Deploy tasks merged into .appz/blueprint.yaml")
    );

    Ok(None)
}

/// Generate a blueprint from the current project.
async fn gen(output: Option<String>, force: bool) -> AppResult {
    let cwd = std::env::current_dir()
        .map_err(|e| miette!("Failed to get current directory: {}", e))?;

    let out_path = output
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| cwd.join(".appz").join("blueprint.yaml"));

    if out_path.exists() && !force {
        return Err(miette!(
            "Blueprint already exists at {}\nUse --force to overwrite.",
            out_path.display()
        ));
    }

    println!("Analyzing project...\n");

    let detected = appz_build::detect::detect_framework(&cwd)
        .await
        .map_err(|e| miette!("Detection failed: {}", e))?;

    let (framework_name, framework_slug, build_cmd, dev_cmd, pm_name) =
        if let Some(ref d) = detected {
            let pm = d.package_manager.as_ref().map(|p| p.manager.as_str()).unwrap_or("npm");
            let dev = d.package_manager.as_ref()
                .and_then(|p| p.dev_script.clone())
                .unwrap_or_else(|| format!("{} run dev", pm));
            (d.name.clone(), d.slug.clone(), d.build_command.clone(), dev, pm.to_string())
        } else {
            let pm = detect_pm(&cwd);
            (
                "Unknown".to_string(),
                None,
                format!("{} run build", pm),
                format!("{} run dev", pm),
                pm,
            )
        };

    println!("  Framework:       {}", framework_name);
    println!("  Package manager: {}", pm_name);
    println!("  Build command:   {}", build_cmd);
    println!("  Dev command:     {}", dev_cmd);

    // Read package.json scripts for additional tasks
    let extra_tasks = read_package_json_scripts(&cwd);

    let mut tasks = serde_json::Map::new();
    tasks.insert("dev".to_string(), serde_json::json!([
        {"desc": "Start development server", "run_locally": dev_cmd}
    ]));
    tasks.insert("build".to_string(), serde_json::json!([
        {"desc": "Build for production", "run_locally": build_cmd}
    ]));

    for (name, script) in &extra_tasks {
        if !tasks.contains_key(name.as_str()) {
            tasks.insert(name.clone(), serde_json::json!([
                {"desc": name, "run_locally": script}
            ]));
        }
    }

    let blueprint = BlueprintSchema {
        version: Some(1),
        meta: Some(BlueprintMeta {
            name: Some(framework_name.clone()),
            description: Some(format!("{} project", framework_name)),
            framework: framework_slug,
            ..Default::default()
        }),
        tasks: Some(serde_json::Value::Object(tasks)),
        ..Default::default()
    };

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| miette!("Failed to create directory: {}", e))?;
    }
    let yaml = serde_yaml::to_string(&blueprint)
        .map_err(|e| miette!("Failed to serialize blueprint: {}", e))?;
    std::fs::write(&out_path, &yaml)
        .map_err(|e| miette!("Failed to write blueprint: {}", e))?;

    let task_count = blueprint.tasks.as_ref()
        .and_then(|t| t.as_object())
        .map(|o| o.len())
        .unwrap_or(0);

    println!(
        "\nGenerated blueprint with {} tasks: {}",
        task_count,
        out_path.display()
    );

    Ok(None)
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

/// Read package.json scripts and return usable ones as task candidates.
fn read_package_json_scripts(project_path: &Path) -> Vec<(String, String)> {
    let pkg_path = project_path.join("package.json");
    let raw = match std::fs::read_to_string(&pkg_path) {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let json: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(j) => j,
        Err(_) => return vec![],
    };
    let scripts = match json.get("scripts").and_then(|s| s.as_object()) {
        Some(s) => s,
        None => return vec![],
    };

    let pm = detect_pm(project_path);
    // Skip lifecycle hooks and scripts we already handle
    let skip = ["dev", "build", "start", "install", "prepare", "preinstall", "postinstall",
                 "prepublish", "prepublishOnly", "prepack", "postpack", "pretest", "posttest",
                 "prebuild", "postbuild"];

    scripts.iter()
        .filter(|(name, _)| !skip.contains(&name.as_str()))
        .filter_map(|(name, val)| {
            val.as_str().map(|_| (name.clone(), format!("{} run {}", pm, name)))
        })
        .collect()
}
