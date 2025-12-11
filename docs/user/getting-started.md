# Getting Started with saasctl

saasctl is a Rust task runner CLI inspired by PHP Deployer, designed to orchestrate complex deployment workflows and task automation.

## Installation

### Prerequisites

- **Rust**: Install via [rustup.rs](https://rustup.rs/)
- **Cargo**: Comes with Rust installation

### Building from Source

```bash
# Clone the repository
git clone <repository-url>
cd saasctl

# Build in release mode
cargo build --release

# The binary will be at: target/release/saasctl (or saasctl.exe on Windows)
```

### Running

```bash
# Run directly with cargo (development)
cargo run -- --help

# Or use the built binary
./target/release/saasctl --help
```

## Quick Start

### 1. List Available Tasks

```bash
cargo run -- list
```

This shows all registered tasks. Hidden tasks (internal helpers) are excluded.

### 2. Plan a Task

See what a task will do and what dependencies it has:

```bash
cargo run -- plan deploy
```

This shows the execution plan in topological order, including all dependencies and hooks.

### 3. Run a Task

Execute a task:

```bash
cargo run -- run deploy
```

### 4. Verbose Output

See detailed progress:

```bash
cargo run -- --verbose run deploy
# or
cargo run -- -v run deploy
```

## Basic Concepts

### Tasks

A **task** is a named action that can:
- Execute commands or scripts
- Have dependencies on other tasks
- Run conditionally based on context
- Be hidden from listings
- Run only once per session

### Registry

The **task registry** stores all tasks and their relationships. Tasks are registered at startup from recipes and custom code.

### Runner

The **runner** plans and executes tasks in the correct order, respecting dependencies and running tasks in parallel when possible.

### Context

The **context** holds variables that tasks can read and write. Variables can be:
- Set via CLI or environment
- Namespace-scoped (e.g., Laravel-specific defaults)
- Used in string interpolation (`{{variable}}`)

### Recipes

**Recipes** are pre-built task collections for common workflows:
- **Common**: Standard deployment pipeline
- **Laravel**: Laravel-specific tasks and deployment
- **Tools**: Installation and verification for tools like mise, Docker, ddev

## Your First Task

Tasks are defined in Rust code. Here's a simple example:

```rust
use task::{Task, TaskRegistry};

pub fn register_my_tasks(reg: &mut TaskRegistry) {
    reg.register(
        Task::new(
            "hello",
            task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| {
                println!("Hello, world!");
                Ok(())
            })
        )
        .desc("Prints a greeting")
    );
}
```

Then call it:

```bash
cargo run -- run hello
```

## Next Steps

- Read the [User Guide](guide.md) for detailed usage
- Explore [Recipes](recipes.md) for common workflows
- Learn about [Context & Variables](context.md) for configuration
- Check [Advanced Features](advanced.md) for hooks, timeouts, and more

