use crate::session::AppzSession;
use camino::Utf8PathBuf;
use miette::{miette, Result};
use ssg_migrator::{analyze_project, generate_astro_project, MigrationConfig};
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
) -> AppResult {
    // Determine source directory
    let source_dir = if let Some(src) = source {
        Utf8PathBuf::from_path_buf(src)
            .map_err(|_| miette!("Source path must be valid UTF-8"))?
    } else {
        Utf8PathBuf::from_path_buf(session.working_dir.clone())
            .map_err(|_| miette!("Working directory path must be valid UTF-8"))?
    };

    if !source_dir.exists() {
        return Err(miette!("Source directory does not exist: {}", source_dir).into());
    }

    // Determine output directory
    let output_dir = if let Some(out) = output {
        Utf8PathBuf::from_path_buf(out)
            .map_err(|_| miette!("Output path must be valid UTF-8"))?
    } else {
        let project_name = name.as_deref().unwrap_or("migrated-astro-app");
        let working_dir = Utf8PathBuf::from_path_buf(session.working_dir.clone())
            .map_err(|_| miette!("Working directory path must be valid UTF-8"))?;
        working_dir.join(project_name)
    };

    // Determine project name
    let project_name = name.unwrap_or_else(|| {
        output_dir
            .file_name()
            .unwrap_or("migrated-astro-app")
            .to_string()
    });

    println!("Analyzing React SPA at: {}", source_dir);
    let analysis = analyze_project(&source_dir)?;

    println!("Found {} routes and {} components", analysis.routes.len(), analysis.components.len());

    let config = MigrationConfig {
        source_dir,
        output_dir: output_dir.clone(),
        project_name,
        force,
    };

    println!("Generating Astro project at: {}", output_dir);
    generate_astro_project(&config, &analysis)?;

    println!("Migration complete! Generated Astro project at: {}", output_dir);
    println!("Next steps:");
    println!("  cd {}", output_dir);
    println!("  npm install");
    println!("  npm run dev");

    Ok(None)
}

