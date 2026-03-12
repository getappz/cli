# Superpowers Plugin Analysis

> Full analysis of the Superpowers plugin — agent-only flows vs. CLI/tool-replaceable operations. Used to identify what appz CLI should implement.

**Date:** 2025-02-21  
**Purpose:** Find places where tools, scripts, or appz CLI can replace agent-only flows for reliability and automation.

---

## 1. Superpowers Overview

Superpowers is a Cursor plugin providing structured skills for AI-assisted development. Skills enforce process discipline: brainstorming before code, TDD, systematic debugging, verification before claims, git worktrees, code review, etc.

**Core principle:** Agent-only flows are brittle. Where a skill instructs the agent to run commands, parse output, or execute git operations, those can often be replaced by a CLI command or script that:
- Runs the same operations deterministically
- Reduces agent token usage and errors
- Enables scripting and CI integration

---

## 2. Skills Inventory

| Skill | Purpose | Agent-Only Operations | CLI-Replaceable? |
|------|---------|------------------------|------------------|
| **using-superpowers** | Skill discovery, invocation order | None (meta-skill) | N/A |
| **brainstorming** | Design before implementation | Context gathering, Q&A | Partial (context scripts) |
| **writing-plans** | Create implementation plans | Document writing | Partial (plan templates) |
| **executing-plans** | Batch execute tasks in parallel session | Task execution, verification | Partial (plan runner) |
| **subagent-driven-development** | Dispatch subagents per task, two-stage review | Subagent dispatch, review orchestration | No (orchestration) |
| **using-git-worktrees** | Create isolated worktrees | `git worktree add`, `check-ignore`, setup, verify | **Yes** → `appz git worktree` |
| **finishing-a-development-branch** | Merge / PR / keep / discard | `git checkout`, `pull`, `merge`, `branch -d`, `worktree remove` | **Yes** → `appz git branch finish` |
| **verification-before-completion** | Run verification before claiming success | Run `npm test`, `cargo build`, etc. | Partial (verification runner) |
| **requesting-code-review** | Get BASE_SHA, HEAD_SHA, dispatch reviewer | `git rev-parse`, `merge-base` | **Yes** → `appz git review prepare` |
| **receiving-code-review** | Process feedback, verify before implementing | Human-in-loop | No |
| **systematic-debugging** | Root cause before fixes | Error analysis, hypothesis testing | No (reasoning) |
| **test-driven-development** | Red-Green-Refactor | Test execution | Partial (TDD runner) |
| **dispatching-parallel-agents** | Parallel subagent dispatch | Task dispatch | No (orchestration) |

---

## 3. Git-Related Skills — Deep Dive

### 3.1 using-git-worktrees

**What the agent does:**
1. Check for `.worktrees` or `worktrees` (priority order)
2. Optionally check CLAUDE.md for preference
3. Ask user if neither exists
4. **Verify parent directory is gitignored** — `git check-ignore -q .worktrees`
5. If not ignored: add to .gitignore, commit
6. Create worktree: `git worktree add "$path" -b "$BRANCH_NAME"`
7. Run project setup: `npm install`, `cargo build`, `pip install`, etc.
8. Run baseline tests: `npm test`, `cargo test`, etc.
9. Report location

**CLI-replaceable operations:**
- `git check-ignore` → `appz git check-ignore`
- `git worktree add -b` → `appz git worktree create`
- Project setup (npm, cargo, pip, go) → `appz git worktree create --setup`
- Baseline tests → `appz git worktree create --verify`
- Worktree list → `appz git worktree list`
- Worktree remove → `appz git worktree remove`

### 3.2 finishing-a-development-branch

**What the agent does:**
1. Verify tests pass
2. Determine base branch: `git merge-base HEAD main` or `master`
3. Present 4 options: Merge, Push+PR, Keep, Discard
4. Execute choice:
   - **Merge:** `git checkout base`, `git pull`, `git merge branch`, `git branch -d branch`, worktree cleanup
   - **Push+PR:** `git push -u origin branch`, `gh pr create`
   - **Keep:** Report, no cleanup
   - **Discard:** Confirm, `git checkout base`, `git branch -D branch`, worktree cleanup
5. Cleanup worktree if needed: `git worktree remove <path>`

**CLI-replaceable operations:**
- `git merge-base` → `appz git merge-base`
- Merge / PR / Keep / Discard flow → `appz git branch finish merge|pr|keep|discard`
- Worktree cleanup → already in worktree remove

### 3.3 requesting-code-review

**What the agent does:**
1. Get SHAs: `BASE_SHA=$(git rev-parse HEAD~1)`, `HEAD_SHA=$(git rev-parse HEAD)` (or origin/main)
2. Dispatch code-reviewer subagent with template placeholders
3. Act on feedback

