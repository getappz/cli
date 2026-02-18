//! Add (install) a skill from GitHub, URL, local path, or generate from code.
//!
//! When installing globally, if the current directory has agent dirs (e.g. `.cursor`, `.claude`),
//! creates symlinks so the installed skill is visible to those agents (e.g. `.cursor/skills/<name>`).

use crate::agents;
use crate::config::{self, AddSkillOptions};
use crate::context::SkillsContext;
use crate::providers;
use crate::skill_lock::{self, AddSkillLockInput};
use crate::source_parser;
use init::sources::git::{download_git, parse_git_source};
use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};
use starbase::AppResult;
use std::path::{Path, PathBuf};
use starbase_utils::{dirs, fs as starbase_fs};

/// Project-local AI agent skills subdirs to check for existing skills (relative to cwd).
fn project_skills_subdirs() -> Vec<&'static str> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for a in agents::all_agents() {
        let p = a.config().project_path;
        if seen.insert(p) {
            out.push(p);
        }
    }
    out
}

/// Install a skill from the given source.
pub async fn add(
    ctx: &SkillsContext,
    mut source: String,
    global: bool,
    project: bool,
    yes: bool,
    mut skill_filter: Option<String>,
    list_only: bool,
    agent: &[String],
    _all: bool,
    full_depth: bool,
    skill_filters_override: Option<Vec<String>>, // from skills.json when installing from config
    no_save: bool, // skip writing to skills.json
    code: bool,
    workdir: Option<PathBuf>,
    name: Option<String>,
) -> AppResult {
    if code {
        return add_from_code(
            ctx, global, project, yes, list_only, agent, workdir, name,
        )
        .await;
    }

    let _ = ui::layout::blank_line();

    // Parse source: owner/repo@skill, owner/repo:skill1,skill2, skills.sh URLs -> extract filters and base URL
    let parsed = source_parser::parse_source(&source);
    let mut skill_filters: Option<Vec<String>> = skill_filters_override;
    match &parsed {
        source_parser::ParsedSource::GitHub {
            url,
            skill_filter: k,
            skill_filters: kf,
            ..
        } => {
            if skill_filters.is_none() {
                if let Some(ref filters) = kf {
                    skill_filters = Some(filters.clone());
                } else if k.is_some() {
                    skill_filter = skill_filter.or_else(|| k.clone());
                }
            }
            source = url.clone();
        }
        source_parser::ParsedSource::DirectUrl { url }
        | source_parser::ParsedSource::WellKnown { url } => {
            return add_from_provider(ctx, url, global, project, yes, list_only, agent).await;
        }
        _ => {}
    }

    // Bare skill name: check project first, then global, then symlink into project
    if is_bare_skill_name(&source) {
        let cwd = ctx.working_dir.as_path();
        if let Some((_, location)) = try_skill_from_project_dirs(cwd, &source) {
            let _ = ui::status::success(&format!(
                "Skill '{}' is already available in this project at {}",
                source,
                location
            ));
            let _ = ui::layout::blank_line();
            return Ok(None);
        }
        if let Some(global_path) = try_skill_from_global_dirs(&source) {
            let _ = ui::status::info(&format!(
                "Found skill '{}' in global skills directory",
                source
            ));
            let linked = symlink_skill_into_project_agent_dirs(cwd, &source, &global_path, yes)?;
            let _ = ui::layout::blank_line();
            if linked {
                let _ = ui::status::success(&format!(
                    "Done. Skill '{}' is now available in this project",
                    source
                ));
            } else {
                let _ = ui::status::info(&format!(
                    "Skill '{}' is available globally but was not linked.",
                    source
                ));
            }
            return Ok(None);
        }
        return Err(miette::miette!(
            "Skill '{}' not found. Use owner/repo, https://..., ./local-path, or install the skill in ~/.appz/skills, ~/.cursor/skills, ~/.codex/skills, or ~/.agents/skills first.",
            source
        )
        .into());
    }

    // Default to global when neither -p nor -g
    let do_project = project;
    let do_global = global || (!project && !global);

    let target_dirs = agents::target_dirs_for_add(
        agent,
        ctx.working_dir.as_path(),
        ctx.user_appz_dir.as_deref(),
        do_project,
        do_global,
    );
    if target_dirs.is_empty() {
        return Err(miette::miette!(
            "No target directory. Use --project (-p) or --global (-g)."
        )
        .into());
    }

    let primary_target = &target_dirs[0].0;
    starbase_fs::create_dir_all(primary_target)
        .map_err(|e| miette::miette!("Failed to create skills directory: {}", e))?;
    for (dir, _) in target_dirs.iter().skip(1) {
        starbase_fs::create_dir_all(dir)
            .map_err(|e| miette::miette!("Failed to create skills directory: {}", e))?;
    }

    let source_dir = if is_git_source(&source) {
        // If URL points to a single skill and it's already installed, skip download
        let filter_for_existing = if skill_filters.is_some() {
            None
        } else {
            skill_filter.as_deref()
        };
        let use_existing = try_existing_skill_from_git_url(&source, primary_target, filter_for_existing);
        if let Some(existing_path) = use_existing {
            let _ = ui::status::info("Skill already installed; skipping download.");
            existing_path
        } else {
            // Show progress bar when not verbose (quiet = verbose to avoid bar + debug fighting)
            let _ = ui::status::info(&format!("Downloading skill from {}...", source));
            download_git(&source, None, None, ctx.verbose, Some("Downloading skill...")).await.map_err(|e| {
                let msg = e.to_string();
                miette::miette!(
                    "Could not download skill from \"{}\".\n\n{}\n\nCheck your network connection and that the repository exists. For GitHub you can use: owner/repo or a full URL (e.g. .../tree/main/path/to/skill).",
                    source,
                    msg
                )
            })?
        }
    } else if is_local_path(&source) {
        let cwd = ctx.working_dir.as_path();
        let path = if source.starts_with('/') || (source.len() >= 2 && &source[1..2] == ":") {
            PathBuf::from(&source)
        } else {
            cwd.join(&source)
        };
        path.canonicalize()
            .map_err(|e| miette::miette!("Local path not found: {} - {}", source, e))?
    } else {
        return Err(miette::miette!(
            "Invalid source: '{}'. Use owner/repo, https://..., or ./local-path",
            source
        )
        .into());
    };

    // Git/GitHub repos (vercel-labs/skills, astrolicious/agent-skills, etc.) use skills/ subdir.
    // Use full depth for git sources to find nested skills, matching npx skills behavior.
    let effective_full_depth = full_depth || is_git_source(&source);
    let skill_dirs = find_skill_dirs(&source_dir, effective_full_depth);

    if skill_dirs.is_empty() {
        return Err(miette::miette!(
            "No skills found (no SKILL.md in subdirectories)"
        )
        .into());
    }

    let to_install: Vec<_> = if let Some(ref filters) = skill_filters {
        let set: std::collections::HashSet<String> =
            filters.iter().map(|s| s.to_lowercase()).collect();
        skill_dirs
            .into_iter()
            .filter(|(n, _)| set.contains(&n.to_lowercase()))
            .collect()
    } else if let Some(ref name) = skill_filter {
        skill_dirs
            .into_iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
            .collect()
    } else {
        skill_dirs
    };

    if to_install.is_empty() {
        return Err(miette::miette!(
            "No matching skill '{}' found",
            skill_filter.as_deref().unwrap_or("")
        )
        .into());
    }

    if list_only {
        let _ = ui::status::info(&format!("Found {} skill(s) at {}", to_install.len(), source));
        for (name, path) in &to_install {
            let _ = ui::layout::indented(&format!("{}: {}", name, common::user_config::path_for_display(path)), 1);
        }
        let _ = ui::layout::blank_line();
        return Ok(None);
    }

    let targets_display = target_dirs
        .iter()
        .map(|(d, l)| format!("{} ({})", common::user_config::path_for_display(d), l))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = ui::status::info(&format!(
        "Found {} skill(s). Installing to {}",
        to_install.len(),
        targets_display
    ));
    let _ = ui::layout::blank_line();

    let show_install_spinner = to_install.len() > 1 && !ctx.verbose;
    #[allow(unused_assignments)]
    let mut install_spinner: Option<ui::progress::SpinnerHandle> = if show_install_spinner {
        Some(ui::progress::spinner("Installing skills..."))
    } else {
        None
    };

    for (name, path) in &to_install {
        for (target_dir, label) in &target_dirs {
            let dest = target_dir.join(name);
            if path.as_path() == dest.as_path() {
                let _ = ui::status::success(&format!("Already installed: {} ({})", name, label));
                continue;
            }
            if dest.exists() && !yes {
            install_spinner = None;
            let overwrite = ui::confirm_interactive(
                &format!("Skill '{}' already exists. Overwrite?", name),
                false,
            )
            .map_err(|e| miette::miette!("Prompt failed: {}", e))?;
                if !overwrite {
                    if show_install_spinner {
                        install_spinner = Some(ui::progress::spinner("Installing skills..."));
                    }
                    continue;
                }
                if show_install_spinner {
                    install_spinner = Some(ui::progress::spinner("Installing skills..."));
                }
            }
            copy_skill_dir(path, &dest)?;
            let _ = ui::status::success(&format!("Installed skill: {} ({})", name, label));
        }
    }

    // When installing to legacy ~/.appz/skills, symlink into project agent dirs if present
    let used_appz_global = agent.is_empty() && do_global;
    if used_appz_global {
        link_skills_into_agent_dirs(&ctx.working_dir, primary_target, &to_install)?;
    }

    // Update lock file for appz global installs from GitHub (enables check/update)
    if used_appz_global && is_git_source(&source) {
        if let Ok(parsed) = parse_git_source(&source) {
            let owner_repo = format!("{}/{}", parsed.user, parsed.repo);
            let source_type = if parsed.platform.contains("github") {
                "github"
            } else if parsed.platform.contains("gitlab") {
                "gitlab"
            } else {
                "git"
            };
            let token = skill_lock::get_github_token();
            for (name, path) in &to_install {
                let skill_path = path
                    .strip_prefix(&source_dir)
                    .ok()
                    .and_then(|p| p.to_str().map(|s| s.replace('\\', "/")))
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("{}/SKILL.md", s.trim_end_matches('/')))
                    .or_else(|| parsed.subfolder.as_ref().map(|sf| format!("{}/{}/SKILL.md", sf.trim_end_matches('/'), name)));
                let path_for_hash = skill_path
                    .as_deref()
                    .map(|s| s.strip_suffix("/SKILL.md").unwrap_or(s).to_string())
                    .unwrap_or_else(|| name.clone());
                let skill_folder_hash = if source_type == "github" {
                    skill_lock::fetch_skill_folder_hash(&owner_repo, &path_for_hash, token.as_deref())
                        .await
                } else {
                    None
                };
                let source_url = build_source_url(&source, &parsed);
                if let Err(e) = skill_lock::add_skill_to_lock(
                    ctx,
                    name.clone(),
                    AddSkillLockInput {
                        source: owner_repo.clone(),
                        source_type: source_type.to_string(),
                        source_url,
                        skill_path,
                        skill_folder_hash,
                    },
                ) {
                    let _ = ui::status::warning(&format!("Could not update lock file: {}", e));
                }
            }
        }
    }

    // Persist to skills.json when installing to project scope from a GitHub source
    if !no_save && do_project && is_git_source(&source) {
        if let Ok(parsed) = parse_git_source(&source) {
            let config_source = format!("{}/{}", parsed.user, parsed.repo);
            let config_skills: Vec<String> = skill_filters
                .unwrap_or_else(|| {
                    skill_filter
                        .map(|s| vec![s])
                        .unwrap_or_default()
                });
            if let Err(e) = config::add_skill_to_config(
                config_source,
                config_skills,
                AddSkillOptions {
                    cwd: None,
                    create_if_not_exists: true,
                },
                &ctx.working_dir,
            ) {
                let _ = ui::status::warning(&format!("Could not update skills.json: {}", e));
            }
        }
    }

    let _ = ui::layout::blank_line();
    let _ = ui::status::success(&format!(
        "Done. {} skill(s) installed to {}",
        to_install.len(),
        targets_display
    ));

    Ok(None)
}

