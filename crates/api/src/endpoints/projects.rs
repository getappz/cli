use crate::client::Client;
use crate::error::ApiError;
use crate::models::{CreateProjectRequest, DeleteResponse, Project, ProjectsListResponse};

pub struct Projects<'a> {
    client: &'a Client,
}

impl<'a> Projects<'a> {
    pub fn new(client: &'a Client) -> Self {
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
        let query_params = vec![
            ("limit", limit.map(|l| l.to_string())),
            ("since", since.map(|s| s.to_string())),
            ("until", until.map(|u| u.to_string())),
        ];

        self.client.get_with_query("/projects", &query_params).await
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
        self.client.post("/projects", Some(request)).await
    }

    /// Get a project by ID
    #[tracing::instrument(skip(self))]
    pub async fn get(&self, id: &str) -> Result<Project, ApiError> {
        let path = format!("/projects/{}", id);
        self.client.get(&path).await
    }

    /// Delete a project
    #[tracing::instrument(skip(self))]
    pub async fn delete(&self, id: &str) -> Result<DeleteResponse, ApiError> {
        let path = format!("/projects/{}", id);
        self.client.delete(&path).await
    }
}
