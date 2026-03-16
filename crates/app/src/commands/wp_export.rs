//! WordPress static site export command.
//!
//! Exports a WordPress site as static HTML using the Simply Static plugin,
//! then optionally deploys to a hosting provider (Vercel, Netlify, etc.).

use crate::wp_runtime;
use crate::session::AppzSession;
use crate::args::WpExportArgs;
use starbase::AppResult;
use std::path::PathBuf;

pub async fn wp_export(session: AppzSession, args: WpExportArgs) -> AppResult {
    let project_path = session.working_dir.clone();

    // Resolve runtime
    let runtime = wp_runtime::resolve(&project_path, args.playground)?;

    // Validate WordPress project
    let has_wp_files = project_path.join("wp-config.php").exists()
        || project_path.join("wp-config-sample.php").exists()
        || project_path.join("wp-content").exists();

    if !has_wp_files {
        return Err(miette::miette!(
            "No WordPress files found in {}. wp-export requires a WordPress project.",
            project_path.display()
        ));
    }

    // Ensure runtime is started (DDEV starts containers, Playground writes state)
    println!("🚀 Starting {}...", runtime.name());
    runtime.start(&project_path)
        .map_err(|e| miette::miette!("{}", e))?;

    let output_dir = args.output;
    let exporter = blueprint::StaticExporter::new(project_path, runtime);

    let export_path = exporter
        .export(output_dir.as_deref())
        .map_err(|e| miette::miette!("Static export failed: {}", e))?;

    println!("\n✓ Static files exported to: {}", export_path.display());
    println!("\nYou can now deploy with:");
    println!("  appz deploy {} --provider vercel", export_path.display());
    println!("  appz deploy {} --provider netlify", export_path.display());

    Ok(None)
}
