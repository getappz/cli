mod context;
mod error;
mod exec;
mod fingerprint;
pub mod markdown;
mod output;
mod provider;
mod providers;
pub mod redirects;
pub mod static_cas;
pub mod tunnel;
pub mod webhooks;

pub use context::{
    DeployConfig, DeployContext, DeployHooks, MarkdownConfig, parse_kv, read_deploy_config,
    read_markdown_config,
};
pub use error::{DeployError, DeployResult};
pub use exec::{ExecOutput, exec_captured, exec_captured_scrubbed};
pub use output::{
    ActivityEvent, ActivityPage, AliasInfo, ApexRecord, AuthState, BackupSummary, CnameRecord,
    DeployOutput, DeployStatus, DeploymentInfo, DeploymentInspect, DeploymentQuery, DetectedConfig,
    DnsRecordChallenge, DomainMove, DomainQuery, DomainVerificationRecord, DomainVerifyResult,
    InspectFileEntry, Page, PrerequisiteStatus, ProjectDomain, ProjectInfo, PromoteResult,
    TeamDeployment, TeamDomain, UsageEntry, UsageReport,
};
pub use provider::{
    DEPLOY_TIMEOUT, DeployProvider, QUERY_TIMEOUT, available_provider_slugs,
    create_provider_registry, detect_providers, ensure_cli_available, get_provider,
    install_cli_with_mise,
};
pub use providers::appzdev::projects::{
    ProjectSettingsUpdate, ProjectState, Transfer, TransferPreview,
};
pub use providers::appzdev::{
    CloudEnvMeta, CloudEnvVar, DeviceCodeResponse, DevicePollResult, ProjectEnvironment,
    StoredAuth, add_project, add_project_domain, add_project_environment, auth_file_path,
    clear_stored_auth, delete_cloud_env_var, enable_cli_deploy_interrupts, format_env_file,
    get_project, is_valid_environment_slug, list_cloud_env, list_project_domains,
    list_project_environments, list_projects, load_stored_auth, parse_api_error, parse_env_file,
    poll_device_token, pull_cloud_env, push_cloud_env, remove_project, remove_project_domain,
    remove_project_environment, rename_project, request_device_code, resolve_auth_token,
    resolve_project_id_from_dir, save_stored_auth, slugify_project_id, verify_project_domain,
};
pub use tunnel::{
    CloudflareTunnelProvider, SshTunnelProvider, TunnelConfig, TunnelProvider, TunnelProviderKind,
    TunnelSession, extract_url_from_line, start_tunnel,
};
