---
name: Professional TUI for Appz CLI
overview: Use ratatui-based TUI components intelligently, only when a command flow requires interactive input. No standalone dashboard — embed focused pickers/selectors at prompt points (init template, deploy provider, task selection, etc.), with inquire as fallback for non-TTY.
todos: []
isProject: false
---

# Professional TUI for Appz CLI

## Summary

Introduce **situational TUI** — ratatui-based components used **only when a command would normally prompt** for interactive input. No standalone `appz tui` or dashboard. When `appz init` needs a template, `appz deploy` needs a provider, or `appz run` needs a task, show a polished TUI selector instead of inquire. Fall back to inquire when not a TTY or when `tui` feature is disabled.

## Architecture

```mermaid
flowchart TB
    subgraph commands [Command Flows]
        Init[appz init]
        Deploy[appz deploy]
        Run[appz run]
        Skills[appz skills add]
    end

    subgraph prompt_points [Prompt Points]
        Init --> NeedTemplate[Need template?]
        Deploy --> NeedProvider[Need provider?]
        Run --> NeedTask[Need task?]
        Skills --> NeedOverwrite[Need overwrite?]
    end

    subgraph decision [Use TUI?]
        NeedTemplate --> Check
        NeedProvider --> Check
        NeedTask --> Check
        NeedOverwrite --> Check
        Check{TTY + tui feature?}
        Check -->|Yes| TuiSelect[ratatui select/picker]
        Check -->|No| InquireFallback[inquire prompt]
    end

    subgraph tui_lib [crates/tui - Focused Components]
        Select[select(options)]
        Confirm[confirm(prompt)]
        TextInput[text_input(prompt)]
        TuiSelect --> Select
        TuiSelect --> Confirm
        TuiSelect --> TextInput
    end

    Select --> Resume[Resume command]
    Confirm --> Resume
    TextInput --> Resume
    InquireFallback --> Resume
```



## Technical Stack


| Component   | Choice            | Rationale                                               |
| ----------- | ----------------- | ------------------------------------------------------- |
| TUI library | ratatui 0.30+     | Rich widgets, industry standard                         |
| Backend     | crossterm 0.29    | Cross-platform, aligns with ratatui                     |
| Pattern     | Short-lived modal | Each prompt runs `ratatui::run()`, returns value, exits |


## Key Files to Create/Modify

### New Crate: `crates/tui/` (library of focused components)

- `crates/tui/Cargo.toml` — ratatui, crossterm (no tokio needed for simple prompts)
- `crates/tui/src/lib.rs` — public API: `select()`, `confirm()`, `text_input()`, `multi_select()`
- `crates/tui/src/select.rs` — single-select picker (list with highlight, Enter to confirm)
- `crates/tui/src/confirm.rs` — Yes/No dialog
- `crates/tui/src/text_input.rs` — text input with cursor
- `crates/tui/src/theme.rs` — colors, styles (align with banner cyan accent)

Each component: takes options/prompt, runs event loop, returns `Result<Option<T>>` or `Result<bool>`.

### Integration Points (replace inquire at prompt sites)


| Command               | File                                                                                                   | Current                      | Replace With            |
| --------------------- | ------------------------------------------------------------------------------------------------------ | ---------------------------- | ----------------------- |
| init                  | [crates/app/src/commands/init.rs](crates/app/src/commands/init.rs)                                     | `inquire::Select` template   | `tui::select` when TTY  |
| deploy                | [crates/app/src/commands/deploy.rs](crates/app/src/commands/deploy.rs)                                 | `inquire::Select` provider   | `tui::select` when TTY  |
| skills add            | [crates/app/src/commands/skills/add.rs](crates/app/src/commands/skills/add.rs)                         | `inquire::Confirm` overwrite | `tui::confirm` when TTY |
| WASM host interaction | [crates/app/src/wasm/host_functions/interaction.rs](crates/app/src/wasm/host_functions/interaction.rs) | inquire prompts              | `tui::*` when TTY       |


### Shared Prompt Abstraction

Introduce `ui::prompt` (or extend [crates/ui/src/prompt.rs](crates/ui/src/prompt.rs)) that chooses TUI vs inquire:

```rust
pub fn select_interactive(msg: &str, options: &[String]) -> Result<Option<usize>> {
    if cfg!(feature = "tui") && atty::is(Stream::Stdout) {
        tui::select(msg, options)
    } else {
        inquire::Select::new(msg, options).prompt().map(|s| Some(index_of(s)))
    }
}
```

### Modifications

- [Cargo.toml](Cargo.toml) (root): Add `crates/tui` to workspace; add `tui` feature
- [crates/ui/Cargo.toml](crates/ui/Cargo.toml): `tui = { path = "../tui", optional = true }`; add `tui` feature
- Commands: Call `ui::prompt::select_interactive` etc. instead of raw inquire where TUI improves UX

## UX / UI Design (Per-Component)

### Select / Picker (e.g. template, provider)

```
┌─ Select a template ─────────────────────────────┐
│  > Next.js (nextjs)                             │
│    React + Vite (react-vite)                     │
│    Astro (astro)                                 │
│    Custom GitHub URL                             │
│    Custom npm package                            │
├─────────────────────────────────────────────────┤
│  ↑↓ navigate  Enter select  Esc cancel           │
└─────────────────────────────────────────────────┘
```

### Confirm (Yes/No)

```
┌─ Overwrite existing skill? ──────────────────────┐
│         [ Yes ]    [ No ]                         │
└─────────────────────────────────────────────────┘
```

### When to Use TUI vs Inquire


| Condition                    | Use TUI | Use Inquire                         |
| ---------------------------- | ------- | ----------------------------------- |
| TTY + `tui` feature enabled  | Yes     | No                                  |
| TTY + `tui` feature disabled | No      | Yes                                 |
| Non-TTY (CI, pipe)           | No      | Yes (or fail with "provide --flag") |


## Implementation Phases

### Phase 1: Core Components

1. Add `crates/tui` with ratatui + crossterm
2. Implement `tui::select(title, options) -> Option<usize>` (short-lived, returns on Enter/Esc)
3. Implement `tui::confirm(prompt) -> bool`
4. Add `ui::prompt` abstraction that switches TUI / inquire based on TTY + feature
5. Integrate in **init** (template selection) as first use case
6. Feature flag `tui`; fallback to inquire when disabled

### Phase 2: Broader Integration

1. Integrate in **deploy** (provider selection)
2. Integrate in **skills add** (overwrite confirm)
3. Add `tui::text_input` for free-form prompts (e.g. project name, URL)
4. Optional: integrate in WASM host interaction for plugin prompts

### Phase 3 (Future): Richer Components

- `multi_select` for checklist-style choices
- Grouped options (sections) in select
- Search/filter in long lists

## Dependencies

```toml
# crates/tui/Cargo.toml
[dependencies]
ratatui = { version = "0.30", features = ["crossterm_0_29"] }
crossterm = "0.29"
```

## Risks and Mitigations


| Risk                                  | Mitigation                                                                           |
| ------------------------------------- | ------------------------------------------------------------------------------------ |
| Binary size increase                  | Gate behind `tui` feature; minimal by default                                        |
| Long-running commands (build, deploy) | Phase 1: spawn subprocess, show status; Phase 3: embed output                        |
| Async + blocking event loop           | Use `tokio::task::spawn_blocking` for command execution; keep event loop synchronous |
| Inquire prompts during TUI            | Phase 1: avoid invoking commands that prompt; Phase 3: replace with TUI dialogs      |


