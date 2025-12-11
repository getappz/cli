# WASM Plugin Implementation Improvements

Based on analysis of moonrepo's warpgate implementation, here are specific improvements for your WASM plugin system.

## Current State Analysis

### Strengths
- ✅ Basic host function pattern working (`register_task`)
- ✅ PluginManager structure is sound
- ✅ Shared state (registry, context) properly managed

### Areas for Improvement

1. **Error Handling**: No structured error codes, limited error context
2. **Function Registration**: Single function, hard to scale
3. **Memory Management**: Direct JSON string handling (can be optimized)
4. **Type Safety**: Using raw `serde_json::Value` instead of typed structs
5. **Missing Host Functions**: Plan calls for many more (context, shell, fs, etc.)

## Recommended Improvements

### 1. Structured Host Function Registration

**Current:**
```rust
let reg_fn = Function::new(...);
let mut plugin = Plugin::new(wasm, [reg_fn], true)?;
```

**Improved (Batch Registration):**
```rust
use extism::{Error, Function, Plugin, UserData, ValType, Val};

pub fn create_host_functions(host_data: PluginHostData) -> Vec<Function> {
    vec![
        // Registry functions
        // Note: Using "extism:host/user::" prefix matches extism-pdk's default #[host_fn] lookup
        // Warpgate uses bare names, but both approaches work - just be consistent!
        Function::new(
            "extism:host/user::saasctl_reg_register_task",
            [ValType::I64],
            [ValType::I32],
            UserData::new(host_data.clone()),
            register_task_host_fn,
        ),
        Function::new(
            "extism:host/user::saasctl_reg_before",
            [ValType::I64, ValType::I64],
            [ValType::I32],
            UserData::new(host_data.clone()),
            before_hook_host_fn,
        ),
        Function::new(
            "extism:host/user::saasctl_reg_after",
            [ValType::I64, ValType::I64],
            [ValType::I32],
            UserData::new(host_data.clone()),
            after_hook_host_fn,
        ),
        
        // Context functions (using extism:host/user:: namespace for consistency)
        Function::new(
            "extism:host/user::saasctl_ctx_get",
            [ValType::I64],
            [ValType::I64], // Returns memory handle or 0 for null
            UserData::new(host_data.clone()),
            ctx_get_host_fn,
        ),
        Function::new(
            "extism:host/user::saasctl_ctx_set",
            [ValType::I64, ValType::I64],
            [ValType::I32], // 0 = success, non-zero = error code
            UserData::new(host_data.clone()),
            ctx_set_host_fn,
        ),
        
        // Shell functions
        Function::new(
            "extism:host/user::saasctl_shell_run",
            [ValType::I64], // JSON input
            [ValType::I64], // JSON output or error
            UserData::new(host_data.clone()),
            shell_run_host_fn,
        ),
        
        // Logging functions
        Function::new(
            "extism:host/user::saasctl_log_info",
            [ValType::I64],
            [],
            UserData::new(host_data.clone()),
            log_info_host_fn,
        ),
        // ... more functions
    ]
}

impl PluginManager {
    pub fn load_plugin<P: AsRef<std::path::Path>>(
        &mut self,
        registry: &mut TaskRegistry,
        id: String,
        wasm_path: P,
    ) -> Result<()> {
        let wasm = std::fs::read(wasm_path.as_ref())
            .into_diagnostic()?;

        let local_registry = Arc::new(Mutex::new(TaskRegistry::new()));
        let host_data = PluginHostData {
            registry: local_registry.clone(),
            context: self.context.clone(),
        };

        // ✅ Batch register all host functions
        let functions = create_host_functions(host_data);

        let mut plugin = Plugin::new(wasm, functions, true)
            .map_err(|e| miette::miette!("Failed to create plugin: {}", e))?;

        // ... rest of load logic
    }
}
```

### 2. Improved Error Handling with Error Codes

**Current:**
```rust
outputs[0] = Val::I32(0); // Always success
Ok(())
```

