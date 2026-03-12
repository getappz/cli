use crate::client::Client;
use crate::error::ApiError;
use crate::models::{map_better_auth_user_to_cli_user, BetterAuthUserResponse, User};
use crate::paths::V0_PREFIX;

pub struct Users<'a> {
    client: &'a Client,
}

impl<'a> Users<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Retrieves information related to the currently authenticated User.
    /// Maps Better Auth user shape to CLI User model.
    #[tracing::instrument(skip(self))]
    pub async fn get_current(&self) -> Result<User, ApiError> {
        let path = format!("{}/user", V0_PREFIX);
        let response: BetterAuthUserResponse = self.client.get(&path).await?;
        Ok(map_better_auth_user_to_cli_user(response.user))
    }

    /// Get telemetry preference for the current user (requires auth).
    #[tracing::instrument(skip(self))]
    pub async fn get_telemetry(&self) -> Result<TelemetryPreference, ApiError> {
        let path = format!("{}/user/telemetry", V0_PREFIX);
        self.client.get(&path).await
    }

    /// Set telemetry preference for the current user (requires auth).
    #[tracing::instrument(skip(self))]
    pub async fn set_telemetry(&self, enabled: bool) -> Result<TelemetryPreference, ApiError> {
        let path = format!("{}/user/telemetry", V0_PREFIX);
        self.client
            .put_json(&path, &TelemetryPreference { enabled })
            .await
    }
}

/// Telemetry preference response from the API.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TelemetryPreference {
    pub enabled: bool,
}
