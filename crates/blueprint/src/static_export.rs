//! Static site export for WordPress projects.
//!
//! Uses the Simply Static plugin to export a WordPress site as static HTML,
//! suitable for deployment to Vercel, Netlify, Cloudflare Pages, etc.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::runtime::{RuntimeError, WordPressRuntime};

/// Default output directory name for static exports (relative to project root).
/// Matches the `.appz/output/static` convention used by `appz preview`.
const DEFAULT_OUTPUT_DIR: &str = ".appz/output/static";

/// DDEV container web root where the project is mounted.
const DDEV_WEB_ROOT: &str = "/var/www/html";

/// Maximum time to wait for export completion (seconds).
const EXPORT_TIMEOUT_SECS: u64 = 300;

/// Poll interval when checking export status (seconds).
const POLL_INTERVAL_SECS: u64 = 2;

/// Exports a WordPress site as static HTML using the Simply Static plugin.
pub struct StaticExporter {
    project_path: PathBuf,
    runtime: Arc<dyn WordPressRuntime>,
}

impl StaticExporter {
    pub fn new(project_path: PathBuf, runtime: Arc<dyn WordPressRuntime>) -> Self {
        Self {
            project_path,
            runtime,
        }
    }

    /// Run the full static export pipeline.
    ///
    /// 1. Install and activate the Simply Static plugin
    /// 2. Configure it to export to the correct path inside the runtime
    /// 3. Trigger the export
    /// 4. Wait for completion
    ///
    /// Returns the host-side path to the output directory.
    pub fn export(&self, output_dir: Option<&Path>) -> Result<PathBuf, RuntimeError> {
        let host_output = output_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| self.project_path.join(DEFAULT_OUTPUT_DIR));

        // The relative path from project root (e.g. ".appz/output/static")
        let relative_output = host_output
            .strip_prefix(&self.project_path)
            .unwrap_or_else(|_| Path::new(DEFAULT_OUTPUT_DIR));

        // The path inside the runtime container
        let container_output = if self.runtime.slug() == "ddev" {
            format!("{}/{}", DDEV_WEB_ROOT, relative_output.display())
        } else {
            host_output.display().to_string()
        };

        println!("Installing Simply Static plugin...");
        self.install_simply_static()?;

        let display_path = relative_output.display();
        println!("Configuring export to {}...", display_path);
        self.configure_simply_static(&container_output)?;

        // Create the output dir inside the container so Simply Static can write to it
        self.runtime.exec_shell(
            &self.project_path,
            &format!("mkdir -p {}", container_output),
        )?;

        println!("Triggering static export...");
        self.trigger_export()?;

        println!("Waiting for export to complete...");
        self.wait_for_export()?;

        println!("Static export complete: {}", display_path);
        Ok(host_output)
    }

    fn install_simply_static(&self) -> Result<(), RuntimeError> {
        self.runtime.wp_cli(
            &self.project_path,
            &["plugin", "install", "simply-static", "--force", "--activate"],
        )
    }

    fn configure_simply_static(&self, container_output_dir: &str) -> Result<(), RuntimeError> {
        // Set delivery method to local directory
        self.runtime.wp_cli(
            &self.project_path,
            &["option", "patch", "update", "simply-static", "delivery_method", "local"],
        )?;

        // Set the local directory path (must be absolute path inside the container)
        self.runtime.wp_cli(
            &self.project_path,
            &["option", "patch", "update", "simply-static", "local_dir", container_output_dir],
        )?;

        Ok(())
    }

    fn trigger_export(&self) -> Result<(), RuntimeError> {
        // Try WP-CLI command first (Simply Static Pro), fall back to PHP action
        let result = self.runtime.wp_cli(
            &self.project_path,
            &["simply-static", "run"],
        );

        match result {
            Ok(()) => Ok(()),
            Err(_) => {
                // Fallback: trigger export via PHP
                self.runtime.wp_cli(
                    &self.project_path,
                    &["eval", "do_action('simply_static_site_export_start');"],
                )
            }
        }
    }

    fn wait_for_export(&self) -> Result<(), RuntimeError> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(EXPORT_TIMEOUT_SECS);
        let poll = std::time::Duration::from_secs(POLL_INTERVAL_SECS);

        loop {
            if start.elapsed() > timeout {
                return Err(RuntimeError::CommandFailed {
                    command: "static export".into(),
                    message: format!("Export timed out after {}s", EXPORT_TIMEOUT_SECS),
                });
            }

            // Check if export is complete
            if let Some(status) = self.runtime.wp_cli_output(
                &self.project_path,
                &["eval", "echo get_option('simply-static-task-status', 'done');"],
            ) {
                let status = status.trim();
                if status == "done" || status.is_empty() {
                    return Ok(());
                }
            } else {
                // Can't read status — assume done
                return Ok(());
            }

            print!(".");
            let _ = std::io::stdout().flush();
            std::thread::sleep(poll);
        }
    }
}
