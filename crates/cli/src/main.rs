mod shared;

use mimalloc::MiMalloc;
use starbase::MainResult;
use std::env;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main(flavor = "current_thread")]
async fn main() -> MainResult {
    let mut timing = common::timing::TimingDebug::new();

    // Apply security hardening before processing any input.
    common::hardening::harden_process();
    timing.checkpoint("harden_process");

    // Detect info about the current process
    let version = get_version();
    timing.checkpoint("get_version (incl. GlobalEnvBag)");

    // Try to parse CLI args - handle gracefully if no command provided
    let cli = match Cli::try_parse() {
        Ok(cli) => {
            timing.checkpoint("Cli::try_parse");
            cli
        }
        Err(e) => {
            timing.checkpoint("Cli::try_parse");
            // Check if it's a help or version request
            let is_help = e.kind() == clap::error::ErrorKind::DisplayHelp
                || e.kind() == clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand;
            let is_version = e.kind() == clap::error::ErrorKind::DisplayVersion;

            // Show banner before help/version if appropriate
            if banner::should_display() && (is_help || is_version) {
                timing.checkpoint("banner::should_display");
                let _ = banner::display("appz", &version, Some("Orchestration & plugin CLI for web apps"));
                timing.checkpoint("banner::display");
            }

            // Print the error (help or version)
            if let Err(io_err) = e.print() {
                eprintln!("Failed to print help: {}", io_err);
            }
            timing.checkpoint("e.print (help/version)");
            timing.print();

            // Exit with success for help/version, error for other cases
            return Ok(if is_help || is_version {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            });
        }
    };

    cli.setup_env_vars();
    timing.checkpoint("setup_env_vars");

    // Setup diagnostics and tracing
    let app = App::default();
    app.setup_diagnostics();
    timing.checkpoint("App::default + setup_diagnostics");

    let _guard = app.setup_tracing(TracingOptions {
        dump_trace: cli.dump,
        filter_modules: get_tracing_modules(),
        log_env: "STARBASE_LOG".into(),
        log_file: cli.log_file.clone(),
        show_spans: cli.log.map(|l| l.is_verbose()).unwrap_or(false),
        ..TracingOptions::default()
    });
    timing.checkpoint("setup_tracing");

    if let Ok(exe) = env::current_exe() {
        debug!("Running appz v{} (with {:?})", version, exe,);
    } else {
        debug!("Running appz v{}", version);
    }

    // Display compact banner if appropriate (only for actual commands)
    if banner::should_display() {
        let _ = banner::display("appz", &version, Some("Orchestration & plugin CLI for web apps"));
    }
    timing.checkpoint("banner (success path)");

    // Run the CLI with starbase session lifecycle
    timing.checkpoint("pre app.run");
    let telemetry_store = std::sync::Arc::new(app::TelemetryEventStore::new());
    let run_result = app
        .run(AppzSession::new(cli, telemetry_store.clone()), |session| async {
            app::record_command(&session.telemetry_store, &session.cli.command).await;
            match session.cli.command.clone() {
                Commands::List => app::commands::list(session).await,
                Commands::Plan { task } => app::commands::plan(session, task).await,
                Commands::Run { task, force, changed } => app::commands::run(session, task, force, changed).await,
                Commands::RecipeValidate { path } => {
                    app::commands::recipe_validate(session, path).await
                }
                Commands::Dev { .. } => app::commands::dev(session).await,
                #[cfg(feature = "dev-server")]
                Commands::DevServer { .. } => app::commands::dev_server(session).await,
                Commands::Build => app::commands::build(session).await,
                #[cfg(feature = "dev-server")]
                Commands::Preview { .. } => app::commands::preview(session).await,
                Commands::Seo { command } => {
                    app::commands::seo::run(session, command).await
                }
                Commands::Ls(args) => app::commands::ls(session, args).await,
                Commands::Open => app::commands::open(session).await,
                Commands::Link { project, team } => {
                    app::commands::link(session, project, team).await
                }
                Commands::Unlink => app::commands::unlink(session).await,
                Commands::Login => app::commands::login(session).await,
                Commands::Logout => app::commands::logout(session).await,
                Commands::Whoami { json, format } => {
                    let as_json = json || format.as_deref() == Some("json");
                    app::commands::whoami(session, as_json).await
                }
                Commands::Init(args) => {
                    app::commands::init(
                        session,
                        args.template_or_name.clone(),
                        args.name.clone(),
                        args.template.clone(),
                        args.skip_install,
                        args.force,
                        args.output.clone(),
                    )
                    .await
                }
                Commands::Switch { team } => {
                    // Backward compatibility: route to teams switch
                    app::commands::teams::switch(session, team).await
                }
                Commands::Teams { command } => {
                    app::commands::teams::run(session, command).await
                }
                Commands::Telemetry { command } => {
                    app::commands::telemetry::run(command).await
                }
                Commands::Projects { command } => {
                    let cmd = command.unwrap_or(app::commands::projects::ProjectsCommands::Ls);
                    app::commands::projects::run(session, cmd).await
                }
                Commands::Transfer { command, project, to_team } => {
                    app::commands::transfer::run(session, command, project, to_team).await
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
                #[cfg(feature = "gen")]
                Commands::Gen { prompt, output, name, model } => {
                    app::commands::gen::run(session, prompt, output, name, model).await
                }
                #[cfg(feature = "deploy")]
                Commands::Deploy { provider, preview, no_build, dry_run, json, all, yes } => {
                    app::commands::deploy(session, provider, preview, no_build, dry_run, json, all, yes).await
                }
                #[cfg(feature = "deploy")]
                Commands::DeployInit { provider } => {
                    app::commands::deploy_init(session, provider).await
                }
                #[cfg(feature = "deploy")]
                Commands::DeployList { provider } => {
                    app::commands::deploy_list(session, provider).await
                }
                // Check and site commands are now downloadable plugins (handled by External)
                Commands::Code { command } => {
                    app::commands::code::run(session, command).await
                }
                Commands::Skills { command } => {
                    app::commands::skills::run(session, command).await
                }
                Commands::Plugin { command } => {
                    app::commands::plugin::run(session, command).await
                }
                #[cfg(feature = "mcp")]
                Commands::McpServer => app::commands::mcp_server::mcp_server(session).await,
                // NOTE: Convert and Migrate commands are now downloadable plugins.
                // It is handled by Commands::External below.
                #[cfg(feature = "self_update")]
                Commands::SelfUpdate { version, force, yes } => {
                    use app::commands::SelfUpdate;
                    let cmd = SelfUpdate::new(version, force, yes);
                    cmd.run().await.map_err(|e| miette::miette!("{}", e))?;
                    Ok(None)
                }
                Commands::External(args) => {
                    app::commands::external::run(session, args).await
                }
            }
        })
        .await;

    timing.checkpoint("app.run (session + command)");
    timing.print();

    let exit_code = match run_result {
        Ok(code) => {
            telemetry_store.set_run_outcome(code == 0);
            telemetry_store.flush().await;
            Ok(ExitCode::from(code))
        }
        Err(e) => {
            if e.downcast_ref::<UserCancellation>().is_some() {
                let msg = e.to_string();
                let _ = ui::status::info(&msg);
                telemetry_store.set_run_outcome(true);
                telemetry_store.flush().await;
                Ok(ExitCode::SUCCESS)
            } else {
                telemetry_store.set_run_outcome(false);
                telemetry_store.flush().await;
                Err(e.into())
            }
        }
    };

    exit_code
}
