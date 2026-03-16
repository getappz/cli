//! CLI run flow — Moon-aligned structure.
//!
//! Thin main calls `run_cli`; session gets full cli with real command for
//! analyze/bootstrap. Command is extracted for dispatch to satisfy Send bound
//! (clap-derived types may contain &str/&Path that compiler cannot prove Send).

use app::{AppzSession, Cli, Commands, UserCancellation};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Asserts that a future is Send. Clap-derived Commands can contain references
/// that block Send inference, but our args are parsed from env and are owned.
unsafe impl<F> Send for AssertSend<F> {}
struct AssertSend<F>(F);
impl<F: Future> Future for AssertSend<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut self.get_unchecked_mut().0).poll(cx) }
    }
}
use clap::Parser;
use env_var::GlobalEnvBag;
use starbase::tracing::TracingOptions;
use starbase::{App, MainResult};
use std::env;
use std::ffi::OsString;
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

pub async fn run_cli(args: Vec<OsString>) -> MainResult {
    let mut timing = common::timing::TimingDebug::new();

    common::hardening::harden_process();
    timing.checkpoint("harden_process");

    let version = get_version();
    timing.checkpoint("get_version (incl. GlobalEnvBag)");

    let mut cli = match Cli::try_parse_from(args.iter().map(OsString::as_os_str)) {
        Ok(c) => {
            timing.checkpoint("Cli::try_parse");
            c
        }
        Err(e) => {
            timing.checkpoint("Cli::try_parse");
            let is_help = e.kind() == clap::error::ErrorKind::DisplayHelp
                || e.kind() == clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand;
            let is_version = e.kind() == clap::error::ErrorKind::DisplayVersion;

            if banner::should_display() && (is_help || is_version) {
                let _ = banner::display(
                    "appz",
                    &version,
                    Some("Orchestration & plugin CLI for web apps"),
                );
            }
            if let Err(io_err) = e.print() {
                eprintln!("Failed to print help: {}", io_err);
            }
            timing.print();
            return Ok(if is_help || is_version {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            });
        }
    };

    cli.setup_env_vars();
    timing.checkpoint("setup_env_vars");

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
    timing.checkpoint("setup_tracing");

    if let Ok(exe) = env::current_exe() {
        debug!("Running appz v{} (with {:?})", version, exe);
    } else {
        debug!("Running appz v{}", version);
    }

    if banner::should_display() {
        let _ = banner::display(
            "appz",
            &version,
            Some("Orchestration & plugin CLI for web apps"),
        );
    }
    timing.checkpoint("banner (success path)");

    let telemetry_store = std::sync::Arc::new(app::TelemetryEventStore::new());

    // Extract command for closure (satisfies Send); restore so session.analyze() sees real command
    let command = std::mem::replace(&mut cli.command, Commands::Build);
    cli.command = command.clone();

    let run_result = app
        .run(
            AppzSession::new(cli, telemetry_store.clone()),
            move |session| {
                AssertSend(async move {
                    let cmd_name = app::command_name_for_telemetry(command.clone());
                    app::record_command(session.telemetry_store.clone(), cmd_name).await;

                    match command {
                    Commands::Dev(args) => app::commands::dev(session, args).await,
                    Commands::Build => app::commands::build(session).await,
                    #[cfg(feature = "dev-server")]
                    Commands::Preview(args) => {
                        app::commands::preview(session, args).await
                    }
                    Commands::Ls(args) => app::commands::ls(session, args).await,
                    #[cfg(feature = "appz-cloud")]
                    Commands::Open => app::commands::open(session).await,
                    #[cfg(feature = "appz-cloud")]
                    Commands::Link(args) => {
                        app::commands::link(session, args.project, args.team).await
                    }
                    #[cfg(feature = "appz-cloud")]
                    Commands::Unlink => app::commands::unlink(session).await,
                    #[cfg(feature = "appz-cloud")]
                    Commands::Login => app::commands::login(session).await,
                    #[cfg(feature = "appz-cloud")]
                    Commands::Logout => app::commands::logout(session).await,
                    #[cfg(feature = "appz-cloud")]
                    Commands::Whoami(args) => {
                        let as_json =
                            args.json || args.format.as_deref() == Some("json");
                        app::commands::whoami(session, as_json).await
                    }
                    Commands::Init(args) => {
                        app::commands::init(
                            session,
                            args.template_or_name,
                            args.name,
                            args.template,
                            args.skip_install,
                            args.force,
                            args.output,
                            args.blueprint,
                            args.playground,
                        )
                        .await
                    }
                    Commands::Blueprint { command } => {
                        app::commands::blueprint::run(session, command).await
                    }
                    #[cfg(feature = "appz-cloud")]
                    Commands::Switch(args) => {
                        app::commands::teams::switch(session, args.team).await
                    }
                    #[cfg(feature = "appz-cloud")]
                    Commands::Teams { command } => {
                        app::commands::teams::run(session, command).await
                    }
                    Commands::Telemetry { command } => {
                        app::commands::telemetry::run(command).await
                    }
                    #[cfg(feature = "appz-cloud")]
                    Commands::Projects { command } => {
                        let cmd = command
                            .unwrap_or(app::commands::projects::ProjectsCommands::Ls);
                        app::commands::projects::run(session, cmd).await
                    }
                    #[cfg(feature = "appz-cloud")]
                    Commands::Transfer {
                        command,
                        project,
                        to_team,
                    } => {
                        app::commands::transfer::run(
                            session,
                            command,
                            project,
                            to_team,
                        )
                        .await
                    }
                    #[cfg(feature = "appz-cloud")]
                    Commands::Aliases { command } => {
                        app::commands::aliases::run(session, command).await
                    }
                    #[cfg(feature = "appz-cloud")]
                    Commands::Domains { command } => {
                        app::commands::domains::run(session, command).await
                    }
                    #[cfg(feature = "appz-cloud")]
                    Commands::Pull(args) => {
                        app::commands::pull(session, args.environment, args.yes)
                            .await
                    }
                    #[cfg(feature = "appz-cloud")]
                    Commands::Logs(args) => {
                        app::commands::logs(session, args.deployment).await
                    }
                    #[cfg(feature = "appz-cloud")]
                    Commands::Inspect(args) => {
                        app::commands::inspect(
                            session,
                            args.deployment,
                            args.json,
                        )
                        .await
                    }
                    #[cfg(feature = "appz-cloud")]
                    Commands::Env { command } => {
                        app::commands::env::run(session, command).await
                    }
                    #[cfg(feature = "appz-cloud")]
                    Commands::Promote(args) => {
                        app::commands::promote(
                            session,
                            args.deployment,
                            args.timeout,
                            args.yes,
                        )
                        .await
                    }
                    #[cfg(feature = "appz-cloud")]
                    Commands::Rollback(args) => {
                        app::commands::rollback(
                            session,
                            args.deployment,
                            args.timeout,
                            args.yes,
                        )
                        .await
                    }
                    #[cfg(feature = "appz-cloud")]
                    Commands::Remove(args) => {
                        app::commands::remove(
                            session,
                            args.resources,
                            args.yes,
                            args.safe,
                        )
                        .await
                    }
                    Commands::WpExport(args) => {
                        app::commands::wp_export(session, args).await
                    }
                    #[cfg(feature = "deploy")]
                    Commands::Deploy(args) => {
                        if args.init {
                            app::commands::deploy_init(session, args.platform).await
                        } else {
                            app::commands::deploy(
                                session,
                                args.project_path,
                                args.platform,
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
                    }
                    #[cfg(feature = "self_update")]
                    Commands::SelfUpdate(args) => {
                        use app::commands::SelfUpdate;
                        let cmd =
                            SelfUpdate::new(args.version, args.force, args.yes);
                        cmd.run()
                            .await
                            .map_err(|e| miette::miette!("{}", e))?;
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
