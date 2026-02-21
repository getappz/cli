use crate::client::Client;
use crate::error::ApiError;
use std::sync::Arc;
use crate::models::{Alias, AliasesListResponse, DeleteResponse};
use crate::paths::V0_PREFIX;

pub struct Aliases {
    client: Arc<Client>,
}

impl Aliases {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// List aliases with optional filters
    #[tracing::instrument(skip(self))]
    pub async fn list(
        &self,
        project_id: Option<String>,
        team_id: Option<String>,
        limit: Option<i64>,
        since: Option<i64>,
        until: Option<i64>,
    ) -> Result<AliasesListResponse, ApiError> {
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

        let path = format!("{}/aliases", V0_PREFIX);
        let result = self.client.get_with_query(&path, &query_params).await;

        // Reset team_id if we set it
        if team_id.is_some() {
            self.client.set_team_id(None).await;
        }

        result
    }

    /// Get an alias by ID or alias string
    #[tracing::instrument(skip(self))]
    pub async fn get(&self, id_or_alias: &str) -> Result<Alias, ApiError> {
        let path = format!("{}/aliases/{}", V0_PREFIX, id_or_alias);
        self.client.get(&path).await
    }

    /// Create an alias: assign a custom domain to a deployment.
    ///
    /// Vercel parity: `POST /now/deployments/{id}/aliases` with `{ alias }`.
    /// Uses `POST /v0/deployments/{id}/aliases` for appz.
    #[tracing::instrument(skip(self))]
    pub async fn create(
        &self,
        deployment_id: &str,
        alias: &str,
    ) -> Result<Alias, ApiError> {
        #[derive(serde::Serialize)]
        struct CreateAliasRequest<'a> {
            alias: &'a str,
        }
        let path = format!("{}/deployments/{}/aliases", V0_PREFIX, deployment_id);
        self.client
            .post(&path, Some(CreateAliasRequest { alias }))
            .await
    }

    /// Delete an alias
    #[tracing::instrument(skip(self))]
    pub async fn delete(&self, alias_id: &str) -> Result<DeleteResponse, ApiError> {
        let path = format!("{}/aliases/{}", V0_PREFIX, alias_id);
        self.client.delete(&path).await
    }
}