**Improved (Structured Errors):**
```rust
#[derive(Debug, Clone, Copy)]
pub enum HostErrorCode {
    Success = 0,
    InvalidInput = 1,
    NotFound = 2,
    PermissionDenied = 3,
    InternalError = 4,
}

impl From<HostErrorCode> for i32 {
    fn from(code: HostErrorCode) -> Self {
        code as i32
    }
}

fn register_task_host_fn(
    plugin: &mut extism::CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    user_data: UserData<PluginHostData>,
) -> Result<(), Error> {
    let json_str: String = plugin.memory_get_val(&inputs[0])
        .map_err(|e| {
            Error::msg(format!("Failed to read memory: {}", e))
        })?;

    // Parse with proper error handling
    let input: RegisterTaskInput = serde_json::from_str(&json_str)
        .map_err(|e| {
            Error::msg(format!("Invalid JSON input: {}", e))
        })?;

    // Validate input
    if input.name.is_empty() {
        outputs[0] = Val::I32(HostErrorCode::InvalidInput.into());
        return Ok(()); // Return error code, not Rust error
    }

    // Validate reserved namespaces
    if input.name.starts_with("core:") || input.name.starts_with("internal:") {
        outputs[0] = Val::I32(HostErrorCode::PermissionDenied.into());
        return Ok(());
    }

    let data_wrapped = user_data.get()
        .map_err(|e| Error::msg(format!("Failed to get user data: {}", e)))?;
    let data = data_wrapped.lock().unwrap();
    let mut reg = data.registry.lock().unwrap();

    // Check for duplicate
    if reg.get(&input.name).is_some() {
        outputs[0] = Val::I32(HostErrorCode::InvalidInput.into());
        return Ok(());
    }

    reg.register(
        Task::new(
            input.name.clone(),
            task::task_fn_sync!(|_ctx| Ok(())),
        )
        .desc(input.desc.unwrap_or_default()),
    );

    outputs[0] = Val::I32(HostErrorCode::Success.into());
    Ok(())
}
```

### 3. Type-Safe Input/Output Structures

**Current:**
```rust
let value: serde_json::Value = serde_json::from_str(&json)?;
let name = value["name"].as_str().unwrap(); // Can panic!
```

**Improved (Typed Structs):**
```rust
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub struct RegisterTaskInput {
    pub name: String,
    pub desc: Option<String>,
    pub deps: Option<Vec<String>>,
    pub only_if: Option<Vec<String>>, // Condition expressions
    pub unless: Option<Vec<String>>,
    pub once: Option<bool>,
    pub hidden: Option<bool>,
    pub timeout: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ShellRunInput {
    pub command: String,
    pub args: Option<Vec<String>>,
    pub cwd: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub show_output: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ShellRunOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub error: Option<String>,
}

// Helper macro for common host function pattern
macro_rules! host_fn_with_json {
    ($fn_name:ident, $input_type:ty, $output_type:ty, $impl:expr) => {
        fn $fn_name(
            plugin: &mut extism::CurrentPlugin,
            inputs: &[Val],
            outputs: &mut [Val],
            user_data: UserData<PluginHostData>,
        ) -> Result<(), Error> {
            // Read JSON input
            let json_str: String = plugin.memory_get_val(&inputs[0])
                .map_err(|e| Error::msg(format!("Failed to read input memory: {}", e)))?;
            
            let input: $input_type = serde_json::from_str(&json_str)
                .map_err(|e| Error::msg(format!("Invalid JSON input: {}", e)))?;
            
            // Execute implementation
            let result = $impl(plugin, input, user_data)?;
            
            // Write JSON output
            let output_json = serde_json::to_string(&result)
                .map_err(|e| Error::msg(format!("Failed to serialize output: {}", e)))?;
            
            let output_handle = plugin.memory_set_val(&mut outputs[0], &output_json)?;
            outputs[0] = Val::I64(output_handle);
            
            Ok(())
        }
    };
}
```

### 4. Context Host Functions with Null Handling

