# Git Commit Helper

Generate a descriptive conventional commit message by analyzing staged changes.

**Use the Git Commit Helper skill.** Read `~/.appz/skills/git-commit-helper/SKILL.md` for full guidelines (conventional commits format, types, scopes, examples, and best practices).

## Instructions

1. **Load the skill**: Read `~/.appz/skills/git-commit-helper/SKILL.md` and follow its conventions
2. **Analyze staged changes**:
   - Run `git status` to see what's staged
   - Run `git diff --staged` to inspect the diff
   - Run `git diff --staged --stat` for file-level overview
3. **Generate the commit message** following conventional commits:
   - Format: `<type>(<scope>): <description>`
   - Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`
   - Imperative mood, first line &lt; 50 chars, no trailing period
4. **Output**:
   - A suggested commit message (subject + optional body)
   - Brief rationale for type/scope choice
   - Optional: `git commit -m "..."` command to run

## Format Rules

- **DO**: imperative mood, capitalize first letter, explain WHY in body
- **DON'T**: vague messages ("update", "fix stuff"), past tense, technical details in summary

## Example Output

```
feat(plugin-build): add manifest schema validation

Validate plugin manifests against schema before build.
Fails fast with clear errors for invalid config.

git commit -m "feat(plugin-build): add manifest schema validation"
```
