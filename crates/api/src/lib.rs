pub mod client;
pub mod endpoints;
pub mod error;
mod middleware;
pub mod models;
pub mod paths;
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

use std::sync::Arc;

/// Extension trait for `Arc<Client>` to obtain endpoint handles.
/// Import this trait when using the API with `Arc<Client>` (e.g. from session).
pub trait ClientExt {
    fn auth(&self) -> Auth;
    fn users(&self) -> Users;
    fn teams(&self) -> Teams;
    fn domains(&self) -> Domains;
    fn aliases(&self) -> Aliases;
    fn deployments(&self) -> Deployments;
    fn projects(&self) -> Projects;
    fn gen(&self) -> Gen;
    fn plugins(&self) -> Plugins;
}

impl ClientExt for Arc<Client> {
    fn auth(&self) -> Auth {
        Auth::new(self.clone())
    }

    fn users(&self) -> Users {
        Users::new(self.clone())
    }

    fn teams(&self) -> Teams {
        Teams::new(self.clone())
    }

    fn domains(&self) -> Domains {
        Domains::new(self.clone())
    }

    fn aliases(&self) -> Aliases {
        Aliases::new(self.clone())
    }

    fn deployments(&self) -> Deployments {
        Deployments::new(self.clone())
    }

    fn projects(&self) -> Projects {
        Projects::new(self.clone())
    }

    fn gen(&self) -> Gen {
        Gen::new(self.clone())
    }

    fn plugins(&self) -> Plugins {
        Plugins::new(self.clone())
    }
}

/// Implement ClientExt for &Client so temporary clients (e.g. in auth flow) can use endpoints.
impl ClientExt for &Client {
    fn auth(&self) -> Auth {
        Auth::new(Arc::new((*self).clone()))
    }

    fn users(&self) -> Users {
        Users::new(Arc::new((*self).clone()))
    }

    fn teams(&self) -> Teams {
        Teams::new(Arc::new((*self).clone()))
    }

    fn domains(&self) -> Domains {
        Domains::new(Arc::new((*self).clone()))
    }

    fn aliases(&self) -> Aliases {
        Aliases::new(Arc::new((*self).clone()))
    }

    fn deployments(&self) -> Deployments {
        Deployments::new(Arc::new((*self).clone()))
    }

    fn projects(&self) -> Projects {
        Projects::new(Arc::new((*self).clone()))
    }

    fn gen(&self) -> Gen {
        Gen::new(Arc::new((*self).clone()))
    }

    fn plugins(&self) -> Plugins {
        Plugins::new(Arc::new((*self).clone()))
    }
}
