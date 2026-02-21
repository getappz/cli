//! Appz deployment client — create deployment, build file tree, upload, continue.
//!
//! Ports the Vercel client flow for prebuilt deployments. Uses content-addressed
//! file upload (SHA1) for deduplication.

pub mod deploy;
pub mod file_tree;
pub mod upload;

pub use deploy::{deploy_prebuilt, deploy_prebuilt_stream, DeployContext, DeployEvent, DeployOutput};
pub use file_tree::build_file_tree;
