use miette::{miette, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for a remote host
#[derive(Debug, Clone, Deserialize)]
pub struct HostConfig {
    /// Hostname or IP address
    pub hostname: String,
    /// SSH username (optional, defaults to current user)
    #[serde(default)]
    pub user: Option<String>,
    /// SSH port (optional, defaults to 22)
    #[serde(default)]
    pub port: Option<u16>,
    /// Path to SSH private key file (optional)
    #[serde(default)]
    pub identity_file: Option<String>,
    /// Enable SSH agent forwarding (optional, defaults to false)
    #[serde(default)]
    pub forward_agent: Option<bool>,
    /// Deployment path on remote server (optional)
    #[serde(default)]
    pub deploy_path: Option<String>,
    /// Alias/name for this host (for display purposes)
    #[serde(default)]
    pub alias: Option<String>,
    /// SSH config file path (optional)
    #[serde(default)]
    pub config_file: Option<String>,
    /// Additional SSH arguments (optional)
    #[serde(default)]
    pub ssh_arguments: Option<Vec<String>>,
}

impl HostConfig {
    /// Get the alias for this host, defaulting to hostname if not set
    pub fn alias(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.hostname)
    }

    /// Get the connection string (user@hostname)
    pub fn connection_string(&self) -> String {
        if let Some(user) = &self.user {
            format!("{}@{}", user, self.hostname)
        } else {
            self.hostname.clone()
        }
    }

    /// Get the SSH port, defaulting to 22
    pub fn port(&self) -> u16 {
        self.port.unwrap_or(22)
    }

    /// Get the identity file path if set
    pub fn identity_file_path(&self) -> Option<PathBuf> {
        self.identity_file.as_ref().map(|p| {
            // Expand ~ to home directory
            if p.starts_with('~') {
                let home = std::env::var("HOME")
                    .or_else(|_| std::env::var("USERPROFILE"))
                    .unwrap_or_else(|_| "~".to_string());
                PathBuf::from(p.replace('~', &home))
            } else {
                PathBuf::from(p)
            }
        })
    }

    /// Whether to forward SSH agent
    pub fn should_forward_agent(&self) -> bool {
        self.forward_agent.unwrap_or(false)
    }
}

/// A registry for managing multiple hosts
#[derive(Debug, Clone, Default)]
pub struct HostRegistry {
    hosts: HashMap<String, HostConfig>,
}

impl HostRegistry {
    /// Create a new empty host registry
    pub fn new() -> Self {
        Self {
            hosts: HashMap::new(),
        }
    }

    /// Register a host with the given name
    pub fn register(&mut self, name: String, config: HostConfig) {
        self.hosts.insert(name, config);
    }

    /// Get a host by name
    pub fn get(&self, name: &str) -> Result<&HostConfig> {
        if name == "default" {
            // If only one host exists, use it as default
            if self.hosts.len() == 1 {
                return Ok(self.hosts.values().next().unwrap());
            }
        }
        self.hosts.get(name).ok_or_else(|| {
            miette!(
                "Host '{}' not found in registry. Available hosts: {}",
                name,
                if self.hosts.is_empty() {
                    "none".to_string()
                } else {
                    self.hosts.keys().cloned().collect::<Vec<_>>().join(", ")
                }
            )
        })
    }

    /// Get all host names
    pub fn names(&self) -> Vec<String> {
        self.hosts.keys().cloned().collect()
    }

    /// Check if a host exists
    pub fn has(&self, name: &str) -> bool {
        self.hosts.contains_key(name)
    }

    /// Get all hosts
    pub fn all(&self) -> &HashMap<String, HostConfig> {
        &self.hosts
    }

    /// Parse hosts from a YAML/JSON value
    pub fn from_value(value: &serde_json::Value) -> Result<Self> {
        let mut registry = Self::new();

        if let Some(hosts_obj) = value.as_object() {
            for (name, host_value) in hosts_obj {
                let config: HostConfig = serde_json::from_value(host_value.clone())
                    .map_err(|e| miette!("Failed to parse host '{}': {}", name, e))?;
                registry.register(name.clone(), config);
            }
        } else if let Some(hosts_array) = value.as_array() {
            // Array format: [{name: "host1", hostname: "..."}, ...]
            for host_value in hosts_array {
                let config: HostConfig = serde_json::from_value(host_value.clone())
                    .map_err(|e| miette!("Failed to parse host in array: {}", e))?;
                let name = config
                    .alias
                    .as_deref()
                    .unwrap_or(&config.hostname)
                    .to_string();
                registry.register(name, config);
            }
        }

        Ok(registry)
    }
}
