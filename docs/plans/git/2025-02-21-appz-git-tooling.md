# Appz Git Tooling Plan

> Git tooling extracted from Superpowers workflow — worktrees, branch finish, review prepare. Uses `git2` crate for all git operations.

**Source:** Superpowers skills: `using-git-worktrees`, `finishing-a-development-branch`, `verification-before-completion`, `requesting-code-review`

**Full analysis:** [2025-02-21-superpowers-analysis.md](./2025-02-21-superpowers-analysis.md)

---

## Design

- **Entry point:** `appz git` with subcommands grouped under one namespace
- **Implementation:** Native subcommand (no extra binary)
- **Git backend:** `git2` crate — no shelling out to `git` where feasible

---

## Commands

| Command | Description | Superpowers Skill |
|---------|-------------|-------------------|
| `appz git worktree create` | Create worktree with branch, optional setup/verify | using-git-worktrees |
| `appz git worktree remove` | Remove worktree by path or current branch | using-git-worktrees |
| `appz git worktree list` | List worktrees | using-git-worktrees |
| `appz git check-ignore <path>` | Verify path is gitignored | worktree safety |
| `appz git merge-base [branch]` | Find merge base with main/master | finishing-a-development-branch |
| `appz git branch finish` | Merge / PR / keep / discard | finishing-a-development-branch |
| `appz git review prepare` | Output BASE_SHA, HEAD_SHA for code review | requesting-code-review |

---

## Implementation Status

### Implemented (git2)

| Module | Operations | Status |
|--------|------------|--------|
| `mod.rs` | `check-ignore` — `Repository::discover`, `repo.is_path_ignored()` | Done |
| `merge_base.rs` | `repo.head()`, `repo.find_branch()`, `repo.merge_base()` | Done |
| `review.rs` | `repo.revparse_single()`, `repo.merge_base()` | Done |
| `worktree.rs` | Create: `repo.branch()`, `repo.worktree()` | Done |
| `worktree.rs` | Remove: `repo.worktrees()`, `repo.find_worktree()`, `Worktree::prune()` | Done |
| `worktree.rs` | List: `repo.workdir()`, `repo.worktrees()`, `find_worktree()` | Done |
| `worktree.rs` | Check-ignore (before create): `repo.is_path_ignored()` | Done |
| `branch.rs` | Repo/branch discovery: `Repository::discover`, `head()`, `find_branch()` | Done |

### Still uses shell (`run_git` in branch.rs)

| Action | Shell command | Future git2 replacement |
|--------|---------------|--------------------------|
| Merge: checkout | `git checkout <base>` | `repo.set_head()`, `repo.checkout()` |
| Merge: pull | `git pull` | `repo.find_remote()`, `remote.fetch()`, `repo.merge()` |
| Merge: merge | `git merge <branch>` | `repo.merge()` |
| Merge: delete branch | `git branch -d <branch>` | `repo.find_branch()`, `branch.delete()` |
| Pr: push | `git push -u origin <branch>` | `remote.push()` (needs credentials) |
| Discard: checkout | `git checkout <base>` | Same as Merge |
| Discard: delete | `git branch -D <branch>` | Same as Merge |

**Note:** Push/pull require credential callbacks; git2 supports this via `RemoteCallbacks`. `gh pr create` remains a shell call (external tool).

---

## File Layout

```
crates/app/src/commands/git/
├── mod.rs        # CheckIgnore, dispatch, CLI defs
├── worktree.rs   # create, remove, list
├── merge_base.rs # merge base with main/master
├── branch.rs     # finish (merge, pr, keep, discard)
└── review.rs     # prepare (BASE_SHA, HEAD_SHA, template)
```

---

## Dependencies

```toml
# crates/app/Cargo.toml
git2 = "0.20"
```

---

## Remaining Work

1. **branch.rs `run_git`** — Replace with git2: checkout, fetch, merge, branch delete, push. Pull and push need credential callbacks.
2. **Worktree list format** — Optionally align output with `git worktree list` format.
3. **repo_root for branch finish** — `repo.commondir().parent()` can be wrong for bare repos; consider more robust path resolution.
