use std::path::PathBuf;
use task::{Context, Task};
pub fn workdir_from_ctx(ctx: &Context, key: &str, default_rel: &str) -> PathBuf {
    let wd = ctx
        .get(key)
        .map(|s| s.to_string())
        .unwrap_or_else(|| default_rel.to_string());
    PathBuf::from(ctx.parse(&wd))
}
pub fn extra_args(ctx: &Context, key: &str) -> Vec<String> {
    ctx.get(key)
        .map(|s| shell_words::split(&s).unwrap_or_default())
        .unwrap_or_default()
}
pub fn with_env_vars(
    ctx: &Context,
    pairs: &[(&str, &str)],
) -> std::collections::HashMap<String, String> {
    let mut env = std::collections::HashMap::new();
    for (ctx_key, env_key) in pairs.iter() {
        if let Some(v) = ctx.get(ctx_key) {
            env.insert((*env_key).to_string(), v.to_string());
        } else if let Ok(v) = std::env::var(env_key) {
            env.insert((*env_key).to_string(), v);
        }
    }
    env
}
pub fn depends_on_many(task: Task, deps: &[&str]) -> Task {
    let mut t = task;
    for d in deps {
        t = t.depends_on(*d);
    }
    t
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn adds_all_dependencies_in_order() {
        let t = depends_on_many(
            Task::new("x", task::task_fn_sync!(|_ctx: Arc<Context>| Ok(()))),
            &["a", "b", "c"],
        );
        assert_eq!(
            t.deps,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }
}
