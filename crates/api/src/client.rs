use crate::error::ApiError;
use crate::middleware::auth::AuthenticationMiddleware;
use crate::middleware::retry::{RetryMiddleware, RetryPolicy};
use crate::middleware::tracing::TracingMiddleware;
use reqwest_middleware::reqwest::{Method, StatusCode};
use reqwest_middleware::ClientWithMiddleware;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

const DEFAULT_BASE_URL: &str = "https://api.appz.dev";

/// Convert reqwest middleware error to a detailed error message
fn format_reqwest_error(err: reqwest_middleware::reqwest::Error, url: &str) -> String {
    use std::error::Error as StdError;

    let mut error_msg = format!("Request to {} failed", url);

    if err.is_timeout() {
        error_msg.push_str(": Request timed out");
    } else if err.is_connect() {
        error_msg.push_str(": Failed to connect to server");
        // Add DNS/network hints
        if let Some(source) = err.source() {
            let source_str = source.to_string().to_lowercase();
            if source_str.contains("dns") || source_str.contains("name resolution") {
                error_msg.push_str(
                    " (DNS resolution failed - check your internet connection and DNS settings)",
                );
            } else if source_str.contains("ssl")
                || source_str.contains("tls")
                || source_str.contains("certificate")
            {
                error_msg.push_str(" (SSL/TLS error - certificate validation failed)");
            } else if source_str.contains("connection refused") {
                error_msg
                    .push_str(" (Connection refused - server may be down or firewall blocking)");
            }
        }
    } else if err.is_request() {
        error_msg.push_str(": Request error");
    }

    // Add URL if available (might be different from the one we tried)
    if let Some(err_url) = err.url() {
        if err_url.as_str() != url {
            error_msg.push_str(&format!(" (URL: {})", err_url));
        }
    }

    // Add status code if available
    if let Some(status) = err.status() {
        error_msg.push_str(&format!(" (Status: {})", status));
    }

    // Add underlying error details
    if let Some(source) = err.source() {
        let source_str = source.to_string();
        // Only add if it provides additional info beyond what we already have
        if !error_msg.contains(&source_str) {
            error_msg.push_str(&format!(": {}", source_str));
        }
    } else {
        // Fallback to the error's Display implementation
        error_msg.push_str(&format!(": {}", err));
    }

    error_msg
}

/// Callback function type for handling unauthorized errors
/// Returns a new authentication token on success
pub type UnauthorizedCallback =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<String, ApiError>> + Send>> + Send + Sync>;

pub struct Client {
    http_client: ClientWithMiddleware,
    base_url: String,
    token: Arc<RwLock<Option<String>>>,
    team_id: Arc<RwLock<Option<String>>>,
    on_unauthorized: Arc<RwLock<Option<UnauthorizedCallback>>>,
}

impl Client {
    /// Create a new API client with default base URL
    /// Checks for APPZ_API_URL environment variable, falls back to DEFAULT_BASE_URL
    pub fn new() -> Result<Self, ApiError> {
        let base_url =
            std::env::var("APPZ_API_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        Self::with_base_url(base_url)
    }

    /// Create a new API client with a custom base URL
    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self, ApiError> {
        let base_url_str = base_url.into();

        // Set User-Agent to avoid Cloudflare blocking
        // Use a browser-like User-Agent that identifies as Appz CLI
        let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 AppzCLI/0.1.0";

        // Build default headers
        let mut default_headers = reqwest::header::HeaderMap::new();
        default_headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_str(user_agent)
                .map_err(|e| ApiError::InvalidResponse(format!("Invalid User-Agent: {}", e)))?,
        );
        // Set Referer to the main site so Cloudflare sees a valid origin
        default_headers.insert(
            reqwest::header::REFERER,
            reqwest::header::HeaderValue::from_static("https://appz.dev"),
        );

        let reqwest_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(user_agent)
            .default_headers(default_headers)
            .build()
            .map_err(ApiError::Http)?;

        // Build retry middleware with exponential backoff
        let retry_policy = RetryPolicy::new(3)
            .with_initial_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_secs(30));

