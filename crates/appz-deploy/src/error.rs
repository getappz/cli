use thiserror::Error;

pub type DeployResult<T> = Result<T, DeployError>;

#[derive(Debug, Error)]
pub enum DeployError {
    #[error("{provider}: CLI tool '{tool}' not found on PATH. Install with: {install_hint}")]
    CliMissing {
        provider: &'static str,
        tool: &'static str,
        install_hint: String,
    },

    #[error("{provider}: not authenticated. Set {env_var} or run: {login_hint}")]
    AuthMissing {
        provider: &'static str,
        env_var: &'static str,
        login_hint: String,
    },

    #[error("{provider}: deploy failed: {reason}")]
    DeployFailed {
        provider: &'static str,
        reason: String,
    },

    #[error("{provider}: '{operation}' is not supported")]
    Unsupported {
        provider: &'static str,
        operation: &'static str,
    },

    #[error("{provider}: command timed out after {secs}s: {command}")]
    Timeout {
        provider: &'static str,
        command: String,
        secs: u64,
    },

    #[error("no deploy provider matched slug '{slug}'")]
    ProviderNotFound { slug: String },

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
