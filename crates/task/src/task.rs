use std::{collections::HashMap, sync::Arc};

use crate::{context::Context, types::AsyncTaskFn};

pub type Condition = Arc<dyn Fn(&Context) -> bool + Send + Sync + 'static>;

#[derive(Clone)]
pub struct Task {
    pub name: String,
    pub description: Option<String>,
    pub group: Option<String>,
    pub deps: Vec<String>,
    pub wait_for: Vec<String>, // Soft dependencies: tasks that should run first if they're in the execution plan
    pub only_if: Vec<Condition>,
    pub unless: Vec<Condition>,
    pub once: bool,
    pub hidden: bool,
    pub timeout: Option<u64>, // Timeout in seconds
    pub sources: Vec<String>, // Glob patterns for source files
    pub outputs: Vec<String>, // Glob patterns for output files
    pub action: AsyncTaskFn,
}

impl Task {
    pub fn new<N: Into<String>>(name: N, action: AsyncTaskFn) -> Self {
        Self {
            name: name.into(),
            description: None,
            group: None,
            deps: Vec::new(),
            wait_for: Vec::new(),
            only_if: Vec::new(),
            unless: Vec::new(),
            once: false,
            hidden: false,
            timeout: None,
            sources: Vec::new(),
            outputs: Vec::new(),
            action,
        }
    }

    pub fn desc<S: Into<String>>(mut self, d: S) -> Self {
        self.description = Some(d.into());
        self
    }
    pub fn group<S: Into<String>>(mut self, g: S) -> Self {
        self.group = Some(g.into());
        self
    }
    pub fn depends_on<S: Into<String>>(mut self, dep: S) -> Self {
        self.deps.push(dep.into());
        self
    }
    pub fn wait_for<S: Into<String>>(mut self, task: S) -> Self {
        self.wait_for.push(task.into());
        self
    }
    pub fn only_if<F>(mut self, cond: F) -> Self
    where
        F: Fn(&Context) -> bool + Send + Sync + 'static,
    {
        self.only_if.push(Arc::new(cond));
        self
    }
    pub fn unless<F>(mut self, cond: F) -> Self
    where
        F: Fn(&Context) -> bool + Send + Sync + 'static,
    {
        self.unless.push(Arc::new(cond));
        self
    }
    pub fn once(mut self) -> Self {
        self.once = true;
        self
    }
    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    pub fn timeout(mut self, seconds: u64) -> Self {
        self.timeout = Some(seconds);
        self
    }

    pub fn sources<S: Into<String>>(mut self, sources: Vec<S>) -> Self {
        self.sources = sources.into_iter().map(|s| s.into()).collect();
        self
    }

    pub fn outputs<S: Into<String>>(mut self, outputs: Vec<S>) -> Self {
        self.outputs = outputs.into_iter().map(|s| s.into()).collect();
        self
    }

    pub fn should_run(&self, ctx: &Context) -> bool {
        if !self.only_if.iter().all(|c| c(ctx)) {
            return false;
        }
        if self.unless.iter().any(|c| c(ctx)) {
            return false;
        }
        true
    }
}

/// Resolve a task dependency pattern, optionally relative to a parent task
/// If pattern starts with ":" and parent_task is provided, resolve relative to parent's path
/// For example: parent "//projects/frontend:test" with pattern ":build" -> "//projects/frontend:build"
/// Copied from mise: src/task/mod.rs:938-968
pub(crate) fn resolve_task_pattern(pattern: &str, parent_task: Option<&Task>) -> String {
    // Check if this is a bare task name that should be treated as relative
    let is_bare_name =
        !pattern.starts_with("//") && !pattern.starts_with("::") && !pattern.starts_with(':');

    // If pattern starts with ":" or is a bare name in monorepo context, resolve relatively
    let should_resolve_relatively = pattern.starts_with(':') && !pattern.starts_with("::")
        || (is_bare_name
            && parent_task
                .map(|p| p.name.starts_with("//"))
                .unwrap_or(false));

    if should_resolve_relatively {
        if let Some(parent) = parent_task {
            // Extract the path portion from the parent task name
            // For monorepo tasks like "//projects/frontend:test:nested", we need to extract "//projects/frontend"
            // by finding the FIRST colon after the "//" prefix, not the last one
            if let Some(stripped) = parent.name.strip_prefix("//") {
                // Find the first colon after "//" prefix
                if let Some(colon_idx) = stripped.find(':') {
                    let path = format!("//{}", &stripped[..colon_idx]);
                    // If pattern is a bare name, add the colon prefix
                    return if is_bare_name {
                        format!("{}:{}", path, pattern)
                    } else {
                        format!("{}{}", path, pattern)
                    };
                }
            } else if let Some((path, _)) = parent.name.rsplit_once(':') {
                // For non-monorepo tasks, use the old logic
                return format!("{}{}", path, pattern);
            }
        }
    }
    pattern.to_string()
}

#[derive(Clone, Default)]
pub struct Hooks {
    pub before: HashMap<String, Vec<String>>, // target -> hooks
    pub after: HashMap<String, Vec<String>>,  // target -> hooks
}

impl Hooks {
    pub fn add_before<T: Into<String>, H: Into<String>>(&mut self, target: T, hook: H) {
        self.before
            .entry(target.into())
            .or_default()
            .push(hook.into());
    }
    pub fn add_after<T: Into<String>, H: Into<String>>(&mut self, target: T, hook: H) {
        self.after
            .entry(target.into())
            .or_default()
            .push(hook.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_run_only_if_and_unless() {
        use crate::types::AsyncTaskFn;
        use futures::future::BoxFuture;
        let action: AsyncTaskFn = Arc::new(|_ctx: Arc<Context>| Box::pin(async move { Ok(()) }));
        let base = Task::new("t", action);
        let mut ctx = crate::context::Context::new();

        // No conditions => runs
        assert!(base.should_run(&ctx));

        // only_if false => skip
        let t = base.clone().only_if(|_c| false);
        assert!(!t.should_run(&ctx));

        // only_if true and unless false => runs
        let t = base.clone().only_if(|_c| true).unless(|_c| false);
        assert!(t.should_run(&ctx));

        // unless true => skip even if only_if true
        let t = base.clone().only_if(|_c| true).unless(|_c| true);
        assert!(!t.should_run(&ctx));

        // Conditions based on context
        ctx.set("flag", "1");
        let t = base.clone().only_if(|c| c.contains("flag"));
        assert!(t.should_run(&ctx));
    }
}
