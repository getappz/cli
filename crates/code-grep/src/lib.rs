//! Safe ripgrep execution for code search.
//!
//! No shell, no flag injection, bounded execution with timeout.

mod execute;
mod parse;
mod schema;
mod validate;

pub use execute::execute;
pub use schema::{RawMatch, SearchRequest, SearchResult};
pub use validate::validate_request;
