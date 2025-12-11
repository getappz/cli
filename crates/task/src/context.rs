use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use tokio::task_local;

task_local! {
    // task-local storage that survives awaits and thread hops when used with `scope`
    // We store Option<String> so `None` means "no namespace" (global).
    static CURRENT_NAMESPACE: RefCell<Option<String>>;
}

#[derive(Clone, Debug)]
pub struct Context {
    // Base/global vars (thread-safe)
    vars: Arc<RwLock<HashMap<String, String>>>,
    // Env and other fields remain process-global and not namespaced
    env: HashMap<String, String>,
    dotenv_path: Option<String>,
    working_path: Option<PathBuf>,
    // Per-namespace overlays: namespace -> (key -> value)
    namespace_overlays: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    pub fn new() -> Self {
        Self {
            vars: Arc::new(RwLock::new(HashMap::new())),
            env: HashMap::new(),
            dotenv_path: None,
            working_path: None,
            namespace_overlays: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new context with initial base variables.
    pub fn with_vars(initial: HashMap<String, String>) -> Self {
        Self {
            vars: Arc::new(RwLock::new(initial)),
            env: HashMap::new(),
            dotenv_path: None,
            working_path: None,
            namespace_overlays: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // Config-like variables (used by {{var}} parsing)
    // Namespace-aware write: writes to current namespace overlay if bound, otherwise global
    pub fn set<K: Into<String>, V: Into<String>>(&self, key: K, val: V) {
        let key = key.into();
        let val = val.into();

        let wrote_to_ns = CURRENT_NAMESPACE
            .try_with(|ns_cell| {
                if let Some(ns) = ns_cell.borrow().as_deref() {
                    let mut overlays = self.namespace_overlays.write().unwrap();
                    let map = overlays.entry(ns.to_string()).or_default();
                    map.insert(key.clone(), val.clone());
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false);

        if !wrote_to_ns {
            let mut g = self.vars.write().unwrap();
            g.insert(key, val);
        }
    }

    // Namespace-aware read: checks overlay first (if bound), then base
    pub fn get(&self, key: &str) -> Option<String> {
        let ns_val = CURRENT_NAMESPACE
            .try_with(|ns_cell| ns_cell.borrow().clone())
            .unwrap_or(None);
        if let Some(ns) = ns_val {
            let overlays = self.namespace_overlays.read().unwrap();
            if let Some(map) = overlays.get(&ns) {
                if let Some(v) = map.get(key) {
                    return Some(v.clone());
                }
            }
        }
        let g = self.vars.read().unwrap();
        g.get(key).cloned()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
    pub fn parse(&self, s: &str) -> String {
        // Very simple {{key}} replacement (namespace-aware)
        let mut out = String::with_capacity(s.len());
        let mut i = 0;
        let bytes = s.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'{' && i + 3 < bytes.len() && bytes[i + 1] == b'{' {
                // find closing }}
                if let Some(mut j) = s[i + 2..].find("}}") {
                    j += i + 2;
                    let key = s[i + 2..j].trim();
                    if let Some(val) = self.get(key) {
                        out.push_str(&val);
                    } else {
                        // leave as-is if not found
                        out.push_str(&s[i..j + 2]);
                    }
                    i = j + 2;
                    continue;
                }
            }
            out.push(bytes[i] as char);
            i += 1;
        }
        out
    }

    /// Execute a future with a temporary namespace set for the current task.
    ///
    /// Usage:
    /// ctx.with_namespace(Some("laravel"), async { /* ... */ }).await
    pub async fn with_namespace<F, T>(&self, ns: Option<&str>, fut: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        let cell = RefCell::new(ns.map(|s| s.to_string()));
        CURRENT_NAMESPACE.scope(cell, fut).await
    }

    /// Remove a key from the current namespace overlay or from global vars if no namespace.
    pub fn remove(&self, key: &str) -> Option<String> {
        let removed = CURRENT_NAMESPACE
            .try_with(|ns_cell| {
                if let Some(ns) = ns_cell.borrow().as_deref() {
                    let mut overlays = self.namespace_overlays.write().unwrap();
                    if let Some(map) = overlays.get_mut(ns) {
                        return map.remove(key);
                    }
                    None
                } else {
                    None
                }
            })
            .ok()
            .flatten();

        if removed.is_some() {
            return removed;
        }

        let mut g = self.vars.write().unwrap();
        g.remove(key)
    }

    // Env management
    pub fn set_env<K: Into<String>, V: Into<String>>(&mut self, key: K, val: V) {
        self.env.insert(key.into(), val.into());
    }
    pub fn env(&self) -> &HashMap<String, String> {
        &self.env
    }
    pub fn set_dotenv<P: Into<String>>(&mut self, path: P) {
        self.dotenv_path = Some(path.into());
    }
    pub fn dotenv(&self) -> Option<&str> {
        self.dotenv_path.as_deref()
    }

    // Working directory support
    pub fn set_working_path<P: Into<PathBuf>>(&mut self, p: P) {
        self.working_path = Some(p.into());
    }
    pub fn working_path(&self) -> Option<&PathBuf> {
        self.working_path.as_ref()
    }

    // Load .env key=value pairs into env (non-strict; ignores malformed lines)
    pub fn load_dotenv_into_env(&mut self) {
        if let Some(p) = &self.dotenv_path {
            let path = PathBuf::from(p);
            if let Ok(md) = fs::metadata(&path) {
                if md.len() == 0 {
                    return;
                }
                if let Ok(contents) = fs::read_to_string(&path) {
                    for line in contents.lines() {
                        let line = line.trim();
                        if line.is_empty() || line.starts_with('#') {
                            continue;
                        }
                        if let Some(eq) = line.find('=') {
                            let (k, v) = (&line[..eq], &line[eq + 1..]);
                            self.env.insert(k.trim().to_string(), v.trim().to_string());
                        }
                    }
                }
            }
        }
    }
}
