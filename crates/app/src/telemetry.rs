//! CLI telemetry event collection and flush (R2 event lake).
//!
//! Collects command usage during CLI invocation and POSTs NDJSON batches to
//! backend when enabled. Respects config + APPZ_TELEMETRY_DISABLED.

use crate::config::{load_config, UserConfig};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tracing::debug;
use uuid::Uuid;

/// Run outcome for telemetry (unknown until command completes).
const OUTCOME_UNKNOWN: u8 = 0;
const OUTCOME_SUCCESS: u8 = 1;
const OUTCOME_FAILURE: u8 = 2;

/// Analytics-friendly event schema (flat, DuckDB-ready).
#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsEvent {
    pub timestamp: u64,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub command: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

/// Single telemetry event (internal key-value structure).
#[derive(Debug, Clone)]
struct TelemetryEvent {
    event_time: u64,
    key: String,
    value: String,
}

/// Central event store for a CLI invocation. Accumulates events, flushes at exit.
pub struct TelemetryEventStore {
    events: Mutex<Vec<TelemetryEvent>>,
    session_id: String,
    team_id: Mutex<Option<String>>,
    user_id: Mutex<Option<String>>,
    run_outcome: AtomicU8,
    config: UserConfig,
    /// Prevent double-flush
    flushed: AtomicBool,
}

impl TelemetryEventStore {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            session_id: Uuid::new_v4().to_string(),
            team_id: Mutex::new(None),
            user_id: Mutex::new(None),
            run_outcome: AtomicU8::new(OUTCOME_UNKNOWN),
            config: load_config().unwrap_or_default(),
            flushed: AtomicBool::new(false),
        }
    }

    /// Whether telemetry is enabled (config + env).
    pub fn enabled(&self) -> bool {
        if let Ok(v) = std::env::var("APPZ_TELEMETRY_DISABLED") {
            if matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on") {
                return false;
            }
        }
        self.config.telemetry_enabled()
    }

    fn event_time_sec() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Record a command (e.g. "deploy", "project", "telemetry").
    pub async fn track_command(&self, command: &str, value: &str) {
        self.add("command", command, value).await;
    }

    /// Record a subcommand (e.g. "status", "enable", "ls").
    pub async fn track_subcommand(&self, subcommand: &str, value: &str) {
        self.add("subcommand", subcommand, value).await;
    }

    /// Record a flag (e.g. "help", "yes").
    pub async fn track_flag(&self, flag: &str) {
        self.add("flag", flag, "TRUE").await;
    }

    async fn add(&self, kind: &str, name: &str, value: &str) {
        let event = TelemetryEvent {
            event_time: Self::event_time_sec(),
            key: format!("{}:{}", kind, name),
            value: value.to_string(),
        };
        let mut events = self.events.lock().await;
        events.push(event);
    }

    /// Update team_id for user-attribution (called when user is logged in).
    pub async fn set_team_id(&self, team_id: Option<String>) {
        let mut t = self.team_id.lock().await;
        *t = team_id;
    }

    /// Update user_id for attribution (optional; Phase 1 often leaves None).
    pub async fn set_user_id(&self, user_id: Option<String>) {
        let mut u = self.user_id.lock().await;
        *u = user_id;
    }

    /// Set run outcome for success field. Call before flush.
    pub fn set_run_outcome(&self, success: bool) {
        self.run_outcome.store(
            if success { OUTCOME_SUCCESS } else { OUTCOME_FAILURE },
            Ordering::SeqCst,
        );
    }

    /// Flush events to backend. Blocks until send completes. Safe to call multiple times (no-op after first).
    pub async fn flush(&self) {
        if self.flushed.swap(true, Ordering::SeqCst) {
            return;
        }
        if !self.enabled() {
            debug!("Telemetry disabled, skipping flush");
            return;
        }
        let events: Vec<TelemetryEvent> = {
            let mut e = self.events.lock().await;
            std::mem::take(&mut *e)
        };
        let primary_command = events
            .iter()
            .find(|e| e.key.starts_with("command:"))
            .map(|e| e.value.clone());

        let Some(command) = primary_command else {
            return;
        };

        let outcome = self.run_outcome.load(Ordering::SeqCst);
        let success = match outcome {
            OUTCOME_SUCCESS => true,
            OUTCOME_FAILURE => false,
            _ => true,
        };

        let session_id = self.session_id.clone();
        let user_id = self.user_id.lock().await.clone();
        let version = env!("CARGO_PKG_VERSION").to_string();
        let os = std::env::consts::OS.to_string();
        let arch = std::env::consts::ARCH.to_string();

        let analytics = AnalyticsEvent {
            timestamp: Self::event_time_sec(),
            session_id,
            user_id,
            version,
            os,
            arch,
            command,
            success,
            duration_ms: None,
        };

        let ndjson = serde_json::to_string(&analytics).unwrap_or_default();
        if ndjson.is_empty() {
            return;
        }

        if let Err(e) = send_telemetry(&ndjson).await {
            debug!("Telemetry flush failed: {}", e);
        }
    }
}

