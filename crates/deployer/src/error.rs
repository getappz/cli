//! Deployer error types with rich diagnostic help messages.
//!
//! Every error variant includes a [`miette::Diagnostic`] code and a
//! human-readable `help` hint so that CLI users get actionable guidance
//! when something goes wrong.

#![allow(unused_assignments)]

use miette::Diagnostic;
use thiserror::Error;

/// Result type alias for deployer operations.
pub type DeployResult<T> = Result<T, DeployError>;

/// Errors that can occur during deployment operations.
#[derive(Error, Debug, Diagnostic)]
pub enum DeployError {
    #[diagnostic(
        code(deployer::provider_not_found),
        help("Available providers: vercel, netlify, cloudflare-pages, github-pages, firebase, aws-s3, azure-static, surge, fly, render.\nRun 'appz deploy --help' to see all options.")
    )]
    #[error("Unknown deploy provider: {slug}")]
    ProviderNotFound { slug: String },

    #[diagnostic(
        code(deployer::no_provider_detected),
        help("No deployment platform was detected in your project.\nRun 'appz deploy <provider>' to set one up, or add a [deploy] section to appz.json.")
    )]
    #[error("No deployment provider detected or configured")]
    NoProviderDetected,

    #[diagnostic(
        code(deployer::missing_config),
        help("Run 'appz deploy init <provider>' to set up deployment configuration,\nor create the config manually in appz.json under the 'deploy' section.")
    )]
    #[error("Missing deployment configuration for provider: {provider}")]
    MissingConfig { provider: String },

    #[diagnostic(
        code(deployer::cli_not_found),
        help("Install the required CLI tool, or let appz install it automatically.\nRun 'appz deploy init {provider}' to set up the provider.")
    )]
    #[error("Required CLI tool not found: {tool}")]
    CliNotFound { tool: String, provider: String },

    #[diagnostic(
        code(deployer::auth_required),
        help("Set the authentication token via environment variable:\n  {env_var}=<your-token>\nOr run the provider's login command:\n  {login_hint}")
    )]
    #[error("Authentication required for {provider}")]
    AuthRequired {
        provider: String,
        env_var: String,
        login_hint: String,
    },

    #[diagnostic(
        code(deployer::deploy_failed),
        help("Check the deployment logs above for details.\nYou can also try running with --verbose for more information.")
    )]
    #[error("Deployment to {provider} failed: {reason}")]
    DeployFailed { provider: String, reason: String },

    #[diagnostic(
        code(deployer::build_failed),
        help("Fix the build errors and try again.\nRun 'appz build' separately to debug build issues.\nUse '--no-build' to skip the build step if already built.")
    )]
    #[error("Build failed before deployment: {reason}")]
    BuildFailed { reason: String },

    #[diagnostic(
        code(deployer::output_dir_not_found),
        help("Ensure the build output directory exists.\nCheck 'outputDirectory' in appz.json or the framework's build configuration.")
    )]
    #[error("Build output directory not found: {path}")]
    OutputDirNotFound { path: String },

    #[diagnostic(
        code(deployer::unsupported),
        help("This operation is not supported by the {provider} provider.")
    )]
    #[error("{provider} does not support: {operation}")]
    Unsupported { provider: String, operation: String },

    #[diagnostic(
        code(deployer::ci_missing_config),
        help("In CI/CD mode, all configuration must be provided upfront.\nEnsure appz.json has a [deploy] section with the target provider configured,\nor set the required environment variables.")
    )]
    #[error("CI/CD mode requires pre-configured deployment (no interactive prompts available)")]
    CiMissingConfig,

    #[diagnostic(
        code(deployer::command_failed),
        help("Check the command output above for details.")
    )]
    #[error("Command failed: {command}\n{reason}")]
    CommandFailed { command: String, reason: String },

    #[diagnostic(
        code(deployer::hook_failed),
        help("Check the hook script for errors. Hooks are configured in appz.json under deploy.hooks.")
    )]
    #[error("Deploy hook '{hook}' failed: {reason}")]
    HookFailed { hook: String, reason: String },

    #[diagnostic(
        code(deployer::io_error),
        help("Check file permissions and available disk space.")
    )]
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[diagnostic(
        code(deployer::json_error),
        help("Verify the configuration file is well-formed JSON.")
    )]
    #[error("JSON error: {reason}")]
    JsonError { reason: String },

    #[diagnostic(code(deployer::other))]
    #[error("{0}")]
    Other(String),
}

impl From<serde_json::Error> for DeployError {
    fn from(err: serde_json::Error) -> Self {
        DeployError::JsonError {
            reason: err.to_string(),
        }
    }
}

impl From<miette::Error> for DeployError {
    fn from(err: miette::Error) -> Self {
        DeployError::Other(err.to_string())
    }
}

impl From<sandbox::SandboxError> for DeployError {
    fn from(err: sandbox::SandboxError) -> Self {
        DeployError::CommandFailed {
            command: "sandbox operation".into(),
            reason: err.to_string(),
        }
    }
}
