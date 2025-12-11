# Extending saasctl

Guide for creating custom recipes, tasks, and extending functionality.

## Creating Recipes

A recipe is a collection of related tasks organized under a namespace.

### Basic Recipe Structure

```rust
use task::{Task, TaskRegistry};

pub fn register_my_recipe(reg: &mut TaskRegistry) {
    let mut my_recipe = reg.with_namespace("myapp");
    
    my_recipe.register(
        Task::new(
            "setup",
            task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| {
                println!("Setting up myapp");
                Ok(())
            })
        )
        .desc("Initial setup")
    );
    
    my_recipe.register(
        Task::new(
            "deploy",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                // Set namespace-scoped defaults
                ctx.set("build_dir", "dist");
                
                Ok(())
            })
        )
        .desc("Deploy application")
        .depends_on(":deploy:prepare")  // Use common recipe
        .depends_on("setup")
        .depends_on(":deploy:publish")
    );
}
```

### Using Namespaces

Namespaces prevent naming conflicts:

```rust
let mut myapp = reg.with_namespace("myapp");
myapp.register(Task::new("deploy", action));
// Task becomes: "myapp:deploy"

let mut other = reg.with_namespace("other");
other.register(Task::new("deploy", action));
// Task becomes: "other:deploy"
```

### Namespace Scoping

When your recipe's task calls `ctx.set()`, it automatically writes to your namespace's overlay:

```rust
task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
    // This writes to overlay["myapp"]["build_dir"]
    ctx.set("build_dir", "dist");
    Ok(())
})
```

Downstream tasks (even cross-namespace) automatically read from your namespace.

### Cross-Namespace Dependencies

Reference tasks from other recipes using `:` prefix:

```rust
Task::new("myapp:deploy", action)
    .depends_on(":deploy:prepare")    // From common recipe
    .depends_on(":deploy:publish")    // From common recipe
    .depends_on("artisan:migrate")     // From laravel recipe (if registered)
```

### Registering Recipes

In your main registry setup:

```rust
use crate::recipe::my_recipe;

fn build_registry() -> TaskRegistry {
    let mut reg = TaskRegistry::new();
    
    // Common recipes
    recipe::common::register_common(&mut reg);
    recipe::laravel::register_laravel(&mut reg);
    
    // Your custom recipe
    my_recipe::register_my_recipe(&mut reg);
    
    reg
}
```

## Creating Tasks

### Sync Tasks

For simple, CPU-bound operations:

```rust
Task::new(
    "process",
    task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
        let input = ctx.get("input")
            .ok_or_else(|| miette!("input not set"))?;
        
        let output = process_data(&input);
        ctx.set("output", output);
        
        Ok(())
    })
)
```

### Async Tasks

For I/O-bound operations:

```rust
Task::new(
    "fetch",
    task::task_fn_async!(|ctx: std::sync::Arc<task::Context>| async move {
        let url = ctx.get("url")
            .ok_or_else(|| miette!("url not set"))?;
        
        let response = reqwest::get(&url).await?;
        let body = response.text().await?;
        
        ctx.set("response", body);
        
        Ok(())
    })
)
```

### Error Handling

Always return proper errors:

```rust
use miette::{miette, Result};

task::task_fn_sync!(|ctx| -> Result<()> {
    // Use ? for automatic error propagation
    let path = std::fs::read_to_string("config.txt")?;
    
    // Use miette! for custom errors
    if path.is_empty() {
        return Err(miette!("config.txt is empty"));
    }
    
    // Use .context() for error chaining
    std::fs::write("output.txt", path)
        .map_err(|e| miette!("Failed to write: {}", e))?;
    
    Ok(())
})
```

### Reading Context

```rust
// Required variable (fail if missing)
let value = ctx.get("key")
    .ok_or_else(|| miette!("key must be set"))?;

// Optional variable with default
let value = ctx.get("key")
    .unwrap_or_else(|| "default".to_string());

// Check if variable exists
if ctx.contains("key") {
    // Use it
}
```

### Writing Context

```rust
// Set variable (namespace-aware)
ctx.set("key", "value");

// Use interpolation
ctx.set("path", "/var/www");
let cmd = ctx.parse("cd {{path}} && ls");
// Result: "cd /var/www && ls"
```

## Creating Helper Functions

### Reusable Task Actions

```rust
fn build_task(command: &str) -> task::AsyncTaskFn {
    task::task_fn_async!(|ctx: std::sync::Arc<task::Context>| async move {
        let workdir = ctx.get("workdir")
            .unwrap_or_else(|| ".".to_string());
        
        let cmd = format!("cd {} && {}", workdir, command);
        crate::shell::run_local(&cmd)
            .map_err(|e| miette!("Build failed: {}", e))?;
        
        Ok(())
    })
}

// Use it
reg.register(Task::new("build", build_task("npm run build")));
```

### Parameterized Tasks

