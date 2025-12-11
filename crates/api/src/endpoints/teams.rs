use crate::client::Client;
use crate::error::ApiError;
use crate::models::{
    AddMemberRequest, CreateInvitationRequest, CreateTeamRequest, DeleteResponse, Invitation,
    InvitationsListResponse, Members, MembersListResponse, Team, TeamsListResponse,
    UpdateTeamRequest,
};

pub struct Teams<'a> {
    client: &'a Client,
}

impl<'a> Teams<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// List teams with optional pagination
    #[tracing::instrument(skip(self))]
    pub async fn list(
        &self,
        limit: Option<i64>,
        since: Option<i64>,
        until: Option<i64>,
    ) -> Result<TeamsListResponse, ApiError> {
        let query_params = vec![
            ("limit", limit.map(|l| l.to_string())),
            ("since", since.map(|s| s.to_string())),
            ("until", until.map(|u| u.to_string())),
        ];

        self.client.get_with_query("/teams", &query_params).await
    }

    /// Create a new team
    #[tracing::instrument(skip(self))]
    pub async fn create(&self, slug: String, name: Option<String>) -> Result<Team, ApiError> {
        let request = CreateTeamRequest { slug, name };
        self.client.post("/teams", Some(request)).await
    }

    /// Get a team by ID
    #[tracing::instrument(skip(self))]
    pub async fn get(&self, id: &str) -> Result<Team, ApiError> {
        let path = format!("/teams/{}", id);
        self.client.get(&path).await
    }

    /// Update a team
    #[tracing::instrument(skip(self))]
    pub async fn update(
        &self,
        id: &str,
        slug: Option<String>,
        name: Option<String>,
        avatar: Option<String>,
    ) -> Result<Team, ApiError> {
        let path = format!("/teams/{}", id);
        let request = UpdateTeamRequest { slug, name, avatar };
        self.client.patch(&path, Some(request)).await
    }

    /// Delete a team
    #[tracing::instrument(skip(self))]
    pub async fn delete(&self, id: &str) -> Result<DeleteResponse, ApiError> {
        let path = format!("/teams/{}", id);
        self.client.delete(&path).await
    }

    /// List members of a team
    #[tracing::instrument(skip(self))]
    pub async fn list_members(
        &self,
        id: &str,
        limit: Option<i64>,
        since: Option<i64>,
        until: Option<i64>,
    ) -> Result<MembersListResponse, ApiError> {
        let path = format!("/teams/{}/members", id);
        let query_params = vec![
            ("limit", limit.map(|l| l.to_string())),
            ("since", since.map(|s| s.to_string())),
            ("until", until.map(|u| u.to_string())),
        ];

        self.client.get_with_query(&path, &query_params).await
    }

    /// Add a member to a team
    #[tracing::instrument(skip(self))]
    pub async fn add_member(
        &self,
        id: &str,
        user_id: String,
        role_id: i64,
    ) -> Result<Members, ApiError> {
        let path = format!("/teams/{}/members", id);
        let request = AddMemberRequest {
            userId: user_id,
            roleId: role_id,
        };
        self.client.post(&path, Some(request)).await
    }

    /// Remove a member from a team
    #[tracing::instrument(skip(self))]
    pub async fn remove_member(&self, id: &str, user_id: &str) -> Result<(), ApiError> {
        let path = format!("/teams/{}/members/{}", id, user_id);
        self.client.delete_no_content(&path).await
    }

    /// List invitations for a team
    #[tracing::instrument(skip(self))]
    pub async fn list_invitations(
        &self,
        id: &str,
        limit: Option<i64>,
        since: Option<i64>,
        until: Option<i64>,
    ) -> Result<InvitationsListResponse, ApiError> {
        let path = format!("/teams/{}/invitations", id);
        let query_params = vec![
            ("limit", limit.map(|l| l.to_string())),
            ("since", since.map(|s| s.to_string())),
            ("until", until.map(|u| u.to_string())),
        ];

        self.client.get_with_query(&path, &query_params).await
    }

    /// Create an invitation for a team
    /// Uses POST /teams/{teamId}/members endpoint with email to invite a user
    #[tracing::instrument(skip(self))]
    pub async fn create_invitation(
        &self,
        id: &str,
        email: String,
        role_id: Option<i64>,
    ) -> Result<Invitation, ApiError> {
        let path = format!("/teams/{}/members", id);
        let request = CreateInvitationRequest {
            email,
            roleId: role_id,
        };
        self.client.post(&path, Some(request)).await
    }

    /// Delete an invitation
    #[tracing::instrument(skip(self))]
    pub async fn delete_invitation(&self, id: &str, invitation_id: &str) -> Result<(), ApiError> {
        let path = format!("/teams/{}/invitations/{}", id, invitation_id);
        self.client.delete_no_content(&path).await
    }
}
