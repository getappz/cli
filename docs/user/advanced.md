# Advanced Features

This guide covers advanced features for power users.

## Timeouts

Tasks can have timeouts to prevent hanging indefinitely.

### Setting Timeouts

```rust
Task::new("long-task", action)
    .timeout(300)  // 300 seconds (5 minutes)
```

### How Timeouts Work

1. Task starts execution
2. If timeout is set, a cancellation token is created
3. If execution exceeds timeout, task is cancelled
4. Task marked as failed with timeout error

### Timeout Behavior

- **Async tasks**: Properly cancelled via `tokio::select!` and `CancellationToken`
- **Sync tasks**: Cancellation is cooperative (task should check cancellation if possible)
- **Dependencies**: Timeouts apply per-task, not to entire dependency chain

### Example

```rust
Task::new("slow-operation", 
    task::task_fn_async!(|_ctx| async move {
        tokio::time::sleep(Duration::from_secs(600)).await;
        Ok(())
    }))
    .timeout(60)  // Fails after 60 seconds
```

## Hooks

Hooks allow running tasks before, after, or on failure of other tasks.

### Before Hooks

Run tasks before another task:

```rust
reg.before("deploy", "lint");
reg.before("deploy", "test");
reg.before("deploy", "build");
```

Execution order:
1. `lint`
2. `test`
3. `build`
4. `deploy`

### After Hooks

Run tasks after another task (even on failure):

```rust
reg.after("deploy", "notify");
reg.after("deploy", "cleanup");
```

Execution order:
1. `deploy`
2. `notify` (runs even if deploy fails)
3. `cleanup` (runs even if deploy fails)

### Failure Hooks

Run a task if another task fails:

```rust
// Register cleanup task
reg.register(
    Task::new("deploy:failed", cleanup_action)
        .hidden()
);

// Register failure hook
reg.fail("deploy", "deploy:failed");
```

If any step in `deploy` fails, `deploy:failed` runs.

**Note**: Failure hooks are best-effort. If the failure hook itself fails, the error may not be reported.

### Hook Execution Order

For task `deploy` with hooks:

1. Before hooks (in registration order)
2. Main task
3. After hooks (even on failure)
4. Failure hook (only on failure)

### Multiple Hooks

You can register multiple hooks of the same type:

```rust
reg.before("deploy", "lint");
reg.before("deploy", "test");
reg.before("deploy", "security-scan");
// All run before deploy
```

## Async Tasks

Tasks can be async for non-blocking operations.

### Defining Async Tasks

```rust
Task::new(
    "fetch-data",
    task::task_fn_async!(|ctx: std::sync::Arc<task::Context>| async move {
        // Async operations
        let data = fetch_from_api().await?;
        ctx.set("api_data", &data);
        Ok(())
    })
)
```

### When to Use Async

- Network I/O (HTTP requests, database queries)
- File I/O (async file operations)
- Waiting operations (timers, delays)
- Concurrent operations

### Sync Tasks

For CPU-bound or simple operations, use sync tasks:

```rust
Task::new(
    "process-data",
    task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
        let data = ctx.get("input").unwrap();
        let processed = heavy_computation(&data);
        ctx.set("output", processed);
        Ok(())
    })
)
```

## Parallel Execution

Tasks in the same wave execute in parallel.

### Wave Calculation

The runner groups tasks into waves based on dependencies:

```rust
// Task dependencies:
// A depends on nothing
// B depends on nothing  
// C depends on A
// D depends on B
// E depends on C and D

// Waves:
// Wave 1: A, B (parallel)
// Wave 2: C, D (parallel, after Wave 1)
// Wave 3: E (after Wave 2)
```

### Maximizing Parallelism

Structure dependencies to maximize parallel execution:

```rust
// Good: Independent tasks can run in parallel
Task::new("task1", action1);
Task::new("task2", action2);
Task::new("task3", action3)
    .depends_on("task1")
    .depends_on("task2");

// Waves:
// Wave 1: task1, task2 (parallel)
// Wave 2: task3
```

### Sequential Execution

Force sequential execution with dependencies:

