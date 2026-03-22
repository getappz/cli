//! Project initialization — delegates to the init crate.
//!
//! Handles interactive prompts when args are missing, then calls init::run().

use crate::app_error::UserCancellation;
use crate::session::AppzSession;
use crate::templates::{get_builtin_template, BUILTIN_TEMPLATES};
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
        blueprint,
        false, // no_cache
    )
    .await
    .map_err(|e| miette!("{}", e))?;

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
