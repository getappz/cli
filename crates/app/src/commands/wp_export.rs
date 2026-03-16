//! WordPress static site export command.
//!
//! Exports a WordPress site as static HTML using the Simply Static plugin,
//! then optionally deploys to a hosting provider (Vercel, Netlify, etc.).

use crate::ddev_helpers::ddev_web_container_name;
use crate::wp_runtime;
use crate::session::AppzSession;
use crate::shell::{run_local_with, RunOptions};
use crate::args::WpExportArgs;
use starbase::AppResult;
use std::path::PathBuf;
use task::Context;

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

    let host_output = args.output
        .unwrap_or_else(|| project_path.join(".appz/output/static"));

    // Ensure the output dir exists on the host BEFORE export.
    // For DDEV bind-mounted projects, this also creates it inside the container.
    std::fs::create_dir_all(&host_output)
        .map_err(|e| miette::miette!("Failed to create output dir: {}", e))?;

    let exporter = blueprint::StaticExporter::new(project_path.clone(), runtime.clone());

    exporter
        .export(Some(host_output.as_path()))
        .map_err(|e| miette::miette!("Static export failed: {}", e))?;

    // For DDEV: if bind-mounted, files are already on host. If mutagen, need docker cp.
    if runtime.slug() == "ddev" {
        let has_files = std::fs::read_dir(&host_output)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false);

        if !has_files {
            // Files not on host — likely mutagen mode, need docker cp
            sync_from_ddev(&project_path, &host_output).await?;
        }
    }

    // Final check
    let has_files = std::fs::read_dir(&host_output)
        .map(|mut d| d.next().is_some())
        .unwrap_or(false);

    if !has_files {
        return Err(miette::miette!(
            "Export completed but no files found in {}\n\
             Check Simply Static settings in WP admin or run:\n  \
             ddev exec ls -la /var/www/html/.appz/output/static/",
            host_output.display()
        ));
    }

    let display_path = host_output.strip_prefix(&project_path).unwrap_or(&host_output);
    println!("\n✓ Static files exported to: {}", display_path.display());
    println!("\nYou can now deploy with:");
    println!("  appz deploy --platform vercel");
    println!("  appz deploy --platform netlify");

    Ok(None)
}

/// Sync the static export output from the DDEV container to the host filesystem.
/// Only needed for mutagen-mode DDEV projects where the filesystem isn't bind-mounted.
async fn sync_from_ddev(
    project_path: &std::path::Path,
    host_output: &std::path::Path,
) -> Result<(), miette::Report> {
    let container = ddev_web_container_name(project_path)
        .ok_or_else(|| miette::miette!("Could not determine DDEV web container name"))?;

    // Ensure host output parent dir exists
    if let Some(parent) = host_output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // For docker cp, we need to copy the contents, not replace the dir
    let copy_cmd = format!(
        "docker cp {}:/var/www/html/.appz/output/static/. {}",
        container,
        host_output.display()
    );

    println!("Syncing static files from DDEV container...");

    let mut ctx = Context::new();
    ctx.set_working_path(project_path.to_path_buf());
    let opts = RunOptions {
        cwd: Some(project_path.to_path_buf()),
        env: None,
        show_output: false,
        package_manager: None,
        tool_info: None,
    };
    run_local_with(&ctx, &copy_cmd, opts).await?;

    println!("✓ Synced static files from DDEV container");
    Ok(())
}
