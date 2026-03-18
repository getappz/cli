//! Static site export for WordPress projects.
//!
//! Uses the Simply Static plugin (via the Appz Static Site Generator WP-CLI
//! wrapper) to export a WordPress site as static HTML, suitable for deployment
//! to Vercel, Netlify, Cloudflare Pages, etc.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::runtime::{RuntimeError, WordPressRuntime};

/// Default output directory name for static exports (relative to project root).
const DEFAULT_OUTPUT_DIR: &str = "dist";

/// DDEV container web root where the project is mounted.
const DDEV_WEB_ROOT: &str = "/var/www/html";

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
    /// 1. Install and activate Simply Static + Appz Static Site Generator plugins
    /// 2. Run `wp appz build --output-dir=<path>` (synchronous, no polling needed)
    ///
    /// Returns the host-side path to the output directory.
    pub fn export(&self, output_dir: Option<&Path>) -> Result<PathBuf, RuntimeError> {
        let host_output = output_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| self.project_path.join(DEFAULT_OUTPUT_DIR));

        // The relative path from project root (e.g. "dist")
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

        println!("Installing Appz Static Site Generator plugin...");
        self.install_appz_plugin()?;

        // Create the output dir inside the container so Simply Static can write to it
        self.runtime.exec_shell(
            &self.project_path,
            &format!("mkdir -p {}", container_output),
        )?;

        let display_path = relative_output.display();
        println!("Running static export to {}...", display_path);
        self.run_build(&container_output)?;

        println!("Static export complete: {}", display_path);
        Ok(host_output)
    }

    fn install_simply_static(&self) -> Result<(), RuntimeError> {
        self.runtime.wp_cli(
            &self.project_path,
            &["plugin", "install", "simply-static", "--force", "--activate"],
        )
    }

    fn install_appz_plugin(&self) -> Result<(), RuntimeError> {
        // Install from wordpress.org once approved; for now install from the
        // mu-plugins copy bundled with the project, or from the plugin directory.
        let result = self.runtime.wp_cli(
            &self.project_path,
            &["plugin", "install", "appz-static-site-generator", "--force", "--activate"],
        );

        match result {
            Ok(()) => Ok(()),
            Err(_) => {
                // Plugin not on wordpress.org yet — activate if already present
                self.runtime.wp_cli(
                    &self.project_path,
                    &["plugin", "activate", "appz-static-site-generator"],
                )
            }
        }
    }

    /// Run `wp appz build --output-dir=<path>` synchronously.
    ///
    /// The Appz plugin runs all Simply Static tasks in sequence without
    /// WP-Cron, so no polling is needed — the command blocks until complete.
    fn run_build(&self, container_output_dir: &str) -> Result<(), RuntimeError> {
        self.runtime.wp_cli(
            &self.project_path,
            &["appz", "build", &format!("--output-dir={}", container_output_dir)],
        )
    }
}
