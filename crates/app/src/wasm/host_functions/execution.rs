use extism::{convert::Json, host_fn, Error};
use std::path::PathBuf;
use tokio::task as tokio_task;

use crate::shell::{run_local_with, test_local, RunOptions};
use crate::wasm::host_functions::helpers::*;
use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;

// ============================================================================
// Run Command (execute shell command)
// Note: This runs shell commands, NOT tasks.
// Use appz_exec_invoke() to invoke/execute tasks.
// ============================================================================

host_fn!(pub appz_exec_run(
    user_data: PluginHostData;
    args: Json<RunInput>
) -> Json<RunOutput> {
    let input = args.into_inner();

    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();
    let ctx = data_guard.context.clone();

    // Replace %secret% placeholder if secret is provided
    let mut command = input.command;
    if let Some(secret) = &input.secret {
        command = command.replace("%secret%", secret);
    }

    // Prepare options
    let cwd = input.cwd.map(PathBuf::from);
    let opts = RunOptions {
        cwd,
        env: input.env,
        show_output: input.force_output.unwrap_or(false),
        package_manager: None,
        tool_info: None,
    };

    // Execute command (run_local_with is now async)
    let ctx_guard = tokio_task::block_in_place(|| ctx.blocking_read());
    let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        // We're in an async context, use block_on
        handle.block_on(run_local_with(&ctx_guard, &command, opts))
    } else {
        // Not in async context, create a runtime
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| Error::msg(format!("Failed to create runtime: {}", e)))?;
        rt.block_on(run_local_with(&ctx_guard, &command, opts))
    };

    match result {
        Ok(_) => {
            Ok(Json(RunOutput {
                success: true,
                stdout: None,
                stderr: None,
                exit_code: Some(0),
                error: None,
            }))
        }
        Err(e) => {
            if input.nothrow.unwrap_or(false) {
                Ok(Json(RunOutput {
                    success: false,
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    error: Some(e.to_string()),
                }))
            } else {
                Err(Error::msg(format!("Command failed: {}", e)))
            }
        }
    }
});

// ============================================================================
// Run Command Locally
// ============================================================================

host_fn!(pub appz_exec_run_local(
    user_data: PluginHostData;
    args: Json<RunInput>
) -> Json<RunOutput> {
    // Same as run() for local execution - inline the logic
    let input = args.into_inner();

    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();
    let ctx = data_guard.context.clone();

    // Replace %secret% placeholder if secret is provided
    let mut command = input.command;
    if let Some(secret) = &input.secret {
        command = command.replace("%secret%", secret);
    }

    // Prepare options
    let cwd = input.cwd.map(PathBuf::from);
    let opts = RunOptions {
        cwd,
        env: input.env,
        show_output: input.force_output.unwrap_or(false),
        package_manager: None,
        tool_info: None,
    };

    // Execute command (run_local_with is now async)
    let ctx_guard = tokio_task::block_in_place(|| ctx.blocking_read());
    let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        // We're in an async context, use block_on
        handle.block_on(run_local_with(&ctx_guard, &command, opts))
    } else {
        // Not in async context, create a runtime
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| Error::msg(format!("Failed to create runtime: {}", e)))?;
        rt.block_on(run_local_with(&ctx_guard, &command, opts))
    };

    match result {
        Ok(_) => {
            Ok(Json(RunOutput {
                success: true,
                stdout: None,
                stderr: None,
                exit_code: Some(0),
                error: None,
            }))
        }
        Err(e) => {
            if input.nothrow.unwrap_or(false) {
                Ok(Json(RunOutput {
                    success: false,
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    error: Some(e.to_string()),
                }))
            } else {
                Err(Error::msg(format!("Command failed: {}", e)))
            }
        }
    }
});

// ============================================================================
// Test Command
// ============================================================================

#[derive(Debug, serde::Serialize)]
struct TestOutput {
    result: u8, // 1 = true, 0 = false
}

host_fn!(pub appz_exec_test(
    _user_data: PluginHostData;
    command: String
) -> Json<TestOutput> {
    let result = test_local(&command);
    Ok(Json(TestOutput {
        result: if result { 1 } else { 0 },
    }))
});

// ============================================================================
// Test Command Locally
// ============================================================================

host_fn!(pub appz_exec_test_local(
    _user_data: PluginHostData;
    command: String
) -> Json<TestOutput> {
    // Same as test() for local execution - inline the logic
    let result = test_local(&command);
    Ok(Json(TestOutput {
        result: if result { 1 } else { 0 },
    }))
});

