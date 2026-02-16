---
name: Codex Adoption Plan
overview: After a thorough review of the OpenAI Codex codebase (65+ Rust crates), this plan identifies 8 concrete adoptions ranked by impact. The highest-value items directly improve the AI fixer's reliability, the LLM client's resilience, and the sandbox's security posture.
todos:
  - id: fuzzy-patch
    content: Port Codex seek_sequence 4-pass fuzzy matching into ai_fixer/patch.rs; rewrite apply_file_patch to use content-based matching with reverse application order
    status: completed
  - id: llm-retry
    content: Add RetryPolicy + exponential backoff with jitter to llm.rs; wrap all provider calls in retry loop (4 attempts, 200ms base, retry 5xx + transport)
    status: completed
  - id: head-tail-buffer
    content: Implement HeadTailBuffer in common crate; integrate into provider output capture and AI context collection
    status: completed
  - id: layered-config
    content: Add ~/.appz/config.toml user-level config with recursive TOML merge under project appz.json; CLI flags override both
    status: completed
  - id: process-hardening
    content: "Add hardening module: disable core dumps, prevent ptrace, clear LD_*/DYLD_* env vars; call at CLI startup"
    status: completed
  - id: streaming-llm
    content: Add SSE streaming support to llm.rs for OpenAI/Anthropic; show real-time progress during Fix agent calls
    status: completed
  - id: token-tracking
    content: Parse token usage from API responses; add TokenUsage struct; display cost estimates in --verbose-ai mode
    status: completed
  - id: custom-prompts
    content: Allow prompt template overrides from ~/.appz/prompts/ or .appz/prompts/; load with fallback to hardcoded defaults
    status: in_progress
isProject: false
---

# Codex Adoption Plan for appz-cli

## What Codex Is

OpenAI Codex CLI is a 65+ crate Rust workspace powering a local coding agent. It includes a TUI, an app-server for IDE integration, multi-provider LLM orchestration, a production-grade patch engine, and deep platform-level sandboxing (bubblewrap/seccomp/Landlock on Linux, Seatbelt on macOS).

## Adoptions Ranked by Impact

---

### 1. Fuzzy Patch Matching (HIGH -- directly fixes AI fixer patch failures)

**Problem**: Our [ai_fixer/patch.rs](crates/checker/src/ai_fixer/patch.rs) applies hunks by line number. AI-generated patches frequently have trailing whitespace mismatches or minor formatting differences, causing patch application to fail silently or error out.

**What Codex does**: [apply-patch/src/seek_sequence.rs](../appz-ref/codex/codex-rs/apply-patch/src/seek_sequence.rs) uses a 4-pass fuzzy matching algorithm with decreasing strictness:

1. **Exact match** -- byte-for-byte
2. **Rstrip match** -- ignores trailing whitespace
3. **Trim match** -- ignores leading and trailing whitespace
4. **Unicode normalization** -- normalizes typographic dashes, quotes, and spaces to ASCII

**Change**: Replace the line-number-based hunk application in `patch.rs` with content-based matching using a `seek_sequence` function ported from Codex. This means hunks are matched by their context lines (the `-` lines) rather than trusting `old_start` line numbers, which makes patches robust against:

- File modifications between patch generation and application
- Whitespace mismatches from the AI model
- Unicode normalization issues

**Files to modify**:

- [crates/checker/src/ai_fixer/patch.rs](crates/checker/src/ai_fixer/patch.rs) -- add `seek_sequence()`, rewrite `apply_file_patch()` to use content matching, apply replacements in reverse order

---

### 2. LLM Retry with Exponential Backoff (HIGH -- prevents flaky API failures)

**Problem**: Our [ai_fixer/llm.rs](crates/checker/src/ai_fixer/llm.rs) makes a single HTTP request with no retry logic. Any transient network error, 5xx, or timeout kills the entire AI fix flow.

