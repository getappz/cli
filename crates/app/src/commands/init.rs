//! Project initialization — delegates to the init crate.
//!
//! Handles interactive prompts when args are missing, then calls init::run().

use crate::app_error::UserCancellation;
use crate::session::AppzSession;
use crate::templates::{get_builtin_template, BUILTIN_TEMPLATES};
use init::registry::RegistryClient;
use miette::miette;
use tracing::instrument;
use starbase::AppResult;
use std::path::PathBuf;

#[instrument(skip_all)]
pub async fn init(
    session: AppzSession,
    template_or_name: Option<String>,
    name: Option<String>,
    template: Option<String>,
    skip_install: bool,
    force: bool,
    output: Option<PathBuf>,
    blueprint: Option<String>,
    dry_run: bool,
    deploy: Option<String>,
) -> AppResult {
    let (template_source, project_name, deploy_target) = resolve_all(
        template_or_name,
        name,
        template,
        deploy,
    )
    .await?;

    if project_name.is_empty() {
        return Err(miette!("Project name cannot be empty"));
    }

    let output_dir = output.unwrap_or_else(|| session.working_dir.clone());

    init::run(
        Some(template_source.clone()),
        Some(project_name.clone()),
        None,
        None,
        None,
        skip_install,
        force,
        Some(output_dir.clone()),
        false,
        blueprint,
        false, // no_cache
        dry_run,
    )
    .await
    .map_err(|e| miette!("{}", e))?;

    // Add deploy target if specified (via --deploy flag or interactive)
    if let Some(target) = deploy_target {
        if dry_run {
            println!("\nDeploy target: {}", target);
            println!("  Would run: appz blueprints add {}", target);
        } else {
            let project_path = output_dir.join(&project_name);
            println!();
            let deploy_bp = init::deploy::add_deploy_target(&project_path, &target, false)
                .await
                .map_err(|e| miette!("{}", e))?;
            init::deploy::run_deploy_setup(&project_path, &deploy_bp)
                .await
                .map_err(|e| miette!("{}", e))?;
            println!("Added deploy target: {}", target);
        }
    }

    Ok(None)
}

/// Resolve template source, project name, and optional deploy target.
/// Falls back to interactive prompts when args are missing.
async fn resolve_all(
    template_or_name: Option<String>,
    name: Option<String>,
    template: Option<String>,
    deploy: Option<String>,
) -> Result<(String, String, Option<String>), miette::Report> {
    // Explicit --template flag
    if let Some(explicit_template) = template {
        let proj_name = name
            .or(template_or_name)
            .ok_or_else(|| miette!("Project name required. Use --name or provide as positional argument."))?;
        return Ok((explicit_template, proj_name, deploy));
    }

    // Positional arg provided — could be source or project name
    if let Some(pos_arg) = template_or_name {
        let is_source = get_builtin_template(&pos_arg).is_some()
            || pos_arg.starts_with("https://")
            || pos_arg.starts_with("http://")
            || pos_arg.starts_with("npm:")
            || pos_arg.starts_with("./")
            || pos_arg.starts_with("../")
            || pos_arg.starts_with('/')
            || pos_arg.contains('/')
            || init::has_create_command(&pos_arg);

        if is_source {
            let proj_name = name.unwrap_or_else(|| pos_arg.clone());
            return Ok((pos_arg, proj_name, deploy));
        }

        // pos_arg is the project name, prompt for template
        let template_src = prompt_for_template()?;
        return Ok((template_src, pos_arg, deploy));
    }

    // Nothing provided — full interactive mode
    interactive_init(name, deploy).await
}

