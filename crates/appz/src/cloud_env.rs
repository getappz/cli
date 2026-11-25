//! appz.dev cloud environment variables: per-environment pull/ls/rm, custom
//! environments and `env run`. The local encrypted vault lives in `secrets`.

use std::path::Path;

use crate::{error, intro, is_tty, outro, success, ui};

/// Environment used when `--environment` is omitted, matching `vercel env pull`.
pub(crate) const DEFAULT_ENVIRONMENT: &str = "development";

/// Environments `env push` writes to by default: all three, so a plain push then pull round-trips.
pub(crate) fn default_push_environments() -> Vec<String> {
    ["production", "preview", "development"]
        .map(String::from)
        .to_vec()
}

pub(crate) struct CloudCtx {
    pub project_id: String,
    pub token: Option<String>,
    pub api_base: String,
}

/// Resolve the project and credentials, exiting with a clear message when not logged in.
pub(crate) fn ctx(root: &Path) -> CloudCtx {
    let project_id = appz_deploy::resolve_project_id_from_dir(root);
    let vault_tok = crate::secrets::deploy_env(root, &["APPZ_TOKEN".to_string()])
        .ok()
        .and_then(|v| v.vars.get("APPZ_TOKEN").cloned());
    let token = std::env::var("APPZ_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .or(vault_tok)
        .or_else(|| appz_deploy::resolve_auth_token(None));
    if token.is_none() {
        error("not authenticated. Run `appz login` or set APPZ_TOKEN");
    }
    let api_base =
        std::env::var("APPZ_API_URL").unwrap_or_else(|_| "https://api.appz.dev".to_string());
    CloudCtx {
        project_id,
        token,
        api_base,
    }
}

/// Validate and de-duplicate environment names (production, preview, development or a custom slug).
pub(crate) fn validate_environments(envs: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in envs {
        if !appz_deploy::is_valid_environment_slug(e) {
            error(&format!(
                "invalid environment {e:?}: use production, preview, development or a custom \
                 environment name (lowercase letters, digits and '-')"
            ));
        }
        if !out.contains(e) {
            out.push(e.clone());
        }
    }
    out
}

/// Decrypted values for each environment; when several are given a later one wins on conflicts.
pub(crate) fn fetch_values(ctx: &CloudCtx, envs: &[String]) -> Vec<(String, String)> {
    let envs = if envs.is_empty() {
        vec![DEFAULT_ENVIRONMENT.to_string()]
    } else {
        validate_environments(envs)
    };
    let mut merged: Vec<(String, String)> = Vec::new();
    for env in &envs {
        let vars = appz_deploy::pull_cloud_env(
            &ctx.api_base,
            &ctx.project_id,
            ctx.token.as_deref(),
            Some(env),
        )
        .unwrap_or_else(|e| error(&e.to_string()));
        for v in vars {
            match merged.iter_mut().find(|(k, _)| *k == v.key) {
                Some(slot) => slot.1 = v.value,
                None => merged.push((v.key, v.value)),
            }
        }
    }
    merged
}

fn confirm(prompt: &str) -> bool {
    if crate::assume_yes() {
        true
    } else if is_tty() {
        cliclack::confirm(prompt)
            .initial_value(false)
            .interact()
            .unwrap_or(false)
    } else {
        error("confirmation required: re-run with --yes")
    }
}

/// `appz env cloud ls`: keys and metadata only, never values.
pub(crate) fn ls(root: &Path, envs: &[String], json: bool) {
    let ctx = ctx(root);
    let targets: Vec<Option<String>> = if envs.is_empty() {
        vec![None]
    } else {
        validate_environments(envs).into_iter().map(Some).collect()
    };
    let mut rows: Vec<appz_deploy::CloudEnvMeta> = Vec::new();
    for t in &targets {
        let got = appz_deploy::list_cloud_env(
            &ctx.api_base,
            &ctx.project_id,
            ctx.token.as_deref(),
            t.as_deref(),
        )
        .unwrap_or_else(|e| error(&e.to_string()));
        for r in got {
            if !rows.iter().any(|x| x.id == r.id) {
                rows.push(r);
            }
        }
    }
    if json {
        println!("{}", serde_json::to_string(&rows).unwrap());
    } else if rows.is_empty() {
        ui::empty(
            "environment variables",
            Some("Run `appz env push` to upload a .env file"),
        );
    } else {
        let mut t = ui::Table::new(&["Key", "Environments", "Branch"]);
        for r in &rows {
            t.row(vec![
                r.key.clone(),
                r.target.join(", "),
                r.git_branch.clone().unwrap_or_default(),
            ]);
        }
        t.print();
    }
}

/// `appz env cloud rm`: the API removes a key from every environment at once.
pub(crate) fn rm(root: &Path, key: &str, json: bool) {
    let ctx = ctx(root);
    if !confirm(&format!(
        "Remove {key} from every environment of project '{}'?",
        ctx.project_id
    )) {
        error("Cancelled");
    }
    match appz_deploy::delete_cloud_env_var(
        &ctx.api_base,
        &ctx.project_id,
        ctx.token.as_deref(),
        key,
    ) {
        Ok(_) if json => println!("{}", serde_json::json!({ "removed": key })),
        Ok(_) => success(&format!("Removed {key} from every environment")),
        Err(e) => error(&e.to_string()),
    }
}

pub(crate) fn environments_ls(root: &Path, json: bool) {
    let ctx = ctx(root);
    match appz_deploy::list_project_environments(
        &ctx.api_base,
        &ctx.project_id,
        ctx.token.as_deref(),
    ) {
        Ok(rows) if json => println!("{}", serde_json::to_string(&rows).unwrap()),
        Ok(rows) if rows.is_empty() => ui::empty("custom environments", None),
        Ok(rows) => {
            let mut t = ui::Table::new(&["Environment", "Branches"]);
            for r in &rows {
                t.row(vec![
                    r.slug.clone(),
                    r.branch_pattern.clone().unwrap_or_default(),
                ]);
            }
            t.print();
        }
        Err(e) => error(&e.to_string()),
    }
}

pub(crate) fn environments_add(root: &Path, slug: &str, branch_pattern: Option<&str>, json: bool) {
    let ctx = ctx(root);
    match appz_deploy::add_project_environment(
        &ctx.api_base,
        &ctx.project_id,
        ctx.token.as_deref(),
        slug,
        branch_pattern,
    ) {
        Ok(env) if json => println!("{}", serde_json::to_string(&env).unwrap()),
        Ok(env) => success(&format!("Created environment {}", env.slug)),
        Err(e) => error(&e.to_string()),
    }
}

pub(crate) fn environments_rm(root: &Path, slug: &str, json: bool) {
    let ctx = ctx(root);
    if !confirm(&format!(
        "Remove environment '{slug}' and its variable targets? Existing deployments remain."
    )) {
        error("Cancelled");
    }
    match appz_deploy::remove_project_environment(
        &ctx.api_base,
        &ctx.project_id,
        ctx.token.as_deref(),
        slug,
    ) {
        Ok(()) if json => println!("{}", serde_json::json!({ "removed": slug })),
        Ok(()) => success(&format!("Removed environment {slug}")),
        Err(e) => error(&e.to_string()),
    }
}

/// `appz env run`: run a command with the cloud values injected into its
/// environment. Values stay in memory and are never printed.
pub(crate) fn run(root: &Path, envs: &[String], cmd: &[String]) -> ! {
    let Some((program, args)) = cmd.split_first() else {
        error("missing command: appz env run [--environment X] -- <cmd> [args...]");
    };
    let ctx = ctx(root);
    let vars = fetch_values(&ctx, envs);
    let build = |p: &str, a: &[String]| {
        let mut c = std::process::Command::new(p);
        c.args(a).envs(vars.iter().map(|(k, v)| (k, v)));
        c
    };
    let mut result = build(program, args).status();
    // Windows resolves `npm`, `pnpm` and friends through .cmd shims only via cmd.exe.
    if cfg!(windows) && matches!(&result, Err(e) if e.kind() == std::io::ErrorKind::NotFound) {
        let mut shim = vec!["/C".to_string(), program.clone()];
        shim.extend(args.iter().cloned());
        result = build("cmd", &shim).status();
    }
    match result {
        Ok(status) => std::process::exit(exit_code(status)),
        Err(e) => error(&format!("failed to run {program}: {e}")),
    }
}

fn exit_code(status: std::process::ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    status.code().unwrap_or(1)
}

/// Shared by `env pull`: write the values and report counts (never values).
pub(crate) fn pull_to_file(root: &Path, file: &Path, envs: &[String], json: bool) {
    if !json {
        intro("appz env pull");
    }
    let ctx = ctx(root);
    let kvs = fetch_values(&ctx, envs);
    let shown: Vec<String> = if envs.is_empty() {
        vec![DEFAULT_ENVIRONMENT.to_string()]
    } else {
        validate_environments(envs)
    };
    let formatted = appz_deploy::format_env_file(&kvs);
    let target_path =
        crate::secrets::resolve_project_file(root, file).unwrap_or_else(|e| error(&e));
    if let Some(parent) = target_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        error(&format!("failed to create parent dir: {e}"));
    }
    if let Err(e) = std::fs::write(&target_path, formatted) {
        error(&format!("failed to write {}: {e}", target_path.display()));
    }
    if json {
        println!(
            "{}",
            serde_json::json!({ "pulled": kvs.len(), "environments": shown,
                "file": target_path.display().to_string() })
        );
    } else {
        if kvs.is_empty() {
            ui::hint(&format!(
                "No variables in {}; pick another with --environment",
                shown.join(", ")
            ));
        }
        success(&format!(
            "Downloaded {} ({}) to {}",
            ui::fmt::plural(kvs.len(), "environment variable"),
            shown.join(", "),
            target_path.display()
        ));
        outro("Done");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn push_default_includes_development() {
        let d = super::default_push_environments();
        assert!(d.contains(&super::DEFAULT_ENVIRONMENT.to_string()));
        assert_eq!(d.len(), 3);
    }
}