**What Codex does**: [codex-client/src/retry.rs](../appz-ref/codex/codex-rs/codex-client/src/retry.rs) implements:

- Configurable `RetryPolicy` with `max_attempts`, `base_delay`, and per-error-type retry decisions
- Exponential backoff: `base_delay * 2^(attempt-1) * jitter(0.9..1.1)`
- Selective retry: 5xx and transport errors yes, 429 rate limits configurable, 4xx no

**Change**: Add a `retry_with_backoff` wrapper to `llm.rs` that retries on 5xx and network errors with exponential backoff + jitter. Default: 4 attempts, 200ms base delay.

**Files to modify**:

- [crates/checker/src/ai_fixer/llm.rs](crates/checker/src/ai_fixer/llm.rs) -- add `RetryPolicy` struct, `backoff()` function, wrap `call_openai`/`call_anthropic`/`call_ollama` in retry loop

**New dependency**: `rand` (for jitter) -- check if already in workspace

---

### 3. HeadTail Output Buffer (HIGH -- prevents OOM on large projects)

**Problem**: Our checker providers capture the full stdout/stderr of linter commands (e.g., `cargo clippy`, `biome check`). On large projects these can be megabytes, causing high memory usage and potentially exceeding LLM context limits when fed to the AI fixer.

**What Codex does**: [core/src/unified_exec/head_tail_buffer.rs](../appz-ref/codex/codex-rs/core/src/unified_exec/head_tail_buffer.rs) implements a `HeadTailBuffer` that:

- Splits capacity 50/50 between a head (prefix) and tail (suffix)
- Fills head first, then rotates tail (dropping the oldest tail chunks when full)
- Tracks `omitted_bytes` for reporting
- Default cap: 1 MiB

**Change**: Add a `HeadTailBuffer` to the `common` crate (or `checker` crate). Use it in:

- Provider helpers when capturing tool output (`crates/checker/src/providers/helpers.rs`)
- AI fixer context collection when reading large files
- The `command` crate for general command execution

**Files to create**:

- `crates/common/src/head_tail_buffer.rs` (or `crates/checker/src/head_tail_buffer.rs`)

**Files to modify**:

- [crates/checker/src/providers/helpers.rs](crates/checker/src/providers/helpers.rs) -- use buffer when capturing output
- [crates/checker/src/ai_fixer/context.rs](crates/checker/src/ai_fixer/context.rs) -- use buffer for large file reads

---

### 4. Layered Configuration (MEDIUM -- better UX for power users)

**Problem**: appz-cli reads config only from project-level `appz.json`. Users cannot set personal defaults (preferred AI model, API keys, safety thresholds) that apply across all projects.

**What Codex does**: [config/src/merge.rs](../appz-ref/codex/codex-rs/config/src/merge.rs) uses a layered TOML config stack:

- System defaults (lowest precedence)
- User config (`~/.codex/config.toml`)
- Project config (`.codex` in project root)
- CLI flags (highest precedence)

Merging is recursive: tables are deep-merged, scalars are overridden.

**Change**: Add a `~/.appz/config.toml` user-level config that is deep-merged under the project `appz.json`. CLI flags override both. This lets users set `aiProvider`, `aiModel`, API keys, and safety defaults once.

**Files to modify**:

- [crates/checker/src/config.rs](crates/checker/src/config.rs) -- add `read_user_config()`, `merge_configs()`, update `read_check_config()` to layer
- [crates/app/src/project/config.rs](crates/app/src/project/config.rs) -- add user config discovery

---

### 5. Process Hardening (MEDIUM -- low effort, high security value)

**Problem**: When the AI fixer executes linter commands via the sandbox, there are no protections against process-level attacks (core dump secrets, LD_PRELOAD injection, ptrace attachment).

**What Codex does**: [process-hardening/src/lib.rs](../appz-ref/codex/codex-rs/process-hardening/src/lib.rs) applies pre-main hardening:

