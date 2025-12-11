# Appz API Client

A Rust client library for the Appz API (https://api.appz.dev/docs).

## Usage

```rust
use api::{Client, ApiError};

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    // Create a new client
    let client = Client::new()?;

    // Authenticate
    let signin_response = client.auth().signin(
        "user@example.com".to_string(),
        Some("My Token".to_string()),
    ).await?;

    println!("Security code: {}", signin_response.securityCode);

    // Verify and get token
    let verify_response = client.auth().verify(
        signin_response.token,
        Some("user@example.com".to_string()), // Optional email
        None, // Optional team ID
    ).await?;

    // Set the authentication token
    client.set_token(verify_response.token).await;

    // Get current user
    let user = client.users().get_current().await?;
    println!("User: {} ({})", user.username, user.email);

    // List teams
    let teams_response = client.teams().list(None, None, None).await?;
    println!("Found {} teams", teams_response.teams.len());

    // Create a team
    let team = client.teams().create(
        "my-team".to_string(),
        Some("My Team".to_string()),
    ).await?;

    // Set team context for subsequent requests
    client.set_team_id(Some(team.id.clone())).await;

    // List domains for the team
    let domains_response = client.domains().list(None, None, None, None).await?;

    // List aliases
    let aliases_response = client.aliases().list(None, None, None, None, None).await?;

    Ok(())
}
```

## Features

- **Authentication**: Sign in and verify endpoints with email-based authentication
- **Users**: Get current user information
- **Teams**: Full CRUD operations, member management, and invitations
- **Domains**: List and delete domains
- **Aliases**: List, get, and delete aliases
- **Team Context**: Automatic team ID injection for requests that support it
- **Error Handling**: Comprehensive error types with miette integration

## API Endpoints

### Authentication
- `POST /auth/signin` - Request login
- `GET /auth/verify` - Verify and get token

### Users
- `GET /user` - Get current user

### Teams
- `GET /teams` - List teams
- `POST /teams` - Create team
- `GET /teams/{id}` - Get team
- `PATCH /teams/{id}` - Update team
- `DELETE /teams/{id}` - Delete team
- `GET /teams/{id}/members` - List members
- `POST /teams/{id}/members` - Add member
- `DELETE /teams/{id}/members/{userId}` - Remove member
- `GET /teams/{id}/invitations` - List invitations
- `POST /teams/{id}/invitations` - Create invitation
- `DELETE /teams/{id}/invitations/{invitationId}` - Delete invitation

### Domains
- `GET /domains` - List domains
- `DELETE /domains/{domain}` - Delete domain

### Aliases
- `GET /aliases` - List aliases
- `GET /aliases/{idOrAlias}` - Get alias
- `DELETE /aliases/{aliasId}` - Delete alias

## Development

When adding or updating API endpoints, refer to [API_ENDPOINT_RULES.md](./API_ENDPOINT_RULES.md) for detailed guidelines on:
- Endpoint structure and organization
- Request/response patterns
- Error handling
- Authentication and team context
- Query parameters and pagination
- Type mappings and validation

The rules document is based on the [Appz API OpenAPI specification](https://api.appz.dev/docs) and ensures consistency across all endpoint implementations.

