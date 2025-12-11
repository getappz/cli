use http::Extensions;
use reqwest_middleware::reqwest::Request;
use reqwest_middleware::{Middleware, Next, Result as MiddlewareResult};
use std::io::Write;
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
        "/oauth/device-authorization", // OAuth 2.0 Device Authorization Flow endpoint
        "/oauth/token", // OAuth 2.0 Token endpoint (for device code exchange)
        "/oauth/authorize", // OAuth 2.0 Authorization endpoint
        "/oauth/register", // OAuth 2.0 Dynamic Client Registration
        "/.well-known", // OpenID Connect Discovery and JWKS
    ];

    // Check if the path starts with any public path
    let is_public = public_paths
        .iter()
        .any(|public_path| {
            let matches = path.starts_with(public_path);
            if matches {
                eprintln!("[DEBUG] Path '{}' matches public endpoint '{}'", path, public_path);
            }
            matches
        });
    
    let requires = !is_public;
    eprintln!("[DEBUG] Path: '{}', is_public: {}, requires_auth: {}", path, is_public, requires);
    requires
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
        
        // Force output immediately - these should always appear
        let _ = std::io::stderr().write_all(
            format!("[AUTH MIDDLEWARE] URL: {}, Path: '{}'\n", url, path).as_bytes()
        );
        let _ = std::io::stderr().flush();
        
        let needs_auth = requires_auth(path);
        
        let _ = std::io::stderr().write_all(
            format!("[AUTH MIDDLEWARE] needs_auth: {}\n", needs_auth).as_bytes()
        );
        let _ = std::io::stderr().flush();
        
        // Also ensure tracing can see this
        tracing::debug!(url = %url, path = %path, needs_auth = needs_auth, "Auth middleware check");
        tracing::debug!(path = %path, needs_auth = needs_auth, "Checking authentication requirement");
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
    }

    #[test]
    fn test_protected_endpoints() {
        // Other endpoints should require auth
        assert!(requires_auth("/teams"));
        assert!(requires_auth("/projects"));
        assert!(requires_auth("/user"));
        assert!(requires_auth("/aliases"));
    }
}
