//! Local provider: copy from local path.

use std::path::PathBuf;

use async_trait::async_trait;
use tracing::instrument;

use crate::config::InitContext;
use crate::error::{InitError, InitResult};
use crate::output::InitOutput;
use crate::provider::InitProvider;
use crate::providers::git::{detect_framework, run_install_if_needed};
use crate::ui;

pub struct LocalProvider;

#[async_trait]
impl InitProvider for LocalProvider {
    fn name(&self) -> &str {
        "Local"
    }

    fn slug(&self) -> &str {
        "local"
    }

    #[instrument(skip_all)]
    async fn init(&self, ctx: &InitContext) -> InitResult<InitOutput> {
        let project_path = ctx.options.project_path();
        let template_path = PathBuf::from(&ctx.source);

        if !template_path.exists() {
            return Err(InitError::NotFound(format!(
                "Template path does not exist: {}",
                template_path.display()
            )));
        }

        if !template_path.is_dir() {
            return Err(InitError::InvalidFormat(format!(
                "Template path is not a directory: {}",
                template_path.display()
            )));
        }

        ui::section_title(&ctx.options, "Copying from local template...");

        ctx.fs()
            .copy_from_external(&template_path, ".")
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
