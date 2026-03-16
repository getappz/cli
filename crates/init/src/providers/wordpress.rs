//! WordPress provider: download full WordPress from wordpress.org and set up.
//!
//! Downloads latest.tar.gz, extracts, and removes default plugins.
//! Leaves wp-config-sample.php as-is so DDEV can manage wp-config.php.

use async_trait::async_trait;
use tracing::instrument;

use crate::config::InitContext;
use crate::error::{InitError, InitResult};
use crate::output::InitOutput;
use crate::provider::InitProvider;
use crate::sources::remote_archive::download_remote_archive;
use crate::ui;

const WORDPRESS_ARCHIVE_URL: &str = "https://wordpress.org/latest.tar.gz";

pub struct WordPressProvider;

#[async_trait]
impl InitProvider for WordPressProvider {
    fn name(&self) -> &str {
        "WordPress"
    }

    fn slug(&self) -> &str {
        "wordpress"
    }

    #[instrument(skip_all)]
    async fn init(&self, ctx: &InitContext) -> InitResult<InitOutput> {
        let project_path = ctx.options.project_path();
        let quiet = ctx.options.json_output || ctx.options.is_ci;

        ui::section_title(&ctx.options, "Downloading WordPress...");

        let template_dir =
            download_remote_archive(WORDPRESS_ARCHIVE_URL, quiet).await?;

        ui::info(&ctx.options, "Copying files to project...");

        ctx.fs()
            .copy_from_external(&template_dir, ".")
            .map_err(|e| InitError::FsError(format!("Copy failed: {}", e)))?;

        ui::info(&ctx.options, "Configuring WordPress...");

        let fs = ctx.fs();

        // Remove default plugins (keep directory)
        let plugins_dir = "wp-content/plugins";
        if fs.exists(plugins_dir) && fs.is_dir(plugins_dir) {
            fs.remove_dir_all(plugins_dir)
                .map_err(|e| InitError::FsError(format!("Empty plugins: {}", e)))?;
        }
        fs.create_dir_all(plugins_dir)
            .map_err(|e| InitError::FsError(format!("Create plugins dir: {}", e)))?;
        fs.write_string(
            "wp-content/plugins/index.php",
            "<?php\n// Silence is golden.\n",
        )
        .map_err(|e| InitError::FsError(format!("Write plugins index: {}", e)))?;


        Ok(InitOutput {
            project_path,
            framework: Some("WordPress".to_string()),
            installed: false,
        })
    }
}
