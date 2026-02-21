use crate::client::Client;
use crate::error::ApiError;
use std::sync::Arc;
use crate::models::{
    AddEnvRequest, CreateProjectRequest, CreateTransferRequestBody, DeleteResponse, Project,
    ProjectEnvListResponse, ProjectEnvPullResponse, ProjectsListResponse,
    TransferRequestResponse,
};
use crate::paths::V0_PREFIX;

pub struct Projects {
    client: Arc<Client>,
}

impl Projects {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// List projects with optional pagination
    #[tracing::instrument(skip(self))]
    pub async fn list(
        &self,
        limit: Option<i64>,
        since: Option<i64>,
        until: Option<i64>,
    ) -> Result<ProjectsListResponse, ApiError> {
        let query_params: Vec<(String, Option<String>)> = vec![
            ("limit".to_string(), limit.map(|l| l.to_string())),
            ("since".to_string(), since.map(|s| s.to_string())),
            ("until".to_string(), until.map(|u| u.to_string())),
        ];

        let path = format!("{}/projects", V0_PREFIX);
        self.client.get_with_query(path, query_params).await
    }

    /// Create a new project
    #[tracing::instrument(skip(self))]
    pub async fn create(
        &self,
        slug: String,
        name: Option<String>,
        team_id: Option<String>,
    ) -> Result<Project, ApiError> {
        let request = CreateProjectRequest {
            slug,
            name,
            teamId: team_id,
        };
        let path = format!("{}/projects", V0_PREFIX);
        self.client.post(path, Some(request)).await
    }

    /// Get a project by ID
    #[tracing::instrument(skip(self, id))]
    pub async fn get(&self, id: impl Into<String>) -> Result<Project, ApiError> {
        let id = id.into();
        let path = format!("{}/projects/{}", V0_PREFIX, id);
        self.client.get(path).await
    }

    /// Delete a project
    #[tracing::instrument(skip(self))]
    pub async fn delete(&self, id: &str) -> Result<DeleteResponse, ApiError> {
        let path = format!("{}/projects/{}", V0_PREFIX, id);
        self.client.delete(path).await
    }

    /// Create a project transfer request (Vercel-aligned).
    /// Returns a code valid for 24 hours to accept the transfer.
    #[tracing::instrument(skip(self))]
    pub async fn create_transfer_request(
        &self,
        id_or_name: &str,
        callback_url: Option<String>,
    ) -> Result<TransferRequestResponse, ApiError> {
        let body = CreateTransferRequestBody {
            callbackUrl: callback_url,
        };
        let path = format!("{}/projects/{}/transfer-request", V0_PREFIX, id_or_name);
        self.client.post(path, Some(body)).await
    }

    /// Accept a project transfer request by code into the current team.
    #[tracing::instrument(skip(self))]
    pub async fn accept_transfer_request(&self, code: &str) -> Result<Project, ApiError> {
        let path = format!(
            "{}/projects/transfer-request/{}",
            V0_PREFIX,
            urlencoding::encode(code)
        );
        self.client.put_json(path, serde_json::json!({})).await
    }

    /// List env vars for a project (Vercel-aligned).
    #[tracing::instrument(skip(self))]
    pub async fn list_env(
        &self,
        project_id: &str,
        target: Option<&str>,
        decrypt: bool,
    ) -> Result<ProjectEnvListResponse, ApiError> {
        let mut query_params: Vec<(String, Option<String>)> = vec![
            ("decrypt".to_string(), Some(decrypt.to_string())),
            ("source".to_string(), Some("appz-cli:env:ls".to_string())),
        ];
        if let Some(t) = target {
            query_params.push(("target".to_string(), Some(t.to_string())));
        }
        let path = format!("{}/projects/{}/env", V0_PREFIX, project_id);
        self.client.get_with_query(path, query_params).await
    }

    /// Add an env var to a project.
    #[tracing::instrument(skip(self, body))]
    pub async fn add_env(
        &self,
        project_id: &str,
        body: &AddEnvRequest,
        upsert: bool,
    ) -> Result<(), ApiError> {
        let suffix = if upsert { "?upsert=true" } else { "" };
        let path = format!("{}/projects/{}/env{}", V0_PREFIX, project_id, suffix);
        let _ = self.client.post::<serde_json::Value>(path, Some(body)).await?;
        Ok(())
    }

    /// Remove an env var by ID.
    #[tracing::instrument(skip(self))]
    pub async fn remove_env(&self, project_id: &str, env_id: &str) -> Result<(), ApiError> {
        let path = format!("{}/projects/{}/env/{}", V0_PREFIX, project_id, env_id);
        self.client.delete_no_content(path).await
    }

    /// Pull env vars (decrypted) for a target.
    /// Uses list with decrypt and builds key-value map.
    #[tracing::instrument(skip(self))]
    pub async fn pull_env(
        &self,
        project_id: &str,
        target: &str,
    ) -> Result<ProjectEnvPullResponse, ApiError> {
        let list = self.list_env(project_id, Some(target), true).await?;
        let mut env = std::collections::HashMap::new();
        for ev in list.envs {
            if let Some(v) = ev.value {
                env.insert(ev.key, v);
            }
        }
        Ok(ProjectEnvPullResponse {
            env,
            buildEnv: std::collections::HashMap::new(),
        })
    }
}
