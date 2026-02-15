pub mod client;
pub mod endpoints;
pub mod error;
mod middleware;
pub mod models;
pub mod http {
    pub mod error_mapper;
    pub mod response_handler;
}

pub use client::Client;
pub use endpoints::{
    Aliases, Auth, Deployments, Domains, Gen, OAuthPollError, Plugins, Projects, Teams, Users,
};
pub use error::ApiError;
pub use models::*;

impl Client {
    /// Get authentication endpoints
    pub fn auth(&self) -> Auth<'_> {
        Auth::new(self)
    }

    /// Get user endpoints
    pub fn users(&self) -> Users<'_> {
        Users::new(self)
    }

    /// Get team endpoints
    pub fn teams(&self) -> Teams<'_> {
        Teams::new(self)
    }

    /// Get domain endpoints
    pub fn domains(&self) -> Domains<'_> {
        Domains::new(self)
    }

    /// Get alias endpoints
    pub fn aliases(&self) -> Aliases<'_> {
        Aliases::new(self)
    }

    /// Get deployment endpoints
    pub fn deployments(&self) -> Deployments<'_> {
        Deployments::new(self)
    }

    /// Get project endpoints
    pub fn projects(&self) -> Projects<'_> {
        Projects::new(self)
    }

    /// Get gen (AI code generation) endpoints
    pub fn gen(&self) -> Gen<'_> {
        Gen::new(self)
    }

    /// Get plugin management endpoints
    pub fn plugins(&self) -> Plugins<'_> {
        Plugins::new(self)
    }
}
