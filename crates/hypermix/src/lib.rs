//! Multi-source AI context builder via Repomix orchestration.
//!
//! Loads pack.config.json or hypermix.config.json, runs Repomix per mix
//! (remote or local), updates ignore files, and reports token counts.

mod add;
mod config;
mod ignore;
mod init;
mod repomix;
mod run;
mod tokens;
mod types;
mod uninstall;

pub use add::{add_repo, validate_github_repo};
pub use config::load_config;
pub use ignore::update_ignore_files;
pub use repomix::{run_mix, RepomixResult};
pub use init::init;
pub use run::run as run_config;
pub use tokens::{count_tokens, count_tokens_in_files};
pub use types::{HypermixConfig, MixConfig};
pub use uninstall::uninstall;
