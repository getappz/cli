use http::Extensions;
use reqwest_middleware::reqwest::Request;
use reqwest_middleware::{Middleware, Next, Result as MiddlewareResult};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct TeamMiddleware {
    team_id: Arc<RwLock<Option<String>>>,
}

impl TeamMiddleware {
    pub fn new(team_id: Arc<RwLock<Option<String>>>) -> Self {
        Self { team_id }
    }
}

#[async_trait::async_trait]
impl Middleware for TeamMiddleware {
    async fn handle(
        &self,
        mut req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> MiddlewareResult<reqwest_middleware::reqwest::Response> {
        if let Some(team_id) = self.team_id.read().await.clone() {
            let mut url = req.url().clone();
            {
                let mut pairs = url.query_pairs_mut();
                pairs.append_pair("teamId", &team_id);
            }
            *req.url_mut() = url;
        }

        next.run(req, extensions).await
    }
}
