use api::Client;
use miette::Result;
use serde::{Deserialize, Serialize};
use starbase_utils::dirs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>, // Unix timestamp in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
}

impl AuthConfig {
    pub fn new() -> Self {
        Self {
            token: None,
            refresh_token: None,
            expires_at: None,
            team_id: None,
        }
    }

    pub fn with_token(token: String) -> Self {
        Self {
            token: Some(token),
            refresh_token: None,
            expires_at: None,
            team_id: None,
        }
    }

    pub fn with_oauth_tokens(
        access_token: String,
        refresh_token: Option<String>,
        expires_in: i64,
    ) -> Self {
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + expires_in;

        Self {
            token: Some(access_token),
            refresh_token,
            expires_at: Some(expires_at),
            team_id: None,
        }
    }

    pub fn has_token(&self) -> bool {
        self.token.as_ref().is_some_and(|t| !t.is_empty())
    }

    pub fn is_token_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            now >= expires_at
        } else {
            false // If no expiration, assume not expired
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the path to the auth.json file
pub fn get_auth_path() -> Result<PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| miette::miette!("Could not determine home directory"))?;
    Ok(home_dir.join(".appz").join("auth.json"))
}

/// Load authentication config from ~/.appz/auth.json
pub fn load_auth() -> Result<AuthConfig> {
    let auth_path = get_auth_path()?;

    if !auth_path.exists() {
        return Ok(AuthConfig::new());
    }

    use starbase_utils::{fs, json};

    // Try to read and parse the auth file
    match json::read_file(&auth_path) {
        Ok(config) => Ok(config),
        Err(e) => {
            // If file doesn't exist or is empty, return default config
            if !auth_path.exists() {
                return Ok(AuthConfig::new());
            }
            // Check if file is empty
            if let Ok(content) = fs::read_file(&auth_path) {
                if content.trim().is_empty() {
                    return Ok(AuthConfig::new());
                }
            }
            Err(miette::miette!(
                "Failed to read/parse auth file {}: {}",
                auth_path.display(),
                e
            ))
        }
    }
}

/// Save authentication config to ~/.appz/auth.json
pub fn save_auth(config: &AuthConfig) -> Result<()> {
    let auth_path = get_auth_path()?;
    let auth_dir = auth_path
        .parent()
        .ok_or_else(|| miette::miette!("Invalid auth path"))?;

    use starbase_utils::{fs, json};

    // Create .appz directory if it doesn't exist
    if !auth_dir.exists() {
        fs::create_dir_all(auth_dir).map_err(|e| {
            miette::miette!("Failed to create directory {}: {}", auth_dir.display(), e)
        })?;
    }

    json::write_file(&auth_path, config, true)
        .map_err(|e| miette::miette!("Failed to write auth file {}: {}", auth_path.display(), e))?;

    Ok(())
}

/// Resolve authentication token from available sources in priority order:
/// 1. CLI argument (--token)
/// 2. Environment variable (APPZ_TOKEN)
/// 3. auth.json file
/// Returns None if no token is found (does not prompt)
pub fn resolve_token(cli: &crate::app::Cli) -> Option<String> {
    // Priority 1: CLI argument
    if let Some(ref token) = cli.token {
        if !token.is_empty() {
            return Some(token.clone());
        }
    }

    // Priority 2: Environment variable
    if let Ok(token) = std::env::var("APPZ_API_TOKEN") {
        if !token.is_empty() {
            return Some(token);
        }
    }

    // Priority 3: auth.json file
    if let Ok(auth_config) = load_auth() {
        if let Some(ref token) = auth_config.token {
            if !token.is_empty() {
                return Some(token.clone());
            }
        }
    }

    None
}

/// Resolve team ID from available sources in priority order:
/// 1. --scope option (CLI, highest priority)
/// 2. Environment variable (APPZ_TEAM_ID)
/// 3. auth.json file
/// Returns None if no team_id is found
///
/// Note: If --scope is provided, it may be a team ID or slug and needs to be resolved.
pub fn resolve_team_id(cli: &crate::app::Cli) -> Option<String> {
    // Priority 1: --scope option (CLI, highest priority)
    if let Some(ref scope) = cli.scope {
        if !scope.is_empty() {
            // Return scope as-is - it will be resolved to team ID when needed
            // Commands that need team ID will use resolve_team_id helper from teams module
            return Some(scope.clone());
        }
    }

    // Priority 2: Environment variable
    if let Ok(team_id) = std::env::var("APPZ_TEAM_ID") {
        if !team_id.is_empty() {
            return Some(team_id);
        }
    }

    // Priority 3: auth.json file
    if let Ok(auth_config) = load_auth() {
        if let Some(ref team_id) = auth_config.team_id {
            if !team_id.is_empty() {
                return Some(team_id.clone());
            }
        }
    }

    None
}

/// Save team ID to auth.json config file
pub fn save_team_id(team_id: String) -> Result<()> {
    let mut auth_config = load_auth().unwrap_or_else(|_| AuthConfig::new());
    auth_config.team_id = Some(team_id);
    save_auth(&auth_config)
}

/// Clear team ID from auth.json config file
pub fn clear_team_id() -> Result<()> {
    let mut auth_config = load_auth().unwrap_or_else(|_| AuthConfig::new());
    auth_config.team_id = None;
    save_auth(&auth_config)
}

/// Clear authentication token from auth.json config file
pub fn clear_token() -> Result<()> {
    let mut auth_config = load_auth().unwrap_or_else(|_| AuthConfig::new());
    auth_config.token = None;
    save_auth(&auth_config)
}