        // Use custom retry middleware compatible with reqwest-middleware 0.4
        let retry_middleware = RetryMiddleware::new_with_policy(retry_policy);

        // Create token storage for middleware
        let token = Arc::new(RwLock::new(None));

        // Create team_id storage for middleware
        let team_id = Arc::new(RwLock::new(None));

        // Create authentication middleware
        let auth_middleware = AuthenticationMiddleware::new(token.clone());
        // Create team middleware to inject teamId
        let team_middleware = crate::middleware::team::TeamMiddleware::new(team_id.clone());

        let http_client = reqwest_middleware::ClientBuilder::new(reqwest_client)
            // Trace HTTP requests
            .with(TracingMiddleware::default())
            // Add authentication token to requests (skips public endpoints)
            .with(auth_middleware)
            // Inject teamId query param when set
            .with(team_middleware)
            // Retry failed requests.
            .with(retry_middleware)
            .build();

        Ok(Self {
            http_client,
            base_url: base_url_str,
            token,
            team_id,
            on_unauthorized: Arc::new(RwLock::new(None)),
        })
    }

    /// Set the authentication token
    #[tracing::instrument(skip(self, token))]
    pub async fn set_token(&self, token: String) {
        *self.token.write().await = Some(token);
    }

    /// Clear the authentication token
    #[tracing::instrument(skip(self))]
    pub async fn clear_token(&self) {
        *self.token.write().await = None;
    }

    /// Get the current token
    #[tracing::instrument(skip(self))]
    pub async fn get_token(&self) -> Option<String> {
        self.token.read().await.clone()
    }

    /// Set the team ID context for requests
    #[tracing::instrument(skip(self))]
    pub async fn set_team_id(&self, team_id: Option<String>) {
        *self.team_id.write().await = team_id;
    }

    /// Get the current team ID
    #[tracing::instrument(skip(self))]
    pub async fn get_team_id(&self) -> Option<String> {
        self.team_id.read().await.clone()
    }

    /// Normalize an endpoint URL to use the client's base URL
    /// If the URL is a full URL that doesn't match the client's base URL,
    /// extract the path and return it as a relative path.
    /// This is useful when discovery metadata returns absolute URLs from a different server.
    pub fn normalize_endpoint_url(&self, url: &str) -> String {
        // If it's not a full URL, return as-is (it's already a path)
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return url.to_string();
        }

        // Parse both URLs to compare their bases
        if let (Ok(parsed_url), Ok(parsed_base)) = (
            url::Url::parse(url),
            url::Url::parse(&self.base_url),
        ) {
            let url_authority = parsed_url.authority();
            let base_authority = parsed_base.authority();
            
            // If the URL's authority matches the client's base authority, use it as-is
            if url_authority == base_authority {
                return url.to_string();
            }

            // Otherwise, extract the path (and query/fragment if present)
            let mut path = parsed_url.path().to_string();
            if let Some(query) = parsed_url.query() {
                path.push('?');
                path.push_str(query);
            }
            if let Some(fragment) = parsed_url.fragment() {
                path.push('#');
                path.push_str(fragment);
            }
            return path;
        }

        // If parsing fails, return as-is
        url.to_string()
    }

    /// Set a callback to be invoked when an Unauthorized error is encountered
    /// The callback should return a new authentication token
    #[tracing::instrument(skip(self, callback))]
    pub async fn set_unauthorized_handler(&self, callback: UnauthorizedCallback) {
        *self.on_unauthorized.write().await = Some(callback);
    }

    /// Clear the unauthorized handler
    #[tracing::instrument(skip(self))]
    pub async fn clear_unauthorized_handler(&self) {
        *self.on_unauthorized.write().await = None;
    }

    /// Handle HTTP response and convert to appropriate result
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest_middleware::reqwest::Response,
    ) -> Result<T, ApiError> {
        crate::http::response_handler::handle_json::<T>(response).await
    }

    /// Handle Unauthorized error by calling callback to refresh token
    /// Returns the new token on success
    async fn refresh_token_on_unauthorized(&self) -> Result<String, ApiError> {
        // Check if we have a callback handler
        let callback = self.on_unauthorized.read().await.clone();

        if let Some(ref callback_fn) = callback {
            // Call the callback to get a new token
            callback_fn().await
        } else {
            // No callback set
            Err(ApiError::Unauthorized(
                "Your session has expired. Please log in again.".to_string(),
            ))
        }
    }

    /// Execute a request with automatic retry on Unauthorized errors
    ///
    /// This method handles:
    /// 1. Sending the request
    /// 2. Detecting Unauthorized errors
    /// 3. Refreshing the token via callback
    /// 4. Retrying the request once
    ///
    /// The `build_request` closure should build a request builder that can be called multiple times
    async fn execute_with_auth_retry(
        &self,
        url: &str,
        build_request: impl Fn() -> reqwest_middleware::RequestBuilder,
    ) -> Result<reqwest_middleware::reqwest::Response, ApiError> {
        // First attempt
        let mut response = build_request().send().await.map_err(|e| match e {
            reqwest_middleware::Error::Reqwest(err) => {
                ApiError::HttpMiddleware(format_reqwest_error(err, url))
            }
            reqwest_middleware::Error::Middleware(err) => ApiError::Middleware(err.to_string()),
        })?;

        // Check for Unauthorized and retry if callback is set
        if response.status() == StatusCode::UNAUTHORIZED {
            // Consume the error response
            let _ = response.text().await;

            // Refresh token
            match self.refresh_token_on_unauthorized().await {
                Ok(new_token) => {
                    // Update token
                    self.set_token(new_token).await;

                    // Retry the request
                    response = build_request().send().await.map_err(|e| match e {
                        reqwest_middleware::Error::Reqwest(err) => {
                            ApiError::HttpMiddleware(format_reqwest_error(err, url))
                        }
                        reqwest_middleware::Error::Middleware(err) => {
                            ApiError::Middleware(err.to_string())
                        }
                    })?;
                }
                Err(e) => return Err(e),
            }
        }

        Ok(response)
    }

    /// Execute a GET request and deserialize JSON response
    #[tracing::instrument(skip(self))]
    pub async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        let url = format!("{}{}", self.base_url, path);

        let response = self
            .execute_with_auth_retry(&url, || self.http_client.request(Method::GET, &url))
            .await?;

        self.handle_response(response).await
    }

    /// Execute a POST request with JSON body and deserialize JSON response
    #[tracing::instrument(skip(self, body))]
    pub async fn post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: Option<impl serde::Serialize>,
    ) -> Result<T, ApiError> {
        let url = format!("{}{}", self.base_url, path);

        // Prepare request body
        let json_body = if let Some(ref body) = body {
            Some(serde_json::to_string(body).map_err(ApiError::Json)?)
        } else {
            None
        };

        let response = self
            .execute_with_auth_retry(&url, || {
                let mut req_builder = self.http_client.request(Method::POST, &url);
                if let Some(ref json_body) = json_body {
                    req_builder = req_builder
                        .header("Content-Type", "application/json")
                        .body(json_body.clone());
                }
                req_builder
            })
            .await?;

        self.handle_response(response).await
    }

    /// Execute a PATCH request with JSON body and deserialize JSON response
    #[tracing::instrument(skip(self, body))]
    pub async fn patch<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: Option<impl serde::Serialize>,
    ) -> Result<T, ApiError> {
        let url = format!("{}{}", self.base_url, path);

        // Prepare request body
        let json_body = if let Some(ref body) = body {
            Some(serde_json::to_string(body).map_err(ApiError::Json)?)
        } else {
            None
        };

        let response = self
            .execute_with_auth_retry(&url, || {
                let mut req_builder = self.http_client.request(Method::PATCH, &url);
                if let Some(ref json_body) = json_body {
                    req_builder = req_builder
                        .header("Content-Type", "application/json")
                        .body(json_body.clone());
                }
                req_builder
            })
            .await?;

        self.handle_response(response).await
    }

    /// Execute a DELETE request
    #[tracing::instrument(skip(self))]
    pub async fn delete<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        let url = format!("{}{}", self.base_url, path);

        let response = self
            .execute_with_auth_retry(&url, || self.http_client.request(Method::DELETE, &url))
            .await?;

        let status = response.status();

        // Handle 204 No Content specially
        if status == StatusCode::NO_CONTENT {
            return Err(ApiError::InvalidResponse(
                "204 No Content - use delete_no_content() instead".to_string(),
            ));
        }

        self.handle_response(response).await
    }

    /// Execute a DELETE request that may return 204 No Content
    #[tracing::instrument(skip(self))]
    pub async fn delete_no_content(&self, path: &str) -> Result<(), ApiError> {
        let url = format!("{}{}", self.base_url, path);

        let response = self
            .execute_with_auth_retry(&url, || self.http_client.request(Method::DELETE, &url))
            .await?;

        let status = response.status();

        if status.is_success() {
            Ok(())
        } else {
            let text = response.text().await.unwrap_or_default();
            let error_message =
                if let Ok(error_obj) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(msg) = error_obj.get("message").and_then(|v| v.as_str()) {
                        msg.to_string()
                    } else {
                        format!(
                            "HTTP {}: {}",
                            status.as_u16(),
                            status.canonical_reason().unwrap_or("Unknown")
                        )
                    }
                } else {
                    format!(
                        "HTTP {}: {}",
                        status.as_u16(),
                        status.canonical_reason().unwrap_or("Unknown")
                    )
                };

            Err(ApiError::ApiError {
                code: status.as_u16(),
                message: error_message,
            })
        }
    }

    /// Execute a GET request with query parameters
    #[tracing::instrument(skip(self, query_params))]
    pub async fn get_with_query<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query_params: &[(&str, Option<String>)],
    ) -> Result<T, ApiError> {
        // Build query string manually
        let mut query_parts = Vec::new();
        for (key, value) in query_params {
            if let Some(val) = value {
                query_parts.push(format!("{}={}", key, urlencoding::encode(val)));
            }
        }

        // Note: teamId is automatically added by TeamMiddleware, so we don't add it here
        // to avoid duplication which would cause it to become an array

        let url = if !query_parts.is_empty() {
            let query_string = query_parts.join("&");
            format!("{}{}?{}", self.base_url, path, query_string)
        } else {
            format!("{}{}", self.base_url, path)
        };

        let response = self
            .execute_with_auth_retry(&url, || self.http_client.request(Method::GET, &url))
            .await?;

        self.handle_response(response).await
    }

    /// Execute a POST request with form-encoded body (for OAuth endpoints)
    #[tracing::instrument(skip(self, form_data))]
    pub async fn post_form<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        form_data: &[(&str, &str)],
    ) -> Result<T, ApiError> {
        let response = self.post_form_raw(path, form_data).await?;
        self.handle_response(response).await
    }

    /// Execute a POST request with form-encoded body and return raw response (for OAuth error handling)
    #[tracing::instrument(skip(self, form_data))]
    pub async fn post_form_raw(
        &self,
        url: &str,
        form_data: &[(&str, &str)],
    ) -> Result<reqwest_middleware::reqwest::Response, ApiError> {
        use reqwest_middleware::reqwest::header::HeaderValue;

        // If URL doesn't start with http, treat it as a path
        let full_url = if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            format!("{}{}", self.base_url, url)
        };

        let mut req_builder = self.http_client.request(Method::POST, &full_url);

        // Build form-encoded body
        let form_body = form_data
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        req_builder = req_builder
            .header(
                reqwest_middleware::reqwest::header::CONTENT_TYPE,
                HeaderValue::from_static("application/x-www-form-urlencoded"),
            )
            .body(form_body);

        let response = req_builder.send().await.map_err(|e| match e {
            reqwest_middleware::Error::Reqwest(err) => {
                ApiError::HttpMiddleware(format_reqwest_error(err, &full_url))
            }
            reqwest_middleware::Error::Middleware(err) => ApiError::Middleware(err.to_string()),
        })?;

        Ok(response)
    }
}

impl Default for Client {
    fn default() -> Self {
        // This should never fail with the default base URL
        // If it does, it indicates a programming error (e.g., invalid User-Agent)
        Self::new().unwrap_or_else(|e| {
            panic!(
                "Failed to create default client: {}. This is a programming error.",
                e
            )
        })
    }
}
