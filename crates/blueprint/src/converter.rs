//! WordPress Playground JSON → generic blueprint converter.
//!
//! Converts a Playground blueprint (identified by its `$schema` URL or
//! top-level `steps` / `plugins` shape) into a framework-neutral
//! `GenericBlueprint` that the rest of appz-cli can execute without knowing
//! anything about WordPress Playground internals.

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// Framework-neutral blueprint produced by the converter.
#[derive(Debug, Default, Clone)]
pub struct GenericBlueprint {
    pub meta: Option<GenericMeta>,
    pub setup: Option<Vec<GenericSetupStep>>,
}

#[derive(Debug, Default, Clone)]
pub struct GenericMeta {
    pub name: Option<String>,
    pub framework: Option<String>,
}

/// A single setup step in the generic blueprint.
#[derive(Debug, Default, Clone)]
pub struct GenericSetupStep {
    pub desc: Option<String>,
    pub run_locally: Option<String>,
    pub write_file: Option<GenericWriteFile>,
    pub mkdir: Option<String>,
    pub add_dependency: Option<String>,
    pub dev: Option<bool>,
    pub set_env: Option<HashMap<String, String>>,
    pub cd: Option<String>,
    pub rm: Option<String>,
    pub cp: Option<GenericCopy>,
    pub patch_file: Option<GenericPatchFile>,
    pub once: Option<bool>,
}

#[derive(Debug, Default, Clone)]
pub struct GenericWriteFile {
    pub path: String,
    pub content: Option<String>,
    pub template: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct GenericCopy {
    pub src: String,
    pub dest: String,
}

#[derive(Debug, Default, Clone)]
pub struct GenericPatchFile {
    pub path: String,
    pub after: Option<String>,
    pub before: Option<String>,
    pub replace: Option<String>,
    pub content: Option<String>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ConvertError(pub String);

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "playground converter: {}", self.0)
    }
}

impl std::error::Error for ConvertError {}

pub type Result<T> = std::result::Result<T, ConvertError>;

// ---------------------------------------------------------------------------
// Playground input types (private — only used for parsing)
// ---------------------------------------------------------------------------

