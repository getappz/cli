//! Self-upgrade implementation using self_update crate.
//!
//! Updates appz itself by downloading the latest release from GitHub.
//! Based on mise's self-update implementation.

use miette::{miette, Result};
use self_update::Status;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::version::{ARCH, OS};
use crate::systems::version_check;

#[derive(Debug, Default, serde::Deserialize)]
struct InstructionsToml {
    message: Option<String>,
    #[serde(flatten)]
    commands: BTreeMap<String, String>,
}

fn read_instructions_file(path: &PathBuf) -> Option<String> {
    let parsed: InstructionsToml = starbase_utils::json::read_file(path).ok()?;
    if let Some(msg) = parsed.message {
        return Some(msg);
    }
    if let Some((_k, v)) = parsed.commands.into_iter().next() {
        return Some(v);
    }
    None
}

pub fn upgrade_instructions_text() -> Option<String> {
    if let Ok(path) = std::env::var("APPZ_SELF_UPDATE_INSTRUCTIONS") {
        let path = PathBuf::from(path);
        if let Some(msg) = read_instructions_file(&path) {
            return Some(msg);
        }
    }
    None
}

/// Appends self-update guidance and packaging instructions (if any) to a message.
pub fn append_self_update_instructions(mut message: String) -> String {
    if SelfUpdate::is_available() {
        message.push_str("\nRun `appz self-update` to update appz");
    }
    if let Some(instructions) = upgrade_instructions_text() {
        message.push('\n');
        message.push_str(&instructions);
    }
    message
}

/// Updates appz itself.
///
/// Uses the GitHub Releases API to find the latest release and binary.
/// Uses the `GITHUB_TOKEN` environment variable if set for higher rate limits.
///
/// This command is not available if appz is installed via a package manager.
#[derive(Debug, Default, clap::Args)]
#[clap(verbatim_doc_comment)]
pub struct SelfUpdate {
    /// Update to a specific version
    version: Option<String>,

    /// Update even if already up to date
    #[clap(long, short)]
    force: bool,

    /// Skip confirmation prompt
    #[clap(long, short)]
    yes: bool,
}

impl SelfUpdate {
    pub fn new(version: Option<String>, force: bool, yes: bool) -> Self {
        Self {
            version,
            force,
            yes,
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn run(self) -> Result<()> {
        if !Self::is_available() && !self.force {
            if let Some(instructions) = upgrade_instructions_text() {
                warn!("{}", instructions);
            }
            return Err(miette!(
                "appz is installed via a package manager, cannot update"
            ));
        }
        let status = self.do_update().await?;

        if status.updated() {
            let version = status.version();
            println!("✓ Updated appz to {}", version);
        } else {
            println!("✓ appz is already up to date");
        }

        Ok(())
    }

    async fn do_update(&self) -> Result<Status> {
        // Use shared configuration helper
        let mut update = version_check::configure_update_builder();

        #[cfg(windows)]
        let bin_path_in_archive = "appz/bin/appz.exe";
        #[cfg(not(windows))]
        let bin_path_in_archive = "appz/bin/appz";
        update.bin_path_in_archive(bin_path_in_archive);

        let v = if let Some(ref version) = self.version {
            format!("v{}", version)
        } else {
            // Use cached version check if not forcing
            if !self.force {
                if let Ok(Some(cached_version)) =
                    version_check::get_latest_version_cached(version_check::CACHE_DURATION_DAILY)
                        .await
                {
                    format!("v{}", cached_version)
                } else {
                    // Fallback to API call - need to fetch version
                    // We can't reuse the update builder here since it's moved, so fetch separately
                    let latest_version = version_check::get_latest_version_from_api()
                        .await?
                        .ok_or_else(|| miette!("Failed to fetch latest version from API"))?;
                    format!("v{}", latest_version)
                }
            } else {
                // Force update, always fetch from API
                let latest_version = version_check::get_latest_version_from_api()
                    .await?
                    .ok_or_else(|| miette!("Failed to fetch latest version from API"))?;
                format!("v{}", latest_version)
            }
        };
        let target = format!("{}-{}", *OS, *ARCH);
        #[cfg(target_env = "musl")]
        let target = format!("{target}-musl");
        if self.force || self.version.is_some() {
            update.target_version_tag(&v);
        }
        #[cfg(windows)]
        let target = format!("appz-{v}-{target}.zip");
        #[cfg(not(windows))]
        let target = format!("appz-{v}-{target}.tar.gz");

        // Verify signatures using public key (matches mise's pattern)
        // Using include_bytes! for compile-time inclusion of zipsign.pub
        // Path: ../../../../zipsign.pub from crates/app/src/commands/self_upgrade.rs to workspace root
        // Path calculation: crates/app/src/commands/ -> ../../../../ -> workspace root
        let status = update
            .verifying_keys([*include_bytes!("../../../../zipsign.pub")])
            .show_download_progress(true)
            .target(&target)
            .no_confirm(self.yes)
            .build()
            .map_err(|e| miette!("Failed to build update: {}", e))?
            .update()
            .map_err(|e| miette!("Failed to update: {}", e))?;

        Ok(status)
    }

    pub fn is_available() -> bool {
        if let Ok(available) = std::env::var("APPZ_SELF_UPDATE_AVAILABLE") {
            if let Ok(b) = available.parse::<bool>() {
                return b;
            }
        }
        let has_disable = std::env::var("APPZ_SELF_UPDATE_DISABLED_PATH").is_ok()
            || std::path::PathBuf::from("/usr/lib/appz/.disable-self-update").exists()
            || std::path::PathBuf::from("/usr/lib64/appz/.disable-self-update").exists();
        let has_instructions = std::env::var("APPZ_SELF_UPDATE_INSTRUCTIONS").is_ok();
        !(has_disable || has_instructions)
    }
}
