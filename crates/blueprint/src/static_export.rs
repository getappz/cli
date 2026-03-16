//! Static site export for WordPress projects.
//!
//! Uses the Simply Static plugin to export a WordPress site as static HTML,
//! suitable for deployment to Vercel, Netlify, Cloudflare Pages, etc.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::runtime::{RuntimeError, WordPressRuntime};

/// Default output directory name for static exports.
const DEFAULT_OUTPUT_DIR: &str = "static-export";

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
    /// 2. Configure it to export to `output_dir`
    /// 3. Trigger the export
    /// 4. Wait for completion
    ///
    /// Returns the path to the output directory containing the static files.
    pub fn export(&self, output_dir: Option<&Path>) -> Result<PathBuf, RuntimeError> {
        let output = output_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| self.project_path.join(DEFAULT_OUTPUT_DIR));

        // Ensure output directory exists
        std::fs::create_dir_all(&output).map_err(|e| RuntimeError::Io {
            path: output.clone(),
            source: e,
        })?;

        println!("Installing Simply Static plugin...");
        self.install_simply_static()?;

        println!("Configuring export to {}...", output.display());
        self.configure_simply_static(&output)?;

        println!("Triggering static export...");
        self.trigger_export()?;

        println!("Waiting for export to complete...");
        self.wait_for_export()?;

        println!("Static export complete: {}", output.display());
        Ok(output)
    }

    fn install_simply_static(&self) -> Result<(), RuntimeError> {
        self.runtime.wp_cli(
            &self.project_path,
            &["plugin", "install", "simply-static", "--activate"],
        )
    }

    fn configure_simply_static(&self, output_dir: &Path) -> Result<(), RuntimeError> {
        let output_str = output_dir.display().to_string();

        // Set delivery method to local directory using wp option patch
        self.runtime.wp_cli(
            &self.project_path,
            &["option", "patch", "update", "simply-static", "delivery_method", "local"],
        )?;

        // Set the local directory path
        self.runtime.wp_cli(
            &self.project_path,
            &["option", "patch", "update", "simply-static", "local_dir", &output_str],
        )?;

        Ok(())
    }

    fn trigger_export(&self) -> Result<(), RuntimeError> {
        // Simply Static provides WP-CLI commands in the Pro version.
        // For the free version, trigger via PHP eval.
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

            // Check if export is complete by querying the task status
            if let Some(status) = self.runtime.wp_cli_output(
                &self.project_path,
                &["eval", "echo get_option(\"simply-static-task-status\", \"done\");"],
            ) {
                let status = status.trim();
                if status == "done" || status.is_empty() {
                    return Ok(());
                }
            } else {
                // If we can't read the status, assume done (may not have the option)
                return Ok(());
            }

            print!(".");
            let _ = std::io::stdout().flush();
            std::thread::sleep(poll);
        }
    }
}
