use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::context::DeployContext;
use crate::error::{DeployError, DeployResult};
use crate::output::{AuthState, DeployOutput, DeploymentInfo, DetectedConfig, PrerequisiteStatus};

/// Timeout for the actual deploy invocation (platform CLIs can legitimately
/// take minutes to build+upload).
pub const DEPLOY_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// Timeout for quick status/list queries.
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(30);

/// A hosting-platform deploy provider. Every operation shells out to the
/// platform's OWN CLI (vercel/netlify/wrangler/...) — appz never talks to a
/// platform's API directly; it's a unified front-end, not a competing cloud.
///
/// Synchronous by design: deploy CLIs are one-shot child processes, not
/// long-lived connections, so there's no need for an async runtime here
/// (the upstream version this was ported from wrapped every method in
/// `async_trait` for no operational benefit).
pub trait DeployProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn slug(&self) -> &'static str;
    fn cli_tool(&self) -> &'static str;
    /// npm package to `npm install -g` when the CLI is missing — not always
    /// the same as `cli_tool()` (e.g. Netlify's binary is `netlify` but the
    /// package is `netlify-cli`).
    fn install_package(&self) -> &'static str;
    fn auth_env_var(&self) -> &'static str;
    /// mise tool spec used to install this provider's CLI (`appz deploy --yes`).
    fn mise_tool(&self) -> String {
        format!("npm:{}", self.install_package())
    }
    /// Args for a read-only "am I signed in" probe. Only its exit code is
    /// used: its output can contain the account email and is never logged.
    fn whoami_args(&self) -> &'static [&'static str];
    /// Args for the CLI's interactive login.
    fn login_args(&self) -> &'static [&'static str] {
        &["login"]
    }
    /// The login command as a user would type it, for messages and prompts.
    fn login_hint(&self) -> String {
        format!("{} {}", self.cli_tool(), self.login_args().join(" "))
    }

    /// Whether the CLI is signed in. A token env var counts as signed in
    /// without a network call. A non-zero probe means not signed in, and a
    /// probe that cannot run or times out is `Unknown` (proceed; the CLI
    /// reports its own error).
    fn auth_status(&self, ctx: &DeployContext) -> AuthState {
        let var = self.auth_env_var();
        let in_ctx = ctx.run_env.get(var).is_some_and(|v| !v.is_empty());
        if in_ctx || std::env::var(var).is_ok_and(|v| !v.is_empty()) {
            return AuthState::Authenticated;
        }
        match ctx.exec(self.cli_tool(), self.whoami_args(), QUERY_TIMEOUT) {
            Ok(out) if out.success => AuthState::Authenticated,
            Ok(_) => AuthState::NotAuthenticated,
            Err(_) => AuthState::Unknown,
        }
    }

    /// Run the CLI's interactive login with the terminal inherited. Callers
    /// must only do this with a real terminal: login waits on a browser.
    fn login(&self, ctx: &DeployContext) -> DeployResult<()> {
        match ctx.exec_interactive(self.cli_tool(), self.login_args()) {
            Ok(true) => Ok(()),
            Ok(false) => Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("`{}` did not complete", self.login_hint()),
            }),
            Err(reason) => Err(DeployError::DeployFailed {
                provider: self.name(),
                reason,
            }),
        }
    }

    fn check_prerequisites(&self) -> DeployResult<PrerequisiteStatus>;
    fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>>;

    fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput>;
    fn deploy_preview(&self, ctx: &DeployContext) -> DeployResult<DeployOutput>;

    fn list_deployments(&self, ctx: &DeployContext) -> DeployResult<Vec<DeploymentInfo>> {
        let _ = ctx;
        Err(DeployError::Unsupported {
            provider: self.name(),
            operation: "list_deployments",
        })
    }

    fn rollback(&self, ctx: &DeployContext, deployment_id: &str) -> DeployResult<DeployOutput> {
        let _ = (ctx, deployment_id);
        Err(DeployError::Unsupported {
            provider: self.name(),
            operation: "rollback",
        })
    }
}

