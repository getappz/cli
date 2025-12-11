# Testing Guide

Guide for writing and running tests in saasctl.

## Running Tests

### All Tests

```bash
cargo test
```

### Specific Crate

```bash
# Test task crate
cargo test -p task

# Test CLI
cargo test -p cli
```

### With Output

```bash
cargo test -- --nocapture
```

### Single Test

```bash
cargo test test_name
```

## Writing Tests

### Basic Task Test

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use task::{Task, TaskRegistry, Context, Runner};

    #[test]
    fn test_task_execution() {
        let mut reg = TaskRegistry::new();
        reg.register(
            Task::new(
                "hello",
                task::task_fn_sync!(|_ctx: std::sync::Arc<Context>| {
                    println!("Hello");
                    Ok(())
                })
            )
        );
        
        let mut ctx = Context::new();
        let mut runner = Runner::new(&reg);
        
        assert!(runner.run("hello", &mut ctx).is_ok());
    }
}
```

### Testing Conditions

```rust
#[test]
fn test_conditional_task() {
    let mut reg = TaskRegistry::new();
    reg.register(
        Task::new(
            "prod-only",
            task::task_fn_sync!(|_ctx| Ok(()))
        )
        .only_if(|ctx| ctx.get("env") == Some("production".to_string()))
    );
    
    let mut ctx = Context::new();
    let mut runner = Runner::new(&reg);
    
    // Should skip when condition false
    ctx.set("env", "development");
    assert!(runner.run("prod-only", &mut ctx).is_ok());
    
    // Should run when condition true
    ctx.set("env", "production");
    assert!(runner.run("prod-only", &mut ctx).is_ok());
}
```

### Testing Dependencies

```rust
#[test]
fn test_dependencies() {
    let mut reg = TaskRegistry::new();
    
    let mut order = Vec::new();
    
    reg.register(
        Task::new(
            "first",
            task::task_fn_sync!(|_ctx| {
                order.push("first");
                Ok(())
            })
        )
    );
    
    reg.register(
        Task::new(
            "second",
            task::task_fn_sync!(|_ctx| {
                order.push("second");
                Ok(())
            })
        )
        .depends_on("first")
    );
    
    let mut ctx = Context::new();
    let mut runner = Runner::new(&reg);
    runner.run("second", &mut ctx).unwrap();
    
    assert_eq!(order, vec!["first", "second"]);
}
```

### Testing Namespace Scoping

```rust
#[tokio::test]
async fn test_namespace_scoping() {
    let ctx = Context::new();
    
    // Set in namespace
    ctx.with_namespace(Some("test"), async {
        ctx.set("key", "namespace-value");
        assert_eq!(ctx.get("key"), Some("namespace-value".to_string()));
    }).await;
    
    // Should not exist in base
    assert_eq!(ctx.get("key"), None);
    
    // Set in base
    ctx.set("key", "base-value");
    assert_eq!(ctx.get("key"), Some("base-value".to_string()));
}
```

### Testing Timeouts

```rust
#[tokio::test]
async fn test_task_timeout() {
    use tokio::time::{sleep, Duration};
    
    let mut reg = TaskRegistry::new();
    reg.register(
        Task::new(
            "slow-task",
            task::task_fn_async!(|_ctx| async move {
                sleep(Duration::from_secs(10)).await;
                Ok(())
            })
        )
        .timeout(1)  // 1 second timeout
    );
    
    let mut ctx = Context::new();
    let mut runner = Runner::new(&reg);
    
    let result = runner.run("slow-task", &mut ctx);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("timeout"));
}
```

### Testing Hooks

```rust
#[test]
fn test_hooks() {
    let mut reg = TaskRegistry::new();
    let mut order = Vec::new();
    
    reg.register(
        Task::new(
            "before-hook",
            task::task_fn_sync!(|_ctx| {
                order.push("before");
                Ok(())
            })
        )
    );
    
    reg.register(
        Task::new(
            "main",
            task::task_fn_sync!(|_ctx| {
                order.push("main");
                Ok(())
            })
        )
    );
    
    reg.register(
        Task::new(
            "after-hook",
            task::task_fn_sync!(|_ctx| {
                order.push("after");
                Ok(())
            })
        )
    );
    
    reg.before("main", "before-hook");
    reg.after("main", "after-hook");
    
    let mut ctx = Context::new();
    let mut runner = Runner::new(&reg);
    runner.run("main", &mut ctx).unwrap();
    
    assert_eq!(order, vec!["before", "main", "after"]);
}
```

### Testing Context Variables

```rust
#[test]
fn test_context_variables() {
    let mut ctx = Context::new();
    
    // Set and get
    ctx.set("key", "value");
    assert_eq!(ctx.get("key"), Some("value".to_string()));
    
    // Contains
    assert!(ctx.contains("key"));
    assert!(!ctx.contains("missing"));
    
    // Remove
    let removed = ctx.remove("key");
    assert_eq!(removed, Some("value".to_string()));
    assert_eq!(ctx.get("key"), None);
}
```

### Testing String Interpolation

```rust
#[test]
fn test_interpolation() {
    let ctx = Context::new();
    ctx.set("name", "world");
    ctx.set("greeting", "Hello");
    
    let result = ctx.parse("{{greeting}}, {{name}}!");
    assert_eq!(result, "Hello, world!");
    
    // Missing variable leaves template
    let result = ctx.parse("Hello, {{missing}}!");
    assert_eq!(result, "Hello, {{missing}}!");
}
```

### Testing Recipes

```rust
#[test]
fn test_recipe_registration() {
    let mut reg = TaskRegistry::new();
    my_recipe::register_my_recipe(&mut reg);
    
    // Check tasks are registered
    assert!(reg.get("myrecipe:deploy").is_some());
    assert!(reg.get("myrecipe:setup").is_some());
    
    // Check dependencies
    let task = reg.get("myrecipe:deploy").unwrap();
    assert!(task.deps.contains(&":deploy:prepare".to_string()));
}
```

### Integration Tests

Create integration tests in `tests/` directory:

```rust
// tests/integration_test.rs
use saasctl::task::{Task, TaskRegistry, Runner, Context};

