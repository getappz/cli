mod context;
mod error;
mod exec;
mod output;
mod provider;
mod providers;

pub use context::{DeployConfig, DeployContext, parse_kv, read_deploy_config};
pub use error::{DeployError, DeployResult};
pub use exec::{ExecOutput, exec_captured, exec_captured_scrubbed};
pub use output::{
    AuthState, DeployOutput, DeployStatus, DeploymentInfo, DetectedConfig, PrerequisiteStatus,
};
pub use provider::{
    DEPLOY_TIMEOUT, DeployProvider, QUERY_TIMEOUT, available_provider_slugs,
    create_provider_registry, detect_providers, ensure_cli_available, get_provider,
    install_cli_with_mise,
};
