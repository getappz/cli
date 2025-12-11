use extism::{Manifest, Plugin, PluginBuilder, UserData, Wasm, PTR};
use miette::{IntoDiagnostic, Result};
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

#[derive(Clone)]
pub struct PluginHostData {
    pub registry: Arc<Mutex<TaskRegistry>>,
    pub context: Arc<RwLock<Context>>,
    pub plugin_id: String, // Track which plugin registered the task
    pub plugin_manager: Arc<std::sync::Mutex<HashMap<String, Mutex<Plugin>>>>, // Shared plugin registry
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
        let wasm_data = std::fs::read(wasm_path.as_ref()).into_diagnostic()?;

        let local_registry = Arc::new(Mutex::new(TaskRegistry::new()));
        let host_data = PluginHostData {
            registry: local_registry.clone(),
            context: self.context.clone(),
            plugin_id: id.clone(),
            plugin_manager: self.plugins.clone(),
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
