//! Application and deployment constants.

/// Binary name for the application
#[cfg(windows)]
pub const BIN_NAME: &str = "appz.exe";

#[cfg(not(windows))]
pub const BIN_NAME: &str = "appz";

/// Application name
pub const APP_NAME: &str = "appz";

/// Configuration directory name
pub const CONFIG_DIRNAME: &str = ".appz";

// Deployment-related constants

/// Directory/file names for deployment
pub mod deploy {
    /// Deployment directory
    pub const DEP_DIR: &str = ".dep";

    /// Releases directory
    pub const RELEASES_DIR: &str = "releases";

    /// Shared directory
    pub const SHARED_DIR: &str = "shared";

    /// Current directory symlink
    pub const CURRENT_DIR: &str = "current";

    /// Lock file for deployments
    pub const LOCK_FILE: &str = "deploy.lock";

    /// Environment file
    pub const DOT_ENV_FILE: &str = ".env";
}

/// Context keys for deployment and execution
pub mod context_keys {
    /// Deployment path
    pub const K_DEPLOY_PATH: &str = "deploy_path";

    /// Releases path
    pub const K_RELEASES_PATH: &str = "releases_path";

    /// Release path
    pub const K_RELEASE_PATH: &str = "release_path";

    /// Previous release
    pub const K_PREVIOUS_RELEASE: &str = "previous_release";

    /// Keep releases count
    pub const K_KEEP_RELEASES: &str = "keep_releases";

    /// Repository URL
    pub const K_REPOSITORY: &str = "repository";

    /// Branch name
    pub const K_BRANCH: &str = "branch";

    /// Local source path
    pub const K_LOCAL_SOURCE: &str = "local_source";

    /// Shared directories
    pub const K_SHARED_DIRS: &str = "shared_dirs";

    /// Shared files
    pub const K_SHARED_FILES: &str = "shared_files";

    /// Environment template
    pub const K_ENV_TEMPLATE: &str = "env_template";

    /// Current path
    pub const K_CURRENT_PATH: &str = "current_path";
}

/// GitHub repository configuration for self-update
pub mod github {
    use std::env;

    /// GitHub repository owner (default: from APPZ_GITHUB_REPO env or fallback)
    pub fn repo_owner() -> String {
        if let Ok(repo) = env::var("APPZ_GITHUB_REPO") {
            if let Some(owner) = repo.split('/').next() {
                return owner.to_string();
            }
        }
        // Default fallback - should be configured per project
        "yourusername".to_string()
    }

    /// GitHub repository name (default: from APPZ_GITHUB_REPO env or fallback)
    pub fn repo_name() -> String {
        if let Ok(repo) = env::var("APPZ_GITHUB_REPO") {
            if let Some((_, name)) = repo.split_once('/') {
                return name.to_string();
            }
        }
        // Default fallback - should be configured per project
        "appz".to_string()
    }
}

// Re-export for convenience
pub use context_keys::*;
pub use deploy::*;
