use crate::client::Client;
use crate::error::ApiError;
use crate::http::response_handler::error_from_status_body_headers;
use crate::models::{
    DeleteResponse, Deployment, DeploymentCreateRequest, DeploymentCreateResult,
    DeploymentsListResponse, PreparedFile,
};
use crate::paths::V0_PREFIX;
use reqwest_middleware::reqwest::StatusCode;

pub struct Deployments<'a> {
    client: &'a Client,
}

impl<'a> Deployments<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// List deployments with optional pagination and filters
    #[tracing::instrument(skip(self))]
    pub async fn list(
        &self,
        project_id: Option<String>,
        limit: Option<i64>,
        since: Option<i64>,
        until: Option<i64>,
        team_id: Option<String>,
    ) -> Result<DeploymentsListResponse, ApiError> {
        let query_params = vec![
            ("projectId", project_id),
            ("limit", limit.map(|l| l.to_string())),
            ("since", since.map(|s| s.to_string())),
            ("until", until.map(|u| u.to_string())),
        ];

        // Temporarily set team_id if provided
        if let Some(ref team_id_val) = team_id {
            self.client.set_team_id(Some(team_id_val.clone())).await;
        }

        let path = format!("{}/deployments", V0_PREFIX);
        let result = self.client.get_with_query(&path, &query_params).await;

        // Reset team_id if we set it
        if team_id.is_some() {
            self.client.set_team_id(None).await;
        }

        result
    }

    /// Get a deployment by ID or URL
    #[tracing::instrument(skip(self))]
    pub async fn get(&self, deployment_id_or_url: &str) -> Result<Deployment, ApiError> {
        // Extract deployment ID from URL if it's a URL
        let deployment_id = if deployment_id_or_url.starts_with("http://")
            || deployment_id_or_url.starts_with("https://")
        {
            // Extract ID from URL (format: https://xxx.appz.dev or similar)
            // For now, assume the ID is the last part after splitting by /
            deployment_id_or_url
                .split('/')
                .next_back()
                .unwrap_or(deployment_id_or_url)
        } else {
            deployment_id_or_url
        };

        let path = format!("{}/deployments/{}", V0_PREFIX, deployment_id);
        self.client.get(&path).await
    }

    /// Promote a deployment to production
    #[tracing::instrument(skip(self))]
    pub async fn promote(
        &self,
        project_id: &str,
        deployment_id: &str,
        team_id: Option<String>,
    ) -> Result<DeleteResponse, ApiError> {
        let path = format!("{}/projects/{}/promote/{}", V0_PREFIX, project_id, deployment_id);

        // Temporarily set team_id if provided
        if let Some(ref team_id_val) = team_id {
            self.client.set_team_id(Some(team_id_val.clone())).await;
        }

        let result = self.client.post(&path, Some(serde_json::json!({}))).await;

        // Reset team_id if we set it
        if team_id.is_some() {
            self.client.set_team_id(None).await;
        }

        result
    }

    /// Rollback to a previous deployment
    #[tracing::instrument(skip(self))]
    pub async fn rollback(
        &self,
        project_id: &str,
        deployment_id: &str,
        team_id: Option<String>,
    ) -> Result<DeleteResponse, ApiError> {
        let path = format!("{}/projects/{}/rollback/{}", V0_PREFIX, project_id, deployment_id);

        // Temporarily set team_id if provided
        if let Some(ref team_id_val) = team_id {
            self.client.set_team_id(Some(team_id_val.clone())).await;
        }

        let result = self.client.post(&path, Some(serde_json::json!({}))).await;

        // Reset team_id if we set it
        if team_id.is_some() {
            self.client.set_team_id(None).await;
        }

        result
    }

    /// Create a deployment (prebuilt flow).
    ///
    /// Sends file list to API. Returns either a created deployment or a list of
    /// missing file SHAs that must be uploaded before calling [`continue_deployment`](Self::continue_deployment).
    #[tracing::instrument(skip(self, payload))]
    pub async fn create_deployment(
        &self,
        payload: DeploymentCreateRequest,
    ) -> Result<DeploymentCreateResult, ApiError> {
        let path = format!("{}/deployments", V0_PREFIX);
        let response = self.client.post_raw(&path, Some(&payload)).await?;
        let status = response.status();
        let headers = response.headers().clone();
        let text = response
            .text()
            .await
            .map_err(|e| ApiError::HttpMiddleware(format!("Failed to read response: {}", e)))?;

        if status.is_success() {
            let deployment: Deployment = serde_json::from_str(&text).map_err(ApiError::Json)?;
            return Ok(DeploymentCreateResult::Created(deployment));
        }

        if status == StatusCode::BAD_REQUEST {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                let code = json
                    .get("error")
                    .and_then(|e| e.get("code"))
                    .or_else(|| json.get("code"))
                    .and_then(|c| c.as_str());
                if code == Some("missing_files") {
                    let missing = json
                        .get("error")
                        .and_then(|e| e.get("missing"))
                        .or_else(|| json.get("missing"))
                        .and_then(|m| serde_json::from_value(m.clone()).ok())
                        .unwrap_or_default();
                    let deployment_id = json
                        .get("error")
                        .and_then(|e| e.get("deploymentId").and_then(|v| v.as_str()))
                        .or_else(|| json.get("deploymentId").and_then(|v| v.as_str()))
                        .unwrap_or_default()
                        .to_string();
                    return Ok(DeploymentCreateResult::MissingFiles {
                        deployment_id,
                        missing,
                    });
                }
            }
        }

        Err(error_from_status_body_headers(
            status,
            &text,
            &headers,
        ))
    }

    /// Upload a single file to a deployment (content-addressed by SHA).
    /// Uses `POST /v0/deployments/:id/files` with `x-now-digest` and `x-now-size` headers.
    #[tracing::instrument(skip(self, data))]
    pub async fn upload_file(
        &self,
        deployment_id: &str,
        sha: &str,
        data: Vec<u8>,
    ) -> Result<(), ApiError> {
        let path = format!("{}/deployments/{}/files", V0_PREFIX, deployment_id);
        let size_str = data.len().to_string();
        let headers: [(&str, &str); 3] = [
            ("Content-Type", "application/octet-stream"),
            ("x-now-digest", sha),
            ("x-now-size", size_str.as_str()),
        ];
        let response = self
            .client
            .post_bytes(&path, data, &headers)
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let headers = response.headers().clone();
            let text = response.text().await.unwrap_or_default();
            return Err(error_from_status_body_headers(status, &text, &headers));
        }
        Ok(())
    }

    /// Continue deployment after uploading files (Vercel-aligned flow).
    /// May return `MissingFiles` if more files are needed.
    #[tracing::instrument(skip(self, files))]
    pub async fn continue_deployment(
        &self,
        deployment_id: &str,
        files: Vec<PreparedFile>,
    ) -> Result<DeploymentCreateResult, ApiError> {
        let path = format!("{}/deployments/{}/continue", V0_PREFIX, deployment_id);
        let body = serde_json::json!({ "files": files });

        let response = self
            .client
            .post_raw(&path, Some(&body))
            .await?;
        let status = response.status();
        let headers = response.headers().clone();
        let text = response
            .text()
            .await
            .map_err(|e| ApiError::HttpMiddleware(format!("Failed to read response: {}", e)))?;

        if status.is_success() {
            let deployment: Deployment = serde_json::from_str(&text).map_err(ApiError::Json)?;
            return Ok(DeploymentCreateResult::Created(deployment));
        }

        if status == StatusCode::BAD_REQUEST {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                let code = json
                    .get("error")
                    .and_then(|e| e.get("code"))
                    .or_else(|| json.get("code"))
                    .and_then(|c| c.as_str());
                if code == Some("missing_files") {
                    let missing = json
                        .get("error")
                        .and_then(|e| e.get("missing"))
                        .or_else(|| json.get("missing"))
                        .and_then(|m| serde_json::from_value(m.clone()).ok())
                        .unwrap_or_default();
                    let deployment_id = json
                        .get("error")
                        .and_then(|e| e.get("deploymentId").and_then(|v| v.as_str()))
                        .or_else(|| json.get("deploymentId").and_then(|v| v.as_str()))
                        .map(str::to_string)
                        .unwrap_or_else(|| deployment_id.to_string());
                    return Ok(DeploymentCreateResult::MissingFiles {
                        deployment_id,
                        missing,
                    });
                }
            }
        }

        Err(error_from_status_body_headers(
            status,
            &text,
            &headers,
        ))
    }

    /// Delete a deployment by ID or URL (soft delete)
    #[tracing::instrument(skip(self))]
    pub async fn delete(&self, deployment_id_or_url: &str) -> Result<DeleteResponse, ApiError> {
        let deployment_id = if deployment_id_or_url.starts_with("http://")
            || deployment_id_or_url.starts_with("https://")
        {
            deployment_id_or_url
                .split('/')
                .next_back()
                .unwrap_or(deployment_id_or_url)
        } else {
            deployment_id_or_url
        };

        let path = format!("{}/deployments/{}", V0_PREFIX, deployment_id);
        self.client.delete(&path).await
    }
}
