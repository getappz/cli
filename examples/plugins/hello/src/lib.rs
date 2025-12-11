//! Hello World Plugin Example
//!
//! This example demonstrates how to create a saasctl WASM plugin using the appz_pdk.

#[macro_use]
extern crate appz_pdk;

use appz_pdk::prelude::*;
use extism_pdk::*;

// ============================================================================
// Host Function Declarations
// ============================================================================

#[host_fn]
extern "ExtismHost" {
    // Registry functions
    fn appz_reg_task(input: Json<TaskInput>) -> Json<TaskResponse>;
    fn appz_reg_after(input: Json<HookInput>) -> Json<HookResponse>;
    
    // Context functions
    fn appz_ctx_set(input: Json<ContextSetInput>) -> Json<HookResponse>;
    fn appz_ctx_get(key: String) -> Json<ContextGetOutput>;
    
    // Execution functions
    fn appz_exec_run_local(input: Json<RunInput>) -> Json<RunOutput>;
    fn appz_exec_invoke(input: Json<InvokeInput>) -> Json<HookResponse>;
    fn appz_exec_cd(path: String) -> Json<HookResponse>;
    
    // Utility functions
    fn appz_util_info(message: String) -> Json<serde_json::Value>;
    fn appz_util_warning(message: String) -> Json<serde_json::Value>;
    fn appz_util_timestamp() -> String;
}

// ============================================================================
// Plugin Registration
// ============================================================================

#[plugin_fn]
pub fn appz_register() -> FnResult<()> {
    unsafe {
        // Set some context variables using helper macros
        appz_set!("plugin_name", "hello");
        appz_set!("plugin_version", "0.1.0");
        
        // Register tasks using helper macro
        appz_task!("hello:world", "A simple hello world task from plugin");
        appz_task!("hello:after", "A task that runs after hello:world", deps: vec!["hello:world".to_string()]);
        appz_task!("hello:command", "Demonstrates command execution");
        appz_task!("hello:invoke", "Demonstrates task invocation");
        
        // Use helper macro for logging
        appz_info!("Hello plugin registered successfully!");
    }
    
    Ok(())
}

// ============================================================================
// Task Execution
// ============================================================================

#[plugin_fn]
pub fn appz_run(input: String) -> FnResult<String> {
    match input.as_str() {
        "hello:world" => {
            unsafe {
                // Use helper macros for logging
                appz_info!("Hello from WASM plugin!");
                
                // Get the plugin name from context using helper macro
                if let Some(name) = appz_get!("plugin_name") {
                    appz_info!("Plugin name from context: {}", name);
                } else {
                    appz_warning!("Failed to get context value");
                }
                
                // Get timestamp - extism-pdk wraps host function calls in Result
                match appz_util_timestamp() {
                    Ok(timestamp) => {
                        appz_info!("Current timestamp: {}", timestamp);
                    }
                    Err(_) => {
                        appz_warning!("Failed to get timestamp");
                    }
                }
            }
            
            Ok("Hello from WASM plugin!".to_string())
        }
        
        "hello:after" => {
            unsafe {
                appz_info!("This task runs after hello:world");
            }
            Ok("After task completed".to_string())
        }
        
        "hello:command" => {
            unsafe {
                appz_info!("Demonstrating command execution with run()");
                
                // Demonstrate run() macro - executes shell commands
                match appz_run!("echo 'Hello from command execution!'") {
                    Ok(Json(output)) => {
                        if output.success {
                            appz_info!("Command executed successfully!");
                        } else {
                            appz_warning!("Command execution failed");
                        }
                    }
                    Err(_) => {
                        appz_warning!("Failed to execute command");
                    }
                }
                
                // Demonstrate cd() - change working directory (need direct call)
                let _ = appz_exec_cd(".".to_string());
            }
            
            Ok("Command demonstration completed".to_string())
        }
        
        "hello:invoke" => {
            unsafe {
                appz_info!("Demonstrating task invocation with invoke()");
                
                // Demonstrate invoke() macro - invokes another task
                match appz_invoke!("hello:world") {
                    Ok(Json(response)) => {
                        if let Some(msg) = &response.message {
                            appz_info!("Invoke result: {}", msg);
                        }
                    }
                    Err(_) => {
                        appz_warning!("Failed to invoke task");
                    }
                }
            }
            
            Ok("Task invocation demonstration completed".to_string())
        }
        
        _ => {
            Err(WithReturnCode::new(
                Error::msg(format!("Unknown task: {}", input)),
                1,
            ))
        }
    }
}
