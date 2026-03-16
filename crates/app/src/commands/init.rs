//! Project initialization — delegates to the init crate.
//!
//! Handles interactive prompts when args are missing, then calls init::run().

use crate::app_error::UserCancellation;
use crate::ddev_helpers::{
    ddev_config_command, ddev_project_type_for_framework, has_ddev_config,
    is_ddev_available, is_ddev_supported_framework,
};
use crate::session::AppzSession;
use crate::shell::{run_local_with, RunOptions};
use crate::templates::{get_builtin_template, BUILTIN_TEMPLATES};
use miette::miette;
use task::Context;
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
) -> AppResult {
    let (template_source, project_name) = resolve_template_and_name(
        template_or_name,
        name,
        template,
    )?;

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
    )
    .await
    .map_err(|e| miette!("{}", e))?;

    // Auto-add DDEV configuration for DDEV-supported PHP/CMS frameworks when DDEV is available
    let project_path = output_dir.join(&project_name);
    if is_ddev_supported_framework(&template_source)
        && is_ddev_available()
        && !has_ddev_config(&project_path)
    {
        if let Some((project_type, docroot)) =
            ddev_project_type_for_framework(&template_source)
        {
            let mut config_cmd = ddev_config_command(project_type, docroot);
            if project_type == "php" {
                config_cmd.push_str(" --php-version=8.2");
            }
            println!("⚙️  Adding DDEV configuration...");
            let mut ctx = Context::new();
            ctx.set_working_path(project_path.clone());
            let opts = RunOptions {
                cwd: Some(project_path),
                env: None,
                show_output: true,
                package_manager: None,
                tool_info: None,
            };
            if run_local_with(&ctx, &config_cmd, opts).await.is_ok() {
                println!("✓ DDEV configured.");
            }
        }
    } else if is_ddev_supported_framework(&template_source) && !is_ddev_available() {
        println!("Tip: Install DDEV for local PHP development: https://docs.ddev.com/en/stable/users/install/ddev-installation/");
    }

    Ok(None)
}

fn resolve_template_and_name(
    template_or_name: Option<String>,
    name: Option<String>,
    template: Option<String>,
) -> Result<(String, String), miette::Report> {
    if let Some(explicit_template) = template {
        let proj_name = name
            .or(template_or_name)
            .ok_or_else(|| miette!("Project name required. Use --name or provide as positional argument."))?;
        return Ok((explicit_template, proj_name));
    }

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
            return Ok((pos_arg, proj_name));
        }

        let template_src = prompt_for_template()?;
        return Ok((template_src, pos_arg));
    }

    let proj_name = name.ok_or_else(|| {
        miette!("Project name required. Use --name or provide as positional argument.")
    })?;
    let template_src = prompt_for_template()?;
    Ok((template_src, proj_name))
}

fn prompt_for_template() -> Result<String, miette::Report> {
    let options: Vec<(String, String)> = BUILTIN_TEMPLATES
        .iter()
        .map(|(slug, name, _, _)| (format!("{} ({})", name, slug), slug.to_string()))
        .collect();

    let selected = ui::select_template_interactive("Select a template (type to search or enter git/npm/path):", &options)
        .map_err(|e| miette!("Failed to select template: {}", e))?
        .ok_or_else(|| miette::Report::from(UserCancellation::selection()))?;

    // selected is already the final value: slug, URL, npm:xx, or path
    resolve_template_source(&selected)
}

fn resolve_template_source(selected: &str) -> Result<String, miette::Report> {
    // Already a custom source (URL, npm:, path)
    if selected.starts_with("http://") || selected.starts_with("https://")
        || selected.starts_with("npm:") || selected.starts_with("./")
        || selected.starts_with("../") || selected.starts_with('/')
    {
        return Ok(selected.to_string());
    }
    // user/repo pattern
    if selected.contains('/') && !selected.contains(' ') {
        return Ok(selected.to_string());
    }
    // Built-in slug
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
