use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, params};
use serde_json;

use crate::protocol::AppInfo;

#[derive(Debug, Clone)]
pub enum AppStatus {
    Running,
    Idle,
    Starting,
}

impl AppStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppStatus::Running => "running",
            AppStatus::Idle => "idle",
            AppStatus::Starting => "starting",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "running" => AppStatus::Running,
            "starting" => AppStatus::Starting,
            _ => AppStatus::Idle,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeApp {
    pub config_path: String,
    pub project_dir: String,
    pub app_name: String,
    pub upstream_port: u16,
    pub command: Vec<String>,
    pub env: HashMap<String, String>,
    pub hosts: Vec<String>,
    pub static_dir: Option<String>,
    pub hot_reload: bool,
    pub status: AppStatus,
    pub pid: Option<u32>,
}

pub struct AppState {
    apps: Mutex<HashMap<String, RuntimeApp>>,
    db: Mutex<Connection>,
}

impl AppState {
    pub fn new(data_dir: &Path) -> rusqlite::Result<Self> {
        std::fs::create_dir_all(data_dir).ok();
        let db_path = data_dir.join("apps.db");
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS apps (
                config_path  TEXT PRIMARY KEY,
                project_dir  TEXT NOT NULL,
                app_name     TEXT NOT NULL,
                upstream_port INTEGER NOT NULL,
                command      TEXT NOT NULL,
                env          TEXT NOT NULL,
                hosts        TEXT NOT NULL,
                static_dir   TEXT,
                hot_reload   INTEGER NOT NULL DEFAULT 0,
                created_at   TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )?;
        let state = Self {
            apps: Mutex::new(HashMap::new()),
            db: Mutex::new(conn),
        };
        state.load_from_db()?;
        Ok(state)
    }

    pub fn load_from_db(&self) -> rusqlite::Result<()> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT config_path, project_dir, app_name, upstream_port, command, env, hosts, static_dir, hot_reload FROM apps",
        )?;
        let rows = stmt.query_map([], |row| {
            let config_path: String = row.get(0)?;
            let project_dir: String = row.get(1)?;
            let app_name: String = row.get(2)?;
            let upstream_port: u16 = row.get(3)?;
            let command_json: String = row.get(4)?;
            let env_json: String = row.get(5)?;
            let hosts_json: String = row.get(6)?;
            let static_dir: Option<String> = row.get(7)?;
            let hot_reload: bool = row.get(8)?;
            Ok((
                config_path,
                project_dir,
                app_name,
                upstream_port,
                command_json,
                env_json,
                hosts_json,
                static_dir,
                hot_reload,
            ))
        })?;

        let mut apps = self.apps.lock().unwrap();
        for row in rows {
            let (config_path, project_dir, app_name, upstream_port, command_json, env_json, hosts_json, static_dir, hot_reload) = row?;
            let command: Vec<String> = serde_json::from_str(&command_json).unwrap_or_default();
            let env: HashMap<String, String> = serde_json::from_str(&env_json).unwrap_or_default();
            let hosts: Vec<String> = serde_json::from_str(&hosts_json).unwrap_or_default();
            let app = RuntimeApp {
                config_path: config_path.clone(),
                project_dir,
                app_name,
                upstream_port,
                command,
                env,
                hosts,
                static_dir,
                hot_reload,
                status: AppStatus::Idle,
                pid: None,
            };
            apps.insert(config_path, app);
        }
        Ok(())
    }

    pub fn register(&self, app: RuntimeApp) -> rusqlite::Result<()> {
        let command_json = serde_json::to_string(&app.command).unwrap_or_default();
        let env_json = serde_json::to_string(&app.env).unwrap_or_default();
        let hosts_json = serde_json::to_string(&app.hosts).unwrap_or_default();
        let hot_reload = app.hot_reload as i32;

        {
            let db = self.db.lock().unwrap();
            db.execute(
                "INSERT OR REPLACE INTO apps (config_path, project_dir, app_name, upstream_port, command, env, hosts, static_dir, hot_reload)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    app.config_path,
                    app.project_dir,
                    app.app_name,
                    app.upstream_port,
                    command_json,
                    env_json,
                    hosts_json,
                    app.static_dir,
                    hot_reload,
                ],
            )?;
        }

        let mut apps = self.apps.lock().unwrap();
        apps.insert(app.config_path.clone(), app);
        Ok(())
    }

    pub fn unregister(&self, config_path: &str) -> rusqlite::Result<bool> {
        let removed = {
            let mut apps = self.apps.lock().unwrap();
            apps.remove(config_path).is_some()
        };
        if removed {
            let db = self.db.lock().unwrap();
            db.execute("DELETE FROM apps WHERE config_path = ?1", params![config_path])?;
        }
        Ok(removed)
    }

    pub fn get(&self, config_path: &str) -> Option<RuntimeApp> {
        let apps = self.apps.lock().unwrap();
        apps.get(config_path).cloned()
    }

    pub fn set_status(&self, config_path: &str, status: AppStatus) -> bool {
        let mut apps = self.apps.lock().unwrap();
        if let Some(app) = apps.get_mut(config_path) {
            app.status = status;
            true
        } else {
            false
        }
    }

    pub fn set_pid(&self, config_path: &str, pid: Option<u32>) -> bool {
        let mut apps = self.apps.lock().unwrap();
        if let Some(app) = apps.get_mut(config_path) {
            app.pid = pid;
            true
        } else {
            false
        }
    }

    pub fn list(&self) -> Vec<AppInfo> {
        let apps = self.apps.lock().unwrap();
        apps.values()
            .map(|app| AppInfo {
                config_path: app.config_path.clone(),
                app_name: app.app_name.clone(),
                upstream_port: app.upstream_port,
                status: app.status.as_str().to_string(),
                pid: app.pid,
                hosts: app.hosts.clone(),
            })
            .collect()
    }

    pub fn find_by_host(&self, host: &str) -> Option<RuntimeApp> {
        let apps = self.apps.lock().unwrap();
        apps.values()
            .find(|app| app.hosts.iter().any(|h| h == host))
            .cloned()
    }

    pub fn app_count(&self) -> usize {
        let apps = self.apps.lock().unwrap();
        apps.len()
    }
}
