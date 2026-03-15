---
name: Appz Git Tooling
overview: Git tooling from Superpowers workflow — worktrees, branch finish, review prepare. Implemented with git2 crate. branch.rs still shells out for checkout/pull/merge/push.
todos:
  - content: Replace run_git in branch.rs with git2 (checkout, fetch, merge, branch delete, push)
    status: pending
  - content: Add credential callbacks for push/pull
    status: pending
isProject: false
---

# Appz Git Tooling

**Full plan:** [docs/plans/git/2025-02-21-appz-git-tooling.md](../../docs/plans/git/2025-02-21-appz-git-tooling.md)

## Implemented

- `appz git worktree create|remove|list` — git2 (Repository, Worktree, WorktreeAddOptions)
- `appz git check-ignore <path>` — git2 `is_path_ignored()`
- `appz git merge-base [branch]` — git2 `merge_base()`
- `appz git branch finish` — merge/pr/keep/discard; repo discovery via git2; **checkout/pull/merge/push still use `run_git`**
- `appz git review prepare` — git2 `revparse_single()`, `merge_base()`

## Location

`crates/app/src/commands/git/`
