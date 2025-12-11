# Architecture Overview

This document describes the architecture and design of saasctl.

## System Overview

saasctl is a task orchestration system built in Rust. It provides:

- **Task registry**: Registration and storage of tasks
- **Dependency resolution**: Automatic topological sorting
- **Parallel execution**: Concurrent task execution within waves
- **Context management**: Variable storage with namespace scoping
- **Async support**: Full async/await for I/O-bound operations

## Core Components

### Task (`crates/task/src/task.rs`)

A `Task` represents a single unit of work:

```rust
pub struct Task {
    pub name: String,
    pub description: Option<String>,
    pub group: Option<String>,
    pub deps: Vec<String>,
    pub only_if: Vec<Condition>,
    pub unless: Vec<Condition>,
    pub once: bool,
    pub hidden: bool,
    pub timeout: Option<u64>,
    pub action: AsyncTaskFn,
}
```

Key features:
- **Dependencies**: List of task names that must run first
- **Conditions**: Functions that determine if task should run
- **Action**: Async function that executes the task
- **Metadata**: Description, group, flags

### Registry (`crates/task/src/registry.rs`)

The `TaskRegistry` stores all tasks and hooks:

```rust
pub struct TaskRegistry {
    tasks: HashMap<String, Task>,
    pub hooks: Hooks,
    pub fail_map: HashMap<String, String>,
}
```

Features:
- **Task storage**: Lookup by name
- **Hooks**: Before/after task relationships
- **Failure hooks**: Tasks to run on failure
- **Namespacing**: Support for recipe namespaces

#### Namespaced Registry

Recipes can use a `NamespacedRegistry` to automatically qualify task names:

```rust
let mut laravel = reg.with_namespace("laravel");
laravel.register(Task::new("deploy", action));
// Task name becomes: "laravel:deploy"
```

This prevents naming conflicts between recipes.

### Runner (`crates/task/src/runner.rs`)

The `Runner` plans and executes tasks:

```rust
pub struct Runner<'a> {
    registry: &'a TaskRegistry,
    ran_once: Arc<Mutex<HashSet<String>>>,
    task_states: Arc<Mutex<HashMap<String, TaskState>>>,
    operations: Arc<Mutex<Vec<Operation>>>,
    verbose: bool,
}
```

#### Planning Phase

1. **Collect reachable tasks**: Starting from target, follow dependencies and hooks
2. **Build dependency graph**: Create adjacency map
3. **Topological sort**: Kahn's algorithm to determine execution order
4. **Group into waves**: Tasks with no unsatisfied dependencies form a wave

#### Execution Phase

1. **Extract namespace**: From entry target (e.g., `laravel:deploy` → `laravel`)
2. **Execute waves sequentially**: Each wave waits for previous to complete
3. **Execute tasks in parallel**: Within a wave, use `rayon` for parallelism
4. **Track operations**: Log all operations for analysis
5. **Handle failures**: Skip remaining tasks, run failure hooks

#### Async Integration

Tasks are async, but runner uses `block_on` within `rayon::scope`:

```rust
tokio::runtime::Handle::block_on(async {
    ctx.with_namespace(run_ns, async {
        execute_task_with_timeout(task_fn, ctx).await
    }).await
})
```

This allows async tasks while maintaining parallel execution via rayon.

### Context (`crates/task/src/context.rs`)

The `Context` manages variables and environment:

```rust
pub struct Context {
    vars: Arc<RwLock<HashMap<String, String>>>,
    namespace_overlays: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
    env: HashMap<String, String>,
    dotenv_path: Option<String>,
    working_path: Option<PathBuf>,
}
```

#### Namespace Scoping

Uses `tokio::task_local!` for async-safe namespace binding:

```rust
tokio::task_local! {
    static CURRENT_NAMESPACE: RefCell<Option<String>>;
}
```

When `ctx.set()` is called:
1. Check `CURRENT_NAMESPACE` (set by `with_namespace()`)
2. If namespace exists, write to `namespace_overlays[namespace]`
3. Otherwise, write to base `vars`

When `ctx.get()` is called:
1. Check namespace overlay first (if namespace bound)
2. Fall back to base `vars`

