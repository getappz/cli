# Recipes

Recipes are pre-built task collections for common workflows. This guide covers all available recipes and how to use them.

## Common Recipe

The common recipe provides a standard deployment pipeline similar to PHP Deployer's common recipe.

### Tasks

#### Main Tasks

- **`deploy:prepare`** - Prepares a new release
  - Runs: `deploy:info`, `deploy:setup`, `deploy:lock`, `deploy:release`, `deploy:update_code`, `deploy:env`, `deploy:shared`, `deploy:writable`

- **`deploy:publish`** - Publishes the release
  - Runs: `deploy:symlink`, `deploy:unlock`, `deploy:cleanup`, `deploy:success`

- **`deploy`** - Complete deployment (combines prepare and publish)
  - Runs: `deploy:prepare` → `deploy:publish`

#### Building Block Tasks

These are typically called via dependencies, not directly:

- **`deploy:info`** - Shows deployment information (hidden)
- **`deploy:setup`** - Creates deployment directory structure (hidden)
- **`deploy:lock`** - Locks deployment (prevents concurrent deploys)
- **`deploy:unlock`** - Removes deployment lock
- **`deploy:release`** - Creates a new release directory
- **`deploy:update_code`** - Updates code (git clone or local copy)
- **`deploy:env`** - Manages `.env` file
- **`deploy:shared`** - Links shared files and directories
- **`deploy:writable`** - Makes directories writable
- **`deploy:symlink`** - Creates symlink to current release
- **`deploy:cleanup`**** - Cleans up old releases
- **`deploy:success`** - Success notification (hidden)

### Required Context Variables

- **`deploy_path`** - Base deployment directory (e.g., `/var/www/myapp`)
- **`release_path`** - Automatically set by `deploy:release`
- **`current_path`** - Symlink target (defaults to `{{deploy_path}}/current`)

### Optional Context Variables

- **`repository`** - Git repository URL (for `deploy:update_code`)
- **`branch`** - Git branch (defaults to `"main"`)
- **`local_source`** - Local directory to copy instead of git clone
- **`shared_dirs`** - Comma-separated list of shared directories
- **`shared_files`** - Comma-separated list of shared files
- **`writable_dirs`** - Comma-separated list of directories to make writable

### Example

```rust
let mut ctx = Context::new();
ctx.set("deploy_path", "/var/www/myapp");
ctx.set("repository", "git@github.com:user/repo.git");
ctx.set("branch", "main");
ctx.set("shared_dirs", "storage,logs");
ctx.set("writable_dirs", "storage");

// Run deployment
runner.run("deploy", &mut ctx)?;
```

## Laravel Recipe

The Laravel recipe provides Laravel-specific tasks and a complete deployment workflow.

### Registration

```rust
use crate::recipe::laravel;

laravel::register_laravel(&mut reg);
```

### Artisan Tasks

All artisan tasks follow the pattern `artisan:<command>`.

#### Maintenance

- **`artisan:down`** - Put application in maintenance mode
- **`artisan:up`** - Bring application out of maintenance mode

#### Keys & Authentication

- **`artisan:key:generate`** - Generate application encryption key
- **`artisan:passport:keys`** - Generate Passport encryption keys

#### Database & Migrations

- **`artisan:migrate`** - Run database migrations (skips if no `.env`)
- **`artisan:migrate:fresh`** - Drop all tables and re-run migrations
- **`artisan:migrate:rollback`** - Rollback the last database migration batch
- **`artisan:migrate:status`** - Show migration status
- **`artisan:db:seed`** - Seed the database

#### Cache & Optimization

- **`artisan:cache:clear`** - Clear application cache
- **`artisan:config:cache`** - Cache configuration files
- **`artisan:config:clear`** - Clear configuration cache
- **`artisan:optimize`** - Cache framework files for better performance
- **`artisan:optimize:clear`** - Clear all cached files
- **`artisan:route:cache`** - Cache route files
- **`artisan:route:clear`** - Clear route cache
- **`artisan:route:list`** - List all registered routes
- **`artisan:view:cache`** - Compile all view files
- **`artisan:view:clear`** - Clear compiled view files

#### Events (Laravel 5.8.9+)

- **`artisan:event:cache`** - Discover and cache events
- **`artisan:event:clear`** - Clear event cache
- **`artisan:event:list`** - List all registered events

