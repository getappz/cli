//! GitHub Pages deploy provider.
//!
//! ## Deployment model
//! `gh-pages` pushes the build output to the `gh-pages` branch of `origin`
//! using git credentials — there is no token variable and no platform login.
//! The deployment URL is derived from the git remote rather than guessed:
//!
//! * user/org pages (repo named `<user>.github.io`) → `https://<user>.github.io`
//! * project pages → `https://<user>.github.io/<repo>`
//!
//! Because `gh-pages` prints no deployment URL and every git repo has a
//! `.git` dir (too broad to auto-detect), this provider is never
//! auto-detected: the user passes `--target github-pages`.

use std::path::Path;
use std::time::Instant;

use crate::context::DeployContext;
use crate::error::{DeployError, DeployResult};
use crate::output::{AuthState, DeployOutput, DeployStatus, DetectedConfig, PrerequisiteStatus};
use crate::provider::{DEPLOY_TIMEOUT, DeployProvider, QUERY_TIMEOUT};

pub struct GithubPagesProvider;

impl GithubPagesProvider {
    fn deploy_internal(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        let start = Instant::now();
        if !std::fs::read_dir(ctx.output_path()).is_ok_and(|mut entries| entries.next().is_some()) {
            return Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: format!(
                    "build output '{}' is missing or empty: run appz build first",
                    ctx.output_dir
                ),
            });
        }
        if ctx.dry_run {
            return Ok(DeployOutput::dry_run(self.slug(), false));
        }

        let out = ctx
            .exec_live(
                "gh-pages",
                // --nojekyll: keep _next/_astro/_nuxt; --dotfiles: publish .well-known etc.
                &[
                    "-d",
                    &ctx.output_path().to_string_lossy(),
                    "--nojekyll",
                    "--dotfiles",
                ],
                DEPLOY_TIMEOUT,
            )
            .map_err(|reason| DeployError::DeployFailed {
                provider: self.name(),
                reason,
            })?;
        if !out.success {
            return Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("gh-pages failed:\n{}\n{}", out.stdout, out.stderr),
            });
        }

        let url = match std::fs::read_to_string(ctx.output_path().join("CNAME")) {
            Ok(domain) => {
                let domain = domain.trim();
                let valid = domain.len() <= 253
                    && domain.split('.').all(|label| {
                        !label.is_empty()
                            && label.len() <= 63
                            && label.as_bytes()[0].is_ascii_alphanumeric()
                            && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                            && label
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    });
                valid.then(|| format!("https://{domain}"))
            }
            Err(_) => ctx
                .exec(
                    "git",
                    &["config", "--get", "remote.origin.url"],
                    QUERY_TIMEOUT,
                )
                .ok()
                .filter(|out| out.success)
                .and_then(|out| github_pages_url(out.stdout.trim())),
        };
        Ok(DeployOutput {
            provider: self.slug().to_string(),
            url,
            is_preview: false,
            // gh-pages only confirms the push, not GitHub's asynchronous Pages build.
            status: DeployStatus::Unknown,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
            auth: None,
            setup_files: Vec::new(),
            cli_missing: false,
            link_needed: false,
            unchanged: false,
        })
    }
}

