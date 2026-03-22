use itertools::Itertools;
use json_comments::StripComments;
use miette::{miette, Result};
use serde::Deserialize;
use starbase_utils::fs;
use std::{
    collections::HashMap,
    io::Read as IoRead,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

use crate::host::HostRegistry;
use crate::shell::{copy_path_recursive, run_local_with, test_local, RunOptions};
use crate::ssh::{RemoteRunOptions, SshClient};
use std::sync::Arc;
use task::{types::AsyncTaskFn, Context, Task, TaskRegistry};
use ui::prompt;

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum TaskDef {
    Steps(Vec<Step>),
    Deps(Vec<String>),
    WithDepends {
        #[serde(default)]
        depends: Vec<String>, // NEW: inline depends for step-based tasks
        #[serde(default)]
        sources: Vec<String>,
        #[serde(default)]
        outputs: Vec<String>,
        steps: Vec<Step>,
    },
    WithWaitFor {
        #[serde(default)]
        wait_for: Vec<String>,
        #[serde(default)]
        sources: Vec<String>,
        #[serde(default)]
        outputs: Vec<String>,
        steps: Vec<Step>,
    },
    WithWaitForDeps {
        #[serde(default)]
        wait_for: Vec<String>,
        depends: Vec<String>,
    },
    WithDependsAndWaitFor {
        #[serde(default)]
        depends: Vec<String>, // NEW: inline depends
        #[serde(default)]
        wait_for: Vec<String>,
        #[serde(default)]
        sources: Vec<String>,
        #[serde(default)]
        outputs: Vec<String>,
        steps: Vec<Step>,
    },
    WithSourcesOutputs {
        #[serde(default)]
        sources: Vec<String>,
        #[serde(default)]
        outputs: Vec<String>,
        steps: Vec<Step>,
    },
}

#[derive(Deserialize, Debug, Clone)]
struct AskConfig {
    pub message: String,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub choices: Option<Vec<String>>, // For choice prompts (if None, text input)
    #[serde(default)]
    pub hidden: Option<bool>, // For password inputs (hidden text)
    pub var: String, // Variable name to store result
}

#[derive(Deserialize, Debug, Default, Clone)]
struct UploadDef {
    src: serde_json::Value,
    dest: String,
}

#[derive(Deserialize, Debug, Default, Clone)]
struct DownloadDef {
    src: String,
    dest: String,
}

#[derive(Deserialize, Debug, Default, Clone)]
struct WriteFileDef {
    path: String,
    content: Option<String>,
    template: Option<String>,
}

#[derive(Deserialize, Debug, Default, Clone)]
struct PatchFileDef {
    path: String,
    after: Option<String>,
    before: Option<String>,
    replace: Option<String>,
    content: String,
}

#[derive(Deserialize, Debug, Default, Clone)]
struct CopyDef {
    src: String,
    dest: String,
}

#[derive(Deserialize, Debug, Default, Clone)]
struct Step {
    #[serde(default)]
    cd: Option<String>,
    #[serde(default)]
    run: Option<String>, // Remote command execution
    #[serde(default)]
    run_locally: Option<String>, // Local command execution
    #[serde(default)]
    host: Option<String>, // Host name for remote execution
    #[serde(default)]
    upload: Option<UploadDef>,
    #[serde(default)]
    download: Option<DownloadDef>,
    #[serde(default)]
    desc: Option<String>,
    #[serde(default)]
    once: Option<bool>,
    #[serde(default)]
    hidden: Option<bool>,
    #[serde(default)]
    add_dependency: Option<Vec<String>>,
    #[serde(default)]
    dev: Option<bool>,
    #[serde(default)]
    write_file: Option<WriteFileDef>,
    #[serde(default)]
    patch_file: Option<PatchFileDef>,
    #[serde(default)]
    set_env: Option<HashMap<String, String>>,
    #[serde(default)]
    mkdir: Option<String>,
    #[serde(default)]
    cp: Option<CopyDef>,
    #[serde(default)]
    rm: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
struct FileSchema {
    #[serde(default)]
    config: serde_json::Value,
    #[serde(default)]
    hosts: serde_json::Value,
    #[serde(default)]
    tools: serde_json::Value,
    #[serde(default)]
    tasks: HashMap<String, TaskDefWithMetadata>,
    #[serde(default)]
    before: HashMap<String, Vec<String>>, // target -> hooks
    #[serde(default)]
    after: HashMap<String, Vec<String>>, // target -> hooks
    #[serde(default)]
    includes: Option<Vec<PathBuf>>, // Top-level includes field
    #[serde(default)]
    setup: Vec<Step>,
}

#[derive(Deserialize, Debug)]
struct TaskDefWithMetadata {
    #[serde(flatten)]
    task_def: TaskDef,
    #[serde(default)]
    confirm: Option<String>, // Confirmation message before task execution
    #[serde(default)]
    ask: Option<AskConfig>, // Interactive prompt configuration
    #[serde(default)]
    only_if: Option<String>, // Condition command (execute task if command succeeds)
    #[serde(default)]
    unless: Option<String>, // Condition command (skip task if command succeeds)
}

// Adapted from mise: src/config/mod.rs:1403-1411
fn default_task_includes() -> Vec<PathBuf> {
    vec![PathBuf::from(".appz").join("tasks")]
}

// From mise: src/config/mod.rs:1437-1452 (adapted)
fn prefix_monorepo_task_names(tasks: &mut [Task], dir: &Path, monorepo_root: &Path) {
    const MONOREPO_PATH_PREFIX: &str = "//";
    const MONOREPO_TASK_SEPARATOR: &str = ":";

    if let Ok(rel_path) = dir.strip_prefix(monorepo_root) {
        let prefix = rel_path
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        for task in tasks.iter_mut() {
            task.name = format!(
                "{}{}{}{}",
                MONOREPO_PATH_PREFIX, prefix, MONOREPO_TASK_SEPARATOR, task.name
            );
        }
    }
}

// Adapted from mise: src/config/mod.rs:1785-1798
fn task_includes_for_dir(dir: &Path, schema: &FileSchema) -> Vec<PathBuf> {
    schema
        .includes
        .clone()
        .unwrap_or_else(default_task_includes)
        .into_iter()
        .map(|p| if p.is_absolute() { p } else { dir.join(p) })
        .filter(|p| p.exists())
        .collect::<Vec<_>>()
        .into_iter()
        .unique()
        .collect::<Vec<_>>()
}

fn parse_file_schema<P: AsRef<Path> + std::fmt::Debug>(path: P) -> Result<FileSchema> {
    let raw = fs::read_file(&path).map_err(|e| {
        miette!(
            "Failed to read blueprint file {}: {}",
            path.as_ref().display(),
            e
        )
    })?;
    let ext = path
        .as_ref()
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Try parsing as recipe FileSchema first
    let recipe_result: std::result::Result<FileSchema, _> = if ext == "yaml" || ext == "yml" {
        serde_yaml::from_str(&raw).map_err(|e| e.to_string())
    } else if ext == "jsonc" {
        let stripped = StripComments::new(raw.as_bytes());
        let mut json_str = String::new();
        if let Err(e) = std::io::BufReader::new(stripped).read_to_string(&mut json_str) {
            return Err(miette!("Failed to strip comments: {}", e));
        }
        serde_json::from_str(&json_str).map_err(|e| e.to_string())
    } else {
        serde_json::from_str(&raw).map_err(|e| e.to_string())
    };

    if let Ok(schema) = recipe_result {
        return Ok(schema);
    }

    // Recipe parse failed — try as BlueprintSchema and convert tasks
    convert_blueprint_to_file_schema(&raw, &ext, path.as_ref())
}

/// Convert a BlueprintSchema file into the recipe-compatible FileSchema.
///
/// Blueprint tasks are simple `Vec<Step>` arrays, while FileSchema expects
/// `TaskDefWithMetadata` wrappers. This converts between the two formats.
fn convert_blueprint_to_file_schema(raw: &str, ext: &str, path: &Path) -> Result<FileSchema> {
    let mut schema_value: serde_json::Value = if ext == "yaml" || ext == "yml" {
        serde_yaml::from_str(raw)
            .map_err(|e| miette!("Invalid YAML in {}: {}", path.display(), e))?
    } else {
        serde_json::from_str(raw)
            .map_err(|e| miette!("Invalid JSON in {}: {}", path.display(), e))?
    };

    // Wrap simple task arrays as {steps: [...]} for TaskDefWithMetadata compat
    if let Some(tasks) = schema_value.get_mut("tasks") {
        if let Some(tasks_obj) = tasks.as_object_mut() {
            for (_name, task_val) in tasks_obj.iter_mut() {
                // If the task is a simple array of steps, wrap it
                if task_val.is_array() {
                    // Vec<Step> is already a valid TaskDef::Steps variant —
                    // the issue is TaskDefWithMetadata expects a struct with flatten.
                    // Wrap the array as {"steps": [...]} so the WithDepends variant matches.
                    let steps = task_val.take();
                    *task_val = serde_json::json!({"steps": steps});
                }
            }
        }
    }

    // Remove blueprint-only fields that FileSchema doesn't know
    if let Some(obj) = schema_value.as_object_mut() {
        obj.remove("version");
        obj.remove("meta");
    }

    // Now parse the modified value as FileSchema
    serde_json::from_value(schema_value)
        .map_err(|e| miette!("Failed to convert blueprint to task format in {}: {}", path.display(), e))
}

fn validate_file_schema(schema: &FileSchema) -> Result<Vec<String>> {
    let mut warnings: Vec<String> = Vec::new();
    // config must be object (serde_json::Value::Null allowed, handled by importer)
    if !(schema.config.is_null() || schema.config.is_object()) {
        return Err(miette!("config must be a map/object"));
    }
    // tools must be object (serde_json::Value::Null allowed, handled by importer)
    if !(schema.tools.is_null() || schema.tools.is_object()) {
        return Err(miette!("tools must be a map/object"));
    }
    // Blueprint must have either tasks or setup
    if schema.tasks.is_empty() && schema.setup.is_empty() {
        return Err(miette!("Blueprint must have either 'tasks' or 'setup' section"));
    }
    // validate tasks
    for (name, def) in &schema.tasks {
        match &def.task_def {
            TaskDef::Deps(deps) => {
                if deps.is_empty() {
                    warnings.push(format!("Task '{}' has empty dependency list", name));
                }
            }
            TaskDef::Steps(steps) => {
                if steps.is_empty() {
                    return Err(miette!("Task '{}' has an empty steps list", name));
                }
                validate_steps(name, steps, &mut warnings)?;
            }
            TaskDef::WithWaitFor { steps, .. } => {
                if steps.is_empty() {
                    return Err(miette!("Task '{}' has an empty steps list", name));
                }
                validate_steps(name, steps, &mut warnings)?;
            }
            TaskDef::WithWaitForDeps { depends, .. } => {
                if depends.is_empty() {
                    warnings.push(format!("Task '{}' has empty dependency list", name));
                }
            }
            TaskDef::WithDepends { steps, .. } => {
                if steps.is_empty() {
                    return Err(miette!("Task '{}' has an empty steps list", name));
                }
                validate_steps(name, steps, &mut warnings)?;
            }
            TaskDef::WithDependsAndWaitFor { steps, .. } => {
                if steps.is_empty() {
                    return Err(miette!("Task '{}' has an empty steps list", name));
                }
                validate_steps(name, steps, &mut warnings)?;
            }
            TaskDef::WithSourcesOutputs { steps, .. } => {
                if steps.is_empty() {
                    return Err(miette!("Task '{}' has an empty steps list", name));
                }
                validate_steps(name, steps, &mut warnings)?;
            }
        }
    }
    // Check placeholders vs config
    if let Some(cfg) = schema.config.as_object() {
        let known: std::collections::HashSet<&str> = cfg.keys().map(|k| k.as_str()).collect();
        let re = regex::Regex::new(r"\{\{\s*([a-zA-Z0-9_:\-]+)\s*\}\}")
            .map_err(|e| miette!("Failed to compile regex pattern: {}", e))?;
        for def in schema.tasks.values() {
            let steps = match &def.task_def {
                TaskDef::Steps(steps) => Some(steps),
                TaskDef::WithWaitFor { steps, .. } => Some(steps),
                TaskDef::WithDepends { steps, .. } => Some(steps),
                TaskDef::WithDependsAndWaitFor { steps, .. } => Some(steps),
                TaskDef::WithSourcesOutputs { steps, .. } => Some(steps),
                _ => None,
            };
            if let Some(steps) = steps {
                for s in steps {
                    let check = |text: &str, warnings: &mut Vec<String>| {
                        for cap in re.captures_iter(text) {
                            if let Some(key_match) = cap.get(1) {
                                let key = key_match.as_str();
                                if !known.contains(key) {
                                    warnings.push(format!(
                                        "Placeholder '{{{{{}}}}}' not found in config",
                                        key
                                    ));
                                }
                            }
                        }
                    };
                    if let Some(cd) = &s.cd {
                        check(cd, &mut warnings);
                    }
                    if let Some(run) = &s.run {
                        check(run, &mut warnings);
                    }
                    if let Some(rl) = &s.run_locally {
                        check(rl, &mut warnings);
                    }
                    if let Some(u) = &s.upload {
                        if let Some(src_str) = u.src.as_str() {
                            check(src_str, &mut warnings);
                        }
                        if let Some(arr) = u.src.as_array() {
                            for v in arr {
                                if let Some(sv) = v.as_str() {
                                    check(sv, &mut warnings);
                                }
                            }
                        }
                        check(&u.dest, &mut warnings);
                    }
                    if let Some(d) = &s.download {
                        check(&d.src, &mut warnings);
                        check(&d.dest, &mut warnings);
                    }
                }
            }
        }
    }
    Ok(warnings)
}

fn create_task_from_steps(
    name: String,
    steps: Vec<Step>,
    sources: Vec<String>,
    outputs: Vec<String>,
    host_registry: &HostRegistry,
    has_hosts: bool,
) -> Task {
    let mut description: Option<String> = None;
    let mut once = false;
    let mut hidden = false;
    for s in &steps {
        if s.desc.is_some() {
            description = s.desc.clone();
        }
        if s.once.unwrap_or(false) {
            once = true;
        }
        if s.hidden.unwrap_or(false) {
            hidden = true;
        }
    }

    // Make host registry static for use in closure
    let host_registry_static: &'static HostRegistry = Box::leak(Box::new(host_registry.clone()));
    let steps_static: &'static [Step] = Box::leak(steps.into_boxed_slice());
    let action = task::task_fn_async!(|ctx: std::sync::Arc<task::Context>| async move {
        let mut cwd_opt: Option<std::path::PathBuf> = None;
        let mut remote_cwd: Option<String> = None;
        for st in steps_static.iter() {
            // Handle working directory
            if let Some(path) = &st.cd {
                let parsed = ctx.parse(path);
                cwd_opt = Some(std::path::PathBuf::from(&parsed));
                remote_cwd = Some(parsed);
            }

            // Handle local execution
            if let Some(cmd) = &st.run_locally {
                let prepared = ctx.parse(cmd);
                let opts = RunOptions {
                    cwd: cwd_opt.clone(),
                    env: None,
                    show_output: true,
                    package_manager: None,
                    tool_info: None,
                };
                run_local_with(&ctx, &prepared, opts)
                    .await
                    .map_err(|e| miette!("Failed to run local command: {}", e))?;
            }

            // Handle remote execution (or local if no hosts configured)
            if let Some(cmd) = &st.run {
                // If host is explicitly specified, use remote execution
                // If no host specified and hosts are configured, default to remote
                // If no host specified and no hosts configured, execute locally
                if st.host.is_some() || has_hosts {
                    let host_name = st.host.as_deref().unwrap_or("default");
                    // Get the host config from static registry (safe to borrow multiple times)
                    let host_config = host_registry_static
                        .get(host_name)
                        .map_err(|e| miette!("Failed to get host '{}': {}", host_name, e))?
                        .clone();

                    let prepared = ctx.parse(cmd);
                    let client = SshClient::new(host_config);
                    let remote_opts = RemoteRunOptions {
                        cwd: remote_cwd.clone(),
                        env: None,
                        show_output: true,
                        timeout: None,
                    };
                    client.run_remote(&prepared, remote_opts).map_err(|e| {
                        miette!("Failed to run remote command on {}: {}", host_name, e)
                    })?;
                } else {
                    // No hosts configured, execute locally
                    let prepared = ctx.parse(cmd);
                    let opts = RunOptions {
                        cwd: cwd_opt.clone(),
                        env: None,
                        show_output: true,
                        package_manager: None,
                        tool_info: None,
                    };
                    run_local_with(&ctx, &prepared, opts)
                        .await
                        .map_err(|e| miette!("Failed to run local command: {}", e))?;
                }
            }
            if let Some(u) = &st.upload {
                // Local copy only: supports string or string array for src
                let dest = ctx.parse(&u.dest);
                match &u.src {
                    serde_json::Value::String(s) => {
                        copy_path_recursive(Path::new(&ctx.parse(s)), Path::new(&dest))
                            .map_err(|e| miette!("Failed to copy path: {}", e))?;
                    }
                    serde_json::Value::Array(arr) => {
                        for v in arr {
                            if let Some(s) = v.as_str() {
                                let parsed = ctx.parse(s);
                                let base_path = Path::new(&parsed);
                                let to = Path::new(&dest)
                                    .join(base_path.file_name().unwrap_or_default());
                                copy_path_recursive(base_path, &to)
                                    .map_err(|e| miette!("Failed to copy path: {}", e))?;
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Some(d) = &st.download {
                // Also local copy in reverse
                let src = ctx.parse(&d.src);
                let dest = ctx.parse(&d.dest);
                copy_path_recursive(Path::new(&src), Path::new(&dest))
                    .map_err(|e| miette!("Failed to copy path: {}", e))?;
            }
            if let Some(deps) = &st.add_dependency {
                let is_dev = st.dev.unwrap_or(false);
                let dep_list: Vec<String> = deps.iter().map(|d| ctx.parse(d)).collect();
                let dep_str = dep_list.join(" ");
                let flag = if is_dev { " --save-dev" } else { "" };
                let cmd = format!("npm install {}{}", dep_str, flag);
                let opts = RunOptions {
                    cwd: cwd_opt.clone(),
                    env: None,
                    show_output: true,
                    package_manager: None,
                    tool_info: None,
                };
                run_local_with(&ctx, &cmd, opts)
                    .await
                    .map_err(|e| miette!("Failed to add dependency: {}", e))?;
            }
            if let Some(wf) = &st.write_file {
                let file_path = ctx.parse(&wf.path);
                let content = if let Some(tmpl) = &wf.template {
                    ctx.parse(tmpl)
                } else if let Some(c) = &wf.content {
                    ctx.parse(c)
                } else {
                    String::new()
                };
                if let Some(parent) = std::path::Path::new(&file_path).parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| miette!("Failed to create directory for write_file: {}", e))?;
                    }
                }
                std::fs::write(&file_path, &content)
                    .map_err(|e| miette!("Failed to write file '{}': {}", file_path, e))?;
            }
            if let Some(env_map) = &st.set_env {
                // Find or create .env file in cwd
                let env_path = cwd_opt
                    .as_deref()
                    .map(|p| p.join(".env"))
                    .unwrap_or_else(|| std::path::PathBuf::from(".env"));
                let existing = std::fs::read_to_string(&env_path).unwrap_or_default();
                let mut lines: Vec<String> = existing
                    .lines()
                    .map(|l| l.to_string())
                    .collect();
                for (key, val) in env_map {
                    let key_parsed = ctx.parse(key);
                    let val_parsed = ctx.parse(val);
                    let new_line = format!("{}={}", key_parsed, val_parsed);
                    // Upsert: replace existing key or append
                    let prefix = format!("{}=", key_parsed);
                    let replaced = lines.iter().position(|l| l.starts_with(&prefix));
                    if let Some(idx) = replaced {
                        lines[idx] = new_line;
                    } else {
                        lines.push(new_line);
                    }
                }
                std::fs::write(&env_path, lines.join("\n") + "\n")
                    .map_err(|e| miette!("Failed to write .env: {}", e))?;
            }
            if let Some(dir) = &st.mkdir {
                let dir_path = ctx.parse(dir);
                std::fs::create_dir_all(&dir_path)
                    .map_err(|e| miette!("Failed to create directory '{}': {}", dir_path, e))?;
            }
            if let Some(cp) = &st.cp {
                let src = ctx.parse(&cp.src);
                let dest = ctx.parse(&cp.dest);
                copy_path_recursive(Path::new(&src), Path::new(&dest))
                    .map_err(|e| miette!("Failed to copy '{}' to '{}': {}", src, dest, e))?;
            }
            if let Some(path_str) = &st.rm {
                let target = ctx.parse(path_str);
                let p = std::path::Path::new(&target);
                if p.is_dir() {
                    std::fs::remove_dir_all(p)
                        .map_err(|e| miette!("Failed to remove directory '{}': {}", target, e))?;
                } else if p.exists() {
                    std::fs::remove_file(p)
                        .map_err(|e| miette!("Failed to remove file '{}': {}", target, e))?;
                }
            }
        }
        Ok(())
    });

    let mut t = Task::new(name.clone(), action);
    if let Some(d) = description {
        t = t.desc(d);
    }
    if once {
        t = t.once();
    }
    if hidden {
        t = t.hidden();
    }
    if !sources.is_empty() {
        t = t.sources(sources);
    }
    if !outputs.is_empty() {
        t = t.outputs(outputs);
    }
    t
}

/// Test a command condition using the task context for variable substitution
fn test_command(ctx: &Context, cmd: &str) -> bool {
    // Parse command to substitute variables
    let parsed_cmd = ctx.parse(cmd);
    // Execute command and check exit code
    test_local(&parsed_cmd)
}

/// Check if non-interactive mode is enabled (following mise pattern)
/// Checks environment variables and TTY status
fn is_non_interactive_mode() -> bool {
    std::env::var("APPZ_YES").is_ok()
        || std::env::var("APPZ_NO_INPUT").is_ok()
        || !atty::is(atty::Stream::Stdin)
}

/// Wrap a task with prompt handling (confirm and ask)
fn wrap_task_with_prompts(task: Task, confirm: Option<String>, ask: Option<AskConfig>) -> Task {
    let original_action = task.action.clone();
    let task_name = task.name.clone();
    let confirm_msg = confirm.clone();
    let ask_config = ask.clone();

    let wrapped_action: AsyncTaskFn = Arc::new(move |ctx: Arc<Context>| {
        let confirm_msg = confirm_msg.clone();
        let ask_config = ask_config.clone();
        let task_name = task_name.clone();
        let original_action = original_action.clone();

        Box::pin(async move {
            let is_non_interactive = is_non_interactive_mode();

            // Handle ask prompt first (before confirm)
            if let Some(ref ask_cfg) = ask_config {
                let var_name = &ask_cfg.var;
                let message = ctx.parse(&ask_cfg.message);

                let value = if is_non_interactive {
                    // Use default value or empty string in non-interactive mode
                    ask_cfg.default.as_deref().unwrap_or("").to_string()
                } else {
                    // Show appropriate prompt based on configuration
                    let result = if ask_cfg.hidden.unwrap_or(false) {
                        // Hidden input (password)
                        prompt::password(&message)
                    } else if let Some(ref choices) = ask_cfg.choices {
                        // Choice selection
                        let choices_refs: Vec<&str> = choices.iter().map(|s| s.as_str()).collect();
                        let default_idx = ask_cfg
                            .default
                            .as_ref()
                            .and_then(|d| choices.iter().position(|c| c == d));
                        prompt::choose(&message, &choices_refs, default_idx).map(|s| s.to_string())
                    } else {
                        // Text input
                        prompt::prompt(&message, ask_cfg.default.as_deref())
                    };

                    result.map_err(|e| miette!("Prompt failed for task '{}': {}", task_name, e))?
                };
                ctx.set(var_name, &value);
            }

            // Handle confirm prompt (following mise pattern - check right before execution)
            if let Some(ref confirm_msg) = confirm_msg {
                if !is_non_interactive {
                    let message = ctx.parse(confirm_msg);
                    match prompt::confirm(&message, false) {
                        Ok(true) => {
                            // User confirmed, continue
                        }
                        Ok(false) => {
                            // User cancelled - abort task (like mise returns "aborted by user")
                            return Err(miette!("aborted by user"));
                        }
                        Err(e) => {
                            return Err(miette!(
                                "Confirmation failed for task '{}': {}",
                                task_name,
                                e
                            ));
                        }
                    }
                }
                // In non-interactive mode, skip confirmation (default to yes, like mise)
            }

            // Execute original task action
            original_action(ctx).await
        })
    });

    // Create new task with wrapped action, preserving all other task properties
    let mut new_task = task.clone();
    new_task.action = wrapped_action;
    new_task
}

fn validate_steps(name: &str, steps: &[Step], warnings: &mut Vec<String>) -> Result<()> {
    for (idx, s) in steps.iter().enumerate() {
        // At least one actionable field recommended
        if s.run.is_none()
            && s.run_locally.is_none()
            && s.upload.is_none()
            && s.download.is_none()
            && s.cd.is_none()
            && s.desc.is_none()
            && s.add_dependency.is_none()
            && s.write_file.is_none()
            && s.patch_file.is_none()
            && s.set_env.is_none()
            && s.mkdir.is_none()
            && s.cp.is_none()
            && s.rm.is_none()
            && !s.once.unwrap_or(false)
            && !s.hidden.unwrap_or(false)
        {
            warnings.push(format!("Task '{}' step #{} is empty", name, idx + 1));
        }
        if let Some(u) = &s.upload {
            if u.dest.trim().is_empty() {
                return Err(miette!(
                    "Task '{}' step #{} upload.dest is required",
                    name,
                    idx + 1
                ));
            }
            if u.src.is_null() {
                return Err(miette!(
                    "Task '{}' step #{} upload.src is required",
                    name,
                    idx + 1
                ));
            }
        }
        if let Some(d) = &s.download {
            if d.dest.trim().is_empty() || d.src.trim().is_empty() {
                return Err(miette!(
                    "Task '{}' step #{} download.src/dest are required",
                    name,
                    idx + 1
                ));
            }
        }
    }
    Ok(())
}

pub fn validate_file<P: AsRef<Path> + std::fmt::Debug>(path: P) -> Result<()> {
    let schema = parse_file_schema(&path)?;
    let warnings = validate_file_schema(&schema)?;
    for w in warnings {
        crate::log::warning(&w);
    }
    Ok(())
}

// From mise: src/config/mod.rs:1722-1760 (copied and adapted for YAML/JSON)
fn load_tasks_includes(
    root: &Path,
    _config_root: &Path,
    registry: &mut TaskRegistry,
) -> Result<()> {
    if root.is_file() {
        // Load single blueprint file (YAML/JSON/JSONC instead of TOML)
        if let Some(ext) = root.extension().and_then(|e| e.to_str()) {
            if ext == "yaml" || ext == "yml" || ext == "json" || ext == "jsonc" {
                import_file(root, registry)?;
            }
        }
    } else if root.is_dir() {
        // Load all YAML/JSON files from directory (mise uses executable files, we use YAML/JSON)
        // Copied from mise: src/config/mod.rs:1730-1746
        // Copied from mise: src/config/mod.rs:1730-1737
        let files: Result<Vec<PathBuf>, walkdir::Error> = WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            // skip hidden directories (if the root is hidden that's ok)
            .filter_entry(|e| e.path() == root || !e.file_name().to_string_lossy().starts_with('.'))
            .filter_ok(|e| e.file_type().is_file())
            .map_ok(|e| e.path().to_path_buf())
            .try_collect();

        let files =
            files.map_err(|e| miette!("Failed to walk directory {}: {}", root.display(), e))?;

        // Filter for YAML/JSON files instead of executable files (mise uses file::is_executable)
        let files: Vec<PathBuf> = files
            .into_iter()
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|ext| ext == "yaml" || ext == "yml" || ext == "json" || ext == "jsonc")
                    .unwrap_or(false)
            })
            .collect();

        for path in files {
            import_file(path, registry)?;
        }
    }
    Ok(())
}

// From mise: src/config/mod.rs:1762-1783 (copied and adapted)
fn load_file_tasks(
    schema: &FileSchema,
    recipe_dir: &Path,
    registry: &mut TaskRegistry,
) -> Result<()> {
    let includes = task_includes_for_dir(recipe_dir, schema);
    for include_path in includes {
        load_tasks_includes(&include_path, recipe_dir, registry)?;
    }
    Ok(())
}

pub fn import_file<P: AsRef<Path> + std::fmt::Debug>(
    path: P,
    reg: &mut TaskRegistry,
) -> Result<()> {
    let schema = parse_file_schema(&path)?;
    let warnings = validate_file_schema(&schema)?;
    for w in warnings {
        crate::log::warning(&w);
    }

    // Extract includes before consuming schema (needed for recursive loading)
    let recipe_dir = path.as_ref().parent().unwrap_or_else(|| Path::new("."));
    let includes = task_includes_for_dir(recipe_dir, &schema);

    // Parse hosts from schema
    let host_registry = if schema.hosts != serde_json::Value::Null {
        HostRegistry::from_value(&schema.hosts)?
    } else {
        HostRegistry::new()
    };
    let has_hosts = schema.hosts != serde_json::Value::Null && !host_registry.all().is_empty();

    // Collect task names up-front (before consuming schema.tasks)
    let all_task_names: Vec<String> = schema.tasks.keys().cloned().collect();

    // Register tools installer that installs tools using mise
    if schema.tools != serde_json::Value::Null {
        let tools_static: &'static serde_json::Value = Box::leak(Box::new(schema.tools.clone()));
        reg.register(
            Task::new(
                "tools:install",
                task::task_fn_async!(|ctx: std::sync::Arc<task::Context>| async move {
                    // Ensure mise is installed first
                    if !crate::shell::command_exists("mise") {
                        crate::log::info("mise not found, installing...");
                        crate::recipe::tools::mise::ensure_mise()
                            .await
                            .map_err(|e| miette!("Failed to install mise: {}", e))?;
                    }

                    // Collect tool@version pairs and install tools
                    let mut tool_version_pairs: Vec<(String, String)> = Vec::new();

                    if let serde_json::Value::Object(map) = tools_static {
                        for (tool_name, version_val) in map.iter() {
                            let version = match version_val {
                                serde_json::Value::String(s) => s.as_str(),
                                serde_json::Value::Number(n) => {
                                    let num_str = n.to_string();
                                    let install_cmd =
                                        format!("mise install {}@{}", tool_name, num_str);
                                    crate::log::info(&format!(
                                        "Installing {}@{} via mise",
                                        tool_name, num_str
                                    ));
                                    crate::shell::run_local(&install_cmd).map_err(|e| {
                                        miette!("Failed to install {}: {}", tool_name, e)
                                    })?;
                                    tool_version_pairs.push((tool_name.clone(), num_str));
                                    continue;
                                }
                                _ => "latest",
                            };

                            let install_cmd = if version == "latest" {
                                format!("mise install {}", tool_name)
                            } else {
                                format!("mise install {}@{}", tool_name, version)
                            };
                            crate::log::info(&format!(
                                "Installing {}@{} via mise",
                                tool_name, version
                            ));
                            crate::shell::run_local(&install_cmd)
                                .map_err(|e| miette!("Failed to install {}: {}", tool_name, e))?;

                            // Resolve "latest" version if needed
                            let resolved_version = if version == "latest" {
                                // Get installed versions and pick the one marked as "latest" or the first one
                                let output = std::process::Command::new("mise")
                                    .arg("ls")
                                    .arg(tool_name)
                                    .output()
                                    .map_err(|e| miette!("Failed to run mise ls: {}", e))?;

                                if output.status.success() {
                                    let stdout = String::from_utf8_lossy(&output.stdout);
                                    // Parse mise ls output format: "tool  version  [config]  [latest]"
                                    // Look for line with "latest" marker, or use first version found
                                    let mut found_latest = None;
                                    let mut first_version = None;

                                    for line in stdout.lines() {
                                        let line = line.trim();
                                        if line.is_empty() || line.starts_with('*') {
                                            continue;
                                        }
                                        // Parse: "tool  version  [config]  [latest]"
                                        let parts: Vec<&str> = line.split_whitespace().collect();
                                        if parts.len() >= 2 && parts[0] == tool_name {
                                            let ver = parts[1].to_string();
                                            if first_version.is_none() {
                                                first_version = Some(ver.clone());
                                            }
                                            // Check if this line has "latest" marker
                                            if line.contains("latest") {
                                                found_latest = Some(ver);
                                                break;
                                            }
                                        }
                                    }

                                    found_latest
                                        .or(first_version)
                                        .unwrap_or_else(|| "latest".to_string())
                                } else {
                                    "latest".to_string()
                                }
                            } else {
                                version.to_string()
                            };

                            tool_version_pairs.push((tool_name.clone(), resolved_version));
                        }
                    }

                    // Call mise env --json with all tools to get environment variables
                    if !tool_version_pairs.is_empty() {
                        let mut mise_cmd = std::process::Command::new("mise");
                        mise_cmd.arg("env").arg("--json");
                        for (tool, version) in &tool_version_pairs {
                            mise_cmd.arg(format!("{}@{}", tool, version));
                        }

                        let output = mise_cmd
                            .output()
                            .map_err(|e| miette!("Failed to run mise env: {}", e))?;

                        if output.status.success() {
                            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                            // Store the entire JSON output in Context for later parsing
                            // This allows us to access all mise env vars without needing to iterate vars
                            ctx.set("_mise_env_json", &stdout);
                        } else {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            crate::log::warning(&format!("mise env failed: {}", stderr));
                        }
                    }

                    Ok(())
                }),
            )
            .hidden()
            .desc("Install tools defined in blueprint using mise"),
        );
        // Ensure tools are installed before config:apply and all imported tasks
        for name in &all_task_names {
            reg.before(name.clone(), "tools:install");
        }
        reg.before("config:apply", "tools:install");
        // Also common deploy tasks for compatibility
        for target in [
            "deploy:prepare",
            "deploy",
            "deploy:lock",
            "deploy:unlock",
            "deploy:is_locked",
        ] {
            reg.before(target, "tools:install");
        }
    }

    // Register config applier that injects config values into Context
    if schema.config != serde_json::Value::Null {
        let config_static: &'static serde_json::Value = Box::leak(Box::new(schema.config.clone()));
        reg.register(
            Task::new(
                "config:apply",
                task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                    if let serde_json::Value::Object(map) = config_static {
                        for (k, v) in map.iter() {
                            // Store simple values as strings
                            let val_str = match v {
                                serde_json::Value::String(s) => s.clone(),
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::Bool(b) => b.to_string(),
                                _ => v.to_string(),
                            };
                            ctx.set(k, &val_str);
                        }
                    }
                    Ok(())
                }),
            )
            .hidden()
            .desc("Apply config values to context"),
        );
        // Ensure config is applied before every imported task so {{var}} work
        for name in &all_task_names {
            reg.before(name.clone(), "config:apply");
        }
        // Also common deploy tasks for compatibility
        for target in [
            "deploy:prepare",
            "deploy",
            "deploy:lock",
            "deploy:unlock",
            "deploy:is_locked",
        ] {
            reg.before(target, "config:apply");
        }
    }

    for (name, def_with_metadata) in schema.tasks {
        // Extract metadata
        let confirm = def_with_metadata.confirm.clone();
        let ask = def_with_metadata.ask.clone();
        let only_if = def_with_metadata.only_if.clone();
        let unless = def_with_metadata.unless.clone();

        let task = match def_with_metadata.task_def {
            TaskDef::Deps(deps) => {
                let mut t = Task::new(
                    name.clone(),
                    task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| Ok(())),
                );
                for d in deps {
                    t = t.depends_on(d);
                }
                t
            }
            TaskDef::WithWaitForDeps { wait_for, depends } => {
                let mut t = Task::new(
                    name.clone(),
                    task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| Ok(())),
                );
                for d in depends {
                    t = t.depends_on(d);
                }
                for w in wait_for {
                    t = t.wait_for(w);
                }
                t
            }
            TaskDef::Steps(steps) => create_task_from_steps(
                name.clone(),
                steps,
                Vec::new(),
                Vec::new(),
                &host_registry,
                has_hosts,
            ),
            TaskDef::WithWaitFor {
                wait_for,
                steps,
                sources,
                outputs,
            } => {
                let mut t = create_task_from_steps(
                    name.clone(),
                    steps,
                    sources,
                    outputs,
                    &host_registry,
                    has_hosts,
                );
                for w in wait_for {
                    t = t.wait_for(w);
                }
                t
            }
            TaskDef::WithDepends {
                depends,
                steps,
                sources,
                outputs,
            } => {
                let mut t = create_task_from_steps(
                    name.clone(),
                    steps,
                    sources,
                    outputs,
                    &host_registry,
                    has_hosts,
                );
                for d in depends {
                    t = t.depends_on(d);
                }
                t
            }
            TaskDef::WithDependsAndWaitFor {
                depends,
                wait_for,
                steps,
                sources,
                outputs,
            } => {
                let mut t = create_task_from_steps(
                    name.clone(),
                    steps,
                    sources,
                    outputs,
                    &host_registry,
                    has_hosts,
                );
                for d in depends {
                    t = t.depends_on(d);
                }
                for w in wait_for {
                    t = t.wait_for(w);
                }
                t
            }
            TaskDef::WithSourcesOutputs {
                steps,
                sources,
                outputs,
            } => create_task_from_steps(
                name.clone(),
                steps,
                sources,
                outputs,
                &host_registry,
                has_hosts,
            ),
        };

        // Apply metadata: conditions, prompts, confirmations
        let mut final_task = task;

        // Add conditional execution (only_if/unless) if specified
        if let Some(ref cmd) = only_if {
            let cmd_clone = cmd.clone();
            final_task =
                final_task.only_if(move |ctx: &task::Context| test_command(ctx, &cmd_clone));
        }
        if let Some(ref cmd) = unless {
            let cmd_clone = cmd.clone();
            final_task =
                final_task.unless(move |ctx: &task::Context| test_command(ctx, &cmd_clone));
        }

        // Handle confirm and ask prompts via wrapping (following mise pattern)
        // Confirm is checked right before task execution, ask stores values in context
        if confirm.is_some() || ask.is_some() {
            final_task = wrap_task_with_prompts(final_task, confirm, ask);
        }

        reg.register(final_task);
    }

    for (target, hooks) in schema.before {
        for h in hooks {
            reg.before(target.clone(), h);
        }
    }
    for (target, hooks) in schema.after {
        for h in hooks {
            reg.after(target.clone(), h);
        }
    }

    // Load tasks from includes after loading main blueprint
    for include_path in includes {
        load_tasks_includes(&include_path, recipe_dir, reg)?;
    }

    Ok(())
}
