//! Individual deploy provider implementations.
//!
//! Each provider implements the [`DeployProvider`](crate::provider::DeployProvider)
//! trait for a specific hosting platform.

pub mod aws_s3;
pub mod azure_static;
pub mod cloudflare_pages;
pub mod firebase;
pub mod fly;
pub mod github_pages;
pub mod netlify;
pub mod render;
pub mod surge;
pub mod vercel;

pub(crate) mod helpers;
