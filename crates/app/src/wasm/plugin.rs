use extism::{convert::Json, Manifest, Plugin, PluginBuilder, UserData, Wasm, PTR};
use starbase_utils::fs;
use miette::{IntoDiagnostic, Result};
use sandbox::{ScopedFs, SandboxProvider};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

use task::{Context, TaskRegistry};

// Import all host functions
use crate::wasm::host_functions::context::{
    appz_ctx_add, appz_ctx_get, appz_ctx_has, appz_ctx_parse, appz_ctx_remove, appz_ctx_set,
};
use crate::wasm::host_functions::execution::{
    appz_exec_become, appz_exec_cd, appz_exec_invoke, appz_exec_on, appz_exec_run,
    appz_exec_run_local, appz_exec_test, appz_exec_test_local, appz_exec_within,
};
use crate::wasm::host_functions::filesystem::{appz_fs_download, appz_fs_upload};
use crate::wasm::host_functions::host::{
    appz_host_create, appz_host_current, appz_host_localhost, appz_host_select, appz_host_selected,
};
use crate::wasm::host_functions::interaction::{
    appz_int_ask, appz_int_ask_choice, appz_int_ask_confirm, appz_int_ask_hidden, appz_int_input,
    appz_int_output,
};
use crate::wasm::host_functions::registry::{
    appz_recipe_import, appz_recipe_option, appz_reg_after, appz_reg_before, appz_reg_desc,
    appz_reg_fail, appz_reg_task,
};
use crate::wasm::host_functions::utils::{
    appz_util_cmd_exists, appz_util_cmd_supports, appz_util_error, appz_util_fetch, appz_util_info,
    appz_util_remote_env, appz_util_timestamp, appz_util_warning, appz_util_which,
    appz_util_writeln,
};
// Import plugin-specific host functions
use crate::wasm::host_functions::plugin_fs::{
    appz_pfs_read_file, appz_pfs_write_file, appz_pfs_walk_dir, appz_pfs_exists,
    appz_pfs_is_file, appz_pfs_is_dir, appz_pfs_mkdir, appz_pfs_copy,
    appz_pfs_remove, appz_pfs_remove_dir, appz_pfs_list_dir,
    appz_pfs_read_json, appz_pfs_write_json,
};
use crate::wasm::host_functions::plugin_git::{
    appz_pgit_changed_files, appz_pgit_staged_files, appz_pgit_is_repo,
};
use crate::wasm::host_functions::plugin_http::appz_phttp_download;
use crate::wasm::host_functions::plugin_sandbox::{
    appz_psandbox_exec, appz_psandbox_exec_with_tool, appz_psandbox_ensure_tool,
};
use crate::wasm::host_functions::plugin_ast::{
    appz_past_transform, appz_past_parse_jsx,
};
#[cfg(feature = "check")]
use crate::wasm::host_functions::plugin_check::appz_pcheck_run;
#[cfg(not(feature = "check"))]
use crate::wasm::host_functions::stubs::appz_pcheck_run_stub;
#[cfg(feature = "site")]
use crate::wasm::host_functions::plugin_site::appz_psite_run;
#[cfg(not(feature = "site"))]
use crate::wasm::host_functions::stubs::appz_psite_run_stub;
use crate::wasm::types::{
    PluginHandshakeChallenge, PluginHandshakeResponse,
    PluginInfo, PluginExecuteInput, PluginExecuteOutput,
};

#[derive(Clone)]
pub struct PluginHostData {
    pub registry: Arc<Mutex<TaskRegistry>>,
    pub context: Arc<RwLock<Context>>,
    pub plugin_id: String, // Track which plugin registered the task
    pub plugin_manager: Arc<std::sync::Mutex<HashMap<String, Mutex<Plugin>>>>, // Shared plugin registry
    /// ScopedFs for downloadable plugins (path-jailed to project dir).
    pub scoped_fs: Option<Arc<ScopedFs>>,
    /// SandboxProvider for downloadable plugins (exec isolation).
    pub sandbox: Option<Arc<dyn SandboxProvider>>,
}

