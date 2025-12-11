use crate::client::Client;
use crate::error::ApiError;
use crate::models::{User, UserResponse};

pub struct Users<'a> {
    client: &'a Client,
}

impl<'a> Users<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Retrieves information related to the currently authenticated User.
    #[tracing::instrument(skip(self))]
    pub async fn get_current(&self) -> Result<User, ApiError> {
        let response: UserResponse = self.client.get("/user").await?;
        Ok(response.user)
    }
}
