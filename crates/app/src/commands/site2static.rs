//! Static site export command (PRO feature).
//!
//! Scrapes any site and exports it as static HTML for deployment
//! to hosting providers (Vercel, Netlify, etc.).

use crate::ddev_helpers::ddev_web_container_name;
use crate::wp_runtime;
use crate::session::AppzSession;
use crate::shell::{run_local_with, RunOptions};
use crate::args::Site2StaticArgs;
use starbase::AppResult;
use std::path::PathBuf;
use task::Context;

pub async fn site2static(session: AppzSession, args: Site2StaticArgs) -> AppResult {
    let project_path = session.working_dir.clone();

    // Resolve runtime
    let runtime = wp_runtime::resolve(&project_path, args.playground)?;

    // Validate WordPress project
    let has_wp_files = project_path.join("wp-config.php").exists()
        || project_path.join("wp-config-sample.php").exists()
        || project_path.join("wp-content").exists();

    if !has_wp_files {
        return Err(miette::miette!(
            "No WordPress files found in {}. site2static requires a running site.",
            project_path.display()
        ));
    }

    // Ensure runtime is started
    println!("Starting {}...", runtime.name());
    runtime.start(&project_path)
        .map_err(|e| miette::miette!("{}", e))?;

    let host_output = args.output
        .unwrap_or_else(|| project_path.join("dist"));

    std::fs::create_dir_all(&host_output)
        .map_err(|e| miette::miette!("Failed to create output dir: {}", e))?;

    let exporter = blueprint::StaticExporter::new(project_path.clone(), runtime.clone());

    exporter
        .export(Some(host_output.as_path()), None)
        .map_err(|e| miette::miette!("Static export failed: {}", e))?;

    // For DDEV: if bind-mounted, files are already on host. If mutagen, need docker cp.
    if runtime.slug() == "ddev" {
        let has_files = std::fs::read_dir(&host_output)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false);

        if !has_files {
            sync_from_ddev(&project_path, &host_output).await?;
        }
    }

    let has_files = std::fs::read_dir(&host_output)
        .map(|mut d| d.next().is_some())
        .unwrap_or(false);

    if !has_files {
        return Err(miette::miette!(
            "Export completed but no files found in {}\n\
             Check Simply Static settings in WP admin or run:\n  \
             ddev exec ls -la /var/www/html/dist/",
            host_output.display()
        ));
    }

    let display_path = host_output.strip_prefix(&project_path).unwrap_or(&host_output);
    println!("\nStatic files exported to: {}", display_path.display());
    println!("\nYou can now deploy with:");
    println!("  appz deploy --platform vercel");
    println!("  appz deploy --platform netlify");

    Ok(None)
}

async fn sync_from_ddev(
    project_path: &std::path::Path,
    host_output: &std::path::Path,
) -> Result<(), miette::Report> {
    let container = ddev_web_container_name(project_path)
        .ok_or_else(|| miette::miette!("Could not determine DDEV web container name"))?;

    if let Some(parent) = host_output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let relative = host_output.strip_prefix(project_path).unwrap_or(host_output);
    let container_path = format!("/var/www/html/{}", relative.display());

    let copy_cmd = format!(
        "docker cp {}:{}/. {}",
        container,
        container_path,
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

    println!("Synced static files from DDEV container");
    Ok(())
}
