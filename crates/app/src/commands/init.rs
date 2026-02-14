//! Project initialization — delegates to the init crate.
//!
//! Handles interactive prompts when args are missing, then calls init::run().

use crate::session::AppzSession;
use tracing::instrument;
use crate::templates::{get_builtin_template, BUILTIN_TEMPLATES};
use inquire::{Select, Text};
use miette::miette;
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
        Some(template_source),
        Some(project_name),
        None,
        None,
        None,
        skip_install,
        force,
        Some(output_dir),
        false,
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
            .or_else(|| template_or_name)
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
            let proj_name = name.ok_or_else(|| {
                miette!("Project name required. Use --name when using a template source.")
            })?;
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
    let template_options_strings: Vec<String> = BUILTIN_TEMPLATES
        .iter()
        .map(|(slug, name, _, _)| format!("{} ({})", name, slug))
        .collect();

    let template_options: Vec<&str> = template_options_strings
        .iter()
        .map(|s| s.as_str())
        .collect();

    let mut options = vec!["Custom GitHub URL", "Custom npm package", "Local path"];
    options.extend(template_options);

    let selected = Select::new("Select a template:", options)
        .prompt()
        .map_err(|e| miette!("Failed to select template: {}", e))?;

    if selected == "Custom GitHub URL" {
        Text::new("Git repository (user/repo or full URL - GitHub, GitLab, Bitbucket):")
            .prompt()
            .map_err(|e| miette!("Failed to get URL: {}", e))
    } else if selected == "Custom npm package" {
        let pkg = Text::new("npm package name:")
            .prompt()
            .map_err(|e| miette!("Failed to get npm package: {}", e))?;
        Ok(format!("npm:{}", pkg))
    } else if selected == "Local path" {
        Text::new("Local template path:")
            .prompt()
            .map_err(|e| miette!("Failed to get local path: {}", e))
    } else {
        let slug = selected
            .split('(')
            .nth(1)
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(selected);
        let slug = slug.to_string();
        if init::has_create_command(&slug) {
            Ok(slug)
        } else if let Some((repo, subfolder)) = get_builtin_template(&slug) {
            let src = match subfolder {
                Some(sf) => format!("{}/{}", repo, sf),
                None => repo.to_string(),
            };
            Ok(src)
        } else {
            Ok(slug)
        }
    }
}
