use crate::session::AppzSession;
use camino::Utf8PathBuf;
use miette::miette;
use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};
use ssg_migrator::{
    analyze_project, generate_astro_project, generate_nextjs_project, MigrationConfig,
};
use starbase::AppResult;
use std::path::PathBuf;
use tracing::instrument;

#[instrument(skip_all)]
pub async fn migrate(
    session: AppzSession,
    source: Option<PathBuf>,
    output: Option<PathBuf>,
    name: Option<String>,
    force: bool,
    target: String,
    static_export: bool,
) -> AppResult {
    // Determine source directory (resolve relative paths against session working_dir)
    let source_dir = if let Some(src) = source {
        let path = if src.is_absolute() {
            src
        } else {
            session.working_dir.join(src)
        };
        Utf8PathBuf::from_path_buf(path)
            .map_err(|_| miette!("Source path must be valid UTF-8"))?
    } else {
        Utf8PathBuf::from_path_buf(session.working_dir.clone())
            .map_err(|_| miette!("Working directory path must be valid UTF-8"))?
    };

    if !source_dir.exists() {
        return Err(miette!("Source directory does not exist: {}", source_dir));
    }

    // Canonicalize to resolve ./ and .. for reliable path operations (e.g. copy_from_external)
    let source_dir = Utf8PathBuf::from_path_buf(
        source_dir
            .as_path()
            .canonicalize()
            .map_err(|e| miette!("Failed to resolve source path: {}", e))?,
    )
    .map_err(|_| miette!("Source path must be valid UTF-8"))?;

    let default_project_name = match target.as_str() {
        "nextjs" => "nextjs",
        _ => "migrated-astro-app",
    };

    // Determine output directory (resolve relative paths against session working_dir)
    let output_dir = if let Some(out) = output {
        let path = if out.is_absolute() {
            out
        } else {
            session.working_dir.join(out)
        };
        Utf8PathBuf::from_path_buf(path)
            .map_err(|_| miette!("Output path must be valid UTF-8"))?
    } else {
        let project_name = name.as_deref().unwrap_or(default_project_name);
        let working_dir = Utf8PathBuf::from_path_buf(session.working_dir.clone())
            .map_err(|_| miette!("Working directory path must be valid UTF-8"))?;
        working_dir.join(project_name)
    };

    // Determine project name
    let project_name = name.unwrap_or_else(|| {
        output_dir
            .file_name()
            .unwrap_or(default_project_name)
            .to_string()
    });

    let _ = ui::status::info(&format!("Analyzing React SPA at: {}", source_dir));
    let analysis = analyze_project(&source_dir)?;

    let _ = ui::status::info(&format!(
        "Found {} routes and {} components",
        analysis.routes.len(),
        analysis.components.len()
    ));

    let config = MigrationConfig {
        source_dir: source_dir.clone(),
        output_dir: output_dir.clone(),
        project_name: project_name.clone(),
        force,
        static_export,
    };

    match target.as_str() {
        "nextjs" => {
            if output_dir.exists() && force {
                std::fs::remove_dir_all(output_dir.as_path())
                    .map_err(|e| miette!("Failed to remove existing directory: {}", e))?;
            }
            if output_dir.exists() && !force {
                return Err(miette!(
                    "Output directory already exists: {}. Use --force to overwrite.",
                    output_dir
                ));
            }

            let _ = ui::status::info("Creating sandbox and generating Next.js project...");

            let sandbox_config = SandboxConfig::new(output_dir.as_path())
                .with_settings(SandboxSettings::default().with_tool("bun", Some("latest")));

            let sandbox = create_sandbox(sandbox_config)
                .await
                .map_err(|e| miette!("Failed to create sandbox: {}", e))?;

            let fs = sandbox.fs();

            generate_nextjs_project(&config, &analysis, fs)
                .map_err(|e| miette!("Failed to generate Next.js project: {}", e))?;

            let _ = ui::status::info("Installing dependencies...");
            let install_out = sandbox
                .exec("bun install")
                .await
                .map_err(|e| miette!("Failed to run bun install: {}", e))?;

            if !install_out.success() {
                return Err(miette!(
                    "bun install failed: {}",
                    install_out.stderr().trim()
                ));
            }

            let _ = ui::status::success("Migration complete!");
            println!("Generated Next.js project at: {}", output_dir);
            println!("Next steps:");
            println!("  cd {}", output_dir);
            println!("  appz dev");
        }
        _ => {
            let _ = ui::status::info(&format!("Generating Astro project at: {}", output_dir));
            generate_astro_project(&config, &analysis)?;

            let _ = ui::status::success("Migration complete!");
            println!("Generated Astro project at: {}", output_dir);
            println!("Next steps:");
            println!("  cd {}", output_dir);
            println!("  npm install");
            println!("  appz dev");
        }
    }

    Ok(None)
}
