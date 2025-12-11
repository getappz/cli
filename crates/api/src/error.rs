use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    #[diagnostic(code(api::http_error))]
    Http(#[from] reqwest::Error),

    #[error("JSON serialization/deserialization failed: {0}")]
    #[diagnostic(code(api::json_error))]
    Json(#[from] serde_json::Error),

    #[error("API returned error {code}: {message}")]
    #[diagnostic(code(api::api_error))]
    ApiError { code: u16, message: String },

    #[error("Invalid security code: {0}")]
    #[diagnostic(code(api::invalid_security_code))]
    InvalidSecurityCode(String),

    #[error("Unauthorized: {0}")]
    #[diagnostic(code(api::unauthorized))]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    #[diagnostic(code(api::forbidden))]
    Forbidden(String),

    #[error("Not found: {0}")]
    #[diagnostic(code(api::not_found))]
    NotFound(String),

    #[error("Validation error: {0}")]
    #[diagnostic(code(api::validation_error))]
    Validation(String),

    #[error("Rate limited: retry after {0:?} seconds")]
    #[diagnostic(code(api::rate_limited))]
    RateLimited(Option<u64>),

    #[error("Cloudflare protection blocked the request")]
    #[diagnostic(code(api::cloudflare_blocked))]
    CloudflareBlocked,

    #[error("Invalid URL: {0}")]
    #[diagnostic(code(api::url_error))]
    Url(#[from] url::ParseError),

    #[error("Authentication required")]
    #[diagnostic(code(api::auth_required))]
    AuthRequired,

    #[error("Invalid response format: {0}")]
    #[diagnostic(code(api::invalid_response))]
    InvalidResponse(String),

    #[error("Middleware error: {0}")]
    #[diagnostic(code(api::middleware_error))]
    Middleware(String),

    #[error("HTTP middleware error: {0}")]
    #[diagnostic(code(api::http_middleware_error))]
    HttpMiddleware(String),

    #[error("OAuth error: {0}")]
    #[diagnostic(code(api::oauth_error))]
    OAuthError(String),

    #[error("Device authorization pending")]
    #[diagnostic(code(api::authorization_pending))]
    AuthorizationPending,

    #[error("Token expired")]
    #[diagnostic(code(api::expired_token))]
    ExpiredToken,
}