/// Full interactive init: framework → blueprint → project name → deploy target.
async fn interactive_init(
    name: Option<String>,
    deploy: Option<String>,
) -> Result<(String, String, Option<String>), miette::Report> {
    // Try fetching registry for framework/blueprint selection
    let registry_result = RegistryClient::new().fetch_index(false).await;

    let (template_source, deploy_target) = if let Ok(index) = registry_result {
        // Build framework options from registry
        let mut fw_options: Vec<(String, String)> = index
            .frameworks
            .iter()
            .map(|(slug, entry)| {
                let bp_count = entry.blueprints.len();
                let label = if bp_count > 1 {
                    format!("{} ({} blueprints)", entry.name, bp_count)
                } else {
                    entry.name.clone()
                };
                (label, slug.clone())
            })
            .collect();
        fw_options.sort_by(|a, b| a.0.cmp(&b.0));

        // Step 1: Select framework
        let fw_display: Vec<(String, String)> = fw_options;
        let selected_fw = ui::select_template_interactive(
            "Select a framework:",
            &fw_display,
        )
        .map_err(|e| miette!("Failed to select framework: {}", e))?
        .ok_or_else(|| miette::Report::from(UserCancellation::selection()))?;

        // Step 2: If framework has multiple blueprints, select one
        let template_source = if let Some(entry) = index.frameworks.get(&selected_fw) {
            if entry.blueprints.len() > 1 {
                let mut bp_options: Vec<(String, String)> = entry
                    .blueprints
                    .iter()
                    .map(|(bp_name, bp_entry)| {
                        let label = format!("{} - {}", bp_name, bp_entry.description);
                        let value = format!("{}/{}", selected_fw, bp_name);
                        (label, value)
                    })
                    .collect();
                bp_options.sort_by(|a, b| a.0.cmp(&b.0));

                let selected_bp = ui::select_template_interactive(
                    &format!("Select a {} blueprint:", entry.name),
                    &bp_options,
                )
                .map_err(|e| miette!("Failed to select blueprint: {}", e))?
                .ok_or_else(|| miette::Report::from(UserCancellation::selection()))?;

                selected_bp
            } else {
                selected_fw.clone()
            }
        } else {
            selected_fw.clone()
        };

        // Step 3: Optionally select deploy target
        let deploy_target = if deploy.is_some() {
            deploy
        } else if !index.deploy.is_empty() {
            let mut deploy_options: Vec<String> = vec!["Skip (no deploy target)".to_string()];
            let mut targets: Vec<_> = index.deploy.iter().collect();
            targets.sort_by_key(|(name, _)| name.as_str());
            for (target_name, entry) in &targets {
                deploy_options.push(format!("{} - {}", target_name, entry.description));
            }

            let selected = ui::select_interactive(
                "Add a deploy target? (optional)",
                &deploy_options,
            )
            .map_err(|e| miette!("Failed to select deploy target: {}", e))?;

            match selected {
                Some(s) if !s.starts_with("Skip") => {
                    // Extract target name from "vercel - Deploy to Vercel"
                    s.split(" - ").next().map(|s| s.trim().to_string())
                }
                _ => None,
            }
        } else {
            None
        };

        (template_source, deploy_target)
    } else {
        // Registry fetch failed — fall back to builtin templates
        let source = prompt_for_template()?;
        (source, deploy)
    };

    // Step 4: Project name
    let proj_name = if let Some(n) = name {
        n
    } else {
        ui::prompt::prompt("Project name:", None)
            .map_err(|e| miette!("Failed to get project name: {}", e))?
    };

    Ok((template_source, proj_name, deploy_target))
}

fn prompt_for_template() -> Result<String, miette::Report> {
    let options: Vec<(String, String)> = BUILTIN_TEMPLATES
        .iter()
        .map(|(slug, name, _, _)| (format!("{} ({})", name, slug), slug.to_string()))
        .collect();

    let selected = ui::select_template_interactive("Select a template (type to search or enter git/npm/path):", &options)
        .map_err(|e| miette!("Failed to select template: {}", e))?
        .ok_or_else(|| miette::Report::from(UserCancellation::selection()))?;

    resolve_template_source(&selected)
}

fn resolve_template_source(selected: &str) -> Result<String, miette::Report> {
    if selected.starts_with("http://") || selected.starts_with("https://")
        || selected.starts_with("npm:") || selected.starts_with("./")
        || selected.starts_with("../") || selected.starts_with('/')
    {
        return Ok(selected.to_string());
    }
    if selected.contains('/') && !selected.contains(' ') {
        return Ok(selected.to_string());
    }
    let slug = selected;
    if slug.eq_ignore_ascii_case("wordpress") {
        return Ok("wordpress".to_string());
    }
    if init::has_create_command(slug) {
        Ok(slug.to_string())
    } else if let Some((repo, subfolder)) = get_builtin_template(slug) {
        let src = match subfolder {
            Some(sf) => format!("{}/{}", repo, sf),
            None => repo.to_string(),
        };
        Ok(src)
    } else {
        Ok(slug.to_string())
    }
}