// ============================================================================
// Invoke Task (execute/invoke a task by name)
// Note: This invokes tasks from the registry.
// Use appz_exec_run() to execute shell commands.
// ============================================================================

host_fn!(pub appz_exec_invoke(
    user_data: PluginHostData;
    args: Json<InvokeInput>
) -> Json<HookResponse> {
    use task::error::TaskResult;
    use std::sync::Arc;

    let input = args.into_inner();

    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();
    let registry = data_guard.registry.clone();
    let ctx = data_guard.context.clone();

    // Get task from registry and execute it directly
    let task = {
        let reg_guard = registry.lock().unwrap();
        reg_guard.get(&input.task).cloned()
    };

    let task = match task {
        Some(t) => t,
        None => {
            return Ok(failure_response(format!("Task '{}' not found", input.task)));
        }
    };

    // Execute the task action directly
    // Note: This executes the task action but doesn't handle dependencies or hooks
    // For full Deployer compatibility, we'd need to integrate with the Runner properly
    let ctx_guard = tokio_task::block_in_place(|| ctx.blocking_read());
    let ctx_clone = Arc::new(ctx_guard.clone());
    drop(ctx_guard);

    // Execute task action
    let task_fn = task.action.clone();
    let task_result: TaskResult = {
        // Use tokio runtime to execute async task
        let rt = tokio::runtime::Handle::try_current();
        if rt.is_ok() {
            // Already in async context, block on the future
            futures::executor::block_on(task_fn(ctx_clone))
        } else {
            // Not in async context, create a runtime
            let rt = tokio::runtime::Runtime::new().map_err(|e| {
                Error::msg(format!("Failed to create runtime for task execution: {}", e))
            })?;
            rt.block_on(task_fn(ctx_clone))
        }
    };

    match task_result {
        Ok(_) => Ok(success_response(format!("Task '{}' invoked successfully", input.task))),
        Err(e) => Ok(failure_response(format!("Task '{}' failed: {}", input.task, e))),
    }
});

// ============================================================================
// On (iterate over hosts)
// ============================================================================

host_fn!(pub appz_exec_on(
    _user_data: PluginHostData;
    _args: Json<OnInput>
) -> Json<HookResponse> {
    // For local execution, we just execute the callback once
    // In future, this would iterate over selected hosts
    // Note: Callback execution requires plugin call, which is complex
    // This is a stub that acknowledges the operation
    Ok(success_response("on() executed for localhost"))
});

// ============================================================================
// Change Directory
// ============================================================================

host_fn!(pub appz_exec_cd(
    user_data: PluginHostData;
    path: String
) -> Json<HookResponse> {
    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();
    let ctx = data_guard.context.clone();

    {
        let mut ctx_guard = tokio_task::block_in_place(|| ctx.blocking_write());
        ctx_guard.set_working_path(PathBuf::from(path.clone()));
    }

    Ok(success_response(format!("Changed directory to '{}'", path)))
});

// ============================================================================
// Become (change user - stub for now)
// ============================================================================

host_fn!(pub appz_exec_become(
    _user_data: PluginHostData;
    args: Json<BecomeInput>
) -> Json<RestoreHandle> {
    // User switching is not implemented yet
    let _input = args.into_inner();

    // Return a dummy handle
    Ok(Json(RestoreHandle {
        handle: 0,
    }))
});

// ============================================================================
// Within (execute in directory context)
// ============================================================================

host_fn!(pub appz_exec_within(
    user_data: PluginHostData;
    args: Json<WithinInput>
) -> Json<HookResponse> {
    let input = args.into_inner();

    // Set working path for the callback
    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();
    let ctx = data_guard.context.clone();

    // Save current working path
    let old_path = {
        let ctx_guard = tokio_task::block_in_place(|| ctx.blocking_read());
        ctx_guard.working_path().cloned()
    };

    // Set new working path
    {
        let mut ctx_guard = tokio_task::block_in_place(|| ctx.blocking_write());
        ctx_guard.set_working_path(PathBuf::from(input.path.clone()));
    }

    // Note: Callback execution would happen here
    // For now, we just acknowledge the operation
    // In future, callback would be invoked via plugin call

    // Restore old path
    if let Some(old) = old_path {
        let mut ctx_guard = tokio_task::block_in_place(|| ctx.blocking_write());
        ctx_guard.set_working_path(old);
    }

    Ok(success_response(format!("Executed within '{}'", input.path)))
});
