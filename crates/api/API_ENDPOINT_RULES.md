# Appz API Endpoint Usage Rules

This document defines rules and patterns for implementing API endpoints based on the [Appz API OpenAPI specification](https://api.appz.dev/docs).

## Base Configuration

- **Base URL**: `https://api.appz.dev`
- **Authentication**: Bearer token (HTTP Authorization header)
- **Content-Type**: `application/json` for request bodies
- **Default Timeout**: 30 seconds
- **Retry Policy**: 3 retries with exponential backoff (100ms initial, 30s max)

## Endpoint Organization

### Structure
- Each API resource group (teams, projects, aliases, domains, etc.) should have its own module in `crates/api/src/endpoints/`
- Each module defines a struct that holds a reference to the `Client`
- Methods on the struct correspond to API endpoints

### Pattern
```rust
pub struct ResourceName<'a> {
    client: &'a Client,
}

impl<'a> ResourceName<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }
    
    // Endpoint methods here
}
```

## HTTP Methods

### GET Requests
- Use `client.get(path)` for simple GET requests
- Use `client.get_with_query(path, query_params)` for GET requests with query parameters
- Query parameters should be `Vec<(&str, Option<String>)>` format
- Always use `#[tracing::instrument(skip(self))]` on public methods

### POST Requests
- Use `client.post(path, Some(request))` for POST requests with body
- Use `client.post(path, None)` for POST requests without body (rare)
- Request body must implement `serde::Serialize`
- Response type must implement `serde::de::DeserializeOwned`

### PATCH Requests
- Use `client.patch(path, Some(request))` for PATCH requests
- Same pattern as POST for body handling

### DELETE Requests
- Use `client.delete(path)` when API returns JSON response (e.g., `{"status": "SUCCESS"}`)
- Use `client.delete_no_content(path)` when API returns 204 No Content
- Check OpenAPI spec to determine which response format is expected

## Query Parameters

### Standard Pagination Parameters
Most list endpoints support these optional query parameters:
- `limit`: `Option<i64>` - Maximum number of items (default: 20)
- `since`: `Option<i64>` - JavaScript timestamp for items created after
- `until`: `Option<i64>` - JavaScript timestamp for items created before

### Team Context Parameter
- `teamId`: `Option<String>` - Team identifier or slug
- **IMPORTANT**: Use `client.set_team_id(Some(team_id))` before request, then reset to `None` after
- OR let `TeamMiddleware` handle it automatically if team context is set globally
- Do NOT manually add `teamId` to query params if using middleware (to avoid duplication)

### Query Parameter Format
```rust
let query_params = vec![
    ("param1", value1.map(|v| v.to_string())),
    ("param2", value2),
    ("param3", Some("static_value".to_string())),
];
```

## Path Parameters

### Format
- Path parameters should be URL-encoded if they contain special characters
- Use `format!()` macro to construct paths: `format!("/resource/{}", id)`
- Path parameters are typically IDs or slugs

### Examples
- `/teams/{id}` → `format!("/teams/{}", id)`
- `/aliases/{idOrAlias}` → `format!("/aliases/{}", id_or_alias)`
- `/domains/{domain}` → `format!("/domains/{}", domain)`

## Request Bodies

### Structure
- Create request structs in `crates/api/src/models.rs`
- Use `serde::Serialize` derive
- Field names should match API exactly (use `#[serde(rename = "camelCase")]` if needed)
- Optional fields should be `Option<T>`

### Example
```rust
#[derive(serde::Serialize)]
pub struct CreateTeamRequest {
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
```

## Response Types

### Structure
- Response structs should be in `crates/api/src/models.rs`
- Use `serde::Deserialize` derive
- Match API schema exactly, including nullable fields (`Option<T>`)
- Timestamps are typically `i64` (JavaScript timestamp in milliseconds)

### Paginated Responses
Paginated list endpoints return:
```rust
pub struct ResourceListResponse {
    pub resources: Vec<Resource>,
    pub pagination: Pagination,
}

pub struct Pagination {
    pub count: i64,
    pub next: Option<i64>,  // Timestamp for next page
    pub prev: Option<i64>,  // Timestamp for previous page
}
```

### Single Resource Responses
- Direct resource type: `Resource`
- No wrapper unless API specifies one

### Delete Responses
- If API returns JSON: `DeleteResponse { status: String }` (typically `"SUCCESS"`)
- If API returns 204: Use `delete_no_content()` returning `Result<(), ApiError>`

## Error Handling

### HTTP Status Codes
- **200**: Success with JSON body
- **204**: Success with no content (DELETE operations)
- **400**: Validation error - check `message` field in error response
- **401**: Unauthorized - token missing or invalid
- **403**: Forbidden - insufficient permissions
- **404**: Not found - resource doesn't exist
- **429**: Rate limited - check `Retry-After` header

### Error Response Format
```json
{
  "code": 400,
  "message": "One of the provided values in the request body/query is invalid"
}
```

### Error Mapping
- All errors are converted to `ApiError` enum variants
- Use `handle_response()` method which calls `response_handler::handle_json()`
- Status codes are automatically mapped to appropriate `ApiError` variants

## Authentication

### Public Endpoints
- `/auth/signin` - POST (no auth required)
- `/auth/verify` - GET (no auth required, uses query params)

### Protected Endpoints
- All other endpoints require Bearer token
- Token is automatically added by `AuthenticationMiddleware`
- Use `client.set_token(token)` to set authentication token
- Use `client.clear_token()` to remove token

## Endpoint-Specific Rules

### Authentication Endpoints

#### POST `/auth/signin`
- **Request**: `{ email: string, tokenName?: string }`
- **Response**: `{ token: string, securityCode: string }`
- **Auth**: Not required
- **Notes**: Returns verification token and security code

#### GET `/auth/verify`
- **Query Params**: `token` (required), `email?` (optional)
- **Response**: `{ token: string }` (authentication token)
- **Auth**: Not required
- **Notes**: Use `get_with_query()` with token and optional email

### Teams Endpoints

#### GET `/teams`
- **Query Params**: `limit?`, `since?`, `until?`, `teamId?`
- **Response**: `TeamsListResponse`
- **Auth**: Required

#### POST `/teams`
- **Request**: `{ slug: string, name?: string }`
- **Response**: `Team`
- **Auth**: Required

#### GET `/teams/{id}`
- **Path**: Team ID or slug
- **Response**: `Team`
- **Auth**: Required

#### PATCH `/teams/{id}`
- **Path**: Team ID
- **Request**: `{ slug?: string, name?: string, avatar?: string }`
- **Response**: `Team`
- **Auth**: Required

#### DELETE `/teams/{id}`
- **Path**: Team ID
- **Response**: `DeleteResponse` (JSON with status)
- **Auth**: Required

#### GET `/teams/{id}/members`
- **Query Params**: `limit?`, `since?`, `until?`
- **Response**: `MembersListResponse`
- **Auth**: Required

#### POST `/teams/{id}/members`
- **Request**: `{ userId: string, roleId: number }` OR `{ email: string, roleId?: number }`
- **Response**: `Members` OR `Invitation`
- **Auth**: Required
- **Notes**: Use email to invite, userId to add existing member

#### DELETE `/teams/{id}/members/{userId}`
- **Response**: 204 No Content
- **Auth**: Required
- **Notes**: Use `delete_no_content()`

#### GET `/teams/{id}/invitations`
- **Query Params**: `limit?`, `since?`, `until?`
- **Response**: `InvitationsListResponse`
- **Auth**: Required

#### DELETE `/teams/{id}/invitations/{invitationId}`
- **Response**: 204 No Content
- **Auth**: Required
- **Notes**: Use `delete_no_content()`

### Projects Endpoints

#### GET `/projects`
- **Query Params**: `limit?`, `since?`, `until?`, `teamId?`
- **Response**: `ProjectsListResponse`
- **Auth**: Required

#### POST `/projects`
- **Request**: Project creation payload
- **Response**: `Project`
- **Auth**: Required

#### GET `/projects/{id}`
- **Path**: Project ID
- **Response**: `Project`
- **Auth**: Required

#### DELETE `/projects/{id}`
- **Path**: Project ID
- **Response**: 204 No Content
- **Auth**: Required
- **Notes**: Use `delete_no_content()`

### Aliases Endpoints

#### GET `/aliases`
- **Query Params**: `projectId?`, `teamId?`, `limit?`, `since?`, `until?`
- **Response**: `AliasesListResponse`
- **Auth**: Required
- **Notes**: `teamId` handled via middleware or temporary set

#### GET `/aliases/{idOrAlias}`
- **Path**: Alias ID or alias string (e.g., `example.appz.dev`)
- **Response**: `Alias`
- **Auth**: Required
- **Notes**: Accepts both ID and alias string

#### DELETE `/aliases/{aliasId}`
- **Path**: Alias ID or alias string
- **Response**: `DeleteResponse` (JSON with status)
- **Auth**: Required

### Domains Endpoints

#### GET `/domains`
- **Query Params**: `teamId?`, `limit?`, `since?`, `until?`
- **Response**: `DomainsListResponse`
- **Auth**: Required

#### DELETE `/domains/{domain}`
- **Path**: Domain name (e.g., `example.com`)
- **Response**: 204 No Content
- **Auth**: Required
- **Notes**: Use `delete_no_content()`

### Users Endpoints

#### GET `/user`
- **Response**: `User`
- **Auth**: Required
- **Notes**: Singular `/user` not `/users/{id}`

## Common Patterns

### List Endpoint Pattern
```rust
#[tracing::instrument(skip(self))]
pub async fn list(
    &self,
    limit: Option<i64>,
    since: Option<i64>,
    until: Option<i64>,
) -> Result<ResourceListResponse, ApiError> {
    let query_params = vec![
        ("limit", limit.map(|l| l.to_string())),
        ("since", since.map(|s| s.to_string())),
        ("until", until.map(|u| u.to_string())),
    ];
    self.client.get_with_query("/resource", &query_params).await
}
```

### Get by ID Pattern
```rust
#[tracing::instrument(skip(self))]
pub async fn get(&self, id: &str) -> Result<Resource, ApiError> {
    let path = format!("/resource/{}", id);
    self.client.get(&path).await
}
```

### Create Pattern
```rust
#[tracing::instrument(skip(self))]
pub async fn create(&self, field1: String, field2: Option<String>) -> Result<Resource, ApiError> {
    let request = CreateResourceRequest { field1, field2 };
    self.client.post("/resource", Some(request)).await
}
```

### Update Pattern
```rust
#[tracing::instrument(skip(self))]
pub async fn update(
    &self,
    id: &str,
    field1: Option<String>,
    field2: Option<String>,
) -> Result<Resource, ApiError> {
    let path = format!("/resource/{}", id);
    let request = UpdateResourceRequest { field1, field2 };
    self.client.patch(&path, Some(request)).await
}
```

### Delete Pattern (JSON Response)
```rust
#[tracing::instrument(skip(self))]
pub async fn delete(&self, id: &str) -> Result<DeleteResponse, ApiError> {
    let path = format!("/resource/{}", id);
    self.client.delete(&path).await
}
```

### Delete Pattern (204 No Content)
```rust
#[tracing::instrument(skip(self))]
pub async fn delete(&self, id: &str) -> Result<(), ApiError> {
    let path = format!("/resource/{}", id);
    self.client.delete_no_content(&path).await
}
```

### Temporary Team Context Pattern
```rust
// Set team_id temporarily
if let Some(ref team_id_val) = team_id {
    self.client.set_team_id(Some(team_id_val.clone())).await;
}

let result = self.client.get_with_query("/resource", &query_params).await;

// Reset team_id
if team_id.is_some() {
    self.client.set_team_id(None).await;
}

result
```

## Type Mapping

### API Types → Rust Types
- `string` → `String`
- `integer` (JavaScript timestamp) → `i64`
- `number` → `f64` or `i64` depending on context
- `boolean` → `bool`
- `null` or optional → `Option<T>`
- Arrays → `Vec<T>`
- Objects → Structs with `serde::Deserialize`

### Timestamp Handling
- All timestamps in API are JavaScript timestamps (milliseconds since epoch)
- Use `i64` type
- Range: `-9007199254740991` to `9007199254740991` (safe integer range)

## Testing Considerations

- Mock HTTP responses for unit tests
- Test error cases (400, 401, 403, 404, 429)
- Test pagination with `since`/`until` parameters
- Test team context handling
- Verify request body serialization matches API spec

## Documentation

- Each endpoint method should have a doc comment describing its purpose
- Include parameter descriptions
- Note any special behavior (e.g., temporary team context)
- Reference OpenAPI spec for detailed schema information

## Validation

- Validate required fields before making request
- Use appropriate types (e.g., `i64` for timestamps, not `String`)
- Handle `Option<T>` correctly - only serialize if `Some`
- Use `#[serde(skip_serializing_if = "Option::is_none")]` for optional fields

## Security

- Never log authentication tokens
- Use `#[tracing::instrument(skip(self, token))]` to exclude sensitive data from logs
- Validate input before sending to API
- Handle authentication errors gracefully

## References

- [Appz API Documentation](https://api.appz.dev/docs)
- OpenAPI Spec: `https://api.appz.dev/docs` (OpenAPI 3.1.0 format)
- Base URL: `https://api.appz.dev`

