//! NPM provider: download from npm registry.

use async_trait::async_trait;
use tracing::instrument;

use crate::config::InitContext;
use crate::error::{InitError, InitResult};
use crate::output::InitOutput;
use crate::provider::InitProvider;
use crate::providers::git::{detect_framework, run_install_if_needed};
use crate::sources::npm::download_npm;
use crate::ui;

pub struct NpmProvider;

#[async_trait]
impl InitProvider for NpmProvider {
    fn name(&self) -> &str {
        "NPM"
    }

    fn slug(&self) -> &str {
        "npm"
    }

    #[instrument(skip_all)]
    async fn init(&self, ctx: &InitContext) -> InitResult<InitOutput> {
        let project_path = ctx.options.project_path();
        let quiet = ctx.options.json_output || ctx.options.is_ci;

        ui::section_title(&ctx.options, "Downloading npm package...");

        let template_dir = download_npm(&ctx.source, quiet).await?;

        ui::info(&ctx.options, "Copying files to project...");

        ctx.fs()
            .copy_from_external(&template_dir, ".")
            .map_err(|e| InitError::FsError(format!("Copy failed: {}", e)))?;

        let framework = detect_framework(&project_path)?;
        run_install_if_needed(ctx, &framework).await?;

        Ok(InitOutput {
            project_path,
            framework,
            installed: !ctx.options.skip_install,
        })
    }
}
