use crate::client::Client;
use crate::error::ApiError;
use crate::models::{DeleteResponse, Deployment, DeploymentsListResponse};
use crate::paths::V0_PREFIX;

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
