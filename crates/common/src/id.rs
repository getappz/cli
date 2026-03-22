//! ID generation and validation utilities.

use regex::Regex;
use rustc_hash::FxHasher;
use std::hash::Hasher;
use std::sync::LazyLock;
use thiserror::Error;

static VALID_ID_CHARS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap());
static MULTI_DASH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-+").unwrap());

/// Error types for ID operations
#[derive(Error, Debug)]
pub enum IdError {
    #[error("Invalid ID format: {0}")]
    InvalidFormat(String),

    #[error("ID contains invalid characters: {0}")]
    InvalidCharacters(String),

    #[error("ID is too long: max length is {max}, got {actual}")]
    TooLong { max: usize, actual: usize },

    #[error("ID is too short: min length is {min}, got {actual}")]
    TooShort { min: usize, actual: usize },
}

/// Valid characters for IDs (alphanumeric, dash, underscore)
pub const ID_CHARS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";

/// Valid symbols for IDs
pub const ID_SYMBOLS: &[char] = &['-', '_'];

/// Default minimum ID length
pub const MIN_ID_LENGTH: usize = 1;

/// Default maximum ID length
pub const MAX_ID_LENGTH: usize = 100;

/// Validate an ID string
pub fn validate_id(id: &str) -> Result<(), IdError> {
    if id.len() < MIN_ID_LENGTH {
        return Err(IdError::TooShort {
            min: MIN_ID_LENGTH,
            actual: id.len(),
        });
    }

    if id.len() > MAX_ID_LENGTH {
        return Err(IdError::TooLong {
            max: MAX_ID_LENGTH,
            actual: id.len(),
        });
    }

    // Check for valid characters
    if !VALID_ID_CHARS.is_match(id) {
        return Err(IdError::InvalidCharacters(id.to_string()));
    }

    Ok(())
}

/// Generate a stable ID from a string
pub fn stable_id<S: AsRef<str>>(id: S) -> String {
    let id = id.as_ref();

    if let Some(suffix) = id.strip_prefix("unstable_") {
        suffix.to_string()
    } else {
        id.to_string()
    }
}

/// Generate an unstable ID from a string
pub fn unstable_id<S: AsRef<str>>(id: S) -> String {
    let id = id.as_ref();

    if id.starts_with("unstable_") {
        id.to_string()
    } else {
        format!("unstable_{id}")
    }
}

/// Generate both stable and unstable IDs
pub fn stable_and_unstable<S: AsRef<str>>(id: S) -> (String, String) {
    let id = id.as_ref();
    (stable_id(id), unstable_id(id))
}

/// Hash a string into a numeric ID
pub fn hash_id<S: AsRef<str>>(value: S) -> String {
    let mut hasher = FxHasher::default();
    hasher.write(value.as_ref().as_bytes());
    format!("{}", hasher.finish())
}

/// Sanitize a string to be used as an ID
pub fn sanitize_id<S: AsRef<str>>(value: S) -> String {
    let value = value.as_ref();
    let mut result = String::new();

    for ch in value.chars() {
        match ch {
            // Keep alphanumeric
            c if c.is_alphanumeric() => result.push(c),
            // Convert spaces and common separators to dash
            ' ' | '_' | '.' => result.push('-'),
            // Skip other characters
            _ => {}
        }
    }

    // Remove leading/trailing dashes and collapse multiple dashes
    result = MULTI_DASH.replace_all(&result, "-").to_string();
    result.trim_matches('-').to_lowercase()
}

/// Generate a short ID from a string (first 8 characters of hash)
pub fn short_id<S: AsRef<str>>(value: S) -> String {
    let hash = hash_id(value);
    hash.chars().take(8).collect()
}