**CLI-replaceable operations:**
- SHA resolution with merge-base → `appz git review prepare`
- Output: BASE_SHA, HEAD_SHA, template for reviewer dispatch

---

## 4. Workflow Integration

```text
brainstorming → writing-plans → [executing-plans | subagent-driven-development]
                                      │
                                      ▼
                          using-git-worktrees (REQUIRED before start)
                                      │
                                      ▼
                          [Task execution with TDD, verification]
                                      │
                                      ▼
                          requesting-code-review (after each task)
                                      │
                                      ▼
                          finishing-a-development-branch (REQUIRED at end)
```

**Key insight:** `using-git-worktrees` and `finishing-a-development-branch` are REQUIRED by both executing-plans and subagent-driven-development. Replacing their git commands with `appz git` reduces agent errors and enables non-agent workflows (scripts, CI, human commands).

---

## 5. Implementation Choices

### 5.1 Design: Native Subcommand vs. Companion Binary

**Chosen:** Native subcommand `appz git` (no extra binary)

**Rationale:**
- Keeps CLI structure clean: one `appz` binary
- Commands grouped under `appz git` namespace
- Aligns with Vercel/Moon CLI patterns

### 5.2 Git Backend: git2 vs. Shell

**Chosen:** `git2` crate for all git operations where feasible

**Rationale:**
- No dependency on system `git` being installed
- Cross-platform, no shell quoting issues
- Consistent error handling

**Exception:** `branch finish` still uses `run_git` for checkout, pull, merge, push — pending git2 replacement with credential callbacks.

---

## 6. What Was Implemented (appz git)

| Command | Superpowers skill | Status |
|---------|-------------------|--------|
| `appz git worktree create` | using-git-worktrees | Done (git2) |
| `appz git worktree remove` | using-git-worktrees, finishing-a-development-branch | Done (git2) |
| `appz git worktree list` | using-git-worktrees | Done (git2) |
| `appz git check-ignore <path>` | using-git-worktrees | Done (git2) |
| `appz git merge-base [branch]` | finishing-a-development-branch | Done (git2) |
| `appz git branch finish merge\|pr\|keep\|discard` | finishing-a-development-branch | Done (repo discovery git2; checkout/merge/push still shell) |
| `appz git review prepare` | requesting-code-review | Done (git2) |

---

## 7. Other Opportunities (Not Implemented)

| Skill | Opportunity | Feasibility |
|-------|-------------|-------------|
| **verification-before-completion** | `appz verify` — run project-appropriate test/build commands, fail if non-zero | Medium (detect project type, run commands) |
| **test-driven-development** | `appz tdd <target>` — run test in red, implement, run green | Low (TDD is process, not single command) |
| **brainstorming** | Context script: gather project files, recent commits, deps | Low (meta-process) |
| **writing-plans** | Plan templates, `appz plan init` | Medium (templates exist) |
| **executing-plans** | Plan runner that reads plan, executes tasks in batches | Low (complex orchestration) |

---

## 8. Skill Dependencies (Call Graph)

```text
executing-plans ──┬── using-git-worktrees (before start)
                  └── finishing-a-development-branch (at end)

subagent-driven-development ──┬── using-git-worktrees (before start)
                             ├── requesting-code-review (after each task)
                             └── finishing-a-development-branch (at end)

brainstorming ── writing-plans
writing-plans ── [executing-plans | subagent-driven-development]

systematic-debugging ── test-driven-development (Phase 4)
requesting-code-review ── git SHAs (merge-base, rev-parse)
```

---

## 9. Red Flags & Common Mistakes (from skills)

| Skill | Red flag | Mitigation |
|-------|----------|------------|
| using-git-worktrees | Skipping ignore verification | `appz git check-ignore` before create |
| finishing-a-development-branch | Proceeding with failing tests | Agent should run verification first; CLI could add `--verify` flag |
| finishing-a-development-branch | No confirmation for discard | `appz git branch finish discard --yes` |
| verification-before-completion | "Should pass" without running | CLI forces running command |
| requesting-code-review | Wrong BASE_SHA (HEAD~1 vs merge-base) | `appz git review prepare` uses merge-base |

---

## 10. Summary

**Implemented:** `appz git` subcommands cover the deterministic git operations from `using-git-worktrees`, `finishing-a-development-branch`, and `requesting-code-review`. All use the `git2` crate except `branch finish`'s checkout/pull/merge/push (pending credential support).

**Remaining:** Replace `run_git` in branch.rs with git2; add credential callbacks for push/pull.

**Other opportunities:** Verification runner, plan templates — lower priority than completing git2 migration.