pub struct PluginManager {
    plugins: Arc<std::sync::Mutex<HashMap<String, Mutex<Plugin>>>>, // Share via Arc for task closures
    context: Arc<RwLock<Context>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(std::sync::Mutex::new(HashMap::new())),
            context: Arc::new(RwLock::new(Context::new())),
        }
    }

    pub fn load_plugin<P: AsRef<std::path::Path>>(
        &mut self,
        registry: &mut TaskRegistry,
        id: String,
        wasm_path: P,
    ) -> Result<()> {
        let wasm_data = fs::read_file_bytes(wasm_path.as_ref()).map_err(|e| miette::miette!("{}", e))?;

        let local_registry = Arc::new(Mutex::new(TaskRegistry::new()));
        let host_data = PluginHostData {
            registry: local_registry.clone(),
            context: self.context.clone(),
            plugin_id: id.clone(),
            plugin_manager: self.plugins.clone(),
            scoped_fs: None,
            sandbox: None,
        };

        let manifest = Manifest::new([Wasm::data(wasm_data)]);

        // Build plugin with all host functions
        let mut builder = PluginBuilder::new(manifest).with_wasi(true);

        // Register all host functions
        let user_data = UserData::new(host_data.clone());

        // Registry functions
        builder = builder.with_function(
            "appz_reg_task",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_reg_task,
        );
        builder = builder.with_function(
            "appz_reg_desc",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_reg_desc,
        );
        builder = builder.with_function(
            "appz_reg_before",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_reg_before,
        );
        builder = builder.with_function(
            "appz_reg_after",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_reg_after,
        );
        builder = builder.with_function(
            "appz_reg_fail",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_reg_fail,
        );
        builder = builder.with_function(
            "appz_recipe_import",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_recipe_import,
        );
        builder = builder.with_function(
            "appz_recipe_option",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_recipe_option,
        );

        // Context functions
        builder = builder.with_function(
            "appz_ctx_set",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_ctx_set,
        );
        builder = builder.with_function(
            "appz_ctx_get",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_ctx_get,
        );
        builder = builder.with_function(
            "appz_ctx_has",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_ctx_has,
        );
        builder = builder.with_function(
            "appz_ctx_add",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_ctx_add,
        );
        builder = builder.with_function(
            "appz_ctx_parse",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_ctx_parse,
        );
        builder = builder.with_function(
            "appz_ctx_remove",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_ctx_remove,
        );

        // Host functions
        builder = builder.with_function(
            "appz_host_create",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_host_create,
        );
        builder = builder.with_function(
            "appz_host_localhost",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_host_localhost,
        );
        builder = builder.with_function(
            "appz_host_current",
            [],
            [PTR],
            user_data.clone(),
            appz_host_current,
        );
        builder = builder.with_function(
            "appz_host_select",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_host_select,
        );
        builder = builder.with_function(
            "appz_host_selected",
            [],
            [PTR],
            user_data.clone(),
            appz_host_selected,
        );

        // Execution functions
        builder = builder.with_function(
            "appz_exec_run",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_exec_run,
        );
        builder = builder.with_function(
            "appz_exec_run_local",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_exec_run_local,
        );
        builder = builder.with_function(
            "appz_exec_test",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_exec_test,
        );
        builder = builder.with_function(
            "appz_exec_test_local",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_exec_test_local,
        );
        builder = builder.with_function(
            "appz_exec_invoke",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_exec_invoke,
        );
        builder = builder.with_function(
            "appz_exec_on",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_exec_on,
        );
        builder = builder.with_function(
            "appz_exec_cd",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_exec_cd,
        );
        builder = builder.with_function(
            "appz_exec_become",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_exec_become,
        );
        builder = builder.with_function(
            "appz_exec_within",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_exec_within,
        );

        // Filesystem functions
        builder = builder.with_function(
            "appz_fs_upload",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_fs_upload,
        );
        builder = builder.with_function(
            "appz_fs_download",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_fs_download,
        );

        // Interaction functions
        builder = builder.with_function(
            "appz_int_ask",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_int_ask,
        );
        builder = builder.with_function(
            "appz_int_ask_choice",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_int_ask_choice,
        );
        builder = builder.with_function(
            "appz_int_ask_confirm",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_int_ask_confirm,
        );
        builder = builder.with_function(
            "appz_int_ask_hidden",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_int_ask_hidden,
        );
        builder = builder.with_function(
            "appz_int_input",
            [],
            [PTR],
            user_data.clone(),
            appz_int_input,
        );
        builder = builder.with_function(
            "appz_int_output",
            [],
            [PTR],
            user_data.clone(),
            appz_int_output,
        );

        // Utility functions
        builder = builder.with_function(
            "appz_util_info",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_util_info,
        );
        builder = builder.with_function(
            "appz_util_warning",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_util_warning,
        );
        builder = builder.with_function(
            "appz_util_writeln",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_util_writeln,
        );
        builder = builder.with_function(
            "appz_util_cmd_exists",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_util_cmd_exists,
        );
        builder = builder.with_function(
            "appz_util_cmd_supports",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_util_cmd_supports,
        );
        builder = builder.with_function(
            "appz_util_which",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_util_which,
        );
        builder = builder.with_function(
            "appz_util_remote_env",
            [],
            [PTR],
            user_data.clone(),
            appz_util_remote_env,
        );
        builder = builder.with_function(
            "appz_util_error",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_util_error,
        );
        builder = builder.with_function(
            "appz_util_timestamp",
            [],
            [PTR],
            user_data.clone(),
            appz_util_timestamp,
        );
        builder = builder.with_function(
            "appz_util_fetch",
            [PTR],
            [PTR],
            user_data.clone(),
            appz_util_fetch,
        );

        let mut plugin = builder.build().map_err(|e| {
            eprintln!("DEBUG: PluginBuilder::build error: {}", e);
            miette::miette!("Failed to create plugin: {}", e)
        })?;

        eprintln!("DEBUG: Plugin created successfully with all host functions");

        // Try both appz_register and saasctl_register for backward compatibility
        let register_called = plugin.call::<(), ()>("appz_register", ()).is_ok()
            || plugin.call::<(), ()>("saasctl_register", ()).is_ok();

        if !register_called {
            eprintln!("DEBUG: Warning: Neither appz_register nor saasctl_register was exported");
        } else {
            eprintln!("DEBUG: Plugin registration completed successfully");
        }

        // Merge registry
        let plugin_reg = local_registry.lock().unwrap();
        let task_count = plugin_reg.all().count();
        eprintln!("DEBUG: Found {} task(s) registered by plugin", task_count);
        for (_k, t) in plugin_reg.all() {
            eprintln!("DEBUG: Registering task: {}", t.name);
            registry.register(t.clone());
        }

        self.plugins.lock().unwrap().insert(id, Mutex::new(plugin));
        Ok(())
    }

    pub fn run_task(&self, plugin_id: &str, task_name: &str) -> Result<()> {
        let plugin_map = self.plugins.lock().unwrap();
        let plugin = plugin_map
            .get(plugin_id)
            .ok_or_else(|| miette::miette!("Plugin not found"))?;

        let mut plugin_guard = plugin.lock().unwrap();

        // Try both appz_run and saasctl_run for backward compatibility
        let result: Result<Vec<u8>, _> = plugin_guard.call("appz_run", task_name);
        let result = result
            .or_else(|_| plugin_guard.call("saasctl_run", task_name))
            .map_err(|e| miette::miette!("Failed to call plugin run function: {}", e))?;

        if result.is_empty() {
            Ok(())
        } else {
            let error_msg = String::from_utf8_lossy(&result).to_string();
            Err(miette::miette!("Plugin error: {}", error_msg))
        }
    }
}

