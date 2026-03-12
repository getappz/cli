use crate::client::Client;
use crate::error::ApiError;
use crate::models::{
    AuthorizationServerMetadata, DeviceAuthorizationResponse, SignInRequest, SignInResponse,
    TokenIntrospection, TokenSet, VerifyResponse,
};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Auth {
    client: std::sync::Arc<Client>,
    // Cache for authorization server metadata
    metadata_cache: Arc<RwLock<Option<AuthorizationServerMetadata>>>,
}

impl Auth {
    pub fn new(client: std::sync::Arc<Client>) -> Self {
        Self {
            client,
            metadata_cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Request a new login for a user to get a token.
    /// This will respond with a verification token and send an email to confirm the request.
    #[tracing::instrument(skip(self))]
    pub async fn signin(
        &self,
        email: String,
        token_name: Option<String>,
    ) -> Result<SignInResponse, ApiError> {
        let request = SignInRequest {
            email,
            tokenName: token_name,
        };

        self.client.post("/auth/signin", Some(request)).await
    }

    /// Verify the user accepted the login request and get an authentication token.
    /// The token is required. The email is optional - include it if the API requires it.
    /// Parameters are added to the URL as a query string.
    #[tracing::instrument(skip(self, token))]
    pub async fn verify(
        &self,
        token: String,
        email: Option<String>,
        team_id: Option<String>,
    ) -> Result<VerifyResponse, ApiError> {
        let mut query_params: Vec<(String, Option<String>)> =
            vec![("token".to_string(), Some(token))];

        // Add email to query params if provided
        if let Some(email) = email {
            query_params.push(("email".to_string(), Some(email)));
        }

        // Temporarily set team_id if provided
        if let Some(ref team_id_val) = team_id {
            self.client.set_team_id(Some(team_id_val.clone())).await;
        }

        let result = self
            .client
            .get_with_query("/auth/verify", query_params)
            .await;

        // Reset team_id if we set it
        if team_id.is_some() {
            self.client.set_team_id(None).await;
        }

        result
    }

    /// Get OpenID Connect discovery metadata (cached)
    #[tracing::instrument(skip(self))]
    pub async fn discovery(&self) -> Result<AuthorizationServerMetadata, ApiError> {
        // Check cache first
        {
            let cache = self.metadata_cache.read().await;
            if let Some(ref metadata) = *cache {
                return Ok(metadata.clone());
            }
        }

        // Fetch from server
        let metadata: AuthorizationServerMetadata =
            self.client.get("/.well-known/openid-configuration").await?;

        // Update cache
        {
            let mut cache = self.metadata_cache.write().await;
            *cache = Some(metadata.clone());
        }

        Ok(metadata)
    }

    /// Request device authorization
    #[tracing::instrument(skip(self))]
    pub async fn device_authorize(
        &self,
        client_id: &str,
    ) -> Result<DeviceAuthorizationResponse, ApiError> {
        let metadata = self.discovery().await?;
        let form_data = [("client_id", client_id), ("scope", "openid offline_access")];

        // Normalize the endpoint URL to use the client's base URL
        let endpoint = self.client.normalize_endpoint_url(&metadata.device_authorization_endpoint);

        self.client.post_form(&endpoint, &form_data).await
    }

    /// Poll for device token
    #[tracing::instrument(skip(self, device_code))]
    pub async fn device_token(
        &self,
        client_id: &str,
        device_code: &str,
    ) -> Result<Result<TokenSet, OAuthPollError>, ApiError> {
        let metadata = self.discovery().await?;
        
        // Normalize the endpoint URL to use the client's base URL
        let url = self.client.normalize_endpoint_url(&metadata.token_endpoint);

        // Make request directly to handle OAuth errors properly
        let response = self
            .client
            .post_form_raw(
                &url,
                &[
                    ("client_id", client_id),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("device_code", device_code),
                ],
            )
            .await?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| ApiError::HttpMiddleware(e.to_string()))?;

        if status.is_success() {
            let token_set: TokenSet = serde_json::from_str(&text).map_err(ApiError::Json)?;
            Ok(Ok(token_set))
        } else {
            // Parse OAuth error
            let oauth_error: crate::models::OAuthErrorResponse = serde_json::from_str(&text)
                .unwrap_or_else(|_| crate::models::OAuthErrorResponse {
                    error: "unknown_error".to_string(),
                    error_description: Some(text.clone()),
                    error_uri: None,
                });

            match oauth_error.error.as_str() {
                "authorization_pending" => Ok(Err(OAuthPollError::AuthorizationPending)),
                "slow_down" => Ok(Err(OAuthPollError::SlowDown)),
                "access_denied" => Err(ApiError::OAuthError(format!(
                    "Access denied: {}",
                    oauth_error
                        .error_description
                        .unwrap_or_else(|| "User denied authorization".to_string())
                ))),
                "expired_token" => Err(ApiError::ExpiredToken),
                _ => Err(ApiError::OAuthError(format!(
                    "OAuth error: {} - {}",
                    oauth_error.error,
                    oauth_error
                        .error_description
                        .unwrap_or_else(|| "Unknown error".to_string())
                ))),
            }
        }
    }

    /// Refresh access token
    #[tracing::instrument(skip(self, refresh_token))]
    pub async fn refresh_token(
        &self,
        client_id: &str,
        refresh_token: &str,
    ) -> Result<TokenSet, ApiError> {
        let metadata = self.discovery().await?;
        let form_data = [
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];

        // Normalize the endpoint URL to use the client's base URL
        let endpoint = self.client.normalize_endpoint_url(&metadata.token_endpoint);

        self.client.post_form(&endpoint, &form_data).await
    }

    /// Introspect token
    #[tracing::instrument(skip(self, token))]
    pub async fn introspect_token(&self, token: &str) -> Result<TokenIntrospection, ApiError> {
        let metadata = self.discovery().await?;
        let form_data = [("token", token)];

        // Normalize the endpoint URL to use the client's base URL
        let endpoint = self.client.normalize_endpoint_url(&metadata.introspection_endpoint);

        self.client.post_form(&endpoint, &form_data).await
    }

    /// Revoke token
    #[tracing::instrument(skip(self, token))]
    pub async fn revoke_token(&self, client_id: &str, token: &str) -> Result<(), ApiError> {
        let metadata = self.discovery().await?;
        
        // Normalize the endpoint URL to use the client's base URL
        let endpoint = self.client.normalize_endpoint_url(&metadata.revocation_endpoint);
        
        let response = self
            .client
            .post_form_raw(
                &endpoint,
                &[("token", token), ("client_id", client_id)],
            )
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(ApiError::OAuthError("Token revocation failed".to_string()))
        }
    }
}

/// OAuth polling errors (non-fatal, should retry)
#[derive(Debug, Clone, Copy)]
pub enum OAuthPollError {
    AuthorizationPending,
    SlowDown,
}