/// Minimal Playground blueprint shape used for conversion.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaygroundBlueprint {
    #[serde(rename = "$schema", default)]
    schema: Option<String>,

    #[serde(default)]
    meta: Option<PlaygroundMeta>,

    #[serde(default)]
    plugins: Vec<Value>,

    #[serde(default)]
    site_options: Option<HashMap<String, Value>>,

    #[serde(default)]
    steps: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaygroundMeta {
    #[serde(default)]
    title: Option<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Heuristic: does this JSON look like a WordPress Playground blueprint?
///
/// Returns `true` when the raw string parses as JSON **and** either:
/// - Contains the Playground schema URL, or
/// - Has a top-level `steps` array together with `plugins` or `siteOptions`.
pub fn is_playground_blueprint(raw: &str) -> bool {
    let Ok(v): std::result::Result<Value, _> = serde_json::from_str(raw) else {
        return false;
    };

    // Definitive signal: the schema URL.
    if let Some(schema) = v.get("$schema").and_then(Value::as_str) {
        if schema.contains("playground.wordpress.net") {
            return true;
        }
    }

    // Secondary signal: has `steps` array + at least one of the WP-specific keys.
    let has_steps = v.get("steps").and_then(Value::as_array).is_some();
    let has_wp_key = v.get("plugins").is_some()
        || v.get("siteOptions").is_some()
        || v.get("preferredVersions").is_some();

    has_steps && has_wp_key
}

/// Convert a Playground JSON string into a `GenericBlueprint`.
pub fn convert_playground_to_generic(raw: &str) -> Result<GenericBlueprint> {
    let pg: PlaygroundBlueprint = serde_json::from_str(raw)
        .map_err(|e| ConvertError(format!("JSON parse error: {e}")))?;

    let mut steps: Vec<GenericSetupStep> = Vec::new();

    // ------------------------------------------------------------------
    // 1. Top-level `plugins` shorthand → wp plugin install
    // ------------------------------------------------------------------
    for plugin in &pg.plugins {
        let slug = plugin_slug_from_value(plugin);
        steps.push(run_locally(format!("wp plugin install {slug} --activate")));
    }

    // ------------------------------------------------------------------
    // 2. Top-level `siteOptions` shorthand → wp option update
    // ------------------------------------------------------------------
    if let Some(opts) = &pg.site_options {
        for (key, val) in opts {
            let value_str = json_value_to_cli_arg(val);
            steps.push(run_locally(format!("wp option update {key} {value_str}")));
        }
    }

    // ------------------------------------------------------------------
    // 3. `steps` array
    // ------------------------------------------------------------------
    for step_val in &pg.steps {
        if let Some(converted) = convert_step(step_val) {
            steps.extend(converted);
        }
    }

    // ------------------------------------------------------------------
    // 4. Meta
    // ------------------------------------------------------------------
    let name = pg
        .meta
        .as_ref()
        .and_then(|m| m.title.clone())
        .or_else(|| {
            pg.schema.as_deref().map(|_| String::from("WordPress Playground"))
        });

    let meta = GenericMeta {
        name,
        framework: Some(String::from("wordpress")),
    };

    Ok(GenericBlueprint {
        meta: Some(meta),
        setup: if steps.is_empty() { None } else { Some(steps) },
    })
}

// ---------------------------------------------------------------------------
// Step converter
// ---------------------------------------------------------------------------

/// Shared helper for installPlugin / installTheme conversion.
fn convert_wp_install(
    obj: &serde_json::Map<String, Value>,
    wp_type: &str,
    data_key: &str,
    zip_key: &str,
) -> Vec<GenericSetupStep> {
    let slug = slug_from_resource(obj.get(data_key))
        .or_else(|| slug_from_resource(obj.get(zip_key)))
        .unwrap_or_else(|| format!("unknown-{wp_type}"));
    let activate = obj
        .get("options")
        .and_then(|o| o.get("activate"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let flag = if activate { " --activate" } else { "" };
    vec![run_locally(format!("wp {wp_type} install {slug}{flag}"))]
}

fn convert_step(val: &Value) -> Option<Vec<GenericSetupStep>> {
    let obj = val.as_object()?;
    let step_type = obj.get("step")?.as_str()?;

    let result: Vec<GenericSetupStep> = match step_type {
        "installPlugin" => convert_wp_install(obj, "plugin", "pluginData", "pluginZipFile"),
        "installTheme" => convert_wp_install(obj, "theme", "themeData", "themeZipFile"),

        "activatePlugin" => {
            // pluginPath is like "woocommerce/woocommerce.php" — take the folder name
            let path = obj.get("pluginPath").and_then(Value::as_str).unwrap_or("unknown");
            let slug = path.split('/').next().unwrap_or(path);
            vec![run_locally(format!("wp plugin activate {slug}"))]
        }
        "activateTheme" => {
            let folder = obj.get("themeFolderName").and_then(Value::as_str).unwrap_or("unknown");
            vec![run_locally(format!("wp theme activate {folder}"))]
        }

        // --- setSiteOptions --------------------------------------------------
        "setSiteOptions" => {
            let opts = obj.get("options")?.as_object()?;
            opts.iter()
                .map(|(k, v)| {
                    let value_str = json_value_to_cli_arg(v);
                    run_locally(format!("wp option update {k} {value_str}"))
                })
                .collect()
        }

        // --- setSiteLanguage -------------------------------------------------
        "setSiteLanguage" => {
            let lang = obj
                .get("language")
                .and_then(Value::as_str)
                .unwrap_or("en_US");
            vec![run_locally(format!(
                "wp language core install {lang} --activate"
            ))]
        }

        // --- writeFile -------------------------------------------------------
        "writeFile" => {
            let path_raw = obj.get("path").and_then(Value::as_str).unwrap_or("");
            let path = strip_wordpress_prefix(path_raw);
            let content = obj
                .get("data")
                .and_then(|d| {
                    if d.is_string() {
                        d.as_str().map(str::to_owned)
                    } else {
                        None
                    }
                });
            let mut s = GenericSetupStep::default();
            s.write_file = Some(GenericWriteFile {
                path,
                content,
                template: None,
            });
            vec![s]
        }

        // --- mkdir -----------------------------------------------------------
        "mkdir" => {
            let path_raw = obj.get("path").and_then(Value::as_str).unwrap_or("");
            let path = strip_wordpress_prefix(path_raw);
            let mut s = GenericSetupStep::default();
            s.mkdir = Some(path);
            vec![s]
        }

        // --- wp-cli ----------------------------------------------------------
        "wp-cli" => {
            let cmd = wpcli_command_string(obj.get("command")?);
            // Ensure the command starts with "wp "
            let full = if cmd.starts_with("wp ") || cmd == "wp" {
                cmd
            } else {
                format!("wp {cmd}")
            };
            vec![run_locally(full)]
        }

        // --- runSql ----------------------------------------------------------
        "runSql" => {
            // The SQL is typically a resource; we map it to wp db query.
            vec![run_locally(String::from("wp db query"))]
        }

        // --- runPHP ----------------------------------------------------------
        "runPHP" => {
            // Map to wp eval; the actual PHP code is not easily portable.
            vec![run_locally(String::from("wp eval"))]
        }

        // --- cp --------------------------------------------------------------
        "cp" => {
            let from = obj
                .get("fromPath")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let to = obj
                .get("toPath")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let mut s = GenericSetupStep::default();
            s.cp = Some(GenericCopy { src: from, dest: to });
            vec![s]
        }

        // --- mv --------------------------------------------------------------
        "mv" => {
            let from = obj
                .get("fromPath")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let to = obj
                .get("toPath")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            vec![run_locally(format!("mv {from} {to}"))]
        }

        // --- rm / rmdir ------------------------------------------------------
        "rm" | "rmdir" => {
            let path = obj
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let mut s = GenericSetupStep::default();
            s.rm = Some(path);
            vec![s]
        }

        // --- Unrecognised step — skip ----------------------------------------
        _ => vec![],
    };

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a step that runs a shell command locally.
fn run_locally(cmd: impl Into<String>) -> GenericSetupStep {
    GenericSetupStep {
        run_locally: Some(cmd.into()),
        ..Default::default()
    }
}

/// Strip the `/wordpress/` or `/wordpress` prefix that Playground uses.
fn strip_wordpress_prefix(path: &str) -> String {
    path.strip_prefix("/wordpress/")
        .or_else(|| path.strip_prefix("/wordpress"))
        .unwrap_or(path)
        .to_owned()
}

/// Extract the slug string from a plugin/theme shorthand value.
fn plugin_slug_from_value(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        return s.to_owned();
    }
    // Object with a `slug` field
    v.get("slug")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned()
}

/// Extract a slug from a resource object (`pluginData` / `themeData`).
fn slug_from_resource(v: Option<&Value>) -> Option<String> {
    let v = v?;
    // Direct slug field
    if let Some(s) = v.get("slug").and_then(Value::as_str) {
        return Some(s.to_owned());
    }
    // URL resource — derive slug from the URL last segment
    if let Some(url) = v.get("url").and_then(Value::as_str) {
        let slug = url.split('/').last().unwrap_or(url);
        return Some(slug.to_owned());
    }
    None
}

/// Convert a JSON value to a CLI-friendly string (quoted if contains spaces).
fn json_value_to_cli_arg(v: &Value) -> String {
    match v {
        Value::String(s) => {
            if s.contains(' ') {
                format!("\"{s}\"")
            } else {
                s.clone()
            }
        }
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => format!("'{other}'"),
    }
}

/// Convert a `wp-cli` command value (string or array) to a string.
fn wpcli_command_string(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        return s.to_owned();
    }
    if let Some(arr) = v.as_array() {
        return arr
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" ");
    }
    String::new()
}