// ============================================================================
// PluginRunner: loads and executes verified downloadable plugins
// ============================================================================

/// Runs verified downloadable plugins with sandbox isolation.
///
/// Unlike `PluginManager` (which handles task-based plugins loaded via `--plugin`),
/// `PluginRunner` handles on-demand plugins downloaded from the CDN. These plugins
/// provide CLI commands rather than tasks, and all their I/O is sandboxed through
/// `ScopedFs` and `SandboxProvider`.
pub struct PluginRunner {
    sandbox: Arc<dyn SandboxProvider>,
    scoped_fs: Arc<ScopedFs>,
    plugin: Option<Plugin>,
    plugin_info: Option<PluginInfo>,
}

impl PluginRunner {
    /// Create a new PluginRunner backed by a sandbox.
    pub fn new(sandbox: Arc<dyn SandboxProvider>, scoped_fs: Arc<ScopedFs>) -> Self {
        Self {
            sandbox,
            scoped_fs,
            plugin: None,
            plugin_info: None,
        }
    }

    /// Load a verified plugin WASM binary, perform handshake, and get plugin info.
    ///
    /// Defense in depth: validates the WASM header so that even direct callers
    /// (e.g. APPZ_DEV_PLUGIN) cannot load WASM from other apps/plugins.
    pub fn load_verified_plugin(
        &mut self,
        wasm_path: &std::path::Path,
        plugin_name: &str,
    ) -> Result<()> {
        let wasm_data = fs::read_file_bytes(wasm_path).map_err(|e| miette::miette!("{}", e))?;

        // Validate header: reject WASM from other apps/plugins (plugin_id must match)
        plugin_manager::security::PluginSecurity::validate_header(&wasm_data, plugin_name)
            .map_err(|e| miette::miette!("{}", e))?;

        // Create host data with sandbox fields
        let host_data = PluginHostData {
            registry: Arc::new(Mutex::new(TaskRegistry::new())),
            context: Arc::new(RwLock::new(Context::new())),
            plugin_id: plugin_name.to_string(),
            plugin_manager: Arc::new(Mutex::new(HashMap::new())),
            scoped_fs: Some(self.scoped_fs.clone()),
            sandbox: Some(self.sandbox.clone()),
        };

        let manifest = Manifest::new([Wasm::data(wasm_data)]);
        let mut builder = PluginBuilder::new(manifest).with_wasi(true);
        let user_data = UserData::new(host_data);

        // Register ALL host functions (existing + new plugin-specific ones)
        builder = register_all_host_functions(builder, &user_data);
        builder = register_plugin_host_functions(builder, &user_data);

        let mut plugin = builder.build().map_err(|e| {
            miette::miette!("Failed to create plugin '{}': {}", plugin_name, e)
        })?;

        // Perform security handshake
        let challenge = plugin_manager::security::PluginSecurity::generate_challenge();
        let challenge_json = serde_json::to_string(&PluginHandshakeChallenge {
            nonce: challenge.nonce.clone(),
            cli_version: challenge.cli_version.clone(),
        })
        .into_diagnostic()?;

        let response_bytes: Vec<u8> = plugin
            .call("appz_plugin_handshake", &challenge_json)
            .map_err(|e| miette::miette!("Plugin handshake call failed: {}", e))?;

        let response: PluginHandshakeResponse =
            serde_json::from_slice(&response_bytes).into_diagnostic()?;

        plugin_manager::security::PluginSecurity::verify_handshake(
            &plugin_manager::security::HandshakeChallenge {
                nonce: challenge.nonce,
                cli_version: challenge.cli_version,
            },
            &plugin_manager::security::HandshakeResponse {
                hmac: response.hmac,
            },
            plugin_name,
        )
        .map_err(|e| miette::miette!("{}", e))?;

        tracing::debug!("Plugin '{}' handshake succeeded", plugin_name);

        // Get plugin info
        let info_bytes: Vec<u8> = plugin
            .call("appz_plugin_info", ())
            .map_err(|e| miette::miette!("Failed to get plugin info: {}", e))?;

        let info: PluginInfo = serde_json::from_slice(&info_bytes).into_diagnostic()?;
        tracing::debug!("Plugin '{}' v{} loaded", info.name, info.version);

        self.plugin_info = Some(info);
        self.plugin = Some(plugin);

        Ok(())
    }