#### Storage

- **`artisan:storage:link`** - Create symbolic link for storage (Laravel 5.3+)

### Deployment Task

- **`laravel:deploy`** - Complete Laravel deployment
  - Sets namespace-scoped defaults (e.g., `writable_dirs`)
  - Runs: `deploy:prepare` → `artisan:*` tasks → `deploy:publish`

#### What it Does

1. Sets Laravel-specific context variables (namespace-scoped):
   - `writable_dirs: "storage/statamic"` (default)

2. Runs deployment pipeline:
   - Common deployment tasks (`deploy:prepare`)
   - Artisan optimizations (cache, route, view, event)
   - Database migrations
   - Storage link creation
   - Final publish (`deploy:publish`)

### Example Usage

```rust
// Register recipes
recipe::common::register_common(&mut reg);
recipe::laravel::register_laravel(&mut reg);

// Set context
let mut ctx = Context::new();
ctx.set("deploy_path", "/var/www/laravel-app");
ctx.set("repository", "git@github.com:user/laravel-app.git");

// Deploy
runner.run("laravel:deploy", &mut ctx)?;
```

### Customization

The `laravel:deploy` task automatically sets `writable_dirs` to `"storage/statamic"`. To override:

```rust
// Before running deploy
ctx.set("writable_dirs", "storage,storage/logs");
```

Or create a custom deployment task:

```rust
let mut laravel = reg.with_namespace("laravel");
laravel.register(
    Task::new(
        "deploy",
        task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
            ctx.set("writable_dirs", "storage,storage/logs,storage/framework");
            Ok(())
        })
    )
    .desc("Custom Laravel deployment")
    .depends_on(":deploy:prepare")
    .depends_on("artisan:migrate")
    .depends_on(":deploy:publish")
);
```

## Tools Recipes

Tools recipes help install and verify development tools.

### Mise Recipe

Install and verify [mise](https://mise.jdx.dev/) (formerly rtx).

#### Tasks

- **`tools:mise:install`** - Installs mise
  - macOS: Uses Homebrew
  - Windows: Uses winget
  - Linux: Uses mise's install script
  - Shows installed version on success

- **`tools:mise:verify`** - Verifies mise installation
  - Checks if mise is on PATH
  - Prints version if found
  - Non-fatal (returns success even if not found)

### Docker Recipe

Install and verify Docker/Podman.

#### Tasks

- **`tools:docker:install`** - Installs Docker
  - Attempts platform-specific installation
  - Shows detailed errors on failure

- **`tools:docker:verify`** - Verifies Docker installation
  - Checks `docker` or `podman` command availability
  - Tests basic functionality

### DDEV Recipe

Install and verify [DDEV](https://ddev.com/).

#### Tasks

- **`tools:ddev:install`** - Installs DDEV
  - Platform-specific installation methods
  - Includes mkcert setup for SSL

- **`tools:ddev:verify`** - Verifies DDEV installation
- **`tools:ddev:install_mkcert`** - Installs mkcert for local SSL
- **`tools:ddev:uninstall`** - Uninstalls DDEV

## Creating Custom Recipes

See the [Developer Guide](../developer/extending.md) for creating your own recipes.

## Recipe Patterns

### Namespaced Recipes

Use namespaces to organize recipe tasks:

```rust
let mut my_recipe = reg.with_namespace("myapp");
my_recipe.register(Task::new("deploy", action));
// Task name becomes: "myapp:deploy"
```

### Cross-Namespace Dependencies

Reference tasks from other recipes using `:` prefix:

```rust
Task::new("myapp:deploy", action)
    .depends_on(":deploy:prepare")  // From common recipe
    .depends_on(":deploy:publish")   // From common recipe
```

### Conditional Recipes

Register recipes conditionally:

```rust
#[cfg(feature = "laravel")]
recipe::laravel::register_laravel(&mut reg);
```

## Best Practices

1. **Use namespaces**: Prevents naming conflicts
2. **Document tasks**: Add descriptions to all user-facing tasks
3. **Hide helpers**: Mark internal tasks as hidden
4. **Set defaults**: Use namespace-scoped context for recipe defaults
5. **Compose, don't duplicate**: Reuse common tasks via dependencies

