//! appz exec — Execute commands with sandbox-by-default.

use crate::exec::{run_exec, ExecConfig};
use crate::session::AppzSession;
use starbase::AppResult;

pub async fn exec(session: AppzSession, args: crate::args::ExecArgs) -> AppResult {
    let cwd = args
        .cwd
        .unwrap_or_else(|| session.working_dir.clone())
        .canonicalize()
        .map_err(|e| miette::miette!("Invalid cwd: {}", e))?;

    let config = ExecConfig {
        command: args.command,
        args: args.args,
        cwd,
        sandbox: !args.no_sandbox,
        stream: args.stream,
        json: args.json,
        shell: args.shell,
        timeout: args.timeout.map(std::time::Duration::from_secs),
    };

    let result = run_exec(config).await?;

    if args.json {
        println!("{}", serde_json::to_string(&result).unwrap());
    } else {
        if !result.stdout.is_empty() {
            print!("{}", result.stdout);
        }
        if !result.stderr.is_empty() {
            eprint!("{}", result.stderr);
        }
    }

    Ok(if result.exit_code == 0 {
        None
    } else {
        Some(result.exit_code.clamp(0, 255) as u8)
    })
}
