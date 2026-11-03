//! `appz ci init`: write a GitHub Actions workflow that deploys this project,
//! and (with consent) store the provider token as a repository secret.
//!
//! The workflow only runs `appz deploy --target <slug> --prod --yes`; all
//! platform logic stays in appz. Tokens arrive as environment variables from
//! GitHub secrets, never as CLI flags, so they do not show up in logs or
//! process listings.

use std::collections::BTreeSet;
use std::path::Path;

use appz_deploy::{DeployConfig, read_deploy_config};
use serde::Serialize;

/// Where the workflow goes. A single fixed name keeps reruns idempotent (with
/// `--force`) and makes the generated file easy to find.
pub const WORKFLOW_PATH: &str = ".github/workflows/appz-deploy.yml";

#[derive(Debug, Clone)]
pub struct CiRequest {
    pub target: Option<String>,
    /// Branch that triggers a production deploy. Default: the branch checked
    /// out now, else `main`.
    pub branch: Option<String>,
    pub force: bool,
    /// `false` writes the workflow only and reports which secrets are needed.
    pub set_secrets: bool,
}

#[derive(Debug, Serialize)]
pub struct CiReport {
    pub workflow: String,
    pub repo: String,
    pub target: String,
    pub branch: String,
    /// Secret names the workflow reads from GitHub.
    pub secret_names: Vec<String>,
    pub secrets: Vec<SecretReport>,
}

