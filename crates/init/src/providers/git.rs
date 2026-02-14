//! Git provider: download from GitHub, GitLab, Bitbucket.

use async_trait::async_trait;
use tracing::instrument;

use crate::config::InitContext;
use crate::error::{InitError, InitResult};
use crate::output::InitOutput;
use crate::provider::InitProvider;
use crate::sources::git::download_git;
use crate::ui;

pub struct GitProvider;

#[async_trait]
impl InitProvider for GitProvider {
    fn name(&self) -> &str {
        "Git"
    }

    fn slug(&self) -> &str {
        "git"
    }

    #[instrument(skip_all)]
    async fn init(&self, ctx: &InitContext) -> InitResult<InitOutput> {
        let project_path = ctx.options.project_path();
        let quiet = ctx.options.json_output || ctx.options.is_ci;

        ui::section_title(&ctx.options, "Downloading from Git repository...");

        let template_dir = download_git(&ctx.source, None, None, quiet).await?;

        ui::info(&ctx.options, "Copying files to project...");

        ctx.fs()
            .copy_from_external(&template_dir, ".")
            .map_err(|e| InitError::FsError(format!("Copy failed: {}", e)))?;

        let framework = detect_framework(&project_path)?;
        run_install_if_needed(ctx, &framework).await?;

        Ok(InitOutput {
            project_path,
            framework,
            installed: !ctx.options.skip_install,
        })
    }
}

pub(crate) fn detect_framework(project_path: &std::path::Path) -> InitResult<Option<String>> {
    let package_json = project_path.join("package.json");
    if package_json.exists() {
        if let Ok(content) = std::fs::read_to_string(&package_json) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                let deps = json.get("dependencies").and_then(|d| d.as_object());
                let framework_names = [
                    ("astro", "Astro"),
                    ("next", "Next.js"),
                    ("vite", "Vite"),
                    ("@sveltejs/kit", "SvelteKit"),
                    ("nuxt", "Nuxt"),
                    ("@remix-run/react", "Remix"),
                ];
                for (pkg, name) in framework_names {
                    if deps.map(|d| d.contains_key(pkg)).unwrap_or(false) {
                        return Ok(Some(name.to_string()));
                    }
                }
            }
        }
    }
    Ok(None)
}

pub(crate) async fn run_install_if_needed(
    ctx: &InitContext,
    _framework: &Option<String>,
) -> InitResult<()> {
    if ctx.options.skip_install {
        return Ok(());
    }
    let project_path = ctx.project_path();
    if !project_path.join("package.json").exists() {
        return Ok(());
    }
    ui::info(&ctx.options, "Installing dependencies...");
    let install_cmd = if project_path.join("pnpm-lock.yaml").exists() {
        "pnpm install"
    } else if project_path.join("yarn.lock").exists() {
        "yarn install"
    } else if project_path.join("bun.lockb").exists() {
        "bun install"
    } else {
        "npm install"
    };
    let out = ctx.exec(install_cmd).await?;
    if !out.success() {
        tracing::warn!("Install command failed: {}", out.stderr());
    }
    Ok(())
}