pub fn create_provider_registry() -> Vec<Box<dyn DeployProvider>> {
    vec![
        Box::new(crate::providers::vercel::VercelProvider),
        Box::new(crate::providers::netlify::NetlifyProvider),
        Box::new(crate::providers::cloudflare::CloudflareProvider),
    ]
}

pub fn get_provider(slug: &str) -> DeployResult<Box<dyn DeployProvider>> {
    create_provider_registry()
        .into_iter()
        .find(|p| p.slug() == slug)
        .ok_or_else(|| DeployError::ProviderNotFound {
            slug: slug.to_string(),
        })
}

pub fn available_provider_slugs() -> Vec<&'static str> {
    create_provider_registry()
        .iter()
        .map(|p| p.slug())
        .collect()
}

/// Slugs of providers whose project config/state files exist in
/// `project_dir`, in registry order. A provider whose detection errors
/// (e.g. unreadable file) is treated as not detected.
pub fn detect_providers(project_dir: &Path) -> Vec<&'static str> {
    create_provider_registry()
        .iter()
        .filter(|p| matches!(p.detect_config(project_dir), Ok(Some(_))))
        .map(|p| p.slug())
        .collect()
}

/// Fail fast when the provider's CLI is on neither `PATH` nor `extra`
/// (normally `appz_core::mise_bin_paths`, so mise-installed CLIs are found).
///
/// A missing auth env var is deliberately NOT checked here: every provider
/// CLI also accepts interactive login state (`vercel login`, `wrangler login`)
/// that we cannot see, so the CLI itself reports an auth failure.
pub fn ensure_cli_available(p: &dyn DeployProvider, extra: &[PathBuf]) -> DeployResult<()> {
    match command::shell::find_command_with_paths(p.cli_tool(), extra) {
        Some(_) => Ok(()),
        None => Err(missing_cli(p)),
    }
}

fn missing_cli(p: &dyn DeployProvider) -> DeployError {
    DeployError::CliMissing {
        provider: p.name(),
        tool: p.cli_tool(),
        install_hint: format!("mise use -g {}", p.mise_tool()),
    }
}