#[derive(Debug, Serialize)]
pub struct SecretReport {
    pub name: String,
    /// `set`, `failed`, or `skipped`.
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

pub fn run(root: &Path, req: &CiRequest) -> Result<CiReport, String> {
    if !root.join(".git").exists() {
        return Err(format!(
            "{} is not a git repository: CI scaffolding needs a GitHub checkout",
            root.display()
        ));
    }
    let repo = origin_url(root)
        .and_then(|url| github_repo(&url))
        .ok_or_else(|| {
            "no GitHub origin remote: add one with `git remote add origin https://github.com/<owner>/<repo>.git`"
                .to_string()
        })?;

    let config = read_deploy_config(root)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let target = resolve_target(root, req.target.as_deref(), &config)?;
    let provider = appz_deploy::get_provider(&target).map_err(|e| e.to_string())?;

    let branch = req
        .branch
        .clone()
        .or_else(|| current_branch(root))
        .unwrap_or_else(|| "main".to_string());
    if !valid_branch(&branch) {
        return Err(format!("'{branch}' is not a safe branch name"));
    }

    let secret_names = secret_names(provider.auth_env_var(), &config.env);
    let contents = workflow_contents(&target, &branch, &secret_names);

    let relative = Path::new(WORKFLOW_PATH);
    let path = root.join(relative);
    if path.exists() && !req.force {
        return Err(format!(
            "{WORKFLOW_PATH} already exists: pass --force to overwrite it"
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    std::fs::write(&path, contents).map_err(|e| format!("cannot write {WORKFLOW_PATH}: {e}"))?;

    let secrets = if req.set_secrets {
        store_secrets(root, &repo, &secret_names)
    } else {
        secret_names
            .iter()
            .map(|name| SecretReport {
                name: name.clone(),
                status: "skipped",
                detail: Some(format!("run `gh secret set {name}` (or re-run with --yes)")),
            })
            .collect()
    };

    Ok(CiReport {
        workflow: relative.to_string_lossy().into_owned(),
        repo,
        target,
        branch,
        secret_names,
        secrets,
    })
}

/// The provider slug to pin in the workflow: `--target`, then `deploy.default`,
/// then the single detected provider (the same rules `appz deploy` uses).
fn resolve_target(
    root: &Path,
    explicit: Option<&str>,
    config: &DeployConfig,
) -> Result<String, String> {
    if let Some(slug) = explicit {
        return Ok(slug.to_string());
    }
    if let Some(slug) = &config.default {
        return Ok(slug.clone());
    }
    let mut found = appz_deploy::detect_providers(root);
    match found.len() {
        1 => Ok(found.remove(0).to_string()),
        0 => Err(
            "no deploy target: pass --target <provider> or set deploy.default in appz.jsonc"
                .to_string(),
        ),
        _ => Err(format!(
            "multiple deploy targets detected ({}): pass --target <provider>",
            found.join(", ")
        )),
    }
}

fn valid_branch(branch: &str) -> bool {
    !branch.is_empty()
        && !branch.starts_with('-')
        && branch
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
}

/// Names the workflow passes from `secrets.*` to the deploy step: the
/// provider's token variable plus any `deploy.env` names, deduplicated. Only
/// valid shell identifiers are emitted.
fn secret_names(token_var: &str, extra: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    std::iter::once(token_var)
        .chain(extra.iter().map(String::as_str))
        .filter(|name| !name.is_empty())
        .filter(|name| crate::secrets::valid_name(name))
        .filter(|name| seen.insert((*name).to_string()))
        .map(String::from)
        .collect()
}

fn workflow_contents(target: &str, branch: &str, names: &[String]) -> String {
    let mut out = format!(
        "name: Deploy

on:
  push:
    branches: [\"{branch}\"]
  workflow_dispatch:

concurrency:
  group: appz-deploy-${{{{ github.ref }}}}
  cancel-in-progress: false

jobs:
  deploy:
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v4
      - uses: jdx/mise-action@v2
      - name: Install appz
        run: npm install -g @getappz/cli
      - name: Deploy
        run: appz deploy --target {target} --prod --yes
"
    );
    if !names.is_empty() {
        out.push_str("        env:\n");
        for name in names {
            out.push_str(&format!("          {name}: ${{{{ secrets.{name} }}}}\n"));
        }
    }
    out
}

// ── git ─────────────────────────────────────────────────────────

/// `origin`'s URL, read from `.git/config` first (works without git on PATH and
/// in tests), then from `git config` for layouts where `.git` is a file (such
/// as linked worktrees) and the config lives elsewhere.
fn origin_url(root: &Path) -> Option<String> {
    std::fs::read_to_string(root.join(".git/config"))
        .ok()
        .and_then(|c| origin_from_git_config(&c))
        .or_else(|| {
            let out = command::Command::new("git")
                .args(["config", "--get", "remote.origin.url"])
                .cwd(root)
                .without_shell()
                .exec()
                .ok()?;
            out.status
                .success()
                .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        })
        .filter(|url| !url.is_empty())
}

/// The `url` of `[remote "origin"]` in a git config file.
fn origin_from_git_config(config: &str) -> Option<String> {
    let mut in_origin = false;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_origin = line.eq_ignore_ascii_case(r#"[remote "origin"]"#);
            continue;
        }
        if in_origin
            && let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("url")
        {
            return Some(value.trim().to_string());
        }
    }
    None
}

/// `owner/repo` from a GitHub remote URL (https or ssh, with or without
/// `.git`). Non-GitHub hosts return `None`.
fn github_repo(url: &str) -> Option<String> {
    let url = url.trim();
    let path = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .or_else(|| url.strip_prefix("https://github.com/"))
        .or_else(|| url.strip_prefix("http://github.com/"))?;
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = path.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

/// Branch name from `.git/HEAD`, falling back to `git symbolic-ref`.
fn current_branch(root: &Path) -> Option<String> {
    std::fs::read_to_string(root.join(".git/HEAD"))
        .ok()
        .and_then(|head| branch_from_head(&head).map(String::from))
        .or_else(|| {
            let out = command::Command::new("git")
                .args(["symbolic-ref", "--short", "HEAD"])
                .cwd(root)
                .without_shell()
                .exec()
                .ok()?;
            out.status
                .success()
                .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        })
        .filter(|b| !b.is_empty())
}

fn branch_from_head(head: &str) -> Option<&str> {
    head.trim().strip_prefix("ref: refs/heads/")
}

// ── secrets ─────────────────────────────────────────────────────

/// Store each available secret with `gh secret set`, reading the value from
/// stdin. Values come from the real environment first, then the vault; a name
/// with no value is reported, never invented. Values are never printed or
/// passed as arguments.
fn store_secrets(root: &Path, repo: &str, names: &[String]) -> Vec<SecretReport> {
    if names.is_empty() {
        return Vec::new();
    }
    let gh_missing = command::shell::find_command_on_path("gh").is_none();
    let vault = crate::secrets::deploy_env(root, names)
        .map(|env| env.vars)
        .unwrap_or_default();
    names
        .iter()
        .map(|name| {
            if gh_missing {
                return SecretReport {
                    name: name.clone(),
                    status: "failed",
                    detail: Some("gh is not installed: https://cli.github.com".into()),
                };
            }
            let value = std::env::var(name)
                .ok()
                .filter(|v| !v.is_empty())
                .or_else(|| vault.get(name).cloned());
            let Some(value) = value else {
                return SecretReport {
                    name: name.clone(),
                    status: "skipped",
                    detail: Some("no value in the environment or vault".into()),
                };
            };
            let out = command::Command::new("gh")
                .args(["secret", "set", name, "--repo", repo])
                .input([value.as_str()])
                .without_shell()
                .exec();
            match out {
                Ok(out) if out.status.success() => SecretReport {
                    name: name.clone(),
                    status: "set",
                    detail: None,
                },
                Ok(out) => SecretReport {
                    name: name.clone(),
                    status: "failed",
                    detail: Some(String::from_utf8_lossy(&out.stderr).trim().to_string()),
                },
                Err(e) => SecretReport {
                    name: name.clone(),
                    status: "failed",
                    detail: Some(e.to_string()),
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_remotes_in_every_common_form() {
        for url in [
            "https://github.com/getappz/cli.git",
            "https://github.com/getappz/cli",
            "git@github.com:getappz/cli.git",
            "ssh://git@github.com/getappz/cli.git",
            "https://github.com/getappz/cli/",
        ] {
            assert_eq!(github_repo(url).as_deref(), Some("getappz/cli"), "{url}");
        }
        assert_eq!(github_repo("git@gitlab.com:getappz/cli.git"), None);
        assert_eq!(github_repo("https://github.com/getappz"), None);
        assert_eq!(github_repo("https://github.com/a/b/c"), None);
    }

    #[test]
    fn reads_origin_url_from_git_config() {
        let config = "[core]\n\tbare = false\n[remote \"origin\"]\n\turl = git@github.com:getappz/cli.git\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n[remote \"upstream\"]\n\turl = https://github.com/other/cli.git\n";
        assert_eq!(
            origin_from_git_config(config).as_deref(),
            Some("git@github.com:getappz/cli.git")
        );
        assert_eq!(origin_from_git_config("[core]\n"), None);
    }

    #[test]
    fn branch_comes_from_head_or_a_safe_flag() {
        assert_eq!(branch_from_head("ref: refs/heads/main\n"), Some("main"));
        assert_eq!(
            branch_from_head("ref: refs/heads/feat/ci\n"),
            Some("feat/ci")
        );
        assert_eq!(branch_from_head("a1b2c3d4\n"), None);
        assert!(valid_branch("main"));
        assert!(valid_branch("release/1.2"));
        assert!(!valid_branch("--upload-pack=evil"));
        assert!(!valid_branch("bad branch"));
    }

    #[test]
    fn secret_names_are_the_token_then_extra_names_without_duplicates() {
        let names = secret_names(
            "VERCEL_TOKEN",
            &[
                "VERCEL_ORG_ID".into(),
                "VERCEL_TOKEN".into(),
                "bad-name".into(),
            ],
        );
        assert_eq!(names, vec!["VERCEL_TOKEN", "VERCEL_ORG_ID"]);
    }

    #[test]
    fn workflow_pins_the_target_and_reads_only_named_secrets() {
        let contents = workflow_contents("vercel", "main", &["VERCEL_TOKEN".into()]);
        assert!(contents.contains("branches: [\"main\"]"), "{contents}");
        assert!(
            contents.contains("run: appz deploy --target vercel --prod --yes"),
            "{contents}"
        );
        assert!(
            contents.contains("VERCEL_TOKEN: ${{ secrets.VERCEL_TOKEN }}"),
            "{contents}"
        );
        assert!(!contents.contains("pull_request"), "{contents}");
    }

    #[test]
    fn workflow_without_secrets_has_no_env_block() {
        let contents = workflow_contents("github-pages", "main", &[]);
        assert!(!contents.contains("        env:"), "{contents}");
    }

    #[test]
    fn refuses_to_overwrite_an_existing_workflow_without_force() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join(".git")).unwrap();
        std::fs::write(
            root.join(".git/config"),
            "[remote \"origin\"]\n\turl = https://github.com/getappz/cli.git\n",
        )
        .unwrap();
        std::fs::write(
            root.join("appz.jsonc"),
            r#"{ "deploy": { "default": "vercel" } }"#,
        )
        .unwrap();
        std::fs::create_dir_all(root.join(".github/workflows")).unwrap();
        std::fs::write(root.join(WORKFLOW_PATH), "old").unwrap();

        let req = CiRequest {
            target: None,
            branch: Some("main".into()),
            force: false,
            set_secrets: false,
        };
        let err = run(root, &req).unwrap_err();
        assert!(err.contains("--force"), "{err}");

        let req = CiRequest { force: true, ..req };
        let report = run(root, &req).unwrap();
        assert_eq!(report.repo, "getappz/cli");
        assert_eq!(report.target, "vercel");
        assert!(
            std::fs::read_to_string(root.join(WORKFLOW_PATH))
                .unwrap()
                .contains("appz deploy --target vercel")
        );
    }
}
