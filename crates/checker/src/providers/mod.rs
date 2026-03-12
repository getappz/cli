//! Individual check provider implementations.
//!
//! Each provider wraps a specific linting/checking tool and translates
//! its output into the unified [`CheckIssue`](crate::output::CheckIssue) format.

pub mod biome;
pub mod clippy;
pub mod helpers;
pub mod phpstan;
pub mod ruff;
pub mod secrets;
pub mod stylelint;
pub mod typescript;
