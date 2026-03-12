use crate::client::Client;
use crate::error::ApiError;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

/// Response from the plugin entitlements endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntitlementsResponse {
    /// List of subscription tiers the user has access to (e.g., ["free", "pro"]).
    pub tiers: Vec<String>,
}

pub struct Plugins {
    client: Arc<Client>,
}

impl Plugins {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Get the plugin entitlements for the currently authenticated user.
    ///
    /// Returns the subscription tiers that determine which plugins are available.
    #[tracing::instrument(skip(self))]
    pub async fn get_entitlements(&self) -> Result<PluginEntitlementsResponse, ApiError> {
        self.client.get("/api/v1/plugins/entitlements").await
    }
}
