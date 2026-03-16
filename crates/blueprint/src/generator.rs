use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::error::BlueprintError;

/// Introspects a running WordPress DDEV project and generates a blueprint.json.
pub struct BlueprintGenerator {
    project_path: PathBuf,
}

/// Represents a plugin from `wp plugin list --format=json`.
#[derive(Debug, serde::Deserialize)]
struct WpPlugin {
    name: String,
    status: String,
}

/// Represents a theme from `wp theme list --format=json`.
#[derive(Debug, serde::Deserialize)]
struct WpTheme {
    name: String,
    status: String,
}

/// Represents a wp-config constant from `wp config list --format=json`.
#[derive(Debug, serde::Deserialize)]
struct WpConfigEntry {
    name: String,
    value: String,
    #[serde(rename = "type")]
    entry_type: String,
}

/// Constants that are managed by WordPress/DDEV — skip these during generation.
const SKIP_CONSTANTS: &[&str] = &[
    "ABSPATH",
    "DB_NAME",
    "DB_USER",
    "DB_PASSWORD",
    "DB_HOST",
    "DB_CHARSET",
    "DB_COLLATE",
    "AUTH_KEY",
    "SECURE_AUTH_KEY",
    "LOGGED_IN_KEY",
    "NONCE_KEY",
    "AUTH_SALT",
    "SECURE_AUTH_SALT",
    "LOGGED_IN_SALT",
    "NONCE_SALT",
    "WP_HOME",
    "WP_SITEURL",
    "table_prefix",
];

/// Default plugins shipped with WordPress — skip unless explicitly activated.
const DEFAULT_PLUGINS: &[&str] = &["akismet", "hello"];

/// Default themes shipped with WordPress — skip these.
const DEFAULT_THEME_PREFIXES: &[&str] = &["twentytwenty", "twentynineteen", "twentyseventeen"];

impl BlueprintGenerator {
    pub fn new(project_path: PathBuf) -> Self {
        Self { project_path }
    }

    /// Generate a blueprint JSON value by introspecting the current WordPress installation.
    pub fn generate(&self) -> Result<serde_json::Value, BlueprintError> {
        println!("  Detecting PHP version...");
        let php_version = self.get_php_version();

        println!("  Detecting WordPress version...");
        let wp_version = self.run_wp_cli_output(&["core", "version"]);

        println!("  Listing plugins...");
        let plugins = self.get_plugins()?;

        println!("  Listing themes...");
        let themes = self.get_themes()?;

        println!("  Reading site options...");
        let site_options = self.get_site_options()?;

        println!("  Reading wp-config constants...");
        let constants = self.get_custom_constants()?;

        println!("  Detecting site language...");
        let language = self.run_wp_cli_output(&["option", "get", "WPLANG"]);

        // Build the blueprint
        let mut bp = serde_json::Map::new();

        bp.insert(
            "$schema".to_string(),
            serde_json::Value::String(
                "https://playground.wordpress.net/blueprint-schema.json".to_string(),
            ),
        );

        // Meta
        let project_name = self
            .project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("wordpress");
        let blogname = site_options
            .get("blogname")
            .and_then(|v| v.as_str())
            .unwrap_or(project_name);

        let mut meta = serde_json::Map::new();
        meta.insert("title".to_string(), serde_json::json!(blogname));
        meta.insert(
            "description".to_string(),
            serde_json::json!(format!(
                "Blueprint generated from {}",
                project_name
            )),
        );
        bp.insert("meta".to_string(), serde_json::Value::Object(meta));

        bp.insert(
            "landingPage".to_string(),
            serde_json::Value::String("/wp-admin/".to_string()),
        );

        // Preferred versions
        let mut versions = serde_json::Map::new();
        if let Some(ref php) = php_version {
            versions.insert("php".to_string(), serde_json::Value::String(php.clone()));
        }
        if let Some(ref wp) = wp_version {
            let trimmed = wp.trim().to_string();
            if !trimmed.is_empty() {
                versions.insert("wp".to_string(), serde_json::Value::String(trimmed));
            }
        }
        if !versions.is_empty() {
            bp.insert(
                "preferredVersions".to_string(),
                serde_json::Value::Object(versions),
            );
        }

        bp.insert(
            "features".to_string(),
            serde_json::json!({ "networking": true }),
        );

        // Build steps array
        let mut steps: Vec<serde_json::Value> = Vec::new();

        // Login step
        steps.push(serde_json::json!({
            "step": "login",
            "username": "admin"
        }));

        // Install and activate plugins
        for plugin in &plugins {
            if DEFAULT_PLUGINS.contains(&plugin.name.as_str()) {
                continue;
            }
            let activate = plugin.status == "active" || plugin.status == "active-network";
            steps.push(serde_json::json!({
                "step": "installPlugin",
                "pluginData": {
                    "resource": "wordpress.org/plugins",
                    "slug": plugin.name
                },
                "options": {
                    "activate": activate
                }
            }));
        }

        // Install and activate non-default themes
        for theme in &themes {
            let is_default = DEFAULT_THEME_PREFIXES
                .iter()
                .any(|prefix| theme.name.starts_with(prefix));
            if is_default && theme.status != "active" {
                continue;
            }
            let activate = theme.status == "active";
            steps.push(serde_json::json!({
                "step": "installTheme",
                "themeData": {
                    "resource": "wordpress.org/themes",
                    "slug": theme.name
                },
                "options": {
                    "activate": activate
                }
            }));
        }

        // Site language
        if let Some(ref lang) = language {
            let lang = lang.trim();
            if !lang.is_empty() && lang != "en_US" {
                steps.push(serde_json::json!({
                    "step": "setSiteLanguage",
                    "language": lang
                }));
            }
        }

        // Site options (filtered to commonly customized ones)
        let mut custom_options = serde_json::Map::new();
        for (key, value) in &site_options {
            match key.as_str() {
                "blogname" | "blogdescription" | "permalink_structure" | "timezone_string"
                | "date_format" | "time_format" | "start_of_week" | "posts_per_page"
                | "default_comment_status" | "default_ping_status" | "show_on_front"
                | "page_on_front" | "page_for_posts" => {
                    custom_options.insert(key.clone(), value.clone());
                }
                _ => {}
            }
        }
        if !custom_options.is_empty() {
            steps.push(serde_json::json!({
                "step": "setSiteOptions",
                "options": serde_json::Value::Object(custom_options)
            }));
        }

        // Custom wp-config constants
        if !constants.is_empty() {
            let consts: serde_json::Map<String, serde_json::Value> = constants
                .into_iter()
                .map(|(k, v)| {
                    let val = match v.as_str() {
                        "true" => serde_json::Value::Bool(true),
                        "false" => serde_json::Value::Bool(false),
                        _ => {
                            if let Ok(n) = v.parse::<i64>() {
                                serde_json::Value::Number(n.into())
                            } else {
                                serde_json::Value::String(v)
                            }
                        }
                    };
                    (k, val)
                })
                .collect();
            steps.push(serde_json::json!({
                "step": "defineWpConfigConsts",
                "consts": serde_json::Value::Object(consts)
            }));
        }

        bp.insert("steps".to_string(), serde_json::Value::Array(steps));

        Ok(serde_json::Value::Object(bp))
    }

