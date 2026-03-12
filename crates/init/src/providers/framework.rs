//! Framework provider: runs framework create commands (e.g. npm create astro@latest).

use async_trait::async_trait;
use tracing::instrument;

use crate::config::InitContext;
use crate::error::{InitError, InitResult};
use crate::output::InitOutput;
use crate::provider::InitProvider;
use crate::ui;

/// Framework slugs that have create commands.
const FRAMEWORK_CREATE: &[(&str, &str)] = &[
    ("astro", "npm create astro@latest"),
    ("nextjs", "npx create-next-app@latest"),
    ("vite", "npm create vite@latest"),
    ("sveltekit", "npm create svelte@latest"),
    ("nuxt", "npx nuxi@latest init"),
    ("remix", "npx create-remix@latest"),
    ("docusaurus", "npx create-docusaurus@latest"),
    ("vitepress", "npx vitepress@latest init"),
    ("gatsby", "npm create gatsby@latest"),
    ("eleventy", "npm create @11ty/eleventy@latest"),
];

/// Check if a slug has a framework create command.
pub fn has_create_command(slug: &str) -> bool {
    FRAMEWORK_CREATE.iter().any(|(s, _)| *s == slug)
}

/// Get the create command for a framework slug.
pub fn get_create_command(slug: &str) -> Option<&'static str> {
    FRAMEWORK_CREATE
        .iter()
        .find(|(s, _)| *s == slug)
        .map(|(_, cmd)| *cmd)
}

pub struct FrameworkProvider;

#[async_trait]
impl InitProvider for FrameworkProvider {
    fn name(&self) -> &str {
        "Framework"
    }

    fn slug(&self) -> &str {
        "framework"
    }

    #[instrument(skip_all)]
    async fn init(&self, ctx: &InitContext) -> InitResult<InitOutput> {
        let slug = &ctx.source;
        let create_cmd = get_create_command(slug)
            .ok_or_else(|| InitError::SourceNotFound(slug.clone()))?;

        ui::section_title(&ctx.options, "Creating project with framework...");
        ui::info(&ctx.options, &format!("Running: {}", create_cmd));

        // Framework create commands typically create a subdirectory. We need to run
        // from the parent of project_path. The sandbox is at project_path - so we
        // run from output_dir. But the sandbox root is project_path. So we need to
        // create sandbox at output_dir and run create_cmd with project_name as argument.
        //
        // Actually: create-astro etc. create a directory. So we run:
        //   cd output_dir && create_cmd project_name
        // The sandbox needs to be at output_dir (parent). Let me check the flow.
        //
        // InitContext.sandbox.project_path() = output_dir/project_name. So the sandbox
        // is ALREADY at the project dir. For framework create, the create command
        // creates the project IN PLACE. So we need a different approach:
        //
        // For framework: the create command creates a NEW directory. So we run it
        // from output_dir with project_name as the target. The sandbox should be
        // at output_dir (parent), not at project_path. The provider receives a
        // sandbox - the contract says sandbox is at the target path.
        //
        // Re-reading the plan: "Framework create: run `npm create astro@latest <name>`
        // with sandbox cwd = parent dir so project is created inside"
        //
        // So the init flow needs to create sandbox at output_dir (parent of project).
        // The InitContext will have options.project_path() = output_dir/project_name.
        // The sandbox should be at output_dir so we can run `create_cmd project_name`.
        //
        // The create_sandbox happens in the run() entry point. So we need run() to
        // create sandbox at the right place. For framework: sandbox at output_dir.
        // For git/npm/etc: sandbox at output_dir/project_name (project dir).
        //
        // This means the run() flow is provider-specific for WHERE the sandbox is.
        // Let me check the plan again...
        //
        // "Create sandbox at output_dir/project_name before any file writes"
        // So sandbox is always at project path. For framework create commands,
        // they create a directory - so we'd be creating output_dir/project_name
        // and then running create_cmd which would try to create project_name inside.
        // That would create output_dir/project_name/project_name.
        //
        // The solution: for framework, run create_cmd with . as the target (current dir)
        // so it creates in the sandbox root. Many create commands support:
        //   create-astro .
        //   create-next-app .
        // Let me check - create-astro: "npm create astro@latest" can take a path.
        // create-next-app: "npx create-next-app@latest ." works.
        //
        // So we run: create_cmd .  (with cwd = sandbox root which is project_path)
        // But wait - the sandbox is at project_path. So we'd run in project_path.
        // create-astro . would create IN the current dir. So we need an EMPTY
        // project_path. The run() creates the dir. So we have output_dir/project_name
        // as an empty dir, sandbox there, we run "create-astro ." and it populates.
        // Perfect!

        let status = ctx.exec_interactive(&format!("{} .", create_cmd)).await?;
        if !status.success() {
            return Err(InitError::CommandFailed(
                create_cmd.to_string(),
                "Framework create command failed".to_string(),
            ));
        }

        let project_path = ctx.project_path();
        let framework = frameworks::find_by_slug(slug)
            .map(|f| f.name.to_string());

        Ok(InitOutput {
            project_path,
            framework,
            installed: false, // Framework create usually runs install
        })
    }
}
