use crate::detectors::{detect_framework_record, DetectFrameworkRecordOptions, StdFilesystem};
use crate::services::{TemplateService, TemplateSource};
use crate::session::AppzSession;
use crate::shell::copy_path_recursive;
use crate::templates::{get_builtin_template, BUILTIN_TEMPLATES};
use frameworks::frameworks;
use inquire::{Select, Text};
use miette::miette;
use starbase::AppResult;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, instrument, warn};

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
    // Determine template and project name based on priority:
    // 1. --template flag (explicit template)
    // 2. positional argument matching built-in template
    // 3. positional argument as project name
    // 4. interactive prompts

    let (template_source, project_name) = if let Some(explicit_template) = template {
        // Priority 1: --template flag takes precedence
        let proj_name = if let Some(n) = name {
            n
        } else if let Some(pos_arg) = &template_or_name {
            // If --template is provided, positional arg is project name
            pos_arg.clone()
        } else {
            Text::new("Project name:")
                .prompt()
                .map_err(|e| miette!("Failed to get project name: {}", e))?
        };
        (explicit_template, proj_name)
    } else if let Some(pos_arg) = &template_or_name {
        // Check if positional argument matches a built-in template
        if get_builtin_template(pos_arg).is_some() {
            // Priority 2: Positional argument is a built-in template
            let proj_name = if let Some(n) = name {
                n
            } else {
                Text::new("Project name:")
                    .prompt()
                    .map_err(|e| miette!("Failed to get project name: {}", e))?
            };
            (pos_arg.clone(), proj_name)
        } else {
            // Priority 3: Positional argument is project name
            let proj_name = pos_arg.clone();
            // Prompt for template interactively
            let template_src = {
                // Show interactive template selection
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
                    Text::new("GitHub repository (user/repo or full URL):")
                        .prompt()
                        .map_err(|e| miette!("Failed to get GitHub URL: {}", e))?
                } else if selected == "Custom npm package" {
                    let pkg = Text::new("npm package name:")
                        .prompt()
                        .map_err(|e| miette!("Failed to get npm package: {}", e))?;
                    format!("npm:{}", pkg)
                } else if selected == "Local path" {
                    Text::new("Local template path:")
                        .prompt()
                        .map_err(|e| miette!("Failed to get local path: {}", e))?
                } else {
                    // Extract slug from selected option (format: "Name (slug)")
                    let slug = selected
                        .split('(')
                        .nth(1)
                        .and_then(|s| s.strip_suffix(')'))
                        .unwrap_or(selected);
                    slug.to_string()
                }
            };
            (template_src, proj_name)
        }
    } else {
        // Priority 4: No positional argument, use interactive mode
        let proj_name = if let Some(n) = name {
            n
        } else {
            Text::new("Project name:")
                .prompt()
                .map_err(|e| miette!("Failed to get project name: {}", e))?
        };

        let template_src = {
            // Show interactive template selection
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
                Text::new("GitHub repository (user/repo or full URL):")
                    .prompt()
                    .map_err(|e| miette!("Failed to get GitHub URL: {}", e))?
            } else if selected == "Custom npm package" {
                let pkg = Text::new("npm package name:")
                    .prompt()
                    .map_err(|e| miette!("Failed to get npm package: {}", e))?;
                format!("npm:{}", pkg)
            } else if selected == "Local path" {
                Text::new("Local template path:")
                    .prompt()
                    .map_err(|e| miette!("Failed to get local path: {}", e))?
            } else {
                // Extract slug from selected option (format: "Name (slug)")
                let slug = selected
                    .split('(')
                    .nth(1)
                    .and_then(|s| s.strip_suffix(')'))
                    .unwrap_or(selected);
                slug.to_string()
            }
        };
        (template_src, proj_name)
    };

    if project_name.is_empty() {
        return Err(miette!("Project name cannot be empty"));
    }

    // Determine output directory
    let output_dir = output.unwrap_or_else(|| session.working_dir.clone());
    let project_path = output_dir.join(&project_name);

    // Check if directory exists
    if project_path.exists() && !force {
        return Err(miette!(
            "Directory '{}' already exists. Use --force to overwrite.",
            project_path.display()
        ));
    }

    if project_path.exists() && force {
        info!("Removing existing directory: {}", project_path.display());
        std::fs::remove_dir_all(&project_path)
            .map_err(|e| miette!("Failed to remove existing directory: {}", e))?;
    }

    // Parse template source
    let parsed_source = TemplateService::parse_template_source(&template_source)
        .map_err(|e| miette!("Invalid template source: {}", e))?;

    // Download/extract template
    info!("Downloading template from: {}", template_source);
    let template_dir = match parsed_source {
        TemplateSource::GitHub {
            url,
            branch,
            subfolder,
        } => {
            TemplateService::download_github_template(&url, subfolder.as_deref(), branch.as_deref())
                .await
                .map_err(|e| miette!("Failed to download GitHub template: {}", e))?
        }
        TemplateSource::Npm(package) => TemplateService::download_npm_template(&package)
            .await
            .map_err(|e| miette!("Failed to download npm template: {}", e))?,
        TemplateSource::Local(path) => {
            let local_path = PathBuf::from(path);
            TemplateService::copy_local_template(&local_path)
                .await
                .map_err(|e| miette!("Failed to access local template: {}", e))?
        }
        TemplateSource::Builtin { repo, subfolder } => {
            TemplateService::download_github_template(&repo, subfolder.as_deref(), None)
                .await
                .map_err(|e| miette!("Failed to download built-in template: {}", e))?
        }
    };

    // Create project directory
    std::fs::create_dir_all(&project_path)
        .map_err(|e| miette!("Failed to create project directory: {}", e))?;

    // Copy template files to project directory
    info!("Copying template files to: {}", project_path.display());
    copy_path_recursive(&template_dir, &project_path)
        .map_err(|e| miette!("Failed to copy template files: {}", e))?;

    // Auto-detect framework
    let fs = Arc::new(StdFilesystem::new(Some(project_path.clone())));
    let framework_list: Vec<_> = frameworks().to_vec();
    let options = DetectFrameworkRecordOptions { fs, framework_list };

    let detected_framework = match detect_framework_record(options).await {
        Ok(Some((framework, _version, _package_manager))) => {
            info!("✓ Detected framework: {}", framework.name);
            Some(framework.name)
        }
        Ok(None) => {
            warn!("No framework detected in project");
            None
        }
        Err(e) => {
            warn!("Failed to detect framework: {}", e);
            None
        }
    };

    // Run install command if not skipped
    if !skip_install {
        // Check for package.json to determine if we should run npm install
        let package_json = project_path.join("package.json");
        if package_json.exists() {
            info!("Installing dependencies...");
            let install_cmd = if project_path.join("pnpm-lock.yaml").exists() {
                "pnpm install"
            } else if project_path.join("yarn.lock").exists() {
                "yarn install"
            } else if project_path.join("bun.lockb").exists() {
                "bun install"
            } else {
                "npm install"
            };

            // Use shell::run_local_with which automatically wraps with mise if supported
            use crate::shell::{run_local_with, RunOptions};
            use std::sync::Arc;
            use task::Context;

            let ctx = Arc::new(Context::new());
            let opts = RunOptions {
                cwd: Some(project_path.clone()),
                env: None,
                show_output: true,
                package_manager: None,
            };

            let result = run_local_with(&ctx, install_cmd, opts).await;

            match result {
                Ok(_) => {
                    info!("✓ Dependencies installed");
                }
                Err(e) => {
                    warn!(
                        "Install command failed, but project was created successfully: {}",
                        e
                    );
                }
            }
        }
    }

    // Display success message
    println!("\n✓ Project initialized successfully!");
    println!("  Location: {}", project_path.display());
    if let Some(fw) = detected_framework {
        println!("  Framework: {}", fw);
    }
    println!("\nNext steps:");
    println!("  cd {}", project_name);
    if !skip_install {
        println!("  # Dependencies are already installed");
    } else {
        println!("  # Install dependencies: npm install (or pnpm/yarn/bun)");
    }
    if detected_framework.is_some() {
        println!("  # Start development server: appz dev");
    }

    Ok(None)
}
