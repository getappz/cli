use app::{AppzSession, Cli, Commands};
use clap::Parser;
use env_var::GlobalEnvBag;
use starbase::tracing::TracingOptions;
use starbase::{App, MainResult};
use std::env;
use std::process::ExitCode;
use tracing::debug;
use ui::banner;

fn get_version() -> String {
    let version = env!("CARGO_PKG_VERSION");

    GlobalEnvBag::instance().set("APPZ_VERSION", version);

    version.to_owned()
}

fn get_tracing_modules() -> Vec<String> {
    let bag = GlobalEnvBag::instance();
    let mut modules = vec![
        "appz".to_string(),
        "app".to_string(),
        "starbase".to_string(),
        "api".to_string(),
    ];

    if bag.should_debug_wasm() {
        modules.push("extism".to_string());
    }

    modules
}

#[tokio::main]
async fn main() -> MainResult {
    // Apply security hardening before processing any input.
    common::hardening::harden_process();

    // Detect info about the current process
    let version = get_version();

    // Try to parse CLI args - handle gracefully if no command provided
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // Check if it's a help or version request
            let is_help = e.kind() == clap::error::ErrorKind::DisplayHelp
                || e.kind() == clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand;
            let is_version = e.kind() == clap::error::ErrorKind::DisplayVersion;

            // Show banner before help/version if appropriate
            if banner::should_display() && (is_help || is_version) {
                let _ = banner::display("appz", &version, Some("Task Runner CLI"));
            }

            // Print the error (help or version)
            if let Err(io_err) = e.print() {
                eprintln!("Failed to print help: {}", io_err);
            }

            // Exit with success for help/version, error for other cases
            return Ok(if is_help || is_version {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            });
        }
    };

    cli.setup_env_vars();

    // Setup diagnostics and tracing
    let app = App::default();
    app.setup_diagnostics();

    let _guard = app.setup_tracing(TracingOptions {
        dump_trace: cli.dump,
        filter_modules: get_tracing_modules(),
        log_env: "STARBASE_LOG".into(),
        log_file: cli.log_file.clone(),
        show_spans: cli.log.map(|l| l.is_verbose()).unwrap_or(false),
        ..TracingOptions::default()
    });

    if let Ok(exe) = env::current_exe() {
        debug!("Running appz v{} (with {:?})", version, exe,);
    } else {
        debug!("Running appz v{}", version);
    }

    // Display compact banner if appropriate (only for actual commands)
    if banner::should_display() {
        let _ = banner::display("appz", &version, Some("Task Runner CLI"));
    }

    // Run the CLI with starbase session lifecycle
    let exit_code = app
        .run(AppzSession::new(cli), |session| async {
            match session.cli.command.clone() {
                Commands::List => app::commands::list(session).await,
                Commands::Plan { task } => app::commands::plan(session, task).await,
                Commands::Run { task, force, changed } => app::commands::run(session, task, force, changed).await,
                Commands::RecipeValidate { path } => {
                    app::commands::recipe_validate(session, path).await
                }
                Commands::Dev { .. } => app::commands::dev(session).await,
                Commands::DevServer { .. } => app::commands::dev_server(session).await,
                Commands::Build => app::commands::build(session).await,
                Commands::Preview { .. } => app::commands::preview(session).await,
                Commands::Ls => app::commands::ls(session).await,
                Commands::Link { project, team } => {
                    app::commands::link(session, project, team).await
                }
                Commands::Unlink => app::commands::unlink(session).await,
                Commands::Login => app::commands::login(session).await,
                Commands::Logout => app::commands::logout(session).await,
                Commands::Init { template_or_name, name, template, skip_install, force, output } => {
                    app::commands::init(session, template_or_name, name, template, skip_install, force, output).await
                }
                Commands::Switch { team } => {
                    // Backward compatibility: route to teams switch
                    app::commands::teams::switch(session, team).await
                }
                Commands::Teams { command } => {
                    app::commands::teams::run(session, command).await
                }
                Commands::Projects { command } => {
                    app::commands::projects::run(session, command).await
                }
                Commands::Aliases { command } => {
                    app::commands::aliases::run(session, command).await
                }
                Commands::Domains { command } => {
                    app::commands::domains::run(session, command).await
                }
                Commands::Promote { deployment, timeout, yes } => {
                    app::commands::promote(session, deployment, timeout, yes).await
                }
                Commands::Rollback { deployment, timeout, yes } => {
                    app::commands::rollback(session, deployment, timeout, yes).await
                }
                Commands::Remove { resources, yes, safe } => {
                    app::commands::remove(session, resources, yes, safe).await
                }
                Commands::Gen { prompt, output, name, model } => {
                    app::commands::gen::run(session, prompt, output, name, model).await
                }
                Commands::Deploy { provider, preview, no_build, dry_run, json, all, yes } => {
                    app::commands::deploy(session, provider, preview, no_build, dry_run, json, all, yes).await
                }
                Commands::DeployInit { provider } => {
                    app::commands::deploy_init(session, provider).await
                }
                Commands::DeployList { provider } => {
                    app::commands::deploy_list(session, provider).await
                }
                // Check and site commands are now downloadable plugins (handled by External)
                Commands::Skills { command } => {
                    app::commands::skills::run(session, command).await
                }
                // NOTE: Migrate command is now a downloadable plugin.
                // It is handled by Commands::External below.
                #[cfg(feature = "self_update")]
                Commands::SelfUpdate { version, force, yes } => {
                    use app::commands::SelfUpdate;
                    let cmd = SelfUpdate::new(version, force, yes);
                    cmd.run().await.map_err(|e| miette::miette!("{}", e))?;
                    Ok(None)
                }
                #[cfg(not(feature = "self_update"))]
                Commands::SelfUpdate { .. } => {
                    unreachable!("SelfUpdate command should not be available when self_update feature is disabled")
                }
                Commands::External(args) => {
                    app::commands::external::run(session, args).await
                }
            }
        })
        .await?;

    Ok(ExitCode::from(exit_code))
}