    /// Execute a command provided by the loaded plugin.
    pub fn execute_command(
        &mut self,
        command: &str,
        args: &[String],
        working_dir: &str,
    ) -> Result<()> {
        let plugin = self
            .plugin
            .as_mut()
            .ok_or_else(|| miette::miette!("No plugin loaded"))?;

        // Build args map from options; collect positional (excluding consumed option values)
        let mut args_map = HashMap::new();
        let mut positional: Vec<serde_json::Value> = Vec::new();
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            if arg.starts_with("--") {
                let key = arg.trim_start_matches("--").to_string();
                if let Some(value) = args.get(i + 1) {
                    if !value.starts_with("--") && !value.starts_with('-') {
                        args_map.insert(key, serde_json::Value::String(value.clone()));
                        i += 2;
                        continue;
                    } else if value.starts_with("--") {
                        args_map.insert(key, serde_json::Value::Bool(true));
                    }
                } else {
                    args_map.insert(key, serde_json::Value::Bool(true));
                }
                i += 1;
            } else if arg == "-o" || arg == "-i" {
                let key = if arg == "-o" { "output" } else { "input" };
                if let Some(value) = args.get(i + 1) {
                    if !value.starts_with('-') {
                        args_map.insert(key.to_string(), serde_json::Value::String(value.clone()));
                        i += 2;
                        continue;
                    }
                }
                i += 1;
            } else {
                positional.push(serde_json::Value::String(arg.clone()));
                i += 1;
            }
        }
        if !positional.is_empty() {
            args_map.insert("_positional".to_string(), serde_json::Value::Array(positional));
        }

