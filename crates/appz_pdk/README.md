# Appz PDK - Plugin Development Kit

The **Appz PDK** (Plugin Development Kit) provides a convenient way to develop WASM plugins for saasctl. It reduces boilerplate by providing shared types and helper macros.

## Installation

Add to your plugin's `Cargo.toml`:

```toml
[dependencies]
appz_pdk = { path = "../../crates/appz_pdk" }  # Adjust path as needed
extism-pdk = "1.4.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## Quick Start

```rust
use appz_pdk::prelude::*;
use extism_pdk::*;

#[host_fn]
extern "ExtismHost" {
    fn appz_reg_task(input: Json<TaskInput>) -> Json<TaskResponse>;
    fn appz_ctx_set(input: Json<ContextSetInput>) -> Json<HookResponse>;
    fn appz_ctx_get(key: String) -> Json<ContextGetOutput>;
    fn appz_exec_run_local(input: Json<RunInput>) -> Json<RunOutput>;
    fn appz_util_info(message: String) -> Json<serde_json::Value>;
}

#[plugin_fn]
pub fn appz_register() -> FnResult<()> {
    unsafe {
        // Set context using helper macro
        appz_set!("plugin_name", "my_plugin");
        
        // Register a task
        let _ = appz_reg_task(Json(TaskInput {
            name: "my:task".to_string(),
            desc: Some("My task description".to_string()),
            deps: None,
            body: None,
            only_if: None,
            unless: None,
            once: None,
            hidden: None,
            timeout: None,
        }));
        
        appz_util_info("Plugin registered!".to_string());
    }
    Ok(())
}

#[plugin_fn]
pub fn appz_run(input: String) -> FnResult<String> {
    if input == "my:task" {
        unsafe {
            // Get context value
            if let Some(value) = appz_get!("plugin_name") {
                appz_util_info(format!("Plugin name: {}", value));
            }
            
            // Run a command
            let _ = appz_run!("echo 'Hello from plugin!'");
        }
        Ok("Task completed".to_string())
    } else {
        Err(WithReturnCode::new(
            Error::msg(format!("Unknown task: {}", input)),
            1,
        ))
    }
}
```

## Available Types

All types are available through `appz_pdk::prelude::*`:

- **Registry**: `TaskInput`, `TaskResponse`, `HookInput`, `HookResponse`
- **Context**: `ContextSetInput`, `ContextGetOutput`, `ContextAddInput`, etc.
- **Execution**: `RunInput`, `RunOutput`, `InvokeInput`
- **Host**: `HostInfo`, `HostResponse`, `HostInput`
- **Filesystem**: `UploadInput`, `DownloadInput`, `FileTransferResult`
- **Interaction**: `AskInput`, `ChoiceInput`, `ConfirmInput`
- **Utility**: `FetchInput`, `FetchOutput`, `ErrorInfo`, `WhichOutput`, etc.

## Helper Macros

### `appz_set!(key, value)`

Set a context value:

```rust
appz_set!("deploy_path", "/var/www");
```

### `appz_get!(key)`

Get a context value:

```rust
let value = appz_get!("deploy_path");
```

### `appz_task!(name, desc)`

Register a task (simplified):

```rust
appz_task!("deploy", "Deploy the application");
appz_task!("deploy", "Deploy the application", deps: vec!["setup".to_string()]);
```

### `appz_run!(command)`

Run a shell command:

```rust
appz_run!("echo hello");
```

### `appz_invoke!(task)`

Invoke another task:

```rust
appz_invoke!("setup");
```

### `appz_info!(...)` and `appz_warning!(...)`

Logging macros:

```rust
appz_info!("Deployment started");
appz_warning!("This is a warning");
```

## Complete Example

See `examples/plugins/hello/src/lib.rs` for a complete example.

## Benefits

1. **Less Boilerplate**: All types are pre-defined and ready to use
2. **Type Safety**: All types match the host function signatures exactly
3. **Helper Macros**: Common operations are simplified with macros
4. **Easy Updates**: When host function signatures change, just update the PDK

## Host Function Reference

For a complete list of available host functions, see the main saasctl documentation or the Deployer API reference at https://deployer.org/docs/8.x/api.