impl DeployProvider for GithubPagesProvider {
    fn name(&self) -> &'static str {
        "GitHub Pages"
    }
    fn slug(&self) -> &'static str {
        "github-pages"
    }
    fn cli_tool(&self) -> &'static str {
        "gh-pages"
    }
    fn install_package(&self) -> &'static str {
        "gh-pages"
    }
    fn auth_env_var(&self) -> &'static str {
        ""
    }
    fn whoami_args(&self) -> &'static [&'static str] {
        &[]
    }
    fn auth_status(&self, _: &DeployContext) -> AuthState {
        AuthState::Unknown
    }
    fn login(&self, _: &DeployContext) -> DeployResult<()> {
        Err(DeployError::Unsupported {
            provider: self.name(),
            operation: "login (configure git credentials)",
        })
    }
    fn login_args(&self) -> &'static [&'static str] {
        &[]
    }
    fn login_hint(&self) -> String {
        "make sure `git push` to origin works".into()
    }

    fn check_prerequisites(&self) -> DeployResult<PrerequisiteStatus> {
        if command::shell::find_command_on_path("gh-pages").is_none() {
            return Ok(PrerequisiteStatus::CliMissing {
                tool: "gh-pages".into(),
                install_hint: "npm install -g gh-pages".into(),
            });
        }
        Ok(PrerequisiteStatus::Ready)
    }

    /// No linked-state file exists for gh-pages — it pushes to `origin`, so a
    /// git repo is the only requirement. `appz init`/`appz deploy` runs inside
    /// a project dir, so treat it as linked.
    fn is_linked(&self, _ctx: &DeployContext) -> bool {
        true
    }

    fn link_args(&self, _ctx: &DeployContext, _interactive: bool) -> Option<Vec<String>> {
        None
    }

    fn detect_config(&self, _project_dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        // Never auto-detect: `.git` is too broad a marker and every project
        // has one. The user opts in with --target github-pages.
        Ok(None)
    }

    fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx)
    }

    fn deploy_preview(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        let _ = ctx;
        Err(DeployError::Unsupported {
            provider: self.name(),
            operation: "preview deploy (use --prod)",
        })
    }

    fn list_deployments(
        &self,
        _ctx: &DeployContext,
    ) -> DeployResult<Vec<crate::output::DeploymentInfo>> {
        Err(DeployError::Unsupported {
            provider: self.name(),
            operation: "list_deployments",
        })
    }

    fn rollback(&self, _ctx: &DeployContext, _deployment_id: &str) -> DeployResult<DeployOutput> {
        Err(DeployError::Unsupported {
            provider: self.name(),
            operation: "rollback",
        })
    }
}

/// Turn a GitHub remote URL into the Pages deployment URL.
///
/// Accepts `https://github.com/<user>/<repo>.git`,
/// `git@github.com:<user>/<repo>.git` and the same without `.git`.
/// A repo named `<user>.github.io` is a user/org Pages site served at the
/// apex; anything else is a project site served at `/{repo}`.
fn github_pages_url(origin: &str) -> Option<String> {
    let cleaned = origin
        .strip_prefix("git@github.com:")
        .or_else(|| origin.strip_prefix("https://github.com/"))
        .or_else(|| origin.strip_prefix("http://github.com/"))
        .or_else(|| origin.strip_prefix("ssh://git@github.com/"))?;
    let cleaned = cleaned.trim_end_matches('/');
    let cleaned = cleaned.strip_suffix(".git").unwrap_or(cleaned);
    let mut parts = cleaned.split('/');
    let user = parts.next()?;
    let repo = parts.next()?;
    if user.is_empty()
        || repo.is_empty()
        || parts.next().is_some()
        || !user.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || !repo
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return None;
    }
    if repo.eq_ignore_ascii_case(&format!("{user}.github.io")) {
        Some(format!("https://{user}.github.io"))
    } else {
        Some(format!("https://{user}.github.io/{repo}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_and_org_pages_serve_at_the_apex() {
        assert_eq!(
            github_pages_url("https://github.com/acme/acme.github.io.git"),
            Some("https://acme.github.io".to_string())
        );
        assert_eq!(
            github_pages_url("git@github.com:acme/acme.github.io.git"),
            Some("https://acme.github.io".to_string())
        );
    }

    #[test]
    fn project_pages_serve_under_the_repo_name() {
        assert_eq!(
            github_pages_url("https://github.com/acme/my-site.git"),
            Some("https://acme.github.io/my-site".to_string())
        );
        assert_eq!(
            github_pages_url("git@github.com:acme/my-site.git"),
            Some("https://acme.github.io/my-site".to_string())
        );
    }

    #[test]
    fn non_github_remotes_and_malformed_urls_return_none() {
        assert_eq!(
            github_pages_url("https://gitlab.com/acme/my-site.git"),
            None
        );
        assert_eq!(github_pages_url("not-a-url"), None);
        assert_eq!(github_pages_url("https://github.com/acme/site/extra"), None);
        assert_eq!(github_pages_url("https://github.com/ /site"), None);
        assert_eq!(
            github_pages_url("ssh://git@github.com/acme/site.git"),
            Some("https://acme.github.io/site".into())
        );
        assert_eq!(github_pages_url("https://github.com/acme"), None);
    }
}
