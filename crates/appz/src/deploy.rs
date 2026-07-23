//! CLI/MCP-shared glue between appz's project detection and `appz-deploy`'s
//! providers: resolve the target, build a `DeployContext`, deploy.

use std::collections::HashMap;
use std::path::Path;

use appz_deploy::{DeployContext, DeployOutput, get_provider, parse_kv, read_deploy_config};

pub struct DeployRequest {
    pub target: Option<String>,
    pub prod: bool,
    pub env: Vec<String>,
    pub build_env: Vec<String>,
    pub dry_run: bool,
    pub json: bool,
}

pub fn run(project_dir: &Path, req: DeployRequest) -> Result<DeployOutput, String> {
    let config = read_deploy_config(project_dir)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    let slug = req.target.clone().or_else(|| config.default.clone()).ok_or_else(|| {
        "no deploy target specified — pass --target <provider> or set \"deploy\": { \"default\": \"vercel\" } in appz.jsonc".to_string()
    })?;

    let provider = get_provider(&slug).map_err(|e| e.to_string())?;

    let toolchains = appz_core::detect_toolchains(project_dir).map_err(|e| e.to_string())?;
    let output_dir = toolchains
        .iter()
        .find_map(|tc| tc.output_directory)
        .unwrap_or("dist")
        .to_string();
    let framework = toolchains
        .iter()
        .flat_map(|tc| tc.frameworks.iter())
        .next()
        .map(|f| f.name.to_string());

    let mut ctx = DeployContext::new(project_dir.to_path_buf(), output_dir);
    ctx.is_preview = !req.prod;
    ctx.dry_run = req.dry_run;
    ctx.json_output = req.json;
    ctx.framework = framework;
    ctx.deploy_config = config;
    ctx.run_env = req
        .env
        .iter()
        .filter_map(|s| parse_kv(s))
        .collect::<HashMap<_, _>>();
    ctx.build_env = req
        .build_env
        .iter()
        .filter_map(|s| parse_kv(s))
        .collect::<HashMap<_, _>>();

    if ctx.is_preview {
        provider.deploy_preview(&ctx).map_err(|e| e.to_string())
    } else {
        provider.deploy(&ctx).map_err(|e| e.to_string())
    }
}
