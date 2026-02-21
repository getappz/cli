use extism::{convert::Json, host_fn};
use std::sync::Arc;

use crate::wasm::host_functions::helpers::*;
use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;
use task::{Context, Task};

// ============================================================================
// Task Registration
// ============================================================================

host_fn!(pub appz_reg_task(
    user_data: PluginHostData;
    args: Json<TaskInput>
) -> Json<TaskResponse> {
    let task_input = args.into_inner();

    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();

    let plugin_id = data_guard.plugin_id.clone();
    let plugin_manager_clone = data_guard.plugin_manager.clone();
    let original_task_name = task_input.name.clone();
    let plugin_id_for_closure = plugin_id.clone();

    // Qualify task name with plugin namespace
    let qualified_task_name = qualify_task_name(&task_input.name, &plugin_id);

    // Build task with dependencies
    let mut task_builder = Task::new(
        qualified_task_name.clone(),
        std::sync::Arc::new(move |_ctx: Arc<Context>| {
            let plugin_id = plugin_id_for_closure.clone();
            let plugin_manager = plugin_manager_clone.clone();
            let task_name_for_call = original_task_name.clone();
            Box::pin(async move {
                let plugin_map = plugin_manager.lock().unwrap();
                let plugin = plugin_map.get(&plugin_id)
                    .ok_or_else(|| miette::miette!("Plugin '{}' not found", plugin_id))?;

                let mut plugin_guard = plugin.lock().unwrap();
                let result = plugin_guard
                    .call::<&str, &str>("appz_run", &task_name_for_call)
                    .map_err(|e| miette::miette!("Failed to call plugin appz_run: {}", e))?;

                if !result.is_empty() {
                    println!("{}", result);
                }

                Ok(())
            }) as futures::future::BoxFuture<'static, task::error::TaskResult>
        }) as task::types::AsyncTaskFn,
    );

    // Set description
    if let Some(desc) = &task_input.desc {
        task_builder = task_builder.desc(desc.clone());
    }

    // Add dependencies
    if let Some(deps) = &task_input.deps {
        for dep in deps {
            task_builder = task_builder.depends_on(dep.clone());
        }
    }

    // Set flags
    if task_input.once.unwrap_or(false) {
        task_builder = task_builder.once();
    }
    if task_input.hidden.unwrap_or(false) {
        task_builder = task_builder.hidden();
    }
    if let Some(timeout) = task_input.timeout {
        task_builder = task_builder.timeout(timeout);
    }

    // Note: only_if and unless conditions are currently skipped as they require closures
    // TODO: Add support for string-based condition evaluation

    // Register task
    {
        let mut reg_guard = data_guard.registry.lock().unwrap();
        reg_guard.register(task_builder);
    }

    Ok(Json(TaskResponse {
        success: true,
        task_name: qualified_task_name,
        message: Some(format!("Task '{}' registered", task_input.name)),
    }))
});

// ============================================================================
// Task Description
// ============================================================================

host_fn!(pub appz_reg_desc(
    user_data: PluginHostData;
    args: Json<DescInput>
) -> Json<HookResponse> {
    let input = args.into_inner();

    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();

    // Qualify task name
    let qualified_name = qualify_task_name(&input.task, &data_guard.plugin_id);

    {
        let mut reg_guard = data_guard.registry.lock().unwrap();
        if let Some(task) = reg_guard.get_mut(&qualified_name) {
            task.description = Some(input.desc);
            Ok(success_response(format!("Description updated for task '{}'", qualified_name)))
        } else {
            Ok(failure_response(format!("Task '{}' not found", qualified_name)))
        }
    }
});

// ============================================================================
// Before Hook
// ============================================================================

host_fn!(pub appz_reg_before(
    user_data: PluginHostData;
    args: Json<HookInput>
) -> Json<HookResponse> {
    let input = args.into_inner();

    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();

    // Qualify names
    let plugin_id = &data_guard.plugin_id;
    let qualified_target = qualify_task_name(&input.target, plugin_id);
    let qualified_hook = qualify_task_name(&input.hook, plugin_id);

    {
        let mut reg_guard = data_guard.registry.lock().unwrap();
        reg_guard.before(qualified_target.clone(), qualified_hook.clone());
    }

    Ok(success_response(format!("Before hook '{}' -> '{}' registered", qualified_target, qualified_hook)))
});

// ============================================================================
// After Hook
// ============================================================================

host_fn!(pub appz_reg_after(
    user_data: PluginHostData;
    args: Json<HookInput>
) -> Json<HookResponse> {
    let input = args.into_inner();

    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();

    // Qualify names
    let plugin_id = &data_guard.plugin_id;
    let qualified_target = qualify_task_name(&input.target, plugin_id);
    let qualified_hook = qualify_task_name(&input.hook, plugin_id);

    {
        let mut reg_guard = data_guard.registry.lock().unwrap();
        reg_guard.after(qualified_target.clone(), qualified_hook.clone());
    }

    Ok(success_response(format!("After hook '{}' -> '{}' registered", qualified_target, qualified_hook)))
});

// ============================================================================
// Fail Hook
// ============================================================================

host_fn!(pub appz_reg_fail(
    user_data: PluginHostData;
    args: Json<HookInput>
) -> Json<HookResponse> {
    let input = args.into_inner();

    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();

    // Qualify names
    let plugin_id = &data_guard.plugin_id;
    let qualified_target = qualify_task_name(&input.target, plugin_id);
    let qualified_hook = qualify_task_name(&input.hook, plugin_id);

    {
        let mut reg_guard = data_guard.registry.lock().unwrap();
        reg_guard.fail(qualified_target.clone(), qualified_hook.clone());
    }

    Ok(success_response(format!("Fail hook '{}' -> '{}' registered", qualified_target, qualified_hook)))
});

// ============================================================================
// Recipe Import
// ============================================================================

#[derive(Debug, serde::Serialize)]
struct ImportResponse {
    success: bool,
    message: Option<String>,
}

host_fn!(pub appz_recipe_import(
    _user_data: PluginHostData;
    _path: String
) -> Json<ImportResponse> {
    // Import is handled by the main CLI, not in plugin context
    // This is a stub that returns success but does nothing in plugin context
    Ok(Json(ImportResponse {
        success: true,
        message: Some("Import is handled at CLI level, not in plugin context".to_string()),
    }))
});

// ============================================================================
// Recipe Option (CLI option definition - stub for now)
// ============================================================================

host_fn!(pub appz_recipe_option(
    _user_data: PluginHostData;
    _args: Json<OptionInput>
) -> Json<HookResponse> {
    // CLI options are defined at CLI level, not in plugin context
    // This is a stub
    Ok(success_response("CLI options are handled at CLI level"))
});
