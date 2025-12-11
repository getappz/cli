# API Reference

Complete API documentation for saasctl.

## Core Types

### Task

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

#### Builder Methods

- `new<N>(name: N, action: AsyncTaskFn) -> Task`
  - Create a new task with name and action

- `desc<S>(self, description: S) -> Task`
  - Set task description

- `group<S>(self, group: S) -> Task`
  - Set task group (for categorization)

- `depends_on<S>(mut self, dep: S) -> Task`
  - Add a dependency

- `only_if<F>(mut self, cond: F) -> Task`
  - Add a condition (task runs only if all conditions are true)

- `unless<F>(mut self, cond: F) -> Task`
  - Add a negative condition (task runs unless any condition is true)

- `once(mut self) -> Task`
  - Mark task to run only once per session

- `hidden(mut self) -> Task`
  - Hide task from listings

- `timeout(mut self, seconds: u64) -> Task`
  - Set execution timeout in seconds

#### Methods

- `should_run(&self, ctx: &Context) -> bool`
  - Check if task should run based on conditions

### TaskRegistry

```rust
pub struct TaskRegistry {
    // ...
}
```

#### Methods

- `new() -> Self`
  - Create a new registry

- `register(&mut self, task: Task)`
  - Register a task

- `get(&self, name: &str) -> Option<&Task>`
  - Get task by name

- `get_mut(&mut self, name: &str) -> Option<&mut Task>`
  - Get mutable reference to task

- `all(&self) -> impl Iterator<Item = (&String, &Task)>`
  - Iterate over all tasks

- `before<T, H>(&mut self, target: T, hook: H)`
  - Register before hook

- `after<T, H>(&mut self, target: T, hook: H)`
  - Register after hook

- `fail<T, H>(&mut self, target: T, hook: H)`
  - Register failure hook

- `with_namespace<S>(&mut self, ns: S) -> NamespacedRegistry`
  - Create namespaced view

- `load_recipe<F>(&mut self, ns: &str, loader: F)`
  - Load recipe under namespace

### Runner

```rust
pub struct Runner<'a> {
    // ...
}
```

#### Constructors

- `new(registry: &'a TaskRegistry) -> Self`
  - Create runner with default verbosity (false)

- `new_verbose(registry: &'a TaskRegistry) -> Self`
  - Create runner with verbose output enabled

#### Methods

- `plan(&self, target: &str) -> TaskResult<Vec<String>>`
  - Plan task execution (returns task names in order)

- `run(&mut self, target: &str, ctx: &mut Context) -> TaskResult`
  - Execute task and all dependencies

- `get_operations(&self) -> Vec<Operation>`
  - Get all tracked operations

- `get_task_operations(&self, task_name: &str) -> Vec<Operation>`
  - Get operations for specific task

- `get_task_state(&self, task_name: &str) -> TaskState`
  - Get current state of task

### Context

```rust
pub struct Context {
    // ...
}
```

#### Constructors

- `new() -> Self`
  - Create empty context

- `with_vars(initial: HashMap<String, String>) -> Self`
  - Create context with initial variables

#### Variable Methods

- `set<K, V>(&self, key: K, val: V)`
  - Set variable (namespace-aware)

- `get(&self, key: &str) -> Option<String>`
  - Get variable (namespace-aware)

- `contains(&self, key: &str) -> bool`
  - Check if variable exists

- `remove(&self, key: &str) -> Option<String>`
  - Remove variable (namespace-aware)

- `parse(&self, s: &str) -> String`
  - Parse string with `{{variable}}` interpolation

#### Namespace Methods

- `with_namespace<F, T>(&self, ns: Option<&str>, fut: F) -> T`
  - Execute future with namespace bound

#### Environment Methods

- `set_env<K, V>(&mut self, key: K, val: V)`
  - Set environment variable

- `env(&self) -> &HashMap<String, String>`
  - Get environment map

- `set_dotenv<P>(&mut self, path: P)`
  - Set dotenv file path

- `dotenv(&self) -> Option<&str>`
  - Get dotenv file path

