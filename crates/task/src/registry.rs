use std::collections::HashMap;

use crate::task::{Hooks, Task};

#[derive(Clone, Default)]
pub struct TaskRegistry {
    tasks: HashMap<String, Task>,
    pub hooks: Hooks,
    pub fail_map: HashMap<String, String>, // target -> failure hook
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            hooks: Hooks::default(),
            fail_map: HashMap::new(),
        }
    }
    pub fn register(&mut self, task: Task) {
        self.tasks.insert(task.name.clone(), task);
    }
    pub fn get(&self, name: &str) -> Option<&Task> {
        self.tasks.get(name)
    }
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Task> {
        self.tasks.get_mut(name)
    }
    pub fn all(&self) -> impl Iterator<Item = (&String, &Task)> {
        self.tasks.iter()
    }

    pub fn before<T: Into<String>, H: Into<String>>(&mut self, target: T, hook: H) {
        self.hooks.add_before(target, hook);
    }
    pub fn after<T: Into<String>, H: Into<String>>(&mut self, target: T, hook: H) {
        self.hooks.add_after(target, hook);
    }
    pub fn fail<T: Into<String>, H: Into<String>>(&mut self, target: T, hook: H) {
        self.fail_map.insert(target.into(), hook.into());
    }

    // Namespaced view helper
    pub fn with_namespace<'a, S: Into<String>>(&'a mut self, ns: S) -> NamespacedRegistry<'a> {
        NamespacedRegistry {
            base: self,
            ns: ns.into(),
        }
    }

    // Convenience to load a recipe under a namespace
    pub fn load_recipe<F>(&mut self, ns: &str, loader: F)
    where
        F: FnOnce(&mut NamespacedRegistry),
    {
        let mut nsreg = self.with_namespace(ns.to_string());
        loader(&mut nsreg);
    }
}

// A namespaced view over TaskRegistry that qualifies task names, deps, and hooks.
pub struct NamespacedRegistry<'a> {
    pub(crate) base: &'a mut TaskRegistry,
    pub(crate) ns: String,
}

impl<'a> NamespacedRegistry<'a> {
    fn qualify(&self, name: &str) -> String {
        if name.starts_with('/') || name.starts_with(':') {
            return name.trim_start_matches(['/', ':']).to_string();
        }
        if name.contains(':') {
            return name.to_string();
        }
        format!("{}:{}", self.ns, name)
    }

    pub fn register(&mut self, mut task: Task) {
        task.name = self.qualify(&task.name);
        task.deps = task.deps.iter().map(|d| self.qualify(d)).collect();
        self.base.register(task);
    }

    pub fn before<T: Into<String>, H: Into<String>>(&mut self, target: T, hook: H) {
        let tgt = self.qualify(&target.into());
        let hk = self.qualify(&hook.into());
        self.base.before(tgt, hk);
    }

    pub fn after<T: Into<String>, H: Into<String>>(&mut self, target: T, hook: H) {
        let tgt = self.qualify(&target.into());
        let hk = self.qualify(&hook.into());
        self.base.after(tgt, hk);
    }

    pub fn fail<T: Into<String>, H: Into<String>>(&mut self, target: T, hook: H) {
        let tgt = self.qualify(&target.into());
        let hk = self.qualify(&hook.into());
        self.base.fail(tgt, hk);
    }

    pub fn with_namespace<S: Into<String>>(&mut self, ns: S) -> NamespacedRegistry<'_> {
        NamespacedRegistry {
            base: self.base,
            ns: format!("{}:{}", self.ns, ns.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::types::AsyncTaskFn;
    use futures::future::BoxFuture;

    #[test]
    fn register_and_get_task() {
        let mut reg = TaskRegistry::new();
        let action: AsyncTaskFn =
            Arc::new(|_c| Box::pin(async move { Ok(()) }) as BoxFuture<'static, _>);
        reg.register(Task::new("a", action.clone()).desc("desc").group("g"));

        let got = reg.get("a").unwrap();
        assert_eq!(got.name, "a");
        assert_eq!(got.description.as_deref(), Some("desc"));
        assert_eq!(got.group.as_deref(), Some("g"));

        // mutate via get_mut without moving the task (adjust deps directly)
        let t = reg.get_mut("a").unwrap();
        t.deps.push("b".to_string());
        assert!(reg.get("a").unwrap().deps.contains(&"b".to_string()));

        // all iterator contains our task
        let names: Vec<_> = reg.all().map(|(n, _)| n.clone()).collect();
        assert!(names.contains(&"a".to_string()));
    }

    #[test]
    fn hooks_registration() {
        let mut reg = TaskRegistry::new();
        let action: AsyncTaskFn =
            Arc::new(|_c| Box::pin(async move { Ok(()) }) as BoxFuture<'static, _>);
        reg.register(Task::new("build", action.clone()));
        reg.register(Task::new("lint", action.clone()));
        reg.register(Task::new("test", action.clone()));

        reg.before("build", "lint");
        reg.after("build", "test");

        assert_eq!(
            reg.hooks.before.get("build").unwrap(),
            &vec!["lint".to_string()]
        );
        assert_eq!(
            reg.hooks.after.get("build").unwrap(),
            &vec!["test".to_string()]
        );
    }

    #[test]
    fn fail_registration() {
        let mut reg = TaskRegistry::new();
        reg.fail("deploy", "deploy:failed");
        assert_eq!(
            reg.fail_map.get("deploy").map(String::as_str),
            Some("deploy:failed")
        );
    }

    #[test]
    fn list_hides_hidden_tasks() {
        let mut reg = TaskRegistry::new();
        let action: AsyncTaskFn =
            Arc::new(|_c| Box::pin(async move { Ok(()) }) as BoxFuture<'static, _>);
        reg.register(Task::new("visible", action.clone()));
        reg.register(Task::new("hidden", action.clone()).hidden());

        let listed: Vec<_> = reg
            .all()
            .filter(|(_, t)| !t.hidden)
            .map(|(n, _)| n.clone())
            .collect();
        assert!(listed.contains(&"visible".to_string()));
        assert!(!listed.contains(&"hidden".to_string()));
    }
}