/// Derive skill name from workdir: package.json name, Cargo.toml [package].name, or dir name.
/// Returns kebab-case normalized name.
fn derive_skill_name(workdir: &Path) -> String {
    if let Some(name) = try_package_json_name(workdir) {
        return to_kebab_case(&name);
    }
    if let Some(name) = try_cargo_toml_name(workdir) {
        return to_kebab_case(&name);
    }
    workdir
        .file_name()
        .map(|n| to_kebab_case(&n.to_string_lossy()))
        .unwrap_or_else(|| "repomix-reference".to_string())
}

fn try_package_json_name(workdir: &Path) -> Option<String> {
    let pkg = workdir.join("package.json");
    let content = starbase_fs::read_file(&pkg).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("name")?.as_str().map(String::from)
}

fn try_cargo_toml_name(workdir: &Path) -> Option<String> {
    let cargo = workdir.join("Cargo.toml");
    let content = starbase_fs::read_file(&cargo).ok()?;
    let toml: toml::Value = toml::from_str(&content).ok()?;
    toml.get("package")?
        .get("name")?
        .as_str()
        .map(String::from)
}

fn to_kebab_case(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c == '_' || c == ' ' {
            if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
        } else if c.is_ascii_uppercase() {
            if i > 0 && !out.ends_with('-') {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii_alphanumeric() || c == '-' {
            out.push(c.to_ascii_lowercase());
        }
    }
    out.trim_matches('-').to_string().replace("--", "-")
}

/// Project-local path for code-ref skill (docs/code-ref).
const CODE_REF_DIR: &str = "docs/code-ref";

/// Generate a reference skill from the project codebase via Repomix.
/// Writes to docs/code-ref in the project and symlinks into AI agent skill dirs.
async fn add_from_code(
    ctx: &SkillsContext,
    _global: bool,
    _project: bool,
    yes: bool,
    list_only: bool,
    _agent: &[String],
    workdir: Option<PathBuf>,
    _name: Option<String>,
) -> AppResult {
    let _ = ui::layout::blank_line();

    let workdir = workdir
        .unwrap_or_else(|| ctx.working_dir.clone())
        .canonicalize()
        .map_err(|e| miette::miette!("Invalid workdir: {}", e))?;

    let code_ref_path = workdir.join(CODE_REF_DIR);

    if list_only {
        let _ = ui::status::info(&format!(
            "Would generate code-ref to {} and symlink into agent dirs (Repomix)",
            code_ref_path.display()
        ));
        let _ = ui::layout::blank_line();
        return Ok(None);
    }

    if code_ref_path.exists() && !yes {
        let overwrite = ui::confirm_interactive(
            &format!("{} already exists. Overwrite?", code_ref_path.display()),
            false,
        )
        .map_err(|e| miette::miette!("Prompt failed: {}", e))?;
        if !overwrite {
            return Ok(None);
        }
    }

    starbase_fs::create_dir_all(code_ref_path.parent().unwrap_or(workdir.as_path()))
        .map_err(|e| miette::miette!("Failed to create docs dir: {}", e))?;
    if code_ref_path.exists() {
        starbase_fs::remove_dir_all(&code_ref_path)
            .map_err(|e| miette::miette!("Failed to remove existing {}: {}", code_ref_path.display(), e))?;
    }

    let _ = ui::status::info(&format!(
        "Generating code-ref from {} (Repomix)...",
        workdir.display()
    ));

    repomix_skill_generate(&workdir, &code_ref_path).await?;

    // Symlink docs/code-ref into project agent skill dirs (.cursor/skills, .claude/skills, etc.)
    let docs_dir = workdir.join("docs");
    let to_install = vec![(
        "code-ref".to_string(),
        code_ref_path.clone(),
    )];
    link_skills_into_agent_dirs(
        ctx.working_dir.as_path(),
        &docs_dir,
        &to_install,
    )?;

    let _ = ui::layout::blank_line();
    let _ = ui::status::success(&format!(
        "Done. Skill written to {} and symlinked into agent dirs",
        common::user_config::path_for_display(&code_ref_path)
    ));

    Ok(None)
}

/// Run Repomix --skill-generate and write directly to docs/code-ref.
/// Requires Node.js (mise). Uses sandbox at workdir.
async fn repomix_skill_generate(
    workdir: &Path,
    output_path: &Path,
) -> Result<(), miette::Report> {
    let config = SandboxConfig::new(workdir)
        .with_settings(SandboxSettings::default().with_tool("node", Some("22")));
    let sandbox = create_sandbox(config)
        .await
        .map_err(|e| miette::miette!("Failed to create sandbox: {}", e))?;

    // Restrict to source, config, and docs - same patterns as code-search pack
    let include_patterns = "**/*.ts,**/*.tsx,**/*.astro,**/*.js,**/*.jsx,**/*.rs,**/*.py,**/*.go,**/*.md,**/*.mdx,**/astro.config.*,**/tailwind.config.*,**/vite.config.*,**/*.config.*,**/Cargo.toml,**/package.json";
    let ignore_patterns =
        ".claude/**,.cursor/**,.codex/**,.aider/**,.continue/**,.github/copilot/**,**/node_modules/**";

    let cmd = format!(
        "npx repomix@latest --skill-generate code-ref --skill-output {} --force --include \"{}\" --ignore \"{}\" .",
        output_path.display(),
        include_patterns,
        ignore_patterns
    );

    let status = sandbox
        .exec_interactive(&cmd)
        .await
        .map_err(|e| miette::miette!("Repomix failed: {}", e))?;

    if !status.success() {
        return Err(miette::miette!(
            "Repomix exited with code {:?}",
            status.code()
        )
        .into());
    }

    if !output_path.join("SKILL.md").exists() {
        return Err(miette::miette!(
            "Repomix did not produce skill folder at {}",
            output_path.display()
        )
        .into());
    }

    Ok(())
}

/// Install a skill from a direct URL or well-known endpoint via providers.
async fn add_from_provider(
    ctx: &SkillsContext,
    url: &str,
    global: bool,
    project: bool,
    yes: bool,
    list_only: bool,
    agent: &[String],
) -> AppResult {
    let provider = providers::find_provider(url).ok_or_else(|| {
        miette::miette!("No provider found for URL. Use owner/repo or a supported direct URL (Mintlify, HuggingFace, well-known).")
    })?;

    let _fetch_spinner = if !ctx.verbose {
        Some(ui::progress::spinner(&format!("Fetching skill from {}...", provider.id())))
    } else {
        let _ = ui::status::info(&format!("Fetching skill from {}...", provider.id()));
        None
    };

    let remote = provider.fetch_skill(url).await.ok_or_else(|| {
        miette::miette!("Could not fetch skill from URL. Check that the URL is valid and the skill has required frontmatter (name, description).")
    })?;

    if list_only {
        let _ = ui::status::info(&format!("Skill: {} - {}", remote.name, remote.description));
        let _ = ui::layout::indented(&format!("Install as: {}", remote.install_name), 1);
        return Ok(None);
    }

    let do_project = project;
    let do_global = global || (!project && !global);
    let target_dirs = agents::target_dirs_for_add(
        agent,
        ctx.working_dir.as_path(),
        ctx.user_appz_dir.as_deref(),
        do_project,
        do_global,
    );
    if target_dirs.is_empty() {
        return Err(miette::miette!("No target directory. Use --project (-p) or --global (-g).").into());
    }

    for (target_dir, _) in &target_dirs {
        starbase_fs::create_dir_all(target_dir)
            .map_err(|e| miette::miette!("Failed to create skills directory: {}", e))?;
    }

    for (target_dir, label) in &target_dirs {
        let dest = target_dir.join(&remote.install_name);
        if dest.exists() && !yes {
            let overwrite = ui::confirm_interactive(
                &format!("Skill '{}' already exists in {}. Overwrite?", remote.install_name, label),
                false,
            )
            .map_err(|e| miette::miette!("Prompt failed: {}", e))?;
            if !overwrite {
                continue;
            }
        }

        if dest.exists() {
            let _ = starbase_fs::remove_dir_all(&dest);
        }
        starbase_fs::create_dir_all(&dest)
            .map_err(|e| miette::miette!("Failed to create skill directory: {}", e))?;
        starbase_fs::write_file(&dest.join("SKILL.md"), &remote.content)
            .map_err(|e| miette::miette!("Failed to write SKILL.md: {}", e))?;

        let _ = ui::status::success(&format!("Installed skill: {} ({})", remote.install_name, label));
    }

    let used_appz_global = agent.is_empty() && do_global;
    if used_appz_global {
        if let Some(ref appz_dir) = ctx.user_appz_dir {
            let appz_skills = appz_dir.join("skills");
            let dest = appz_skills.join(&remote.install_name);
            link_skills_into_agent_dirs(
                &ctx.working_dir,
                &appz_skills,
                &[(remote.install_name.clone(), dest)],
            )?;
        }
    }

    if used_appz_global {
        let source_id = format!("{}/{}", provider.id(), remote.install_name);
        let _ = skill_lock::add_skill_to_lock(
            ctx,
            remote.install_name.clone(),
            AddSkillLockInput {
                source: source_id,
                source_type: provider.id().to_string(),
                source_url: url.to_string(),
                skill_path: None,
                skill_folder_hash: None,
            },
        );
    }

    let targets_display = target_dirs
        .iter()
        .map(|(d, l)| format!("{} ({})", common::user_config::path_for_display(d), l))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = ui::layout::blank_line();
    let _ = ui::status::success(&format!(
        "Done. Skill '{}' installed to {}",
        remote.install_name,
        targets_display
    ));
    Ok(None)
}

fn build_source_url(source: &str, parsed: &init::sources::git::GitSource) -> String {
    if source.starts_with("http://") || source.starts_with("https://") {
        return source.to_string();
    }
    format!(
        "https://{}/{}/{}.git",
        parsed.platform, parsed.user, parsed.repo
    )
}

/// For each agent project dir present in cwd, create symlinks from installed skills (legacy global install).
fn link_skills_into_agent_dirs(
    cwd: &Path,
    installed_skills_dir: &Path,
    installed: &[(String, PathBuf)],
) -> Result<(), miette::Report> {
    for subdir in project_skills_subdirs() {
        let skills_dir = cwd.join(subdir);
        // Only symlink into dirs whose parent exists (e.g. .cursor exists for .cursor/skills)
        let parent = match skills_dir.parent() {
            Some(p) => p,
            None => continue,
        };
        if !parent.exists() {
            continue;
        }
        starbase_fs::create_dir_all(&skills_dir)
            .map_err(|e| miette::miette!("Failed to create {}: {}", skills_dir.display(), e))?;

        for (name, _) in installed {
            let dest = installed_skills_dir.join(name);
            let link_path = skills_dir.join(name);
            if !dest.exists() {
                continue;
            }
            if let Err(e) = create_skill_symlink(&dest, &link_path) {
                let _ = ui::status::warning(&format!(
                    "Could not link {} into {}: {}",
                    name,
                    subdir,
                    e
                ));
            } else {
                let _ = ui::status::success(&format!("Linked {} into {}", name, subdir));
            }
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn create_skill_symlink(target: &Path, link_path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::symlink;
    if link_path.exists() {
        let _ = starbase_fs::remove_file(link_path);
        let _ = starbase_fs::remove_dir_all(link_path);
    }
    let target_canon = target.canonicalize().unwrap_or_else(|_| target.to_path_buf());
    symlink(target_canon, link_path)
}

#[cfg(target_os = "windows")]
fn create_skill_symlink(target: &Path, link_path: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::symlink_dir;
    if link_path.exists() {
        let _ = starbase_fs::remove_file(link_path);
        let _ = starbase_fs::remove_dir_all(link_path);
    }
    let target_canon = target.canonicalize().unwrap_or_else(|_| target.to_path_buf());
    symlink_dir(target_canon, link_path)
}

/// If the git URL points to a single skill and that skill is already installed at target_dir, return its path so we skip download.
fn try_existing_skill_from_git_url(
    source: &str,
    target_dir: &Path,
    skill_filter: Option<&str>,
) -> Option<PathBuf> {
    let parsed = parse_git_source(source).ok()?;
    let subfolder = parsed.subfolder.as_deref()?;
    let name = subfolder.split('/').last()?.to_string();
    if name.is_empty() {
        return None;
    }
    if let Some(filter) = skill_filter {
        if !name.eq_ignore_ascii_case(filter) {
            return None;
        }
    }
    let existing = target_dir.join(&name);
    if existing.is_dir() && existing.join("SKILL.md").exists() {
        Some(existing)
    } else {
        None
    }
}

/// Global skills dirs to check when source is a bare name (no https://, no path).
fn global_skills_subdirs(home: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    out.push(home.join(".appz/skills"));
    for a in agents::all_agents() {
        let p = a.global_dir();
        if seen.insert(p.clone()) {
            out.push(p);
        }
    }
    out
}

/// If the skill exists in project-local AI agent dirs, return (path, human-readable location).
fn try_skill_from_project_dirs(cwd: &Path, name: &str) -> Option<(PathBuf, String)> {
    if name.is_empty() || name.contains('/') || name.starts_with('.') {
        return None;
    }
    for subdir in project_skills_subdirs() {
        let skill_dir = cwd.join(subdir).join(name);
        if skill_dir.join("SKILL.md").exists() {
            let location = format!("{}/{}", subdir, name);
            return skill_dir.canonicalize().ok().map(|p| (p, location));
        }
    }
    None
}

/// If the source is a bare skill name, look for it in global skills directories.
/// Returns the canonical path to the skill dir if found, None otherwise.
fn try_skill_from_global_dirs(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') || name.starts_with('.') {
        return None;
    }
    let home = dirs::home_dir()?;
    for skill_root in global_skills_subdirs(&home) {
        let skill_dir = skill_root.join(name);
        let skill_file = skill_dir.join("SKILL.md");
        if skill_file.exists() {
            return skill_dir.canonicalize().ok();
        }
    }
    None
}

/// Symlink a skill from its source path into project AI agent tool dirs.
/// Only symlinks into dirs that already exist; never creates .cursor, .claude, .agents.
/// Returns true if at least one link was created.
fn symlink_skill_into_project_agent_dirs(
    cwd: &Path,
    skill_name: &str,
    source_path: &Path,
    yes: bool,
) -> Result<bool, miette::Report> {
    let skills_dirs: Vec<_> = [".cursor/skills", ".claude/skills", ".agents/skills"]
        .into_iter()
        .map(|subdir| (subdir, cwd.join(subdir)))
        .filter(|(_, p)| p.is_dir())
        .collect();

    let mut linked = false;
    for (subdir_name, skills_dir) in skills_dirs {
        let link_path = skills_dir.join(skill_name);
        if link_path.exists() && !yes {
            let overwrite = ui::confirm_interactive(
                &format!("Skill '{}' already exists in {}. Overwrite link?", skill_name, subdir_name),
                false,
            )
            .map_err(|e| miette::miette!("Prompt failed: {}", e))?;
            if !overwrite {
                continue;
            }
        }
        if let Err(e) = create_skill_symlink(source_path, &link_path) {
            let _ = ui::status::warning(&format!(
                "Could not link {} into {}: {}",
                skill_name,
                subdir_name,
                e
            ));
        } else {
            let _ = ui::status::success(&format!("Linked {} into {}", skill_name, subdir_name));
            linked = true;
        }
    }
    Ok(linked)
}

fn is_git_source(s: &str) -> bool {
    if s.starts_with("https://") || s.starts_with("http://") {
        let lower = s.to_lowercase();
        return lower.contains("github.com")
            || lower.contains("gitlab.com")
            || lower.contains("bitbucket.org");
    }
    if s.contains('/') && !s.starts_with("./") && !s.starts_with("../") {
        let parts: Vec<&str> = s.split('/').collect();
        return parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty();
    }
    false
}

fn is_local_path(s: &str) -> bool {
    s.starts_with("./") || s.starts_with("../") || s.starts_with('/')
        || (s.len() >= 2 && s.chars().nth(1) == Some(':') && !s.contains("github.com") && !s.contains("gitlab.com"))
}

fn is_bare_skill_name(s: &str) -> bool {
    !s.is_empty()
        && !s.contains('/')
        && !s.starts_with('.')
        && !is_git_source(s)
        && !is_local_path(s)
}

/// Find directories containing SKILL.md (returns (skill_name, path)).
/// If root itself contains SKILL.md (e.g. when URL points at a single skill folder), it is included.
/// full_depth: if true, recurse into all subdirs; if false, only one level.
/// Also searches plugin manifest paths (.claude-plugin).
fn find_skill_dirs(root: &Path, full_depth: bool) -> Vec<(String, PathBuf)> {
    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let install_internal = should_install_internal_skills();

    fn search(
        r: &Path,
        full_depth: bool,
        install_internal: bool,
        results: &mut Vec<(String, PathBuf)>,
        seen: &mut std::collections::HashSet<PathBuf>,
    ) {
        if r.join("SKILL.md").exists() {
            if is_internal_skill(r) && !install_internal {
                return;
            }
            if let Some(name) = r.file_name() {
                if let Ok(canon) = r.canonicalize() {
                    if seen.insert(canon) {
                        results.push((name.to_string_lossy().to_string(), r.to_path_buf()));
                    }
                }
            }
        }
        let Ok(entries) = starbase_fs::read_dir(r) else {
            return;
        };
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                let skill_file = path.join("SKILL.md");
                if skill_file.exists() {
                    if is_internal_skill(&path) && !install_internal {
                        continue;
                    }
                    if let Ok(canon) = path.canonicalize() {
                        if seen.insert(canon) {
                            if let Some(name) = path.file_name() {
                                results.push((name.to_string_lossy().to_string(), path));
                            }
                        }
                    }
                } else if full_depth {
                    search(&path, true, install_internal, results, seen);
                }
            }
        }
    }

    search(root, full_depth, install_internal, &mut results, &mut seen);
    for plugin_dir in crate::plugin_manifest::get_plugin_skill_paths(root) {
        if plugin_dir.exists() {
            search(&plugin_dir, full_depth, install_internal, &mut results, &mut seen);
        }
    }
    results
}

const EXCLUDE_FILES: &[&str] = &["README.md", "metadata.json"];
const EXCLUDE_DIRS: &[&str] = &[".git"];

fn should_install_internal_skills() -> bool {
    std::env::var("INSTALL_INTERNAL_SKILLS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn is_internal_skill(skill_path: &Path) -> bool {
    let skill_file = skill_path.join("SKILL.md");
    let Ok(content) = starbase_fs::read_file(&skill_file) else {
        return false;
    };
    let content = content.trim_start();
    let rest = match content.strip_prefix("---") {
        Some(r) => r,
        None => return false,
    };
    let end = rest.find("\n---").or_else(|| rest.find("\r\n---")).unwrap_or(0);
    let yaml = rest[..end].trim();
    #[derive(serde::Deserialize)]
    struct Fm {
        internal: Option<bool>,
    }
    let Ok(fm): Result<Fm, _> = serde_yaml::from_str(yaml) else {
        return false;
    };
    fm.internal == Some(true)
}

fn should_exclude(name: &str, is_dir: bool) -> bool {
    if EXCLUDE_FILES.contains(&name) {
        return true;
    }
    if name.starts_with('_') {
        return true;
    }
    if is_dir && EXCLUDE_DIRS.contains(&name) {
        return true;
    }
    false
}

fn copy_skill_dir(src: &Path, dest: &Path) -> Result<(), miette::Report> {
    if dest.exists() {
        starbase_fs::remove_dir_all(dest)
            .map_err(|e| miette::miette!("Failed to remove existing skill: {}", e))?;
    }
    starbase_fs::create_dir_all(dest)
        .map_err(|e| miette::miette!("Failed to create directory: {}", e))?;

    let entries = starbase_fs::read_dir(src)
        .map_err(|e| miette::miette!("Failed to read source: {}", e))?;
    for entry in entries {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy();
        if should_exclude(&name, path.is_dir()) {
            continue;
        }
        let dest_path = dest.join(&*name);
        if path.is_dir() {
            copy_skill_dir(&path, &dest_path)?;
        } else {
            starbase_fs::copy_file(&path, &dest_path)
                .map_err(|e| miette::miette!("Failed to copy file: {}", e))?;
        }
    }
    Ok(())
}
