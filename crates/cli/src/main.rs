use app::{AppzSession, Cli, Commands, UserCancellation};
use clap::Parser;
use env_var::GlobalEnvBag;
use mimalloc::MiMalloc;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Wraps a future to assert Send. Used when the compiler cannot prove Send due to
/// higher-rank trait bound issues with &str/&Path in clap-derived types, but our
/// CLI args are owned (parsed from env::args()) so the future is safe to send.
struct AssertSend<F>(F);

impl<F: Future> Future for AssertSend<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut self.get_unchecked_mut().0).poll(cx) }
    }
}
// SAFETY: Our Commands/Cli are parsed from env::args() and all values are owned
// (String, PathBuf). The compiler fails HRTB for &str/&Path but at runtime we have
// no references. With current_thread runtime the future stays on main thread.
unsafe impl<F> Send for AssertSend<F> {}

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
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
    let mut cli = match Cli::try_parse() {
        Ok(c) => {
            timing.checkpoint("Cli::try_parse");
            c
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
    // Extract command before run to avoid capturing &str/&Path in Send future
    timing.checkpoint("pre app.run");
    let telemetry_store = std::sync::Arc::new(app::TelemetryEventStore::new());
    let command = std::mem::replace(&mut cli.command, Commands::List);
    let run_result = app
        .run(AppzSession::new(cli, telemetry_store.clone()), move |session| {
            AssertSend(async move {
            let cmd_name = app::command_name_for_telemetry(command.clone());
            app::record_command(session.telemetry_store.clone(), cmd_name).await;

            match command {
                Commands::List => app::commands::list(session).await,
                Commands::Plan(args) => app::commands::plan(session, args.task).await,
                Commands::Run(args) => app::commands::run(session, args.task, args.force, args.changed).await,
                Commands::RecipeValidate(args) => {
                    app::commands::recipe_validate(session, args.path).await
                }
                Commands::Dev(args) => app::commands::dev(session, args).await,
                #[cfg(feature = "dev-server")]
                Commands::DevServer(args) => app::commands::dev_server(session, args).await,
                Commands::Build => app::commands::build(session).await,
                #[cfg(feature = "dev-server")]
                Commands::Preview(args) => app::commands::preview(session, args).await,
                Commands::Ls(args) => app::commands::ls(session, args.policy).await,
                Commands::Open => app::commands::open(session).await,
                Commands::Link(args) => {
                    app::commands::link(session, args.project, args.team).await
                }
                Commands::Unlink => app::commands::unlink(session).await,
                Commands::Login => app::commands::login(session).await,
                Commands::Logout => app::commands::logout(session).await,
                Commands::Whoami(args) => {
                    let as_json = args.json || args.format.as_deref() == Some("json");
                    app::commands::whoami(session, as_json).await
                }
                Commands::Init(args) => {
                    app::commands::init(session, args.template_or_name, args.name, args.template, args.skip_install, args.force, args.output).await
                }
                Commands::Switch(args) => {
                    // Backward compatibility: route to teams switch
                    app::commands::teams::switch(session, args.team).await
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
                Commands::Pull(args) => {
                    app::commands::pull(session, args.environment, args.yes).await
                }
                Commands::Logs(args) => {
                    app::commands::logs(session, args.deployment).await
                }
                Commands::Inspect(args) => {
                    app::commands::inspect(session, args.deployment, args.json).await
                }
                Commands::Env { command } => {
                    app::commands::env::run(session, command).await
                }
                Commands::Promote(args) => {
                    app::commands::promote(session, args.deployment, args.timeout, args.yes).await
                }
                Commands::Rollback(args) => {
                    app::commands::rollback(session, args.deployment, args.timeout, args.yes).await
                }
                Commands::Remove(args) => {
                    app::commands::remove(session, args.resources, args.yes, args.safe).await
                }
                #[cfg(feature = "gen")]
                Commands::Gen(args) => {
                    app::commands::gen::run(session, args.prompt, args.output, args.name, args.model).await
                }
                #[cfg(feature = "deploy")]
                Commands::Deploy(args) => {
                    app::commands::deploy(
                        session,
                        args.project_path,
                        args.provider,
                        args.prod,
                        args.preview,
                        args.target,
                        args.prebuilt,
                        args.no_build,
                        args.build_env,
                        args.env,
                        args.force,
                        args.guidance,
                        args.logs,
                        args.meta,
                        args.no_wait,
                        args.public,
                        args.skip_domain,
                        args.with_cache,
                        args.dry_run,
                        args.json,
                        args.all,
                        args.yes,
                    )
                    .await
                }
                #[cfg(feature = "deploy")]
                Commands::DeployInit(args) => {
                    app::commands::deploy_init(session, args.provider).await
                }
                #[cfg(feature = "deploy")]
                Commands::DeployList(args) => {
                    app::commands::deploy_list(session, args.provider).await
                }
                Commands::Pack(args) => {
                    app::commands::pack::run(session, args.subcommand, args.run_opts).await
                }
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
                Commands::SelfUpdate(args) => {
                    use app::commands::SelfUpdate;
                    let cmd = SelfUpdate::new(args.version, args.force, args.yes);
                    cmd.run().await.map_err(|e| miette::miette!("{}", e))?;
                    Ok(None)
                }
                Commands::External(args) => {
                    app::commands::external::run(session, args).await
                }
            }
        })
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
