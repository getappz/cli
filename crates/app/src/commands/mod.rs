#[cfg(feature = "appz-cloud")]
pub mod aliases;
pub mod build;
mod install_helpers;
#[cfg(feature = "check")]
pub mod check;
#[cfg(feature = "deploy")]
pub mod deploy;
pub mod deployment_utils;
pub mod dev;
#[cfg(feature = "dev-server")]
pub mod dev_server;
#[cfg(feature = "appz-cloud")]
pub mod domains;
#[cfg(feature = "appz-cloud")]
pub mod env;
pub mod external;
#[cfg(feature = "appz-cloud")]
pub mod inspect;
#[cfg(feature = "appz-cloud")]
pub mod logs;
#[cfg(feature = "appz-cloud")]
pub mod pull;
pub mod init;
pub mod link;
pub mod list;
pub mod login;
#[cfg(feature = "appz-cloud")]
pub mod open;
pub mod logout;
#[cfg(feature = "appz-cloud")]
pub mod ls;
pub mod migrate;
pub mod plan;
#[cfg(feature = "dev-server")]
pub mod preview;
pub mod projects;
#[cfg(feature = "appz-cloud")]
pub mod promote;
pub mod recipe_validate;
#[cfg(feature = "appz-cloud")]
pub mod remove;
#[cfg(feature = "appz-cloud")]
pub mod rollback;
pub mod run;
#[cfg_attr(not(feature = "self_update"), path = "self_upgrade_stub.rs")]
pub mod self_upgrade;
#[cfg(feature = "appz-cloud")]
pub mod switch;
pub mod teams;
#[cfg(feature = "appz-cloud")]
pub mod telemetry;
#[cfg(feature = "appz-cloud")]
pub mod transfer;
pub mod unlink;
pub mod version;
pub mod whoami;

#[cfg(feature = "appz-cloud")]
pub use aliases::*;
pub use build::build;
#[cfg(feature = "check")]
pub use check::check;
#[cfg(feature = "deploy")]
pub use deploy::{deploy, deploy_init, deploy_list};
pub use dev::dev;
#[cfg(feature = "dev-server")]
pub use dev_server::dev_server;
#[cfg(feature = "appz-cloud")]
pub use domains::*;
#[cfg(feature = "appz-cloud")]
pub use env::{run as env_run, EnvCommands};
#[cfg(feature = "appz-cloud")]
pub use inspect::inspect;
#[cfg(feature = "appz-cloud")]
pub use logs::logs;
#[cfg(feature = "appz-cloud")]
pub use pull::pull;
pub use init::init;
pub use link::link;
pub use list::list;
pub use login::login;
#[cfg(feature = "appz-cloud")]
pub use open::open;
pub use logout::logout;
#[cfg(feature = "appz-cloud")]
pub use ls::ls;
// migrate is now a downloadable plugin; no public exports needed
pub use plan::plan;
#[cfg(feature = "dev-server")]
pub use preview::preview;
pub use projects::resolve_project_id;
#[cfg(feature = "appz-cloud")]
pub use projects::{run as projects_run, ProjectsCommands};
#[cfg(feature = "appz-cloud")]
pub use promote::{promote, status as promote_status};
pub use recipe_validate::recipe_validate;
#[cfg(feature = "appz-cloud")]
pub use remove::remove;
#[cfg(feature = "appz-cloud")]
pub use rollback::{rollback, status as rollback_status};
pub use run::run;
#[cfg(not(feature = "self_update"))]
pub use self_upgrade::{append_self_update_instructions, upgrade_instructions_text};
#[cfg(feature = "self_update")]
pub use self_upgrade::{append_self_update_instructions, upgrade_instructions_text, SelfUpdate};
#[cfg(feature = "appz-cloud")]
pub use switch::switch;
pub use teams::resolve_team_id;
#[cfg(feature = "appz-cloud")]
pub use teams::{run as teams_run, TeamsCommands};
#[cfg(feature = "appz-cloud")]
pub use telemetry::{run as telemetry_run, TelemetryCommands};
#[cfg(feature = "appz-cloud")]
pub use transfer::{run as transfer_run, TransferCommands};
pub use unlink::unlink;
pub use version::{ARCH, OS};
pub use whoami::whoami;