impl Default for TelemetryEventStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract command name from Commands for telemetry (takes owned clone to avoid refs in Send futures).
pub fn command_name_for_telemetry(command: crate::app::Commands) -> String {
    use crate::app::Commands;
    let name = match command {
        Commands::List => "list",
        Commands::Plan(_) => "plan",
        Commands::Run(_) => "run",
        Commands::RecipeValidate(_) => "recipe-validate",
        Commands::Dev(_) => "dev",
        #[cfg(feature = "dev-server")]
        Commands::DevServer(_) => "dev-server",
        Commands::Build => "build",
        #[cfg(feature = "dev-server")]
        Commands::Preview(_) => "preview",
        Commands::Ls(_) => "ls",
        Commands::Open => "open",
        Commands::Link(_) => "link",
        Commands::Unlink => "unlink",
        Commands::Login => "login",
        Commands::Logout => "logout",
        Commands::Whoami(_) => "whoami",
        Commands::Init(_) => "init",
        Commands::Switch(_) => "switch",
        Commands::Teams { .. } => "teams",
        Commands::Telemetry { .. } => "telemetry",
        Commands::Projects { .. } => "project",
        Commands::Transfer { .. } => "transfer",
        Commands::Aliases { .. } => "aliases",
        Commands::Domains { .. } => "domains",
        Commands::Pull(_) => "pull",
        Commands::Logs(_) => "logs",
        Commands::Inspect(_) => "inspect",
        Commands::Env { .. } => "env",
        Commands::Promote(_) => "promote",
        Commands::Rollback(_) => "rollback",
        Commands::Remove(_) => "remove",
        #[cfg(feature = "deploy")]
        Commands::Deploy(_) => "deploy",
        #[cfg(feature = "deploy")]
        Commands::DeployInit(_) => "deploy-init",
        #[cfg(feature = "deploy")]
        Commands::DeployList(_) => "deploy-list",
        Commands::Pack(_) => "pack",
        Commands::Code { .. } => "code",
        Commands::Skills { .. } => "skills",
        Commands::Plugin { .. } => "plugin",
        Commands::Git { .. } => "git",
        Commands::Exec(_) => "exec",
        #[cfg(feature = "mcp")]
        Commands::McpServer => "mcp",
        #[cfg(feature = "self_update")]
        Commands::SelfUpdate(_) => "self-update",
        Commands::External(_) => "external",
    };
    name.to_string()
}

/// Record the current command from the CLI. Call at start of command dispatch.
/// Takes owned `store` and `cmd_name` to avoid capturing references in the async future (Send).
pub async fn record_command(store: std::sync::Arc<TelemetryEventStore>, cmd_name: String) {
    let s = cmd_name.as_str();
    store.track_command(s, s).await;
}

async fn send_telemetry(ndjson: &str) -> Result<(), String> {
    let url = std::env::var("APPZ_API_URL")
        .unwrap_or_else(|_| "https://api.appz.dev".to_string());
    let endpoint = format!("{}/v0/telemetry/events", url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let res = client
        .post(&endpoint)
        .body(ndjson.to_string())
        .header("Content-Type", "application/x-ndjson")
        .header("User-Agent", format!("appz-cli/{}", env!("CARGO_PKG_VERSION")))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if res.status().is_success() || res.status().as_u16() == 204 {
        Ok(())
    } else {
        Err(format!("Unexpected status: {}", res.status()))
    }
}