/// Install the provider's CLI through mise (installing mise itself if needed)
/// and return mise's bin dirs for `project_dir`, which now include the new CLI.
pub fn install_cli_with_mise(
    p: &dyn DeployProvider,
    project_dir: &Path,
) -> DeployResult<Vec<PathBuf>> {
    let fail = |reason: String| DeployError::CliInstallFailed {
        provider: p.name(),
        reason,
    };
    appz_core::ensure_mise(project_dir).map_err(fail)?;
    // npm-backed CLIs need Node to install and to run; provision it first
    // (through mise too) when it is on neither PATH nor mise's bin dirs.
    let tool = p.mise_tool();
    if tool.starts_with("npm:")
        && command::shell::find_command_with_paths("node", &appz_core::mise_bin_paths(project_dir))
            .is_none()
    {
        eprintln!("installing node (required by {tool}) with mise...");
        appz_core::install_global_tool("node@lts").map_err(fail)?;
    }
    appz_core::install_global_tool(&tool).map_err(fail)?;
    let paths = appz_core::mise_bin_paths(project_dir);
    if command::shell::find_command_with_paths(p.cli_tool(), &paths).is_none() {
        return Err(fail(format!(
            "installed {} but '{}' is not in mise's bin paths",
            p.mise_tool(),
            p.cli_tool()
        )));
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::cloudflare::CloudflareProvider;

    fn temp_project(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("appz-deploy-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detect_providers_finds_each_configured_provider_in_registry_order() {
        let dir = temp_project("detect-two");
        std::fs::write(dir.join("wrangler.jsonc"), "{\"name\":\"w\"}").unwrap();
        std::fs::write(dir.join("netlify.toml"), "").unwrap();

        assert_eq!(detect_providers(&dir), vec!["netlify", "cloudflare"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_providers_empty_when_nothing_configured() {
        let dir = temp_project("detect-none");
        assert!(detect_providers(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_cli_error_tells_the_user_how_to_install_it_with_mise() {
        let err = missing_cli(&CloudflareProvider).to_string();
        assert!(err.contains("mise use -g npm:wrangler"), "{err}");
    }

    #[test]
    fn cli_found_only_in_extra_paths_counts_as_available() {
        let dir = temp_project("cli-extra");
        #[cfg(windows)]
        std::fs::write(dir.join("wrangler.cmd"), "@echo off\r\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let p = dir.join("wrangler");
            std::fs::write(&p, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        assert!(ensure_cli_available(&CloudflareProvider, std::slice::from_ref(&dir)).is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }
    /// Provider with a unique CLI name and token variable, so results never
    /// depend on what is installed or set on the machine running the tests.
    struct Stub {
        cli: &'static str,
    }

    impl DeployProvider for Stub {
        fn name(&self) -> &'static str {
            "Stub"
        }
        fn slug(&self) -> &'static str {
            "stub"
        }
        fn cli_tool(&self) -> &'static str {
            self.cli
        }
        fn install_package(&self) -> &'static str {
            "stub"
        }
        fn auth_env_var(&self) -> &'static str {
            "APPZ_TEST_NO_SUCH_TOKEN_VAR"
        }
        fn whoami_args(&self) -> &'static [&'static str] {
            &["whoami"]
        }
        fn check_prerequisites(&self) -> DeployResult<PrerequisiteStatus> {
            Ok(PrerequisiteStatus::Ready)
        }
        fn detect_config(&self, _: &Path) -> DeployResult<Option<DetectedConfig>> {
            Ok(None)
        }
        fn deploy(&self, _: &DeployContext) -> DeployResult<DeployOutput> {
            unreachable!()
        }
        fn deploy_preview(&self, _: &DeployContext) -> DeployResult<DeployOutput> {
            unreachable!()
        }
    }

    /// Put a CLI named `name` into a fresh temp dir that exits with `code`,
    /// and return a context whose `extra_path` is that dir.
    fn ctx_with_cli(label: &str, name: &str, code: i32) -> (DeployContext, std::path::PathBuf) {
        let dir = temp_project(label);
        #[cfg(windows)]
        std::fs::write(
            dir.join(format!("{name}.cmd")),
            format!(
                "@echo off
exit /b {code}
"
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let p = dir.join(name);
            std::fs::write(
                &p,
                format!(
                    "#!/bin/sh
exit {code}
"
                ),
            )
            .unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut ctx = DeployContext::new(dir.clone(), "dist".into());
        ctx.extra_path = vec![dir.clone()];
        (ctx, dir)
    }

    #[test]
    fn whoami_success_means_authenticated() {
        let (ctx, dir) = ctx_with_cli("auth-ok", "appz-auth-ok", 0);
        assert_eq!(
            Stub {
                cli: "appz-auth-ok"
            }
            .auth_status(&ctx),
            AuthState::Authenticated
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn whoami_failure_means_not_authenticated() {
        let (ctx, dir) = ctx_with_cli("auth-no", "appz-auth-no", 1);
        assert_eq!(
            Stub {
                cli: "appz-auth-no"
            }
            .auth_status(&ctx),
            AuthState::NotAuthenticated
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn token_in_env_skips_the_probe() {
        // The CLI would report "not signed in", but a token means we never ask it.
        let (mut ctx, dir) = ctx_with_cli("auth-token", "appz-auth-token", 1);
        ctx.run_env
            .insert("APPZ_TEST_NO_SUCH_TOKEN_VAR".into(), "tok".into());
        assert_eq!(
            Stub {
                cli: "appz-auth-token"
            }
            .auth_status(&ctx),
            AuthState::Authenticated
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn probe_that_cannot_run_is_unknown_not_a_failure() {
        let ctx = DeployContext::new(std::env::temp_dir(), "dist".into());
        assert_eq!(
            Stub {
                cli: "appz-no-such-cli-xyz"
            }
            .auth_status(&ctx),
            AuthState::Unknown
        );
    }

    #[test]
    fn login_hint_is_the_command_a_user_would_type() {
        assert_eq!(Stub { cli: "stubcli" }.login_hint(), "stubcli login");
    }
}
