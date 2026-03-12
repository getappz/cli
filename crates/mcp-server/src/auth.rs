//! Minimal auth check for MCP tool handlers.
//!
//! Mirrors app's resolve_token logic: APPZ_API_TOKEN env, then ~/.appz/auth.json.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct AuthConfig {
    token: Option<String>,
}

/// Check if authentication is available (env var or auth.json).
/// Used before invoking auth-required appz commands.
pub fn has_auth() -> bool {
    // Priority 1: Environment variable
    if let Ok(token) = std::env::var("APPZ_API_TOKEN") {
        if !token.trim().is_empty() {
            return true;
        }
    }

    // Priority 2: auth.json file
    if let Ok(path) = get_auth_path() {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<AuthConfig>(&content) {
                    if config
                        .token
                        .as_ref()
                        .map(|t| !t.trim().is_empty())
                        .unwrap_or(false)
                    {
                        return true;
                    }
                }
            }
        }
    }

    false
}

fn get_auth_path() -> Result<PathBuf, ()> {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| ())?;
    Ok(PathBuf::from(home).join(".appz").join("auth.json"))
}