/// Determine if a command requires authentication
pub fn requires_auth(command: &crate::app::Commands) -> bool {
    matches!(
        command,
        crate::app::Commands::Ls
            | crate::app::Commands::Run { .. }
            | crate::app::Commands::Plan { .. }
            | crate::app::Commands::Switch { .. }
            | crate::app::Commands::Teams { .. }
            | crate::app::Commands::Projects { .. }
            | crate::app::Commands::Aliases { .. }
            | crate::app::Commands::Domains { .. }
            | crate::app::Commands::Promote { .. }
            | crate::app::Commands::Rollback { .. }
            | crate::app::Commands::Remove { .. }
    )
}

/// Get OAuth 2.0 Device Flow client ID from environment variable or use default
/// 
/// Priority:
/// 1. OAUTH_CLI_CLIENT_ID environment variable
/// 2. Default: cl_appz_cli_550e8400e29b41d4a716446655440000
fn get_cli_client_id() -> String {
    std::env::var("OAUTH_CLI_CLIENT_ID")
        .unwrap_or_else(|_| "cl_appz_cli_550e8400e29b41d4a716446655440000".to_string())
}

/// Run OAuth 2.0 Device Flow login
pub async fn device_flow_login(client: &Client) -> Result<String> {
    use api::OAuthPollError;
    use std::time::Duration;
    use tokio::time::sleep;
    use ui::status;
    use ui::prompt;

    // Step 1: Request device authorization
    let client_id = get_cli_client_id();
    let auth_response = client
        .auth()
        .device_authorize(&client_id)
        .await
        .map_err(|e| miette::miette!("Failed to request device authorization: {}", e))?;

    let device_code = auth_response.device_code.clone();
    let user_code = auth_response.user_code.clone();
    let verification_uri = auth_response.verification_uri.clone();
    let verification_uri_complete = auth_response.verification_uri_complete.clone();
    let expires_in = auth_response.expires_in;
    let interval = auth_response.interval.unwrap_or(5); // Default to 5 seconds

    let expires_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + expires_in;

    // Step 2: Display user code and verification URL (Vercel CLI style)
    println!();
    println!("Visit {}", verification_uri_complete);
    println!("and enter code: {}", user_code);
    println!();
    
    // Prompt user to press ENTER to open browser
    let _ = prompt::prompt("Press [ENTER] to open the browser", Some(""));

    // Try to open browser
    if let Err(e) = webbrowser::open(&verification_uri_complete) {
        tracing::debug!("Failed to open browser: {}", e);
        // Continue anyway - user can open manually
    }

    println!();
    status::info("Waiting for authentication...")
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    // Step 3: Poll for token
    let mut poll_interval_ms = interval * 1000;
    let mut consecutive_errors = 0;
    const MAX_CONSECUTIVE_ERRORS: u32 = 5; // Allow up to 5 consecutive network errors before giving up

    loop {
        // Check if expired
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        if now >= expires_at {
            return Err(miette::miette!(
                "Device authorization expired. Please try again."
            ));
        }

        // Poll for token
        let client_id = get_cli_client_id();
        match client.auth().device_token(&client_id, &device_code).await {
            Ok(Ok(token_set)) => {
                // Success! Save tokens
                let auth_config = AuthConfig::with_oauth_tokens(
                    token_set.access_token.clone(),
                    token_set.refresh_token.clone(),
                    token_set.expires_in,
                );
                save_auth(&auth_config)
                    .map_err(|e| miette::miette!("Failed to save authentication: {}", e))?;

                println!();
                status::success("Congratulations! You are now signed in.")
                    .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

                return Ok(token_set.access_token);
            }
            Ok(Err(OAuthPollError::AuthorizationPending)) => {
                // Continue polling - reset error counter on successful API call
                consecutive_errors = 0;
                sleep(Duration::from_millis(poll_interval_ms.max(0) as u64)).await;
                continue;
            }
            Ok(Err(OAuthPollError::SlowDown)) => {
                // Increase interval by 5 seconds
                poll_interval_ms += 5000;
                consecutive_errors = 0; // Reset error counter
                tracing::debug!(
                    "Server requested slow down. Polling every {}ms",
                    poll_interval_ms
                );
                sleep(Duration::from_millis(poll_interval_ms.max(0) as u64)).await;
                continue;
            }
            Err(e) => {
                // Check if it's a network/connection error that we should retry
                let error_str = e.to_string().to_lowercase();
                let is_network_error = error_str.contains("connection")
                    || error_str.contains("timeout")
                    || error_str.contains("dns")
                    || error_str.contains("network")
                    || error_str.contains("sendrequest")
                    || error_str.contains("failed to connect");

                if is_network_error {
                    consecutive_errors += 1;
                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        return Err(miette::miette!(
                            "Failed to connect to server after {} attempts. Please check your internet connection and try again.",
                            MAX_CONSECUTIVE_ERRORS
                        ));
                    }
                    
                    // Log warning but continue polling
                    tracing::warn!(
                        "Network error during polling (attempt {}/{}): {}. Retrying...",
                        consecutive_errors,
                        MAX_CONSECUTIVE_ERRORS,
                        e
                    );
                    
                    // Wait a bit longer on network errors before retrying
                    sleep(Duration::from_millis((poll_interval_ms + 2000).max(0) as u64)).await;
                    continue;
                } else {
                    // Fatal error (not a network issue)
                return Err(miette::miette!("Failed to get token: {}", e));
            }
        }
    }
}
}