        let input = PluginExecuteInput {
            command: command.to_string(),
            args: args_map,
            working_dir: working_dir.to_string(),
        };

        let input_json = serde_json::to_string(&input).into_diagnostic()?;

        let result_bytes: Vec<u8> = plugin
            .call("appz_plugin_execute", &input_json)
            .map_err(|e| miette::miette!("Plugin execution failed: {}", e))?;

        let output: PluginExecuteOutput =
            serde_json::from_slice(&result_bytes).into_diagnostic()?;

        if output.exit_code != 0 {
            let msg = output
                .message
                .unwrap_or_else(|| format!("Plugin command '{}' failed", command));
            return Err(miette::miette!("{}", msg));
        }

        if let Some(msg) = output.message {
            println!("{}", msg);
        }

        Ok(())
    }

    /// Get the plugin info (available after loading).
    pub fn plugin_info(&self) -> Option<&PluginInfo> {
        self.plugin_info.as_ref()
    }
}

/// Register all existing host functions on an Extism PluginBuilder.
fn register_all_host_functions<'a>(
    mut builder: PluginBuilder<'a>,
    user_data: &UserData<PluginHostData>,
) -> PluginBuilder<'a> {
    // Registry functions
    builder = builder.with_function("appz_reg_task", [PTR], [PTR], user_data.clone(), appz_reg_task);
    builder = builder.with_function("appz_reg_desc", [PTR], [PTR], user_data.clone(), appz_reg_desc);
    builder = builder.with_function("appz_reg_before", [PTR], [PTR], user_data.clone(), appz_reg_before);
    builder = builder.with_function("appz_reg_after", [PTR], [PTR], user_data.clone(), appz_reg_after);
    builder = builder.with_function("appz_reg_fail", [PTR], [PTR], user_data.clone(), appz_reg_fail);
    builder = builder.with_function("appz_recipe_import", [PTR], [PTR], user_data.clone(), appz_recipe_import);
    builder = builder.with_function("appz_recipe_option", [PTR], [PTR], user_data.clone(), appz_recipe_option);

    // Context functions
    builder = builder.with_function("appz_ctx_set", [PTR], [PTR], user_data.clone(), appz_ctx_set);
    builder = builder.with_function("appz_ctx_get", [PTR], [PTR], user_data.clone(), appz_ctx_get);
    builder = builder.with_function("appz_ctx_has", [PTR], [PTR], user_data.clone(), appz_ctx_has);
    builder = builder.with_function("appz_ctx_add", [PTR], [PTR], user_data.clone(), appz_ctx_add);
    builder = builder.with_function("appz_ctx_parse", [PTR], [PTR], user_data.clone(), appz_ctx_parse);
    builder = builder.with_function("appz_ctx_remove", [PTR], [PTR], user_data.clone(), appz_ctx_remove);

    // Host functions
    builder = builder.with_function("appz_host_create", [PTR], [PTR], user_data.clone(), appz_host_create);
    builder = builder.with_function("appz_host_localhost", [PTR], [PTR], user_data.clone(), appz_host_localhost);
    builder = builder.with_function("appz_host_current", [], [PTR], user_data.clone(), appz_host_current);
    builder = builder.with_function("appz_host_select", [PTR], [PTR], user_data.clone(), appz_host_select);
    builder = builder.with_function("appz_host_selected", [], [PTR], user_data.clone(), appz_host_selected);

    // Execution functions
    builder = builder.with_function("appz_exec_run", [PTR], [PTR], user_data.clone(), appz_exec_run);
    builder = builder.with_function("appz_exec_run_local", [PTR], [PTR], user_data.clone(), appz_exec_run_local);
    builder = builder.with_function("appz_exec_test", [PTR], [PTR], user_data.clone(), appz_exec_test);
    builder = builder.with_function("appz_exec_test_local", [PTR], [PTR], user_data.clone(), appz_exec_test_local);
    builder = builder.with_function("appz_exec_invoke", [PTR], [PTR], user_data.clone(), appz_exec_invoke);
    builder = builder.with_function("appz_exec_on", [PTR], [PTR], user_data.clone(), appz_exec_on);
    builder = builder.with_function("appz_exec_cd", [PTR], [PTR], user_data.clone(), appz_exec_cd);
    builder = builder.with_function("appz_exec_become", [PTR], [PTR], user_data.clone(), appz_exec_become);
    builder = builder.with_function("appz_exec_within", [PTR], [PTR], user_data.clone(), appz_exec_within);

    // Filesystem functions
    builder = builder.with_function("appz_fs_upload", [PTR], [PTR], user_data.clone(), appz_fs_upload);
    builder = builder.with_function("appz_fs_download", [PTR], [PTR], user_data.clone(), appz_fs_download);

    // Interaction functions
    builder = builder.with_function("appz_int_ask", [PTR], [PTR], user_data.clone(), appz_int_ask);
    builder = builder.with_function("appz_int_ask_choice", [PTR], [PTR], user_data.clone(), appz_int_ask_choice);
    builder = builder.with_function("appz_int_ask_confirm", [PTR], [PTR], user_data.clone(), appz_int_ask_confirm);
    builder = builder.with_function("appz_int_ask_hidden", [PTR], [PTR], user_data.clone(), appz_int_ask_hidden);
    builder = builder.with_function("appz_int_input", [], [PTR], user_data.clone(), appz_int_input);
    builder = builder.with_function("appz_int_output", [], [PTR], user_data.clone(), appz_int_output);

    // Utility functions
    builder = builder.with_function("appz_util_info", [PTR], [PTR], user_data.clone(), appz_util_info);
    builder = builder.with_function("appz_util_warning", [PTR], [PTR], user_data.clone(), appz_util_warning);
    builder = builder.with_function("appz_util_writeln", [PTR], [PTR], user_data.clone(), appz_util_writeln);
    builder = builder.with_function("appz_util_cmd_exists", [PTR], [PTR], user_data.clone(), appz_util_cmd_exists);
    builder = builder.with_function("appz_util_cmd_supports", [PTR], [PTR], user_data.clone(), appz_util_cmd_supports);
    builder = builder.with_function("appz_util_which", [PTR], [PTR], user_data.clone(), appz_util_which);
    builder = builder.with_function("appz_util_remote_env", [], [PTR], user_data.clone(), appz_util_remote_env);
    builder = builder.with_function("appz_util_error", [PTR], [PTR], user_data.clone(), appz_util_error);
    builder = builder.with_function("appz_util_timestamp", [], [PTR], user_data.clone(), appz_util_timestamp);
    builder = builder.with_function("appz_util_fetch", [PTR], [PTR], user_data.clone(), appz_util_fetch);

    builder
}

