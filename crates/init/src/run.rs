//! Main entry point for the init flow.

use std::path::PathBuf;
use std::sync::Arc;

use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};

use crate::config::{InitContext, InitOptions};
use crate::detect::resolve_source;
use crate::error::{InitError, InitResult};
use crate::output::InitOutput;
use crate::ui;

/// Run the init flow.
///
/// When template_source and project_name are both provided (e.g. from interactive
/// prompts), they are used directly. Otherwise, resolution is done from CLI args.
///
/// 1. Resolve source type from the template/source string
/// 2. Handle --force (remove existing dir)
/// 3. Create sandbox at project path
/// 4. Run provider's init()
/// 5. Display success message
pub async fn run(
    template_source: Option<String>,
    project_name: Option<String>,
    template_or_name: Option<String>,
    name: Option<String>,
    template: Option<String>,
    skip_install: bool,
    force: bool,
    output: Option<PathBuf>,
    json_output: bool,
) -> InitResult<Option<InitOutput>> {
    let (source, project_name) = if let (Some(src), Some(proj)) = (template_source, project_name) {
        (src, proj)
    } else {
        resolve_template_and_name(template_or_name, name, template)?
    };
    let output_dir = output.unwrap_or_else(|| std::env::current_dir().unwrap());
    let project_path = output_dir.join(&project_name);

    if project_path.exists() && !force {
        return Err(InitError::DirectoryExists(project_path.display().to_string()));
    }

    if project_path.exists() && force {
        starbase_utils::fs::remove_dir_all(&project_path)
            .map_err(|e| InitError::FsError(format!("Failed to remove existing dir: {}", e)))?;
    }

    let resolved = resolve_source(&source)?;
    let is_ci = crate::config::is_ci_environment();

    let options = InitOptions {
        project_name: project_name.clone(),
        output_dir: output_dir.clone(),
        skip_install,
        force,
        json_output,
        is_ci,
    };

    let settings = SandboxSettings::default();
    let config = SandboxConfig::new(project_path.clone()).with_settings(settings);

    let sandbox = create_sandbox(config)
        .await
        .map_err(|e| InitError::FsError(format!("Sandbox setup failed: {}", e)))?;

    let ctx = InitContext::new(Arc::from(sandbox), options, source.clone());

    let output = resolved.provider.init(&ctx).await?;

    if !json_output {
        ui::blank_line(&ctx.options);
        ui::success(&ctx.options, "Project initialized successfully!");
        ui::info(
            &ctx.options,
            &format!("  Location: {}", common::user_config::path_for_display(&output.project_path)),
        );
        if let Some(ref fw) = output.framework {
            ui::info(&ctx.options, &format!("  Framework: {}", fw));
        }
        ui::blank_line(&ctx.options);
        ui::info(&ctx.options, "Next steps:");
        ui::info(&ctx.options, &format!("  cd {}", project_name));
        if output.installed {
            ui::info(&ctx.options, "  # Dependencies are already installed");
        } else {
            ui::info(&ctx.options, "  # Install dependencies: npm install (or pnpm/yarn/bun)");
        }
        if output.framework.is_some() {
            ui::info(&ctx.options, "  # Start development server: appz dev");
        }
    }

    Ok(Some(output))
}

/// Resolve template source and project name from CLI args.
/// Mirrors the logic in app's init command.
fn resolve_template_and_name(
    template_or_name: Option<String>,
    name: Option<String>,
    template: Option<String>,
) -> InitResult<(String, String)> {
    // If --template is explicitly provided, use it; positional is project name
    if let Some(t) = template {
        let proj = name.or(template_or_name).ok_or_else(|| {
            InitError::InvalidFormat(
                "Project name required. Use --name or provide as positional argument.".to_string(),
            )
        })?;
        return Ok((t, proj));
    }

    // Positional is either template/source or project name
    if let Some(pos) = template_or_name {
        if is_source(&pos) {
            let proj = name.ok_or_else(|| {
                InitError::InvalidFormat(
                    "Project name required. Use --name when using a template source.".to_string(),
                )
            })?;
            return Ok((pos, proj));
        }
        return Ok((pos.clone(), pos));
    }

    Err(InitError::InvalidFormat(
        "Template or project name required. Run 'appz init --help' for usage.".to_string(),
    ))
}

fn is_source(s: &str) -> bool {
    s.starts_with("https://")
        || s.starts_with("http://")
        || s.starts_with("npm:")
        || s.starts_with("./")
        || s.starts_with("../")
        || s.starts_with('/')
        || (s.len() > 1 && s.chars().nth(1) == Some(':') && !s.contains("github.com"))
        || s.contains('/')
        || crate::providers::framework::has_create_command(s)
}
