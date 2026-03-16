use crate::args::{BlueprintApplyArgs, BlueprintGenArgs};
use crate::ddev_helpers::{has_ddev_config, is_ddev_available};
use crate::wp_runtime;
use crate::session::AppzSession;
use clap::Subcommand;
use starbase::AppResult;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Subcommand, Debug, Clone)]
pub enum BlueprintCommands {
    /// Apply a WordPress Playground blueprint to the current project
    Apply(BlueprintApplyArgs),
    /// Generate a blueprint.json from the current WordPress setup
    Gen(BlueprintGenArgs),
}

pub async fn run(session: AppzSession, command: BlueprintCommands) -> AppResult {
    match command {
        BlueprintCommands::Apply(args) => apply(session, args).await,
        BlueprintCommands::Gen(args) => gen(session, args).await,
    }
}

// ---------------------------------------------------------------------------
// apply
// ---------------------------------------------------------------------------

async fn apply(session: AppzSession, args: BlueprintApplyArgs) -> AppResult {
    let project_path = session.working_dir.clone();
    let runtime = resolve_runtime(&project_path, args.playground)?;
    validate_wordpress_project(&project_path, runtime.as_ref())?;

    let blueprint_path = args
        .file
        .unwrap_or_else(|| project_path.join("blueprint.json"));

    if args.dry_run {
        dry_run_blueprint(&blueprint_path)?;
    } else {
        apply_blueprint_with_runtime(&project_path, &blueprint_path, runtime)?;
    }

    Ok(None)
}

/// Apply a blueprint file to a WordPress project using the DDEV runtime.
/// Shared between `appz blueprint apply` and `appz init wordpress --blueprint`.
/// This is the backward-compatible entry point.
pub fn apply_blueprint(
    project_path: &Path,
    blueprint_path: &Path,
) -> Result<(), miette::Report> {
    let runtime: Arc<dyn blueprint::WordPressRuntime> = Arc::new(blueprint::DdevRuntime::new());
    apply_blueprint_with_runtime(project_path, blueprint_path, runtime)
}

/// Apply a blueprint file to a WordPress project using a specific runtime.
pub fn apply_blueprint_with_runtime(
    project_path: &Path,
    blueprint_path: &Path,
    runtime: Arc<dyn blueprint::WordPressRuntime>,
) -> Result<(), miette::Report> {
    println!("Loading blueprint: {}", blueprint_path.display());

    let bp = blueprint::load(blueprint_path)
        .map_err(|e| miette::miette!("{}", e))?;

    if let Some(ref meta) = bp.meta {
        if let Some(ref title) = meta.title {
            println!("Blueprint: {}", title);
        }
        if let Some(ref desc) = meta.description {
            println!("  {}", desc);
        }
    }

    let step_count = bp.steps.len();
    let shorthand_count = bp.plugins.len()
        + bp.constants.as_ref().map(|c| c.len()).unwrap_or(0)
        + bp.site_options.as_ref().map(|o| o.len()).unwrap_or(0);

    if step_count > 0 || shorthand_count > 0 {
        println!(
            "Applying {} step(s) via {}...",
            step_count + shorthand_count,
            runtime.name()
        );
    }

    let executor = blueprint::BlueprintExecutor::new(project_path.to_path_buf(), runtime);
    executor
        .execute(&bp)
        .map_err(|e| miette::miette!("{}", e))?;

    println!("Blueprint applied successfully.");
    Ok(())
}

