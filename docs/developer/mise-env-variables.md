# How Mise Handles Environment Variables in Tasks

## Overview

Mise supports environment variables at two levels:
1. **Global [env] section** - Available to all tasks and commands
2. **Task-specific env** - Override/extend global env for individual tasks

## Global Environment Variables

Define environment variables in the `[env]` section at the root of `mise.toml`:

```toml
[env]
NODE_ENV = "production"
DATABASE_URL = "postgresql://localhost/myapp"
API_KEY = "secret-key"
```

These variables are:
- Available to all tasks automatically
- Available when using `mise x|exec` and `mise r|run`
- Automatically set when `mise` is activated in the shell
- Merged with environment variables from tool installations

## Task-Specific Environment Variables

You can override or add environment variables for specific tasks:

```toml
[tasks.lint]
description = 'Lint with clippy'
env = { RUST_BACKTRACE = '1' }  # Only for this task
run = "cargo clippy"

[tasks.test]
description = 'Run tests'
env = { 
    RUST_BACKTRACE = '1',
    TEST_TIMEOUT = '30',
    NO_COLOR = '1'
}
run = "cargo test"
```

**Behavior:**
- Task-specific `env` overrides global `[env]` for that task only
- Other tasks still use global `[env]` values
- Task env is merged with global env (task env takes precedence)

## Special Directives

### PATH Management (`_.path`)

Add directories to PATH:

```toml
[env]
_.path = ["./target/debug", "./node_modules/.bin"]

# Or for a specific task
[tasks.build]
env = { _.path = ["./build-tools"] }
run = "make"
```

### File Loading (`_.file`)

Load environment variables from `.env` files:

```toml
[env]
_.file = ".env"  # Load from .env file

# Or multiple files
_.file = [".env", ".env.local"]

# Or task-specific
[tasks.deploy]
env = { _.file = ".env.production" }
run = "deploy.sh"
```

## Variable Resolution Order

1. **System environment** (lowest precedence)
2. **Global [env] section** in `mise.toml`
3. **Task-specific env** (highest precedence)

## Examples from Mise's Own Codebase

### Example 1: Simple Task Env
```toml
[tasks.render:usage]
description = 'Generate usage documentation'
env = { CLICOLOR_FORCE = "0" }  # Disable colors for this task
run = "mise usage > mise.usage.kdl"
```

### Example 2: Multiple Env Vars
```toml
[tasks.test]
env = { 
    CARGO_TERM_COLOR = "always",
    RUST_TEST_THREADS = "1"
}
run = "cargo test"
```

### Example 3: Global + Task Override
```toml
# Global env available to all tasks
[env]
NODE_ENV = "development"
DEBUG = "false"

[tasks.production]
description = "Production build"
env = { 
    NODE_ENV = "production",  # Overrides global
    DEBUG = "false"            # Same as global
}
run = "npm run build"
```

## Lazy Evaluation

You can defer environment variable resolution until after tools are loaded:

```toml
[env]
# This will resolve after tools are installed
MY_VAR = { value = "tools path: {{env.PATH}}", tools = true }
_.path = { path = ["{{env.GEM_HOME}}/bin"], tools = true }
```

This is useful when:
- Environment variables depend on tool installation paths
- You need tool-specific paths in your env vars

## Redaction (Security)

Mark sensitive variables to hide them from output:

```toml
[env]
SECRET_KEY = { value = "my_secret", redact = true }
API_TOKEN = { value = "token_123", redact = true }

# Or use wildcards
redactions = ["SECRET_*", "*_TOKEN", "PASSWORD"]

[env]
SECRET_KEY = "sensitive_value"      # Will be redacted
API_TOKEN = "token_123"              # Will be redacted
PASSWORD = "my_password"            # Will be redacted
```

## Comparison with saasctl

### saasctl (recipe.yaml)
```yaml
config:
  api_key: "secret"

tasks:
  deploy:
    - desc: "Deploy"
      run_locally: "echo $API_KEY"  # Uses context variable
```

**How it works:**
- `config:` section defines variables
- `config:apply` task (auto-injected) loads config into Context
- Tasks access via `{{variable_name}}` in commands
- Environment variables are set via `ctx.set()` or command execution

### mise (mise.toml)
```toml
[env]
API_KEY = "secret"

[tasks.deploy]
description = "Deploy"
run = "echo $API_KEY"  # Direct env var access
```

**How it works:**
- `[env]` section defines environment variables
- Variables are automatically available to tasks
- No special "apply" step needed
- Direct shell variable access

## Key Differences

| Feature | mise | saasctl |
|---------|------|---------|
| **Variable scope** | Environment variables | Context variables |
| **Access method** | `$VAR` in shell | `{{var}}` in templates |
| **Global vs task** | `[env]` + `task.env` | `config:` + task-specific via code |
| **File loading** | `_.file = ".env"` | Not built-in (can add) |
| **PATH management** | `_.path = [...]` | Not built-in (can add) |
| **Lazy evaluation** | `tools = true` | Not built-in |
| **Redaction** | Built-in support | Not built-in |

## Best Practices

1. **Use global [env] for shared variables** across multiple tasks
2. **Use task-specific env** only when you need to override or add task-specific variables
3. **Use redaction** for secrets and sensitive data
4. **Use `_.file`** to load from `.env` files for local development
5. **Use `_.path`** to add project-specific binaries to PATH

## Example: Complete mise.toml with Env

```toml
# Global environment variables
[env]
NODE_ENV = "development"
DATABASE_URL = "postgresql://localhost/myapp"
_.path = ["./node_modules/.bin", "./bin"]
_.file = ".env"

# Task with default env (uses global)
[tasks.dev]
description = "Start dev server"
run = "npm run dev"  # Has access to NODE_ENV, DATABASE_URL

# Task with overridden env
[tasks.test]
description = "Run tests"
env = { 
    NODE_ENV = "test",           # Override global
    TEST_TIMEOUT = "30"          # Add new var
}
run = "npm test"

# Task with production env
[tasks.build]
description = "Production build"
env = { 
    NODE_ENV = "production",
    DATABASE_URL = "postgresql://prod-server/myapp"
}
run = "npm run build"
```