#[test]
fn test_full_deployment() {
    let mut reg = TaskRegistry::new();
    // Register all recipes
    recipe::common::register_common(&mut reg);
    recipe::laravel::register_laravel(&mut reg);
    
    let mut ctx = Context::new();
    ctx.set("deploy_path", "/tmp/test-deploy");
    ctx.set("repository", "https://github.com/test/repo.git");
    
    let mut runner = Runner::new_verbose(&reg);
    
    // Plan
    let plan = runner.plan("laravel:deploy").unwrap();
    assert!(plan.len() > 0);
    
    // In a real test, you might mock filesystem or skip actual execution
}
```

## Async Testing

Use `#[tokio::test]` for async tests:

```rust
#[tokio::test]
async fn test_async_task() {
    let mut reg = TaskRegistry::new();
    reg.register(
        Task::new(
            "async-task",
            task::task_fn_async!(|_ctx| async move {
                tokio::time::sleep(Duration::from_secs(1)).await;
                Ok(())
            })
        )
    );
    
    let mut ctx = Context::new();
    let mut runner = Runner::new(&reg);
    
    // Note: Runner.run is sync, but internally uses async
    runner.run("async-task", &mut ctx).unwrap();
}
```

## Mocking

### Mocking External Commands

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_with_mocked_command() {
        // In a real scenario, you might use a crate like `mockall`
        // or create a test double
        
        let mut reg = TaskRegistry::new();
        reg.register(
            Task::new(
                "test-task",
                task::task_fn_sync!(|ctx| {
                    // Use test-friendly implementation
                    if cfg!(test) {
                        ctx.set("mock_result", "success");
                    } else {
                        // Real implementation
                        crate::shell::run_local("real-command")?;
                    }
                    Ok(())
                })
            )
        );
    }
}
```

## Test Utilities

### Helper Functions

```rust
#[cfg(test)]
mod test_utils {
    use task::{TaskRegistry, Context, Runner};
    
    pub fn create_test_registry() -> TaskRegistry {
        let mut reg = TaskRegistry::new();
        // Add test tasks
        reg
    }
    
    pub fn create_test_context() -> Context {
        let mut ctx = Context::new();
        ctx.set("test_mode", "true");
        ctx
    }
    
    pub fn run_task(reg: &TaskRegistry, name: &str) -> Result<()> {
        let mut ctx = create_test_context();
        let mut runner = Runner::new(reg);
        runner.run(name, &mut ctx)
    }
}
```

## Best Practices

1. **Test in isolation**: Each test should be independent
2. **Use descriptive names**: Test names should describe what they test
3. **Test edge cases**: Empty inputs, missing variables, etc.
4. **Test error cases**: Ensure errors are handled correctly
5. **Keep tests fast**: Use mocks for slow operations
6. **Test behavior, not implementation**: Focus on what the code does, not how
7. **Use fixtures**: Create helper functions for common setup
8. **Test namespaces**: Verify namespace scoping works correctly
9. **Test dependencies**: Ensure execution order is correct
10. **Test hooks**: Verify before/after hooks execute in right order

## Continuous Integration

Add to `.github/workflows/test.yml` or similar:

```yaml
name: Test

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo test --all
      - run: cargo test --all -- --nocapture
```

## Coverage

Generate test coverage:

```bash
# Install cargo-tarpaulin
cargo install cargo-tarpaulin

# Run with coverage
cargo tarpaulin --out Html
```