/// Dry-run: parse and display what would happen without executing.
fn dry_run_blueprint(blueprint_path: &Path) -> Result<(), miette::Report> {
    println!("Dry run: {}", blueprint_path.display());

    let bp = blueprint::load(blueprint_path)
        .map_err(|e| miette::miette!("{}", e))?;

    if let Some(ref meta) = bp.meta {
        if let Some(ref title) = meta.title {
            println!("Blueprint: {}", title);
        }
    }

    // Show preferred versions
    if let Some(ref versions) = bp.preferred_versions {
        if let Some(ref php) = versions.php {
            println!("  PHP version: {}", php);
        }
        if let Some(ref wp) = versions.wp {
            println!("  WP version: {}", wp);
        }
    }

    // Show top-level shorthands
    if !bp.plugins.is_empty() {
        println!("  Plugins (shorthand): {} to install", bp.plugins.len());
    }
    if let Some(ref consts) = bp.constants {
        if !consts.is_empty() {
            println!("  Constants: {} to define", consts.len());
        }
    }
    if let Some(ref opts) = bp.site_options {
        if !opts.is_empty() {
            println!("  Site options: {} to set", opts.len());
        }
    }

    // Show steps
    if bp.steps.is_empty() {
        println!("  No steps defined.");
    } else {
        println!("  Steps ({}):", bp.steps.len());
        for (i, entry) in bp.steps.iter().enumerate() {
            match entry {
                blueprint::types::StepEntry::Step(step) => {
                    println!("    [{}/{}] {}", i + 1, bp.steps.len(), describe_step(step));
                }
                blueprint::types::StepEntry::String(s) => {
                    println!("    [{}/{}] installPlugin: {} (shorthand)", i + 1, bp.steps.len(), s);
                }
                blueprint::types::StepEntry::Bool(false) | blueprint::types::StepEntry::Null => {
                    println!("    [{}/{}] (skipped)", i + 1, bp.steps.len());
                }
                blueprint::types::StepEntry::Bool(true) => {
                    println!("    [{}/{}] (no-op)", i + 1, bp.steps.len());
                }
            }
        }
    }

    println!("\nDry run complete. No changes were made.");
    Ok(())
}

/// Human-readable description of a step for dry-run output.
fn describe_step(step: &blueprint::types::Step) -> String {
    use blueprint::types::*;
    match step {
        Step::InstallPlugin(s) => {
            let source = match &s.plugin_data {
                Some(ResourceData::File(FileResource::WordPressPlugin { slug })) => slug.clone(),
                Some(ResourceData::File(FileResource::Url { url, .. })) => url.clone(),
                _ => "unknown".to_string(),
            };
            let activate = s.options.as_ref().and_then(|o| o.activate).unwrap_or(true);
            format!("installPlugin: {} (activate: {})", source, activate)
        }
        Step::InstallTheme(s) => {
            let source = match &s.theme_data {
                Some(ResourceData::File(FileResource::WordPressTheme { slug })) => slug.clone(),
                Some(ResourceData::File(FileResource::Url { url, .. })) => url.clone(),
                _ => "unknown".to_string(),
            };
            format!("installTheme: {}", source)
        }
        Step::ActivatePlugin(s) => format!("activatePlugin: {}", s.plugin_path),
        Step::ActivateTheme(s) => format!("activateTheme: {}", s.theme_folder_name),
        Step::SetSiteOptions(s) => {
            let keys: Vec<&String> = s.options.keys().collect();
            format!("setSiteOptions: {}", keys.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", "))
        }
        Step::DefineWpConfigConsts(s) => {
            let keys: Vec<&String> = s.consts.keys().collect();
            format!("defineWpConfigConsts: {}", keys.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", "))
        }
        Step::DefineSiteUrl(s) => format!("defineSiteUrl: {}", s.site_url),
        Step::SetSiteLanguage(s) => format!("setSiteLanguage: {}", s.language),
        Step::Login(s) => format!("login: {}", s.username.as_deref().unwrap_or("admin")),
        Step::WpCli(s) => {
            let cmd = match &s.command {
                WpCliCommand::String(c) => c.clone(),
                WpCliCommand::Args(a) => a.join(" "),
            };
            format!("wp-cli: wp {}", cmd)
        }
        Step::RunPHP(_) => "runPHP: <code>".to_string(),
        Step::RunPHPWithOptions(_) => "runPHPWithOptions: <code>".to_string(),
        Step::WriteFile(s) => format!("writeFile: {}", s.path),
        Step::WriteFiles(s) => format!("writeFiles: {}", s.write_to_path),
        Step::Mkdir(s) => format!("mkdir: {}", s.path),
        Step::Rm(s) => format!("rm: {}", s.path),
        Step::Rmdir(s) => format!("rmdir: {}", s.path),
        Step::Cp(s) => format!("cp: {} -> {}", s.from_path, s.to_path),
        Step::Mv(s) => format!("mv: {} -> {}", s.from_path, s.to_path),
        Step::RunSql(_) => "runSql: <query>".to_string(),
        Step::ResetData(_) => "resetData: DROP ALL TABLES (destructive!)".to_string(),
        Step::EnableMultisite(_) => "enableMultisite".to_string(),
        Step::UpdateUserMeta(s) => format!("updateUserMeta: user {}", s.user_id),
        Step::ImportWxr(_) => "importWxr: <file>".to_string(),
        Step::ImportThemeStarterContent(s) => {
            format!("importThemeStarterContent: {}", s.theme_slug.as_deref().unwrap_or("current"))
        }
        Step::ImportWordPressFiles(_) => "importWordPressFiles: <zip>".to_string(),
        Step::Unzip(s) => format!("unzip: -> {}", s.extract_to_path),
        Step::Request(_) => "request: <http request>".to_string(),
        Step::RunWpInstallationWizard(_) => "runWpInstallationWizard".to_string(),
    }
}

