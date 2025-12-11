//! Dev Server crate - Local development server with hot reloading and form data processing

pub mod config;
pub mod error;
pub mod handlers;
pub mod server;
pub mod watcher;

pub use config::ServerConfig;
pub use error::{DevServerError, Result};
pub use server::DevServer;
