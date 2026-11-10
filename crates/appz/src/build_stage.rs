//! Library build stage for `deploy`: install + build through mise's cached
//! tasks, so an unchanged project is not reinstalled or rebuilt (the platform
//! CLIs' own builds start from scratch every time). Unlike the `appz build`
//! command helpers it never exits the process or draws spinners, so it is safe
//! from the MCP server.

use std::ffi::OsString;
use std::time::Duration;

use appz_deploy::DeployContext;

const STEP_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Install and build the project. Returns `Ok(false)` when the project has no
/// build command (nothing to do), `Ok(true)` once `ctx.output_dir` is built.
/// `-b` values reach the build; `node_modules/.bin` is first on `PATH`.
pub fn run(ctx: &DeployContext) -> Result<bool, String> {
    let root = ctx.project_dir.clone();
    let toolchains = appz_core::detect_toolchains(&root)?;
    if !toolchains.iter().any(|tc| tc.build_command.is_some()) {
        return Ok(false);
    }
    appz_core::write_state(&root, &toolchains)?;
    appz_core::ensure_mise(&root)?;
    if let Err(e) = appz_core::trust(&root) {
        eprintln!("warning: mise trust: {e}");
    }

    // A context for the build steps only: build env and node bins on top of the
    // provider's, without leaking either into the platform CLI's run.
    let mut step_ctx = DeployContext::new(root.clone(), ctx.output_dir.clone());
    step_ctx.json_output = ctx.json_output;
    step_ctx.extra_path = ctx.extra_path.clone();
    step_ctx.run_env = ctx.run_env.clone();
    step_ctx.run_env.extend(ctx.build_env.clone());
    let base: OsString = ctx
        .run_env
        .get("PATH")
        .map(OsString::from)
        .or_else(|| std::env::var_os("PATH"))
        .unwrap_or_default();
    let path = std::env::join_paths(
        appz_core::find_node_modules_bin_paths(&root)
            .into_iter()
            .chain(std::env::split_paths(&base)),
    )
    .map_err(|e| format!("cannot build PATH: {e}"))?;
    step_ctx
        .run_env
        .insert("PATH".into(), path.to_string_lossy().into_owned());

    let mut steps: Vec<&[&str]> = vec![&["install"]];
    if toolchains.iter().any(|tc| tc.install_command.is_some()) {
        steps.push(&["run", "install"]);
    }
    steps.push(&["run", "build"]);
    for args in steps {
        let out = step_ctx.exec_live("mise", args, STEP_TIMEOUT)?;
        if !out.success {
            return Err(format!(
                "mise {} failed:\n{}\n{}",
                args.join(" "),
                out.stdout,
                out.stderr
            ));
        }
    }
    if !ctx.output_path().is_dir() {
        return Err(format!(
            "build finished but '{}' was not produced",
            ctx.output_dir
        ));
    }
    Ok(true)
}
