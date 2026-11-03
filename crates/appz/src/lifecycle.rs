//! Post-deploy lifecycle commands shared by `appz ls` and `appz rollback`:
//! resolve the target the same way `appz deploy` does, build a lightweight
//! `DeployContext` (auth token + link ids from the vault, mise bin dirs on
//! `PATH`), then hand off to the provider's `list_deployments` / `rollback`.

use std::path::Path;

use appz_deploy::{
    DeployContext, DeployOutput, DeployProvider, DeploymentInfo, get_provider, read_deploy_config,
};

/// Resolve the target slug (explicit `--target`, else `deploy.default`, else
/// auto-detected) and build the provider + context for a read-only lifecycle
/// call. The context has the provider's auth token and link ids in `run_env`
/// (from the vault) and mise's bin dirs on `extra_path` — enough for
/// `list_deployments` / `rollback`, which never build or upload.
fn resolve(
    project_dir: &Path,
    target: Option<&str>,
) -> Result<(Box<dyn DeployProvider>, DeployContext), String> {
    let config = read_deploy_config(project_dir)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let slug = match target.map(String::from).or_else(|| config.default.clone()) {
        Some(slug) => slug,
        None => crate::deploy::autodetect_target(project_dir)?,
    };
    let provider = get_provider(&slug).map_err(|e| e.to_string())?;

    let mut ctx = DeployContext::new(project_dir.to_path_buf(), "dist".into());
    ctx.deploy_config = config;
    ctx.extra_path = appz_core::mise_bin_paths(project_dir);

    // Vault values this provider reads: its token, link ids, and `deploy.env`.
    let allowed: Vec<String> = std::iter::once(provider.auth_env_var().to_string())
        .chain(crate::deploy::LINK_VARS.map(String::from))
        .chain(ctx.deploy_config.env.iter().cloned())
        .collect();
    let vault = crate::secrets::deploy_env(project_dir, &allowed)?;
    ctx.run_env = vault
        .vars
        .into_iter()
        .filter(|(k, _)| std::env::var_os(k).is_none())
        .collect();

    Ok((provider, ctx))
}

pub fn list(project_dir: &Path, target: Option<&str>) -> Result<Vec<DeploymentInfo>, String> {
    let (provider, ctx) = resolve(project_dir, target)?;
    provider.list_deployments(&ctx).map_err(|e| e.to_string())
}

pub fn rollback(
    project_dir: &Path,
    target: Option<&str>,
    deployment_id: &str,
) -> Result<DeployOutput, String> {
    let (provider, ctx) = resolve(project_dir, target)?;
    provider
        .rollback(&ctx, deployment_id)
        .map_err(|e| e.to_string())
}

pub fn logs(project_dir: &Path, target: Option<&str>, deployment: &str) -> Result<String, String> {
    let (provider, ctx) = resolve(project_dir, target)?;
    provider.logs(&ctx, deployment).map_err(|e| e.to_string())
}

pub fn inspect(
    project_dir: &Path,
    target: Option<&str>,
    deployment: &str,
) -> Result<String, String> {
    let (provider, ctx) = resolve(project_dir, target)?;
    provider
        .inspect(&ctx, deployment)
        .map_err(|e| e.to_string())
}

pub fn promote(
    project_dir: &Path,
    target: Option<&str>,
    deployment: &str,
) -> Result<DeployOutput, String> {
    let (provider, ctx) = resolve(project_dir, target)?;
    provider
        .promote(&ctx, deployment)
        .map_err(|e| e.to_string())
}

/// Push every name in the project vault to the target. The vault must be
/// unlocked (its values are read, then handed to the platform CLI).
pub fn env_push(project_dir: &Path, target: Option<&str>) -> Result<Vec<String>, String> {
    let (provider, ctx) = resolve(project_dir, target)?;
    let path = crate::secrets::vault_path(project_dir, false).map_err(|e| e.to_string())?;
    let names = crate::secrets::names(&path).map_err(|e| e.to_string())?;
    if names.is_empty() {
        return Ok(Vec::new());
    }
    let vault = crate::secrets::deploy_env(project_dir, &names)?;
    if vault.locked {
        return Err("vault is locked — run `appz env unlock` first".to_string());
    }
    let mut pushed = Vec::new();
    for name in &names {
        let Some(value) = vault.vars.get(name) else {
            continue;
        };
        provider
            .push_env(&ctx, name, value)
            .map_err(|e| e.to_string())?;
        pushed.push(name.clone());
    }
    Ok(pushed)
}

/// Pull the target's env vars into the project vault.
pub fn env_pull(project_dir: &Path, target: Option<&str>) -> Result<Vec<String>, String> {
    let (provider, ctx) = resolve(project_dir, target)?;
    let pairs = provider.pull_env(&ctx).map_err(|e| e.to_string())?;
    let path = crate::secrets::vault_path(project_dir, false).map_err(|e| e.to_string())?;
    let mut pulled = Vec::new();
    for (name, value) in pairs {
        crate::secrets::set(&path, &name, Some(value)).map_err(|e| e.to_string())?;
        pulled.push(name);
    }
    Ok(pulled)
}
