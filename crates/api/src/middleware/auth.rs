use http::Extensions;
use reqwest_middleware::reqwest::Request;
use reqwest_middleware::{Middleware, Next, Result as MiddlewareResult};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Helper function to check if a path requires authentication
pub(crate) fn requires_auth(path: &str) -> bool {
    // Public endpoints that don't require authentication
    let public_paths = [
        "/auth/signin",
        "/auth/verify",
        "/auth/device/authorize",
        "/auth/token",
        "/auth/introspect",
        "/auth/revoke",
        "/auth/device",        // Device flow at api.appz.dev/auth
        "/auth/revoke",        // Token revocation
        "/auth/introspect",    // Token introspection
        "/oauth/device-authorization", // OAuth 2.0 Device Authorization Flow endpoint
        "/oauth/token",        // OAuth 2.0 Token endpoint (for device code exchange)
        "/oauth/authorize",    // OAuth 2.0 Authorization endpoint
        "/oauth/register",     // OAuth 2.0 Dynamic Client Registration
        "/.well-known",       // OpenID Connect Discovery and JWKS
    ];

    // Check if the path starts with any public path
    let is_public = public_paths.iter().any(|public_path| path.starts_with(public_path));
    !is_public
}

/// Authentication middleware that automatically adds Bearer token headers
/// to requests that require authentication
pub struct AuthenticationMiddleware {
    token: Arc<RwLock<Option<String>>>,
}

impl AuthenticationMiddleware {
    pub fn new(token: Arc<RwLock<Option<String>>>) -> Self {
        Self { token }
    }
}

#[async_trait::async_trait]
impl Middleware for AuthenticationMiddleware {
    #[tracing::instrument(skip(self, next, extensions))]
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> MiddlewareResult<reqwest_middleware::reqwest::Response> {
        // Check if this endpoint requires authentication
        let url = req.url();
        let path = url.path();
        let needs_auth = requires_auth(path);

        tracing::debug!(url = %url, path = %path, needs_auth, "Auth middleware");
        if needs_auth {
            // Read the token
            let token = self.token.read().await;
            if let Some(ref token_value) = *token {
                // Clone the request to modify it
                let mut req = req;

                // Add the Authorization header
                let auth_header_value = format!("Bearer {}", token_value);
                let header_value =
                    reqwest_middleware::reqwest::header::HeaderValue::from_str(&auth_header_value)
                        .map_err(|e| {
                            reqwest_middleware::Error::Middleware(anyhow::anyhow!(
                                "Invalid auth header: {}",
                                e
                            ))
                        })?;
                req.headers_mut().insert(
                    reqwest_middleware::reqwest::header::AUTHORIZATION,
                    header_value,
                );

                next.run(req, extensions).await
            } else {
                // No token available for authenticated endpoint - return error
                Err(reqwest_middleware::Error::Middleware(anyhow::anyhow!(
                    "Authentication required for {} but no token is set. Call set_token() first.",
                    path
                )))
            }
        } else {
            // Public endpoint, proceed without auth
            next.run(req, extensions).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::requires_auth;

    #[test]
    fn test_public_endpoints() {
        // Well-known endpoints should be public
        assert!(!requires_auth("/.well-known/openid-configuration"));
        assert!(!requires_auth("/.well-known/jwks.json"));

        // Auth endpoints should be public
        assert!(!requires_auth("/auth/signin"));
        assert!(!requires_auth("/auth/verify"));
        assert!(!requires_auth("/auth/device/authorize"));
        assert!(!requires_auth("/auth/token"));
        assert!(!requires_auth("/auth/introspect"));
        assert!(!requires_auth("/auth/revoke"));

        // OAuth endpoints should be public
        assert!(!requires_auth("/oauth/device-authorization"));
        assert!(!requires_auth("/oauth/token"));
        assert!(!requires_auth("/oauth/authorize"));
        assert!(!requires_auth("/oauth/register"));

        // Auth endpoints at api.appz.dev/auth
        assert!(!requires_auth("/auth/device/code"));
        assert!(!requires_auth("/auth/device/token"));
        assert!(!requires_auth("/auth/revoke"));
        assert!(!requires_auth("/auth/introspect"));
    }

    #[test]
    fn test_protected_endpoints() {
        // Platform API endpoints under /v0 require auth
        assert!(requires_auth("/v0/teams"));
        assert!(requires_auth("/v0/projects"));
        assert!(requires_auth("/v0/user"));
        assert!(requires_auth("/v0/aliases"));
    }
}
