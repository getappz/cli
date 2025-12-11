# Context & Variables

The context system manages variables that tasks can read and write. It supports namespace scoping, string interpolation, and thread-safe access.

## Overview

The `Context` holds:
- **Variables**: Key-value pairs for configuration (`vars`)
- **Environment variables**: For subprocess execution (`env`)
- **Namespace overlays**: Per-namespace variable overrides
- **Working directory**: Base path for relative operations
- **Dotenv path**: Path to `.env` file for loading

## Basic Usage

### Creating a Context

```rust
use task::Context;

let mut ctx = Context::new();
```

### Setting Variables

```rust
ctx.set("deploy_path", "/var/www/myapp");
ctx.set("repository", "git@github.com:user/repo.git");
ctx.set("branch", "main");
```

Variables are stored as strings.

### Reading Variables

```rust
match ctx.get("deploy_path") {
    Some(path) => println!("Deploy path: {}", path),
    None => println!("Deploy path not set"),
}
```

### Checking for Variables

```rust
if ctx.contains("deploy_path") {
    // Variable exists
}
```

### Removing Variables

```rust
ctx.remove("deploy_path");
```

## String Interpolation

Use `{{variable}}` syntax to interpolate variables in strings:

```rust
ctx.set("release_path", "/var/www/myapp/releases/1234567890");
let cmd = ctx.parse("cd {{release_path}} && ls -la");
// Result: "cd /var/www/myapp/releases/1234567890 && ls -la"
```

### Multiple Variables

```rust
ctx.set("deploy_path", "/var/www/myapp");
ctx.set("releases_dir", "releases");
let path = ctx.parse("{{deploy_path}}/{{releases_dir}}");
// Result: "/var/www/myapp/releases"
```

### Missing Variables

If a variable is not found, the template string is left unchanged:

```rust
let result = ctx.parse("cd {{missing_var}}");
// Result: "cd {{missing_var}}" (not replaced)
```

## Namespace Scoping

When running a namespaced task (e.g., `laravel:deploy`), variables can be scoped to that namespace.

### How It Works

1. **Entry target determines namespace**: Running `laravel:deploy` binds namespace `"laravel"` for the entire run
2. **Automatic scoping**: `ctx.set()` inside a namespaced task writes to that namespace's overlay
3. **Inheritance**: Cross-namespace tasks (like `:deploy:writable`) automatically read from the caller's namespace

### Example

```rust
// In laravel:deploy task
ctx.set("writable_dirs", "storage/statamic");
// Writes to overlay["laravel"]["writable_dirs"]

// In :deploy:writable task (called as dependency)
let dirs = ctx.get("writable_dirs");
// Reads from overlay["laravel"]["writable_dirs"] first,
// then falls back to base vars if not found
```

### Benefits

- **Isolation**: One recipe can't overwrite another's defaults
- **Automatic inheritance**: Dependencies automatically see caller's namespace
- **No API changes**: Tasks use `ctx.set()` as normal

### Setting Namespace Variables

You don't need special syntax. Just call `ctx.set()` inside a namespaced task:

```rust
task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
    // This automatically writes to the current namespace overlay
    ctx.set("writable_dirs", "storage,storage/logs");
    Ok(())
})
```

### Reading Across Namespaces

Tasks automatically check namespace overlay first, then base:

```rust
// Resolution order:
// 1. Check namespace overlay (if namespace bound)
// 2. Check base vars
let value = ctx.get("writable_dirs");
```

## Environment Variables

Context can manage environment variables for subprocess execution.

### Setting Environment Variables

```rust
ctx.set_env("APP_ENV", "production");
ctx.set_env("APP_DEBUG", "false");
```

### Reading Environment Map

```rust
let env_map = ctx.env();
for (key, value) in env_map {
    println!("{}={}", key, value);
}
```

### Loading from .env File

```rust
ctx.set_dotenv("./.env");
ctx.load_dotenv_into_env();
```

This loads `KEY=value` pairs from the `.env` file into the context's environment map.

## Working Directory

Set a base working directory for relative paths:

```rust
ctx.set_working_path("/var/www/myapp");
```

Tasks can use this for relative path resolution.

## Thread Safety

Context is thread-safe:
- Uses `Arc<RwLock<>>` for shared state
- Multiple tasks can read concurrently
- Writes are serialized via `RwLock`
- Namespace binding uses `tokio::task_local` for async-safe scoping

## Variable Resolution Order

When reading a variable:

1. **Namespace overlay** (if namespace bound via task execution)
2. **Base variables** (global scope)

When writing a variable:

1. **Namespace overlay** (if namespace bound)
2. **Base variables** (if no namespace)

## Common Patterns

### Required Variables

```rust
let path = ctx.get("deploy_path")
    .ok_or_else(|| miette!("deploy_path must be set"))?;
```

### Variables with Defaults

```rust
let branch = ctx.get("branch")
    .unwrap_or_else(|| "main".to_string());
```

### Building Commands

```rust
ctx.set("release_path", "/var/www/releases/123");
let cmd = format!("cd {} && php artisan migrate", 
    ctx.get("release_path").unwrap());
```

Or with interpolation:

```rust
ctx.set("release_path", "/var/www/releases/123");
let cmd = ctx.parse("cd {{release_path}} && php artisan migrate");
```

### Conditional Logic

```rust
if ctx.get("env") == Some("production".to_string()) {
    // Production-specific logic
}
```

## Best Practices

1. **Set variables early**: Set required variables before running tasks
2. **Use meaningful names**: Clear variable names help debugging
3. **Document requirements**: Document which variables each task needs
4. **Use namespaces for defaults**: Recipe defaults should use namespace scoping
5. **Provide defaults where appropriate**: Use `unwrap_or` for optional variables
6. **Validate required variables**: Fail fast if required variables are missing

## Example: Complete Deployment Context

```rust
let mut ctx = Context::new();

// Required
ctx.set("deploy_path", "/var/www/myapp");
ctx.set("repository", "git@github.com:user/repo.git");

// Optional with defaults
if !ctx.contains("branch") {
    ctx.set("branch", "main");
}

// Shared resources
ctx.set("shared_dirs", "storage,logs");
ctx.set("shared_files", ".env");

// Writable directories
ctx.set("writable_dirs", "storage");

// Environment variables
ctx.set_env("APP_ENV", "production");
ctx.set_dotenv("./.env.production");
ctx.load_dotenv_into_env();

// Run deployment
runner.run("deploy", &mut ctx)?;
```

