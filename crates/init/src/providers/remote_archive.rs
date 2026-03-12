//! Remote archive provider: download from any .zip/.tar.gz URL.

use async_trait::async_trait;
use tracing::instrument;

use crate::config::InitContext;
use crate::error::{InitError, InitResult};
use crate::output::InitOutput;
use crate::provider::InitProvider;
use crate::sources::remote_archive::download_remote_archive;
use crate::ui;

pub struct RemoteArchiveProvider;

#[async_trait]
impl InitProvider for RemoteArchiveProvider {
    fn name(&self) -> &str {
        "Remote Archive"
    }

    fn slug(&self) -> &str {
        "remote-archive"
    }

    #[instrument(skip_all)]
    async fn init(&self, ctx: &InitContext) -> InitResult<InitOutput> {
        let project_path = ctx.options.project_path();
        let quiet = ctx.options.json_output || ctx.options.is_ci;

        ui::section_title(&ctx.options, "Downloading archive...");

        let template_dir = download_remote_archive(&ctx.source, quiet).await?;

        ui::info(&ctx.options, "Copying files to project...");

        ctx.fs()
            .copy_from_external(&template_dir, ".")
            .map_err(|e| InitError::FsError(format!("Copy failed: {}", e)))?;

        let framework = crate::providers::git::detect_framework(&project_path)?;
        crate::providers::git::run_install_if_needed(ctx, &framework).await?;

        Ok(InitOutput {
            project_path,
            framework,
            installed: !ctx.options.skip_install,
        })
    }
}
