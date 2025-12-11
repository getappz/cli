# User Guide

Complete reference for using saasctl effectively.

## Command Reference

### List Tasks

```bash
cargo run -- list
```

Lists all available tasks (excluding hidden tasks). Shows:
- Task name
- Description (if provided)
- Group (if provided)

### Plan Task Execution

```bash
cargo run -- plan <task-name>
```

Shows the execution plan:
- All tasks that will run (including dependencies)
- Execution order (topological sort)
- Before/after hooks
- Any warnings or errors

Example:
```bash
cargo run -- plan laravel:deploy
```

Output shows all tasks in the deployment pipeline:
```
deploy:info
deploy:setup
deploy:lock
deploy:release
deploy:update_code
...
laravel:deploy
```

### Run a Task

```bash
cargo run -- run <task-name>
```

Executes a task and all its dependencies.

#### Options

- `--verbose` or `-v`: Show detailed progress
  - Wave numbers
  - Task start/success/failure indicators
  - Useful for debugging

Example:
```bash
cargo run -- -v run laravel:deploy
```

Output:
```
--- Wave 1: 5 task(s)
→ Running deploy:info
✓ Passed deploy:info
→ Running deploy:setup
✓ Passed deploy:setup
...
```

## Task Features

### Dependencies

Tasks can depend on other tasks. Dependencies are executed before the task itself:

```rust
Task::new("build", action)
    .depends_on("clean")
    .depends_on("compile")
```

Execution order: `clean` → `compile` → `build`

### Conditions

Tasks can run conditionally:

```rust
Task::new("prod-deploy", action)
    .only_if(|ctx| ctx.get("env") == Some("production"))
```

Only runs if context variable `env` equals `"production"`.

You can also use `unless`:

```rust
Task::new("dev-build", action)
    .unless(|ctx| ctx.get("ci") == Some("true"))
```

Runs unless `ci` is `"true"`.

### Once Flag

Mark a task to run only once, even if referenced multiple times:

```rust
Task::new("setup", action).once()
```

Useful for initialization tasks that should only run once per session.

### Hidden Tasks

Hide tasks from listings (but they still execute if called):

```rust
Task::new("internal-helper", action).hidden()
```

Useful for helper tasks that users shouldn't call directly.

### Timeouts

Set a timeout for task execution:

```rust
Task::new("long-running", action)
    .timeout(300)  // 300 seconds (5 minutes)
```

If the task exceeds the timeout, it's cancelled and marked as failed.

## Hooks

### Before Hooks

Run a task before another task:

```rust
reg.before("deploy", "lint");
reg.before("deploy", "test");
```

Both `lint` and `test` run before `deploy`.

### After Hooks

Run a task after another task completes:

```rust
reg.after("deploy", "notify");
```

`notify` runs after `deploy` completes (even if `deploy` fails).

### Failure Hooks

Run a task if another task fails:

```rust
reg.register(Task::new("deploy:failed", cleanup_action).hidden());
reg.fail("deploy", "deploy:failed");
```

If any step in `deploy` fails, `deploy:failed` runs.

## Context Variables

### Setting Variables

Variables can be set programmatically or via environment:

```rust
let mut ctx = Context::new();
ctx.set("deploy_path", "/var/www/myapp");
ctx.set("repository", "git@github.com:user/repo.git");
```

### Reading Variables

Tasks read variables from context:

```rust
task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
    let path = ctx.get("deploy_path")
        .ok_or_else(|| miette!("deploy_path not set"))?;
    // Use path...
    Ok(())
})
```

### String Interpolation

Use `{{variable}}` syntax in strings:

```rust
let cmd = ctx.parse("cd {{release_path}} && php artisan migrate");
// If release_path = "/var/www/myapp/releases/1234567890"
// cmd = "cd /var/www/myapp/releases/1234567890 && php artisan migrate"
```

### Namespace Scoping

When running a namespaced task (e.g., `laravel:deploy`), variables set by that recipe are scoped to its namespace:

```rust
// Inside laravel:deploy task
ctx.set("writable_dirs", "storage/statamic");
// This is automatically scoped to "laravel" namespace
```

Downstream tasks (like `:deploy:writable`) automatically read from the caller's namespace overlay first, then fall back to global variables.

## Execution Flow

### Planning Phase

1. Resolve task and all dependencies (transitive)
2. Include before/after hooks
3. Topological sort (dependency order)
4. Group into waves (parallel execution groups)

### Execution Phase

1. Execute each wave sequentially
2. Within a wave, tasks run in parallel (using `rayon`)
3. If any task fails, remaining tasks in the wave are skipped
4. After hooks run even if the main task fails

### Example Flow

For `laravel:deploy`:

```
Wave 1 (parallel):
  - deploy:info
  - deploy:setup

Wave 2 (sequential after Wave 1):
  - deploy:lock

Wave 3 (parallel):
  - deploy:release
  - artisan:storage:link

Wave 4:
  - laravel:deploy (sets namespace variables)
  - deploy:writable (reads namespace variables)

Wave 5:
  - deploy:publish
```

## Error Handling

### Task Failures

If a task returns an error:
- The task is marked as failed
- Remaining tasks in the current wave are skipped
- Failure hooks (if any) are executed
- The run terminates with an error

### Dependency Failures

If a dependency fails:
- Dependent tasks are skipped
- The run terminates

### Timeout Failures

If a task times out:
- The task is cancelled
- Marked as failed
- Run terminates

## Best Practices

1. **Use meaningful task names**: Follow the pattern `namespace:action` (e.g., `laravel:deploy`)

2. **Set context early**: Set required variables before running tasks

3. **Use hidden tasks**: Mark internal helpers as hidden

4. **Group related tasks**: Use dependencies to compose workflows

5. **Add descriptions**: Help users understand what tasks do:
   ```rust
   Task::new("deploy", action)
       .desc("Deploys the application to production")
   ```

6. **Use conditions wisely**: Don't overcomplicate logic in conditions

7. **Leverage recipes**: Use existing recipes rather than reinventing workflows

## Troubleshooting

### Task Not Found

```
Error: Unknown task 'my-task'
```

- Check task name spelling
- Ensure the recipe is registered
- Use `list` to see available tasks

### Dependency Loop

```
Error: Circular dependency detected
```

- Check task dependencies for cycles
- Review the plan output to identify the loop

### Context Variable Missing

```
Error: deploy_path not set
```

- Set the variable before running:
  ```rust
  ctx.set("deploy_path", "/path/to/deploy");
  ```
- Or check the task's requirements

### Task Timing Out

- Increase the timeout if the task legitimately takes longer
- Check for infinite loops or hanging operations
- Review task logic for efficiency

