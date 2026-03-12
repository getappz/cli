use crate::client::Client;
use crate::error::ApiError;
use std::sync::Arc;
use serde::Serialize;

/// Request body for the generate (AI code gen) endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct GenerateRequest {
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

pub struct Gen {
    client: Arc<Client>,
}

impl Gen {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Call the Appz backend to generate code from a prompt.
    /// Returns the raw generated text (file blocks, packages, commands in open-lovable format).
    #[tracing::instrument(skip(self))]
    pub async fn generate(
        &self,
        prompt: String,
        model: Option<String>,
    ) -> Result<String, ApiError> {
        let request = GenerateRequest { prompt, model };
        self.client
            .post_return_text("/v1/gen", Some(request))
            .await
    }
}
