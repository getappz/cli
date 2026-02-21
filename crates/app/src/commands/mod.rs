pub mod aliases;
pub mod build;
mod install_helpers;
#[cfg(feature = "check")]
pub mod check;
pub mod code;
pub mod pack;
#[cfg(feature = "deploy")]
pub mod deploy;
pub mod deployment_utils;
pub mod dev;
#[cfg(feature = "dev-server")]
pub mod dev_server;
pub mod domains;
pub mod env;
pub mod exec;
pub mod external;
pub mod git;
pub mod inspect;
pub mod logs;
pub mod pull;
#[cfg(feature = "gen")]
pub mod gen;
pub mod init;
pub mod link;
pub mod list;
pub mod login;
pub mod open;
pub mod logout;
pub mod ls;
#[cfg(feature = "mcp")]
pub mod mcp_server;
pub mod migrate;
pub mod plan;
pub mod plugin;
#[cfg(feature = "dev-server")]
pub mod preview;
pub mod projects;
pub mod promote;
pub mod recipe_validate;
pub mod remove;
pub mod rollback;
pub mod run;
#[cfg_attr(not(feature = "self_update"), path = "self_upgrade_stub.rs")]
pub mod self_upgrade;
#[cfg(feature = "site")]
pub mod site;
pub mod skills;
pub mod switch;
pub mod teams;
pub mod telemetry;
pub mod transfer;
pub mod unlink;
pub mod version;
pub mod whoami;

pub use aliases::*;
pub use build::build;
#[cfg(feature = "check")]
pub use check::check;
#[cfg(feature = "deploy")]
pub use deploy::{deploy, deploy_init, deploy_list};
pub use dev::dev;
#[cfg(feature = "dev-server")]
pub use dev_server::dev_server;
pub use domains::*;
pub use env::{run as env_run, EnvCommands};
pub use exec::exec;
pub use inspect::inspect;
pub use logs::logs;
pub use pull::pull;
pub use init::init;
pub use link::link;
pub use list::list;
pub use login::login;
pub use open::open;
pub use logout::logout;
pub use ls::ls;
// migrate is now a downloadable plugin; no public exports needed
pub use plan::plan;
#[cfg(feature = "dev-server")]
pub use preview::preview;
pub use projects::{resolve_project_id, run as projects_run, ProjectsCommands};
pub use promote::{promote, status as promote_status};
pub use recipe_validate::recipe_validate;
pub use remove::remove;
pub use rollback::{rollback, status as rollback_status};
pub use run::run;
#[cfg(not(feature = "self_update"))]
pub use self_upgrade::{append_self_update_instructions, upgrade_instructions_text};
#[cfg(feature = "self_update")]
pub use self_upgrade::{append_self_update_instructions, upgrade_instructions_text, SelfUpdate};
pub use git::{run as git_run, GitCommands};
pub use plugin::PluginCommands;
pub use skills_lib::SkillsCommands;
pub use switch::switch;
pub use teams::{resolve_team_id, run as teams_run, TeamsCommands};
pub use telemetry::{run as telemetry_run, TelemetryCommands};
pub use transfer::{run as transfer_run, TransferCommands};
pub use unlink::unlink;
pub use version::{ARCH, OS};
pub use whoami::whoami;
