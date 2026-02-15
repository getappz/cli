//! # Appz PDK - Plugin Development Kit
//!
//! This crate provides utilities and types for developing WASM plugins for saasctl.
//! It reduces boilerplate by providing shared types and helper macros.
//!
//! # Usage
//!
//! ```rust,no_run
//! use appz_pdk::prelude::*;
//! use extism_pdk::*;
//!
//! #[host_fn]
//! extern "ExtismHost" {
//!     fn appz_reg_task(input: Json<TaskInput>) -> Json<TaskResponse>;
//!     // ... other host functions
//! }
//!
//! #[plugin_fn]
//! pub fn appz_register() -> FnResult<()> {
//!     unsafe {
//!         appz_set!("key", "value");
//!         let _ = appz_reg_task(Json(TaskInput {
//!             name: "my:task".to_string(),
//!             desc: Some("My task".to_string()),
//!             deps: None,
//!             body: None,
//!             only_if: None,
//!             unless: None,
//!             once: None,
//!             hidden: None,
//!             timeout: None,
//!         }));
//!     }
//!     Ok(())
//! }
//! ```

pub mod prelude;
pub mod security;
pub mod types;

// Re-export types at crate root for macros
pub use types::*;

// Re-export macros at crate root
#[macro_use]
mod macros;
#[macro_use]
mod plugin_macros;
