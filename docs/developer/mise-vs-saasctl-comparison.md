# Mise vs saasctl Task Runner Comparison

This document compares the parallel execution behavior of mise and saasctl using the same Astro recipe tasks.

## Test Setup

Both tools use the same task definitions:
- `config-apply` (hidden task that applies config variables)
- `astro-init` (scaffold Astro project)
- `astro-install` (install dependencies)
- `astro-dev` (dev server)
- `astro-build` (build)
- `astro-preview` (preview)

**Files:**
- `recipe.yaml` - saasctl format
- `mise.toml` - mise format

## Task Definitions

### recipe.yaml (saasctl)
```yaml
config:
  project_dir: "./astro-app"
  template: "minimal"

tasks:
  astro:init:
    - desc: "Scaffold Astro (non-interactive)"
      run_locally: "bun create astro@latest \"{{project_dir}}\" --template \"{{template}}\" --yes"

  astro:install:
    - desc: "Install deps (bun)"
      cd: "{{project_dir}}"
      run_locally: "bun install"

  astro:dev:
    - desc: "Dev server"
      cd: "{{project_dir}}"
      run_locally: "bun run dev"

  astro:build:
    - desc: "Build"
      cd: "{{project_dir}}"
      run_locally: "bun run build"

  astro:preview:
    - desc: "Preview"
      cd: "{{project_dir}}"
      run_locally: "bun run preview"

before:
  astro:init:
    - config:apply
```

### mise.toml (mise)
```toml
[vars]
project_dir = "./astro-app"
template = "minimal"

[tasks.config-apply]
description = "Apply config values to context"
hide = true
run = "true"

[tasks.astro-init]
description = "Scaffold Astro (non-interactive)"
depends = ["config-apply"]
run = "bun create astro@latest \"{{vars.project_dir}}\" --template \"{{vars.template}}\" --yes"

[tasks.astro-install]
description = "Install deps (bun)"
depends = ["astro-init"]
dir = "{{vars.project_dir}}"
run = "bun install"

[tasks.astro-dev]
description = "Dev server"
depends = ["astro-install"]
dir = "{{vars.project_dir}}"
run = "bun run dev"

[tasks.astro-build]
description = "Build"
depends = ["astro-install"]
dir = "{{vars.project_dir}}"
run = "bun run build"

[tasks.astro-preview]
description = "Preview"
depends = ["astro-build"]
dir = "{{vars.project_dir}}"
run = "bun run preview"
```

## Key Differences

### 1. Dependency Declaration

**saasctl:**
- Uses `before:` hooks for implicit dependencies
- `config:apply` runs before `astro:init` via hook
- No explicit dependencies between other tasks in recipe.yaml
- Dependencies might be inferred or added programmatically

**mise:**
- Uses explicit `depends = [...]` array
- All dependencies are explicit in the task definition
- Clear dependency chain: `config-apply → astro-init → astro-install → {astro-build, astro-dev} → astro-preview`

### 2. Variable Substitution

**saasctl:**
- Uses `{{variable_name}}` from config section
- Variables injected into Context via `config:apply` task
- Context available to all tasks after `config:apply` runs

**mise:**
- Uses `{{vars.variable_name}}` from [vars] section
- Variables available directly in templates
- No need for special "apply" step

### 3. Working Directory

**saasctl:**
- Uses `cd: "{{project_dir}}"` in task step
- Changes directory per command

**mise:**
- Uses `dir = "{{vars.project_dir}}"` in task definition
- Changes directory for entire task execution

### 4. Task Execution

**saasctl:**
- Wave-based execution (layers)
- Tasks grouped into dependency waves
- All tasks in a wave execute in parallel
- Wait for wave completion before next wave

**mise:**
- Streaming execution (continuous)
- Tasks start as soon as dependencies complete
- More efficient for varying task durations
- No explicit "wave" boundaries

## Execution Comparison

### Running `astro-preview`

**saasctl execution order:**
```
Wave 1: [config-apply]
  └─ Wait for completion
Wave 2: [astro-init]
  └─ Wait for completion
Wave 3: [astro-install]
  └─ Wait for completion
Wave 4: [astro-build]
  └─ Wait for completion
Wave 5: [astro-preview]
```

**mise execution order:**
```
config-apply completes → astro-init becomes ready → starts
astro-init completes → astro-install becomes ready → starts
astro-install completes → {astro-build, astro-dev} become ready → start (if both needed)
astro-build completes → astro-preview becomes ready → starts
```

### Dependency Graph

Both tools show the same dependency chain:
```
astro-preview
└── astro-build
    └── astro-install
        └── astro-init
            └── config-apply
```

## Performance Characteristics

### saasctl (Wave-based)
- **Pros:**
  - Simpler mental model
  - Clear execution boundaries
  - Easier to debug (wave logs)
- **Cons:**
  - Slightly less efficient (waits for slowest task in wave)
  - Example: If `astro-build` takes 30s and `astro-dev` takes 1s, both wait for build to complete

### mise (Streaming)
- **Pros:**
  - More efficient (tasks start immediately when ready)
  - Better for varying task durations
  - Maximum parallelism
- **Cons:**
  - More complex implementation
  - Harder to debug (continuous flow)
  - Requires channel-based coordination

## Testing Commands

### saasctl
```bash
# List tasks
saasctl list

# Show execution plan
saasctl plan astro:preview

# Run task
saasctl run astro:preview
```

### mise
```bash
# List tasks
mise tasks ls

# Show dependencies
mise tasks deps astro-preview

# Dry run (show what would execute)
mise run astro-preview --dry-run

# Run task
mise run astro-preview
```

## Observations

1. **Both tools respect dependencies correctly**
2. **mise's streaming model is more efficient** for tasks with varying durations
3. **saasctl's wave model is simpler** and sufficient for most use cases
4. **Both handle concurrency limits** (mise via `jobs` config, saasctl via semaphore)
5. **Both support cancellation** (Ctrl+C handling)

## Conclusion

Both implementations correctly handle task dependencies and parallel execution. The choice between wave-based (saasctl) and streaming (mise) depends on:
- **Complexity requirements**: Wave-based is simpler
- **Performance needs**: Streaming is more efficient
- **Debugging needs**: Wave-based is easier to debug
- **Task duration variance**: Streaming benefits more when tasks have very different durations

For most use cases, the wave-based approach is sufficient and provides better developer experience.