- `load_dotenv_into_env(&mut self)`
  - Load dotenv into environment map

#### Working Directory Methods

- `set_working_path<P>(&mut self, p: P)`
  - Set working directory

- `working_path(&self) -> Option<&PathBuf>`
  - Get working directory

## Function Types

### AsyncTaskFn

```rust
pub type AsyncTaskFn = Arc<
    dyn Fn(Arc<Context>) -> BoxFuture<'static, TaskResult> + Send + Sync
>;
```

Async task function that:
- Takes `Arc<Context>`
- Returns `BoxFuture<'static, TaskResult>`
- Is `Send + Sync` for thread safety

### Condition

```rust
pub type Condition = Arc<dyn Fn(&Context) -> bool + Send + Sync + 'static>;
```

Condition function that:
- Takes `&Context`
- Returns `bool`
- Used in `only_if` and `unless`

## Macros

### task_fn_async!

Wraps an async closure into `AsyncTaskFn`:

```rust
task_fn_async!(|ctx: Arc<Context>| async move {
    // async code
    Ok(())
})
```

### task_fn_sync!

Wraps a sync closure into `AsyncTaskFn`:

```rust
task_fn_sync!(|ctx: Arc<Context>| {
    // sync code
    Ok(())
})
```

## Enums

### TaskState

```rust
pub enum TaskState {
    Pending,
    Passed,
    Failed,
    Skipped,
}
```

### OperationType

```rust
pub enum OperationType {
    TaskExecution { task_name: String },
    DependencyCheck { task_name: String, dependencies: Vec<String> },
    ConditionCheck { task_name: String },
    TaskPlanning { target: String },
    WaveExecution { wave_index: usize, tasks: Vec<String> },
}
```

### OperationStatus

```rust
pub enum OperationStatus {
    Running,
    Passed,
    Failed,
    Skipped,
}
```

## Error Types

### TaskResult

```rust
pub type TaskResult = miette::Result<()>;
```

Alias for `miette::Result<()>`. Use `miette::miette!` for errors:

```rust
Err(miette!("Something went wrong"))
```

## Helper Functions

### extract_namespace

```rust
fn extract_namespace(task_name: &str) -> Option<String>
```

Extracts namespace from task name:
- `"laravel:deploy"` → `Some("laravel")`
- `":deploy:writable"` → `None` (global)
- `"deploy"` → `None` (global)

## Constants

### Task Name Patterns

- **Namespaced**: `"namespace:task"` (e.g., `"laravel:deploy"`)
- **Global**: `":task"` or `"task"` (e.g., `":deploy:writable"`)
- **Absolute**: `":task"` starts with `:` (explicitly global)

## Thread Safety

All public types are designed for concurrent access:

- **`Task`**: Immutable after creation (except via `get_mut`)
- **`TaskRegistry`**: Read-only during execution
- **`Runner`**: Uses `Arc<Mutex<>>` for shared state
- **`Context`**: Uses `Arc<RwLock<>>` for thread-safe variables

## Lifetime Considerations

- **`Runner<'a>`**: Borrows registry for lifetime `'a`
- **`AsyncTaskFn`**: `'static` (owned data)
- **`Context`**: `Clone` for sharing across threads

## Example: Complete API Usage

```rust
use task::{Task, TaskRegistry, Runner, Context};
use std::sync::Arc;

// Create registry
let mut reg = TaskRegistry::new();

// Register task
reg.register(
    Task::new(
        "hello",
        task::task_fn_sync!(|ctx: Arc<Context>| {
            println!("Hello from {}", ctx.get("name").unwrap_or("world".to_string()));
            Ok(())
        })
    )
    .desc("Greeting task")
    .depends_on("setup")
    .only_if(|ctx| ctx.contains("name"))
    .timeout(10)
);

// Create runner
let mut runner = Runner::new_verbose(&reg);

// Create context
let mut ctx = Context::new();
ctx.set("name", "saasctl");

// Plan and run
let plan = runner.plan("hello")?;
println!("Plan: {:?}", plan);

runner.run("hello", &mut ctx)?;
```

