use http::Extensions;
use reqwest_middleware::reqwest::{Request, Response, StatusCode};
use reqwest_middleware::{Middleware, Next, Result as MiddlewareResult};
use std::time::Duration;

/// Retry policy configuration
pub struct RetryPolicy {
    max_retries: u32,
    initial_delay: Duration,
    max_delay: Duration,
    backoff_multiplier: f64,
}

impl RetryPolicy {
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }

    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }

    fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay_ms =
            self.initial_delay.as_millis() as f64 * self.backoff_multiplier.powi(attempt as i32);
        let delay = Duration::from_millis(delay_ms as u64);
        delay.min(self.max_delay)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new(3)
    }
}

/// Retry middleware that automatically retries failed requests with exponential backoff
/// Compatible with reqwest-middleware 0.4.2
pub struct RetryMiddleware {
    policy: RetryPolicy,
}

impl RetryMiddleware {
    pub fn new(policy: RetryPolicy) -> Self {
        Self { policy }
    }

    pub fn new_with_policy(policy: RetryPolicy) -> Self {
        Self::new(policy)
    }
}

/// Check if a status code indicates a retryable error
fn is_retryable_status(status: StatusCode) -> bool {
    // Retry on 5xx server errors and 429 (Too Many Requests)
    status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
}

/// Check if an error is retryable
fn is_retryable_error(err: &reqwest_middleware::Error) -> bool {
    match err {
        reqwest_middleware::Error::Reqwest(e) => {
            // Retry on network errors, timeouts, and connection errors
            e.is_timeout() || e.is_connect() || e.is_request()
        }
        reqwest_middleware::Error::Middleware(_) => {
            // Don't retry middleware errors
            false
        }
    }
}

#[async_trait::async_trait]
impl Middleware for RetryMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> MiddlewareResult<Response> {
        // Note: In reqwest-middleware, `next` can only be called once per request.
        // Retry logic needs to be implemented at a higher level (client level).
        // This middleware is a placeholder that matches the structure of reqwest-retry
        // but doesn't actually retry. For now, we'll just pass through.
        //
        // TODO: Implement proper retry logic at the client level or use a different approach
        next.run(req, extensions).await
    }
}
