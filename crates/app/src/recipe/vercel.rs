use std::collections::HashMap;
use std::path::PathBuf;

use crate::recipe::common;
use task::{Context, TaskRegistry};

fn env_or_ctx(ctx: &Context, key: &str, env_key: &str) -> Option<String> {
    if let Some(v) = ctx.get(key) {
        return Some(v.to_string());
    }
    std::env::var(env_key).ok()
}

fn workdir_from_ctx(ctx: &Context) -> PathBuf {
    common::workdir_from_ctx(ctx, "vercel_workdir", ".")
}

fn extra_args(ctx: &Context) -> Vec<String> {
    common::extra_args(ctx, "vercel_extra_args")
}

fn vercel_env(ctx: &Context) -> String {
    ctx.get("vercel_env")
        .unwrap_or("production".to_string())
        .to_string()
}

fn with_token_env(ctx: &Context) -> HashMap<String, String> {
    common::with_env_vars(ctx, &[("vercel_token", "VERCEL_TOKEN")])
}

pub fn register_vercel(_reg: &mut TaskRegistry) {
    // Minimal placeholder to restore compilation.
}