**Improved Implementation:**
```rust
fn ctx_get_host_fn(
    plugin: &mut extism::CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    user_data: UserData<PluginHostData>,
) -> Result<(), Error> {
    let key: String = plugin.memory_get_val(&inputs[0])
        .map_err(|e| Error::msg(format!("Failed to read key: {}", e)))?;

    let data_wrapped = user_data.get()
        .map_err(|e| Error::msg(format!("Failed to get user data: {}", e)))?;
    let data = data_wrapped.lock().unwrap();
    let ctx = data.context.read().unwrap();

    match ctx.get(&key) {
        Some(value) => {
            // Return value as memory handle
            let handle = plugin.memory_set_val(&mut outputs[0], &value)?;
            outputs[0] = Val::I64(handle);
        }
        None => {
            // Return 0 (null handle) if not found
            outputs[0] = Val::I64(0);
        }
    }

    Ok(())
}

fn ctx_set_host_fn(
    plugin: &mut extism::CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    user_data: UserData<PluginHostData>,
) -> Result<(), Error> {
    let key: String = plugin.memory_get_val(&inputs[0])?;
    let value: String = plugin.memory_get_val(&inputs[1])?;

    let data_wrapped = user_data.get()?;
    let data = data_wrapped.lock().unwrap();
    let mut ctx = data.context.write().unwrap();

    ctx.set(key, value);

    outputs[0] = Val::I32(HostErrorCode::Success.into());
    Ok(())
}
```

### 5. Shell Execution Host Function

**Implementation:**
```rust
fn shell_run_host_fn(
    plugin: &mut extism::CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    user_data: UserData<PluginHostData>,
) -> Result<(), Error> {
    let json_str: String = plugin.memory_get_val(&inputs[0])?;
    let input: ShellRunInput = serde_json::from_str(&json_str)?;

    // Build command
    let mut cmd = input.command;
    if let Some(args) = input.args {
        for arg in args {
            cmd.push_str(" ");
            cmd.push_str(&arg);
        }
    }

    // Execute on host
    let result = if let Some(cwd) = input.cwd {
        std::env::set_current_dir(&cwd).map_err(|e| {
            Error::msg(format!("Failed to change directory: {}", e))
        })?;
        crate::shell::run_local(&cmd)
    } else {
        crate::shell::run_local(&cmd)
    };

    let output = match result {
        Ok(_) => ShellRunOutput {
            success: true,
            exit_code: Some(0),
            stdout: None,
            stderr: None,
            error: None,
        },
        Err(e) => ShellRunOutput {
            success: false,
            exit_code: None,
            stdout: None,
            stderr: None,
            error: Some(format!("{}", e)),
        },
    };

    // Return JSON output
    let output_json = serde_json::to_string(&output)?;
    let handle = plugin.memory_set_val(&mut outputs[0], &output_json)?;
    outputs[0] = Val::I64(handle);

    Ok(())
}
```

### 6. Helper Utilities for Common Patterns

```rust
pub mod host_utils {
    use extism::{CurrentPlugin, Error, Val};

    /// Read JSON-encoded input from WASM memory
    pub fn read_json_input<T: serde::de::DeserializeOwned>(
        plugin: &mut CurrentPlugin,
        input_val: &Val,
    ) -> Result<T, Error> {
        let json_str: String = plugin.memory_get_val(input_val)
            .map_err(|e| Error::msg(format!("Failed to read input memory: {}", e)))?;
        
        serde_json::from_str(&json_str)
            .map_err(|e| Error::msg(format!("Invalid JSON input: {}", e)))
    }

    /// Write JSON-encoded output to WASM memory
    pub fn write_json_output<T: serde::Serialize>(
        plugin: &mut CurrentPlugin,
        output_val: &mut Val,
        data: &T,
    ) -> Result<i64, Error> {
        let json_str = serde_json::to_string(data)
            .map_err(|e| Error::msg(format!("Failed to serialize output: {}", e)))?;
        
        let handle = plugin.memory_set_val(output_val, &json_str)?;
        Ok(handle)
    }

    /// Write error response to WASM memory
    pub fn write_error(
        plugin: &mut CurrentPlugin,
        output_val: &mut Val,
        code: HostErrorCode,
        message: Option<&str>,
    ) -> Result<(), Error> {
        #[derive(serde::Serialize)]
        struct ErrorResponse {
            code: i32,
            message: Option<String>,
        }

        let response = ErrorResponse {
            code: code.into(),
            message: message.map(|s| s.to_string()),
        };

        write_json_output(plugin, output_val, &response)?;
        Ok(())
    }
}

// Usage example:
fn register_task_host_fn(
    plugin: &mut extism::CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    user_data: UserData<PluginHostData>,
) -> Result<(), Error> {
    use host_utils::*;

    let input: RegisterTaskInput = read_json_input(plugin, &inputs[0])?;

    // ... validation and processing ...

    // Success response
    #[derive(serde::Serialize)]
    struct SuccessResponse {
        code: i32,
    }
    write_json_output(plugin, &mut outputs[0], &SuccessResponse {
        code: HostErrorCode::Success.into(),
    })?;

    Ok(())
}
```

