//! Error types for the code-search crate.

use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("{0}")]
pub struct CodeSearchError(pub String);

impl From<String> for CodeSearchError {
    fn from(s: String) -> Self {
        CodeSearchError(s)
    }
}

impl From<&str> for CodeSearchError {
    fn from(s: &str) -> Self {
        CodeSearchError(s.to_string())
    }
}
