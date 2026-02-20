use crate::client::Client;
use crate::error::ApiError;
use crate::models::DomainsListResponse;
use crate::paths::V0_PREFIX;

pub struct Domains<'a> {
    client: &'a Client,
}

impl<'a> Domains<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// List domains with optional pagination
    #[tracing::instrument(skip(self))]
    pub async fn list(
        &self,
        limit: Option<i64>,
        since: Option<i64>,
        until: Option<i64>,
        team_id: Option<String>,
    ) -> Result<DomainsListResponse, ApiError> {
        let query_params = vec![
            ("limit", limit.map(|l| l.to_string())),
            ("since", since.map(|s| s.to_string())),
            ("until", until.map(|u| u.to_string())),
        ];

        // Temporarily set team_id if provided
        if let Some(ref team_id_val) = team_id {
            self.client.set_team_id(Some(team_id_val.clone())).await;
        }

        let path = format!("{}/domains", V0_PREFIX);
        let result = self.client.get_with_query(&path, &query_params).await;

        // Reset team_id if we set it
        if team_id.is_some() {
            self.client.set_team_id(None).await;
        }

        result
    }

    /// Delete a domain
    #[tracing::instrument(skip(self))]
    pub async fn delete(&self, domain: &str, team_id: Option<String>) -> Result<(), ApiError> {
        let path = format!("{}/domains/{}", V0_PREFIX, domain);

        // Temporarily set team_id if provided
        if let Some(ref team_id_val) = team_id {
            self.client.set_team_id(Some(team_id_val.clone())).await;
        }

        // DELETE /domains/{domain} returns 204 No Content
        let result = self.client.delete_no_content(&path).await;

        // Reset team_id if we set it
        if team_id.is_some() {
            self.client.set_team_id(None).await;
        }

        result
    }
}