### 7. Logging Host Functions

```rust
fn log_info_host_fn(
    plugin: &mut extism::CurrentPlugin,
    inputs: &[Val],
    _outputs: &mut [Val],
    _user_data: UserData<PluginHostData>,
) -> Result<(), Error> {
    let message: String = plugin.memory_get_val(&inputs[0])?;
    crate::log::info(&message);
    Ok(())
}

fn log_warning_host_fn(
    plugin: &mut extism::CurrentPlugin,
    inputs: &[Val],
    _outputs: &mut [Val],
    _user_data: UserData<PluginHostData>,
) -> Result<(), Error> {
    let message: String = plugin.memory_get_val(&inputs[0])?;
    crate::log::warning(&message);
    Ok(())
}

fn log_error_host_fn(
    plugin: &mut extism::CurrentPlugin,
    inputs: &[Val],
    _outputs: &mut [Val],
    _user_data: UserData<PluginHostData>,
) -> Result<(), Error> {
    let message: String = plugin.memory_get_val(&inputs[0])?;
    eprintln!("ERROR: {}", message);
    Ok(())
}
```

## File Organization

Restructure for scalability:

```
crates/cli/src/wasm/
├── mod.rs                    # Module exports
├── plugin.rs                 # PluginManager
├── host_functions/
│   ├── mod.rs                # Host function registry
│   ├── registry.rs           # Registry host functions
│   ├── context.rs            # Context host functions
│   ├── shell.rs              # Shell host functions
│   ├── filesystem.rs         # FS host functions
│   └── utils.rs              # Logging, errors, helpers
└── types.rs                  # Shared types (ErrorCode, Input/Output structs)
```

## Migration Path

1. **Phase 1**: Refactor existing `register_task` with improved error handling
2. **Phase 2**: Add context host functions (`ctx_get`, `ctx_set`, `ctx_parse`)
3. **Phase 3**: Add shell host functions
4. **Phase 4**: Add filesystem host functions
5. **Phase 5**: Add remaining functions from plan

## Testing Improvements

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_task_error_handling() {
        // Test invalid input
        // Test duplicate task
        // Test reserved namespace
        // Test success case
    }

    #[test]
    fn test_context_functions() {
        // Test ctx_get with existing key
        // Test ctx_get with missing key (returns null)
        // Test ctx_set
        // Test ctx_parse with interpolation
    }

    #[tokio::test]
    async fn test_shell_execution() {
        // Test successful command
        // Test failed command
        // Test with cwd
        // Test with env vars
    }
}
```

## Benefits of These Improvements

1. **Scalability**: Easy to add new host functions following the pattern
2. **Type Safety**: Typed structs prevent runtime panics
3. **Error Handling**: Structured error codes enable better WASM-side handling
4. **Maintainability**: Organized code structure, helper utilities
5. **Performance**: Efficient memory handling, batch function registration
6. **Compatibility**: Aligns with warpgate patterns for familiarity

These improvements will make your plugin system production-ready and easier to extend with the full API surface from your plan.