/// Register plugin-specific host functions (ScopedFs, git, sandbox, AST).
fn register_plugin_host_functions<'a>(
    mut builder: PluginBuilder<'a>,
    user_data: &UserData<PluginHostData>,
) -> PluginBuilder<'a> {
    // ScopedFs filesystem functions
    builder = builder.with_function("appz_pfs_read_file", [PTR], [PTR], user_data.clone(), appz_pfs_read_file);
    builder = builder.with_function("appz_pfs_write_file", [PTR], [PTR], user_data.clone(), appz_pfs_write_file);
    builder = builder.with_function("appz_pfs_walk_dir", [PTR], [PTR], user_data.clone(), appz_pfs_walk_dir);
    builder = builder.with_function("appz_pfs_exists", [PTR], [PTR], user_data.clone(), appz_pfs_exists);
    builder = builder.with_function("appz_pfs_is_file", [PTR], [PTR], user_data.clone(), appz_pfs_is_file);
    builder = builder.with_function("appz_pfs_is_dir", [PTR], [PTR], user_data.clone(), appz_pfs_is_dir);
    builder = builder.with_function("appz_pfs_mkdir", [PTR], [PTR], user_data.clone(), appz_pfs_mkdir);
    builder = builder.with_function("appz_pfs_copy", [PTR], [PTR], user_data.clone(), appz_pfs_copy);
    builder = builder.with_function("appz_pfs_remove", [PTR], [PTR], user_data.clone(), appz_pfs_remove);
    builder = builder.with_function("appz_pfs_remove_dir", [PTR], [PTR], user_data.clone(), appz_pfs_remove_dir);
    builder = builder.with_function("appz_pfs_list_dir", [PTR], [PTR], user_data.clone(), appz_pfs_list_dir);
    builder = builder.with_function("appz_pfs_read_json", [PTR], [PTR], user_data.clone(), appz_pfs_read_json);
    builder = builder.with_function("appz_pfs_write_json", [PTR], [PTR], user_data.clone(), appz_pfs_write_json);

    // Git functions
    builder = builder.with_function("appz_pgit_changed_files", [PTR], [PTR], user_data.clone(), appz_pgit_changed_files);
    builder = builder.with_function("appz_pgit_staged_files", [PTR], [PTR], user_data.clone(), appz_pgit_staged_files);
    builder = builder.with_function("appz_pgit_is_repo", [PTR], [PTR], user_data.clone(), appz_pgit_is_repo);

    // Sandbox exec functions
    builder = builder.with_function("appz_psandbox_exec", [PTR], [PTR], user_data.clone(), appz_psandbox_exec);
    builder = builder.with_function("appz_psandbox_exec_with_tool", [PTR], [PTR], user_data.clone(), appz_psandbox_exec_with_tool);
    builder = builder.with_function("appz_psandbox_ensure_tool", [PTR], [PTR], user_data.clone(), appz_psandbox_ensure_tool);

    // AST functions
    builder = builder.with_function("appz_past_transform", [PTR], [PTR], user_data.clone(), appz_past_transform);
    builder = builder.with_function("appz_past_parse_jsx", [PTR], [PTR], user_data.clone(), appz_past_parse_jsx);

    // HTTP download function
    builder = builder.with_function("appz_phttp_download", [PTR], [PTR], user_data.clone(), appz_phttp_download);

    // Check plugin host function (stub when feature disabled)
    #[cfg(feature = "check")]
    { builder = builder.with_function("appz_pcheck_run", [PTR], [PTR], user_data.clone(), appz_pcheck_run); }
    #[cfg(not(feature = "check"))]
    { builder = builder.with_function("appz_pcheck_run", [PTR], [PTR], user_data.clone(), appz_pcheck_run_stub); }

    // Site plugin host function (stub when feature disabled)
    #[cfg(feature = "site")]
    { builder = builder.with_function("appz_psite_run", [PTR], [PTR], user_data.clone(), appz_psite_run); }
    #[cfg(not(feature = "site"))]
    { builder = builder.with_function("appz_psite_run", [PTR], [PTR], user_data.clone(), appz_psite_run_stub); }

    // Note: migrate/convert host functions have been removed entirely.
    // The ssg-migrator plugin is now self-contained and calls ssg-migrator
    // directly through the Vfs trait, using only the generic pfs/pgit host
    // functions for I/O.

    builder
}
