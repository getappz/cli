use http::Extensions;
use reqwest_middleware::reqwest::Request;
use reqwest_middleware::{Middleware, Next, Result as MiddlewareResult};
use std::time::Instant;

/// Tracing middleware that logs HTTP requests and responses
/// Compatible with reqwest-middleware 0.4.2
///
/// Logs the request URL, response status code, and response time for each HTTP request.
pub struct TracingMiddleware;

impl TracingMiddleware {
    pub fn new() -> Self {
        Self
    }

    pub fn default() -> Self {
        Self
    }
}

impl Default for TracingMiddleware {
    fn default() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Middleware for TracingMiddleware {
    #[tracing::instrument(skip(self, next, extensions))]
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> MiddlewareResult<reqwest_middleware::reqwest::Response> {
        let url = req.url().clone();
        let method = req.method().clone();

        // Record the start time
        let start = Instant::now();

        // Execute the request
        let result = next.run(req, extensions).await;

        // Calculate elapsed time
        let duration = start.elapsed();
        let duration_ms = duration.as_millis();

        // Log the request details
        match &result {
            Ok(response) => {
                let status = response.status();
                tracing::debug!(
                    method = %method,
                    url = %url,
                    status = %status.as_u16(),
                    duration_ms = duration_ms,
                    "HTTP request completed"
                );
            }
            Err(e) => {
                tracing::warn!(
                    method = %method,
                    url = %url,
                    duration_ms = duration_ms,
                    error = %e,
                    "HTTP request failed"
                );
            }
        }

        result
    }
}
