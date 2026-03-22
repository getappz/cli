//! Blueprint provider: initializes projects from universal blueprint definitions.

use async_trait::async_trait;

use crate::config::InitContext;
use crate::error::{InitError, InitResult};
use crate::output::InitOutput;
use crate::provider::InitProvider;

pub struct BlueprintProvider;

#[async_trait]
impl InitProvider for BlueprintProvider {
    fn name(&self) -> &str {
        "Blueprint"
    }

    fn slug(&self) -> &str {
        "blueprint"
    }

    async fn init(&self, _ctx: &InitContext) -> InitResult<InitOutput> {
        Err(InitError::SourceNotFound(
            "Blueprint provider not yet implemented".to_string(),
        ))
    }
}
