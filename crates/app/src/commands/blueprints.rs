//! `appz blueprints` subcommands.

use clap::Subcommand;
use init::deploy::{add_deploy_target, run_deploy_setup};
use init::registry::RegistryClient;
use miette::miette;
use starbase::AppResult;

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
}

pub async fn run(command: BlueprintsCommand) -> AppResult {
    match command {
        BlueprintsCommand::List { framework, no_cache } => list(framework, no_cache).await,
        BlueprintsCommand::Add { target, no_cache, skip_setup } => add(target, no_cache, skip_setup).await,
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