This enables automatic namespace scoping without API changes.

### Types (`crates/task/src/types.rs`)

Defines async task function type and macros:

```rust
pub type AsyncTaskFn = Arc<dyn Fn(Arc<Context>) -> BoxFuture<'static, TaskResult> + Send + Sync>;
```

#### Macros

- **`task_fn_async!`**: For async tasks
  ```rust
  task_fn_async!(|ctx| async move {
      // async code
      Ok(())
  })
  ```

- **`task_fn_sync!`**: For sync tasks (wraps in async)
  ```rust
  task_fn_sync!(|ctx| {
      // sync code
      Ok(())
  })
  ```

## Execution Flow

### High-Level Flow

```
1. User runs: cargo run -- run laravel:deploy
2. CLI parses arguments, builds registry
3. Runner.plan("laravel:deploy")
   - Collects all dependencies
   - Topological sort
   - Groups into waves
4. Runner.run("laravel:deploy", &mut ctx)
   - Extract namespace: "laravel"
   - For each wave:
     - For each task in wave (parallel):
       - Bind namespace via ctx.with_namespace()
       - Execute task action
       - Track operation
5. Report results
```

### Task Execution Details

```
1. Check conditions (only_if, unless)
2. Check dependencies are complete
3. Check once flag
4. Execute action:
   - Wrap in ctx.with_namespace()
   - Set timeout if configured
   - Run async task function
   - Handle cancellation on timeout
5. Mark task state (Passed/Failed/Skipped)
6. Run after hooks
7. Run failure hook if failed
```

## Concurrency Model

### Rayon for Parallelism

Within a wave, tasks execute in parallel using `rayon::scope`:

```rust
rayon::scope(|s| {
    for task_name in wave {
        s.spawn(|_| {
            // Execute task (may block on tokio)
        });
    }
});
```

### Tokio for Async

Tasks are async and run on tokio runtime:

```rust
tokio::runtime::Handle::block_on(async {
    // Execute async task
})
```

### Thread Safety

- **Context**: `Arc<RwLock<>>` for thread-safe sharing
- **Registry**: Read-only during execution (tasks stored before execution)
- **Runner state**: `Arc<Mutex<>>` for shared mutable state

## Error Handling

### Error Types

Uses `miette` for error reporting:

```rust
pub type TaskResult = miette::Result<()>;
```

### Error Propagation

- Task errors propagate to runner
- Failed tasks cause dependent tasks to be skipped
- Failure hooks can handle cleanup

### Error Context

Rich error messages via `miette::Context`:

```rust
task_fn.await
    .context(format!("Task '{}' failed", task_name))
```

## Timeout Handling

### Implementation

Uses `tokio::select!` and `CancellationToken`:

```rust
let timeout_token = CancellationToken::new();
tokio::spawn(async move {
    tokio::time::sleep(Duration::from_secs(timeout)).await;
    timeout_token.cancel();
});

tokio::select! {
    _ = timeout_token.cancelled() => Err(timeout_error),
    result = task_fn(ctx) => result,
}
```

### Cancellation

- Async tasks: Properly cancelled via token
- Sync tasks: Cooperative (should check if possible)
- Resources: Tokio handles cleanup

## Extension Points

### Recipes

Recipes register tasks:

```rust
pub fn register_my_recipe(reg: &mut TaskRegistry) {
    reg.register(Task::new("my:task", action));
}
```

### Custom Tasks

Users can define custom tasks in their codebase.

### Context Extensions

Context can be extended with custom methods (via wrapper structs).

## Design Principles

1. **Composability**: Tasks compose via dependencies
2. **Parallelism**: Maximize parallel execution where possible
3. **Safety**: Thread-safe, no data races
4. **Ergonomics**: Simple API, macros for common patterns
5. **Extensibility**: Easy to add recipes and tasks
6. **Performance**: Efficient dependency resolution and execution

## Future Improvements

- Full async runner (remove `block_on` in rayon)
- Task caching (like moonrepo)
- Retry mechanisms
- Better cancellation for sync tasks
- Task profiling and metrics