    // -----------------------------------------------------------------------
    // WP-CLI introspection helpers
    // -----------------------------------------------------------------------

    fn get_php_version(&self) -> Option<String> {
        // Read from .ddev/config.yaml
        let config_path = self.project_path.join(".ddev/config.yaml");
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            for line in content.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("php_version:") {
                    let ver = rest.trim().trim_matches('"').trim_matches('\'');
                    if !ver.is_empty() {
                        return Some(ver.to_string());
                    }
                }
            }
        }
        None
    }

    fn get_plugins(&self) -> Result<Vec<WpPlugin>, BlueprintError> {
        let output = self.run_wp_cli_output(&["plugin", "list", "--format=json"]);
        match output {
            Some(json_str) => {
                let plugins: Vec<WpPlugin> = serde_json::from_str(&json_str)
                    .map_err(|e| BlueprintError::Parse(format!("Failed to parse plugin list: {}", e)))?;
                // Filter to installed plugins only (not must-use, dropin)
                Ok(plugins
                    .into_iter()
                    .filter(|p| {
                        p.status == "active"
                            || p.status == "inactive"
                            || p.status == "active-network"
                    })
                    .collect())
            }
            None => Ok(Vec::new()),
        }
    }

    fn get_themes(&self) -> Result<Vec<WpTheme>, BlueprintError> {
        let output = self.run_wp_cli_output(&["theme", "list", "--format=json"]);
        match output {
            Some(json_str) => {
                let themes: Vec<WpTheme> = serde_json::from_str(&json_str)
                    .map_err(|e| BlueprintError::Parse(format!("Failed to parse theme list: {}", e)))?;
                Ok(themes)
            }
            None => Ok(Vec::new()),
        }
    }

    fn get_site_options(&self) -> Result<HashMap<String, serde_json::Value>, BlueprintError> {
        let mut options = HashMap::new();
        let keys = [
            "blogname",
            "blogdescription",
            "permalink_structure",
            "timezone_string",
            "date_format",
            "time_format",
            "start_of_week",
            "posts_per_page",
            "default_comment_status",
            "default_ping_status",
            "show_on_front",
            "page_on_front",
            "page_for_posts",
            "WPLANG",
        ];
        for key in &keys {
            if let Some(value) = self.run_wp_cli_output(&["option", "get", key]) {
                let value = value.trim().to_string();
                if !value.is_empty() {
                    options.insert(key.to_string(), serde_json::Value::String(value));
                }
            }
        }
        Ok(options)
    }

    fn get_custom_constants(&self) -> Result<HashMap<String, String>, BlueprintError> {
        let output = self.run_wp_cli_output(&["config", "list", "--format=json"]);
        match output {
            Some(json_str) => {
                let entries: Vec<WpConfigEntry> = serde_json::from_str(&json_str)
                    .unwrap_or_default();
                let mut constants = HashMap::new();
                for entry in entries {
                    if entry.entry_type != "constant" {
                        continue;
                    }
                    if SKIP_CONSTANTS.contains(&entry.name.as_str()) {
                        continue;
                    }
                    // Skip constants with empty values
                    if entry.value.is_empty() {
                        continue;
                    }
                    constants.insert(entry.name, entry.value);
                }
                Ok(constants)
            }
            None => Ok(HashMap::new()),
        }
    }

    /// Run a WP-CLI command via DDEV and capture stdout.
    fn run_wp_cli_output(&self, args: &[&str]) -> Option<String> {
        let mut ddev_args = vec!["exec", "wp"];
        ddev_args.extend_from_slice(args);

        let output = Command::new("ddev")
            .args(&ddev_args)
            .current_dir(&self.project_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            if stdout.trim().is_empty() {
                None
            } else {
                Some(stdout.trim().to_string())
            }
        } else {
            None
        }
    }
}
