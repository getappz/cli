# Branch status (as of last fetch)

## Main vs devel

| Branch | Commit | Notes |
|--------|--------|--------|
| **main** (origin/main) | `16f36f6` | In sync with remote. Has 5 commits not in devel (ai-seo merge line). |
| **origin/devel** | `5fc2da0` | **21 commits ahead of main** (domains, DDEV, WordPress, browser extension, exec, deploy, etc.). |
| **Local devel** | `fc2f7f5` | **21 commits behind origin/devel** — update with `git checkout devel && git pull`. |

**Common ancestor:** `fc2f7f5` (feat(code): add code-mix crate and pack command for AI context).

So **main** and **origin/devel** have diverged: main has the ai-seo merge history; devel has the latest feature work. To have “everything”, a branch should merge both **main** and **origin/devel**.

---

## Other branches

| Branch | Has main? | Has origin/devel? | Action |
|--------|-----------|-------------------|--------|
| **ai-seo** (current) | ✅ Yes | ❌ No (missing 21 commits) | Merge `origin/devel` into `ai-seo` to get latest from both. |
| **origin/fix/ssg-gen** | ❌ No (40 behind main) | ❌ No (56 behind devel) | Merge `main` then `origin/devel` into this branch. |
| **origin/master** | — | — | Long history (750 commits); relationship to main/devel is separate. |

---

## Recommended next steps

### 1. ai-seo (current branch)

To merge `origin/devel` into `ai-seo` you need to clear two blockers:

- **`appz-cli.code-workspace`**  
  Close it in the IDE (or any editor) so Git can update/delete it during merge. If it’s deleted on `origin/devel`, the merge will remove it.

- **Untracked files**  
  You have many untracked files that exist on `origin/devel`. Git refuses to overwrite them. Choose one:

  **Option A – Prefer devel’s versions (replace your untracked copies)**  
  ```bash
  git checkout ai-seo
  # Backup if needed, then remove the listed untracked paths so merge can run
  git clean -fd -n   # dry run: see what would be removed
  git clean -fd      # then run for real (removes untracked files/dirs)
  git merge origin/devel -m "Merge origin/devel into ai-seo for latest changes"
  ```

  **Option B – Keep your untracked work and merge**  
  ```bash
  git checkout ai-seo
  git stash -u -m "Untracked work before merging devel"   # stash untracked too
  # Close appz-cli.code-workspace in IDE if open
  git merge origin/devel -m "Merge origin/devel into ai-seo for latest changes"
  git stash pop   # reapply your changes; resolve any overlaps
  ```

After a successful merge, push:  
`git push origin ai-seo`

---

### 2. fix/ssg-gen (remote-only branch)

Bring it up to date with main and devel:

```bash
git fetch origin
git checkout -b fix/ssg-gen origin/fix/ssg-gen
git merge main -m "Merge main into fix/ssg-gen"
git merge origin/devel -m "Merge origin/devel into fix/ssg-gen"
# resolve any conflicts, then:
git push origin fix/ssg-gen
```

---

### 3. Local devel

Update to match remote (21 commits behind):

```bash
git checkout devel
git pull origin devel
```

---

### 4. Optional: main

To add devel’s 21 commits to main (e.g. for a release):

```bash
git checkout main
git merge origin/devel -m "Merge devel into main"
git push origin main
```
