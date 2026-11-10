mod context;
mod error;
mod exec;
mod output;
mod provider;
mod providers;
pub mod static_cas;

pub use context::{DeployConfig, DeployContext, parse_kv, read_deploy_config};
pub use error::{DeployError, DeployResult};
pub use exec::ExecOutput;
pub use output::{
    AuthState, DeployOutput, DeployStatus, DeploymentInfo, DeploymentInspect, DetectedConfig,
    InspectFileEntry, PrerequisiteStatus,
};
pub use provider::{
    DEPLOY_TIMEOUT, DeployProvider, QUERY_TIMEOUT, available_provider_slugs,
    create_provider_registry, detect_providers, ensure_cli_available, get_provider,
    install_cli_with_mise,
};
pub use providers::appzdev::{
    DeviceCodeResponse, DevicePollResult, StoredAuth, auth_file_path, load_stored_auth,
    parse_api_error, poll_device_token, request_device_code, resolve_auth_token, save_stored_auth,
    slugify_project_id,
};