// ---------------------------------------------------------------------------
// gen
// ---------------------------------------------------------------------------

async fn gen(session: AppzSession, args: BlueprintGenArgs) -> AppResult {
    let project_path = session.working_dir.clone();
    let runtime = resolve_runtime(&project_path, args.playground)?;
    validate_wordpress_project(&project_path, runtime.as_ref())?;

    let output_path = args
        .output
        .unwrap_or_else(|| project_path.join("blueprint.json"));

    // If blueprint already exists and --force not set, pass through
    if output_path.exists() && !args.force {
        println!(
            "Blueprint already exists: {}",
            output_path.display()
        );
        println!("Use --force to overwrite, or edit the existing file.");
        return Ok(None);
    }

    println!("Generating blueprint from current WordPress setup via {}...", runtime.name());

    let generator = blueprint::BlueprintGenerator::new(project_path, runtime);
    let bp_value = generator
        .generate()
        .map_err(|e| miette::miette!("{}", e))?;

    let json_str = serde_json::to_string_pretty(&bp_value)
        .map_err(|e| miette::miette!("Failed to serialize blueprint: {}", e))?;

    std::fs::write(&output_path, &json_str)
        .map_err(|e| miette::miette!("Failed to write {}: {}", output_path.display(), e))?;

    println!("Generated: {}", output_path.display());
    println!("\nYou can now:");
    println!("  - Edit the blueprint to customize it");
    println!("  - Commit it to git for reproducible setups");
    println!("  - Apply it with: appz blueprint apply");
    println!("  - Preview it with: appz blueprint apply --dry-run");

    Ok(None)
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Resolve the WordPress runtime based on project state and flags.
fn resolve_runtime(
    project_path: &Path,
    force_playground: bool,
) -> Result<Arc<dyn blueprint::WordPressRuntime>, miette::Report> {
    wp_runtime::resolve(project_path, force_playground)
}

/// Validate that the project is a WordPress project with a runtime configured.
fn validate_wordpress_project(
    project_path: &Path,
    runtime: &dyn blueprint::WordPressRuntime,
) -> Result<(), miette::Report> {
    if !runtime.is_available() {
        return Err(miette::miette!(
            "{} is not available. {}",
            runtime.name(),
            if runtime.slug() == "ddev" {
                "Install it: https://docs.ddev.com/en/stable/users/install/ddev-installation/"
            } else {
                "Install Node.js 20.18+: https://nodejs.org/"
            }
        ));
    }

    if !runtime.is_configured(project_path) {
        return Err(miette::miette!(
            "No {} configuration found in {}. Run `appz dev` first to set up the project{}.",
            runtime.name(),
            project_path.display(),
            if runtime.slug() == "playground" { " or use `appz init wordpress --playground`" } else { "" }
        ));
    }

    // Also check for WordPress files
    let has_wp_files = project_path.join("wp-config.php").exists()
        || project_path.join("wp-config-sample.php").exists()
        || project_path.join("wp-content").exists();

    if !has_wp_files {
        return Err(miette::miette!(
            "No WordPress files found in {}. blueprint commands require a WordPress project.",
            project_path.display()
        ));
    }

    Ok(())
}