```rust
pub struct TaskOptions {
    pub skip_if_no_env: bool,
    pub show_output: bool,
    pub timeout: Option<u64>,
}

fn create_task(name: &str, command: &str, opts: TaskOptions) -> Task {
    let action = task::task_fn_sync!(|ctx| {
        if opts.skip_if_no_env && !ctx.contains("env_file") {
            return Ok(());
        }
        
        if opts.show_output {
            println!("Running: {}", command);
        }
        
        crate::shell::run_local(command)
            .map_err(|e| miette!("Command failed: {}", e))
    });
    
    let mut task = Task::new(name, action);
    if let Some(timeout) = opts.timeout {
        task = task.timeout(timeout);
    }
    task
}
```

## Adding Hooks

### Before Hooks

```rust
reg.before("deploy", "lint");
reg.before("deploy", "test");
```

### After Hooks

```rust
reg.after("deploy", "notify");
reg.after("deploy", "cleanup");
```

### Failure Hooks

```rust
// Register cleanup task
reg.register(
    Task::new("deploy:failed", cleanup_action)
        .hidden()
);

// Register failure hook
reg.fail("deploy", "deploy:failed");
```

## Conditional Tasks

### Only If

```rust
Task::new("prod-deploy", action)
    .only_if(|ctx| ctx.get("env") == Some("production".to_string()))
    .only_if(|ctx| ctx.contains("deploy_token"))
```

### Unless

```rust
Task::new("dev-build", action)
    .unless(|ctx| ctx.get("ci") == Some("true".to_string()))
```

## Timeouts

```rust
Task::new("long-operation", action)
    .timeout(300)  // 5 minutes
```

## Best Practices

### 1. Use Namespaces

Always use namespaces for recipe tasks:

```rust
let mut recipe = reg.with_namespace("myrecipe");
// Task names become: "myrecipe:task"
```

### 2. Set Namespace Defaults

Use namespace scoping for recipe-specific defaults:

```rust
task::task_fn_sync!(|ctx| {
    // Automatically scoped to recipe namespace
    ctx.set("build_dir", "dist");
    ctx.set("output_dir", "public");
    Ok(())
})
```

### 3. Document Tasks

Add descriptions to all user-facing tasks:

```rust
Task::new("deploy", action)
    .desc("Deploys the application to production")
```

### 4. Hide Helper Tasks

Mark internal tasks as hidden:

```rust
Task::new("internal-helper", action)
    .hidden()
```

### 5. Handle Errors Gracefully

Provide clear error messages:

```rust
let path = ctx.get("path")
    .ok_or_else(|| miette!("path must be set for deployment"))?;
```

### 6. Use Dependencies

Compose tasks via dependencies rather than calling directly:

```rust
Task::new("deploy", action)
    .depends_on("build")
    .depends_on("test")
    .depends_on(":deploy:prepare")
```

### 7. Test Your Recipes

Write tests for your recipes:

```rust
#[test]
fn test_my_recipe() {
    let mut reg = TaskRegistry::new();
    my_recipe::register_my_recipe(&mut reg);
    
    assert!(reg.get("myrecipe:deploy").is_some());
    
    let mut ctx = Context::new();
    ctx.set("path", "/tmp/test");
    
    let mut runner = Runner::new(&reg);
    runner.run("myrecipe:setup", &mut ctx).unwrap();
}
```

## Example: Complete Recipe

```rust
use task::{Task, TaskRegistry};
use miette::{miette, Result};

pub fn register_nodejs(reg: &mut TaskRegistry) {
    let mut nodejs = reg.with_namespace("nodejs");
    
    // Setup task
    nodejs.register(
        Task::new(
            "install",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                let package_manager = ctx.get("package_manager")
                    .unwrap_or_else(|| "npm".to_string());
                
                let cmd = match package_manager.as_str() {
                    "npm" => "npm install",
                    "yarn" => "yarn install",
                    "pnpm" => "pnpm install",
                    _ => return Err(miette!("Unknown package manager: {}", package_manager)),
                };
                
                crate::shell::run_local(cmd)
                    .map_err(|e| miette!("Install failed: {}", e))
            })
        )
        .desc("Install dependencies")
    );
    
    // Build task
    nodejs.register(
        Task::new(
            "build",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                // Set namespace-scoped defaults
                ctx.set("build_command", "npm run build");
                ctx.set("output_dir", "dist");
                
                let cmd = ctx.get("build_command").unwrap();
                crate::shell::run_local(&cmd)
                    .map_err(|e| miette!("Build failed: {}", e))
            })
        )
        .desc("Build application")
        .depends_on("install")
    );
    
    // Deploy task
    nodejs.register(
        Task::new(
            "deploy",
            task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| {
                println!("Deploying Node.js application");
                Ok(())
            })
        )
        .desc("Deploy Node.js application")
        .depends_on("build")
        .depends_on(":deploy:prepare")
        .depends_on(":deploy:publish")
    );
}
```

## Testing Recipes

See [Testing Guide](testing.md) for detailed testing strategies.