- Disables core dumps (`setrlimit(RLIMIT_CORE, 0)`)
- Linux: `prctl(PR_SET_DUMPABLE, 0)` prevents ptrace
- macOS: `ptrace(PT_DENY_ATTACH)`
- Removes dangerous env vars (`LD_*`, `DYLD_*`)

**Change**: Add a `hardening` module to the `sandbox` crate or `common` crate. Call it at CLI startup and before spawning sandboxed processes.

**Files to create**:

- `crates/common/src/hardening.rs`

**Files to modify**:

- [crates/cli/src/main.rs](crates/cli/src/main.rs) -- call `hardening::pre_main()` at startup

---

### 6. Streaming LLM Responses (MEDIUM -- better UX for AI fixer)

**Problem**: Our LLM calls in `llm.rs` wait for the complete response before returning. For large patches, the user sees nothing for 10-30 seconds.

**What Codex does**: Streams SSE responses from the API, emitting delta events as tokens arrive. The TUI renders these in real-time.

**Change**: Add a `call_llm_streaming` function that yields chunks via a callback or channel. The AI fixer can display a spinner with a character count or show partial reasoning as it arrives. Start with OpenAI's SSE streaming (`stream: true`), which returns `data: {...}` lines.

**Files to modify**:

- [crates/checker/src/ai_fixer/llm.rs](crates/checker/src/ai_fixer/llm.rs) -- add streaming variant
- [crates/checker/src/ai_fixer/agents.rs](crates/checker/src/ai_fixer/agents.rs) -- use streaming for the Fix agent (longest response)

---

### 7. Token Estimation and Cost Tracking (LOW-MEDIUM -- cost awareness)

**Problem**: Users have no visibility into how much the AI fixer costs per invocation. No token counting or budget limits.

**What Codex does**: Uses a `bytes / 4` heuristic for rough token estimation. Tracks input/output tokens from API responses. Emits token count events.

**Change**: Add token tracking to `LlmResponse` (parse usage from API responses -- both OpenAI and Anthropic return token counts). Display cost estimates in verbose mode. Add an optional `maxTokenBudget` to `SafetyConfig`.

**Files to modify**:

- [crates/checker/src/ai_fixer/llm.rs](crates/checker/src/ai_fixer/llm.rs) -- parse token usage from responses, add `TokenUsage` struct
- [crates/checker/src/ai_fixer/mod.rs](crates/checker/src/ai_fixer/mod.rs) -- accumulate and display cost in verbose mode

---

### 8. Custom Prompt Templates (LOW -- extensibility)

**Problem**: AI fixer prompts are hardcoded in `agents.rs`. Users cannot customize them for their codebase conventions.

**What Codex does**: Discovers custom prompts from `~/.codex/prompts/*.md` with YAML frontmatter for metadata and `$ARGUMENTS` substitution.

**Change**: Allow users to override planner/fixer/verifier system prompts via `~/.appz/prompts/` or project-level `.appz/prompts/`. Lower priority since current prompts work well, but enables power-user customization.

**Files to modify**:

- [crates/checker/src/ai_fixer/agents.rs](crates/checker/src/ai_fixer/agents.rs) -- load custom prompts if they exist, fall back to defaults

---

## What NOT to Adopt

- **Full sandbox rewrite (bubblewrap/seccomp/Landlock)**: Codex needs kernel-level isolation because it executes arbitrary AI-generated commands. Our sandbox only runs trusted linter commands -- the existing `ScopedFs` path-safety is sufficient.
- **TUI with ratatui**: Over-engineering for a CLI tool. Our indicatif spinners + streaming output is the right level.
- **MCP/RMCP protocol**: Not applicable -- we are not building an IDE integration server.
- **SQLite session persistence**: Our check runs are stateless by design. No need for session storage.
- **WebSocket API transport**: Only relevant for persistent chat sessions, not single-shot repair calls.

