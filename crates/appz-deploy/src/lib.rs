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
    DomainVerificationRecord, DomainVerifyResult, InspectFileEntry, PrerequisiteStatus,
    ProjectDomain,
};
pub use provider::{
    DEPLOY_TIMEOUT, DeployProvider, QUERY_TIMEOUT, available_provider_slugs,
    create_provider_registry, detect_providers, ensure_cli_available, get_provider,
    install_cli_with_mise,
};
pub use providers::appzdev::{
    CloudEnvVar, DeviceCodeResponse, DevicePollResult, StoredAuth, add_project_domain,
    auth_file_path, format_env_file, list_project_domains, load_stored_auth, parse_api_error,
    parse_env_file, poll_device_token, pull_cloud_env, push_cloud_env, remove_project_domain,
    request_device_code, resolve_auth_token, resolve_project_id_from_dir, save_stored_auth,
    slugify_project_id, verify_project_domain,
};
