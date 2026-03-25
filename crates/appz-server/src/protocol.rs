use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    Ping,
    Info,
    RegisterApp {
        config_path: String,
        project_dir: String,
        app_name: String,
        upstream_port: u16,
        command: Vec<String>,
        env: HashMap<String, String>,
        hosts: Vec<String>,
        static_dir: Option<String>,
        hot_reload: bool,
    },
    UnregisterApp {
        config_path: String,
    },
    SetAppStatus {
        config_path: String,
        status: String,
    },
    HandoffApp {
        config_path: String,
    },
    RestartApp {
        config_path: String,
    },
    ListApps,
    SubscribeEvents,
    StopServer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    Pong,
    ServerInfo {
        version: String,
        app_count: usize,
    },
    AppRegistered {
        config_path: String,
    },
    AppUnregistered {
        config_path: String,
    },
    AppStatusUpdated {
        config_path: String,
        status: String,
    },
    AppHandedOff {
        config_path: String,
    },
    AppRestarted {
        config_path: String,
    },
    Apps {
        apps: Vec<AppInfo>,
    },
    Subscribed,
    Event {
        event: ServerEvent,
    },
    Ok,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub config_path: String,
    pub app_name: String,
    pub upstream_port: u16,
    pub status: String,
    pub pid: Option<u32>,
    pub hosts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ServerEvent {
    RequestStarted {
        host: String,
        method: String,
        path: String,
    },
    RequestFinished {
        host: String,
        method: String,
        path: String,
        status: u16,
    },
    AppStatusChanged {
        config_path: String,
        app_name: String,
        status: String,
    },
    RestartRequested {
        config_path: String,
        app_name: String,
    },
}
