use serde::{Deserialize, Serialize};

// Models match the OpenAPI specification which uses camelCase

/// Parse optional timestamp from JSON value (number as ms, or ISO 8601 string).
fn timestamp_from_value(v: &serde_json::Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_f64().map(|f| f as i64))
        .or_else(|| {
            v.as_str().and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.timestamp_millis())
                    .or_else(|| s.parse::<i64>().ok())
            })
        })
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(default)]
    pub id: Option<String>,
    pub username: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub createdAt: Option<i64>,
    #[serde(default)]
    pub updatedAt: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub slug: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub createdBy: Option<String>,
    #[serde(default)]
    pub createdAt: Option<i64>,
    #[serde(default)]
    pub updatedAt: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastAliasRequest {
    #[serde(default)]
    pub jobStatus: Option<String>, // "pending" | "in-progress" | "succeeded" | "failed" | "skipped"
    #[serde(default)]
    pub requestedAt: Option<i64>,
    #[serde(default)]
    pub toDeploymentId: Option<String>,
    #[serde(default, rename = "type")]
    pub type_: Option<String>, // "promote" | "rollback"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub teamId: Option<String>,
    #[serde(default)]
    pub createdBy: Option<String>,
    #[serde(default)]
    pub createdAt: Option<i64>,
    #[serde(default)]
    pub updatedAt: Option<i64>,
    #[serde(default)]
    pub lastAliasRequest: Option<LastAliasRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invitation {
    pub id: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub teamId: Option<String>,
    #[serde(default)]
    pub roleId: Option<String>,
    #[serde(default)]
    pub createdBy: Option<String>,
    pub createdAt: i64,
    /// Optional: create-invitation response from v0 does not include updatedAt
    #[serde(default)]
    pub updatedAt: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Members {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub name: Option<String>,
    pub email: String,
    pub createdAt: Option<i64>,
    pub updatedAt: Option<i64>,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alias {
    pub id: i64,
    #[serde(default)]
    pub teamId: Option<String>,
    pub alias: String,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub zoneId: Option<String>,
    pub target: String,
    #[serde(default)]
    pub redirect: Option<String>,
    #[serde(default)]
    pub redirectStatusCode: Option<i64>,
    pub createdAt: i64,
    pub updatedAt: i64,
    #[serde(default)]
    pub createdBy: Option<String>,
    pub deploymentId: String,
    pub projectId: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub id: String,
    pub teamId: String,
    pub name: String,
    #[serde(default)]
    pub serviceType: Option<String>,
    #[serde(default)]
    pub createdBy: Option<String>,
    pub createdAt: i64,
    pub updatedAt: i64,
    #[serde(default)]
    pub expiresAt: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub count: i64,
    #[serde(default)]
    pub next: Option<i64>,
    #[serde(default)]
    pub prev: Option<i64>,
}

// Request/Response types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub user: User,
}

/// Better Auth user shape (from get-session or /v0/user).
/// Maps to CLI User via `map_better_auth_user_to_cli_user`.
/// Uses Value for timestamps to accept both number (ms) and ISO 8601 string.
#[allow(non_snake_case)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetterAuthUser {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub createdAt: Option<serde_json::Value>,
    #[serde(default)]
    pub updatedAt: Option<serde_json::Value>,
}

/// Response when API returns Better Auth user shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetterAuthUserResponse {
    pub user: BetterAuthUser,
}

/// Map Better Auth user to CLI User model.
/// Derives username from name, email prefix, or id.
pub fn map_better_auth_user_to_cli_user(ba: BetterAuthUser) -> User {
    let username = ba
        .name
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            ba.email
                .as_ref()
                .and_then(|e| e.split('@').next().map(String::from))
        })
        .or_else(|| ba.id.clone())
        .unwrap_or_else(|| "user".to_string());
    User {
        id: ba.id,
        username,
        name: ba.name,
        email: ba.email,
        createdAt: ba.createdAt.as_ref().and_then(timestamp_from_value),
        updatedAt: ba.updatedAt.as_ref().and_then(timestamp_from_value),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignInRequest {
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenName: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignInResponse {
    // Some deployments only return a security code; token may be omitted
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub securityCode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResponse {
    pub token: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teamId: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTeamRequest {
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectRequest {
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teamId: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTeamRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInvitationRequest {
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roleId: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMemberRequest {
    pub userId: String,
    pub roleId: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamsListResponse {
    pub teams: Vec<Team>,
    #[serde(default)]
    pub pagination: Option<Pagination>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectsListResponse {
    pub projects: Vec<Project>,
    #[serde(default)]
    pub pagination: Option<Pagination>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembersListResponse {
    pub members: Vec<Members>,
    pub pagination: Pagination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitationsListResponse {
    pub invitations: Vec<Invitation>,
    pub pagination: Pagination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainsListResponse {
    pub domains: Vec<Domain>,
    pub pagination: Pagination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasesListResponse {
    pub aliases: Vec<Alias>,
    pub pagination: Pagination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployment {
    pub id: String,
    #[serde(default)]
    pub teamId: Option<String>,
    #[serde(default)]
    pub projectId: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub createdBy: Option<String>,
    pub createdAt: i64,
    pub updatedAt: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentsListResponse {
    pub deployments: Vec<Deployment>,
    pub pagination: Pagination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResponse {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteResponse {
    #[serde(default)]
    pub inspectorUrl: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
}

// OAuth 2.0 Device Flow Models

/// OpenID Connect Discovery metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationServerMetadata {
    pub issuer: String,
    #[serde(rename = "device_authorization_endpoint")]
    pub device_authorization_endpoint: String,
    #[serde(rename = "token_endpoint")]
    pub token_endpoint: String,
    #[serde(rename = "revocation_endpoint")]
    pub revocation_endpoint: String,
    #[serde(rename = "introspection_endpoint")]
    pub introspection_endpoint: String,
    #[serde(rename = "jwks_uri")]
    pub jwks_uri: String,
}

/// Device authorization request (form-encoded)
#[derive(Debug, Clone)]
pub struct DeviceAuthorizationRequest {
    pub client_id: String,
    pub scope: String,
}

/// Device authorization response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceAuthorizationResponse {
    #[serde(rename = "device_code")]
    pub device_code: String,
    #[serde(rename = "user_code")]
    pub user_code: String,
    #[serde(rename = "verification_uri")]
    pub verification_uri: String,
    #[serde(rename = "verification_uri_complete")]
    pub verification_uri_complete: String,
    #[serde(rename = "expires_in")]
    pub expires_in: i64,
    #[serde(default)]
    pub interval: Option<i64>,
}

/// Token set response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSet {
    #[serde(rename = "access_token")]
    pub access_token: String,
    #[serde(rename = "token_type")]
    pub token_type: String,
    #[serde(rename = "expires_in")]
    pub expires_in: i64,
    #[serde(rename = "refresh_token", default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// OAuth error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthErrorResponse {
    pub error: String,
    #[serde(rename = "error_description", default)]
    pub error_description: Option<String>,
    #[serde(rename = "error_uri", default)]
    pub error_uri: Option<String>,
}

/// Token introspection response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenIntrospection {
    pub active: bool,
    #[serde(rename = "client_id", default)]
    pub client_id: Option<String>,
    #[serde(rename = "session_id", default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub exp: Option<i64>,
}