```rust
Task::new("step1", action1);
Task::new("step2", action2)
    .depends_on("step1");
Task::new("step3", action3)
    .depends_on("step2");
```

## Verbose Mode

Enable detailed output for debugging:

```bash
cargo run -- -v run deploy
```

### What You See

- **Wave information**:
  ```
  --- Wave 1: 3 task(s)
  ```

- **Task execution**:
  ```
  → Running deploy:info
  ✓ Passed deploy:info
  → Running deploy:setup
  ✗ Failed deploy:setup
  ```

- **Error details**: Full error messages with context

### When to Use

- Debugging task execution
- Understanding execution order
- Verifying hooks run
- Troubleshooting failures

## Operation Tracking

The runner tracks operations for analysis:

### Operation Types

- **TaskExecution**: Main task execution
- **DependencyCheck**: Checking dependency states
- **ConditionCheck**: Evaluating task conditions
- **TaskPlanning**: Building execution plan
- **WaveExecution**: Executing a wave

### Accessing Operations

```rust
let operations = runner.get_operations();
for op in operations {
    println!("{:?}: {:?}", op.op_type, op.status);
}
```

### Operation Status

- **Running**: Operation in progress
- **Passed**: Operation completed successfully
- **Failed**: Operation failed
- **Skipped**: Operation was skipped

## Error Handling

### Task Errors

Tasks return `TaskResult` (alias for `miette::Result<()>`):

```rust
use miette::{miette, Result};

task::task_fn_sync!(|_ctx| -> Result<()> {
    // Success
    Ok(())
    
    // Error
    Err(miette!("Something went wrong"))
})
```

### Error Propagation

Errors automatically propagate through the dependency chain:

```rust
// If "setup" fails, "build" is skipped
Task::new("build", build_action)
    .depends_on("setup");
```

### Error Context

Use `miette` for rich error messages:

```rust
use miette::{miette, Context};

task::task_fn_sync!(|ctx| -> Result<()> {
    let path = ctx.get("path")
        .ok_or_else(|| miette!("path not set"))
        .context("Failed to get deployment path")?;
    
    // More operations...
    Ok(())
})
```

## Conditional Execution

### Only If

Run task only if condition is true:

```rust
Task::new("prod-deploy", action)
    .only_if(|ctx| ctx.get("env") == Some("production".to_string()));
```

### Unless

Run task unless condition is true:

```rust
Task::new("dev-build", action)
    .unless(|ctx| ctx.get("ci") == Some("true".to_string()));
```

### Multiple Conditions

Conditions are combined with AND logic:

```rust
Task::new("deploy", action)
    .only_if(|ctx| ctx.get("env") == Some("production".to_string()))
    .only_if(|ctx| ctx.contains("deploy_path"));
// Runs only if BOTH conditions are true
```

### Condition Evaluation

- Conditions are evaluated **before** task execution
- If condition fails, task is **skipped** (not failed)
- Skipped tasks don't block dependencies (they're treated as passed)

## Once Flag

Prevent task from running multiple times:

```rust
Task::new("setup", setup_action).once();
```

### Use Cases

- Initialization tasks
- One-time setup operations
- Resource acquisition

### Behavior

- First execution: Task runs normally
- Subsequent references: Task is skipped (treated as already completed)
- Tracking: Uses a process-wide set (`ran_once`)

## Hidden Tasks

Hide tasks from listings:

```rust
Task::new("internal-helper", action).hidden();
```

### When to Use

- Internal helper tasks
- Tasks only called via dependencies
- Utility tasks not meant for direct execution

### Still Executable

Hidden tasks can still be:
- Called via dependencies
- Referenced in hooks
- Executed directly (if you know the name)

## Best Practices

1. **Use timeouts for long-running tasks**: Prevent indefinite hangs
2. **Structure for parallelism**: Minimize unnecessary dependencies
3. **Handle errors gracefully**: Provide clear error messages
4. **Use hooks wisely**: Don't create complex hook chains
5. **Test conditions**: Ensure conditions work as expected
6. **Document behavior**: Especially for complex task compositions
7. **Use verbose mode for debugging**: Helps understand execution flow

