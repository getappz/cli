use miette::{IntoDiagnostic, Result};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use command::Command;
use task::Context;

fn has_mise() -> bool {
    command_exists("mise")
}

fn is_mise_tool(tool: &str) -> bool {
    matches!(
        tool,
        // Node ecosystem tools
        "node" | "npm" | "npx" | "pnpm" | "yarn" | "corepack" | "bun" | "bunx" |
        // Static site generators (binaries)
        "hugo" | "zola" | "mdbook"
    )
}

/// Check if a command is a multi-command script (contains && or ||)
fn is_multi_command(cmd: &str) -> bool {
    cmd.contains(" && ") || cmd.contains(" || ")
}

/// Extract all mise tools from a command string
/// Returns a vector of (tool_name, version_option) tuples
/// For multi-command scripts, extracts tools from all parts
fn extract_mise_tools_from_command(
    cmd: &str,
    pm_name: Option<&str>,
    pm_version: Option<&str>,
) -> Vec<(String, Option<String>)> {
    tracing::debug!("extract_mise_tools_from_command: cmd={:?}, pm_name={:?}, pm_version={:?}", cmd, pm_name, pm_version);
    let mut tools = Vec::new();
    let mut seen_tools = std::collections::HashSet::new();

    // Split by && and || to handle multi-command scripts
    let parts: Vec<&str> = cmd
        .split(" && ")
        .flat_map(|s| s.split(" || "))
        .map(|s| s.trim())
        .collect();
    tracing::debug!("extract_mise_tools_from_command: split into {} parts: {:?}", parts.len(), parts);

    for part in parts {
        // Parse the first token from each command part
        if let Ok(parts) = shell_words::split(part) {
            if let Some(first) = parts.first() {
                tracing::debug!("extract_mise_tools_from_command: checking first token: {:?}", first);
                if is_mise_tool(first) {
                    let tool_name = first.to_string();
                    tracing::debug!("extract_mise_tools_from_command: found mise tool: {}", tool_name);
                    
                    // Skip if we've already seen this tool
                    if seen_tools.contains(&tool_name) {
                        tracing::debug!("extract_mise_tools_from_command: skipping duplicate tool: {}", tool_name);
                        continue;
                    }
                    seen_tools.insert(tool_name.clone());

                    // Determine version for this tool
                    let version = if tool_name == pm_name.unwrap_or("") {
                        tracing::debug!("extract_mise_tools_from_command: tool {} matches pm_name, using version {:?}", tool_name, pm_version);
                        pm_version.map(|v| v.to_string())
                    } else {
                        tracing::debug!("extract_mise_tools_from_command: tool {} doesn't match pm_name, no version", tool_name);
                        None
                    };

                    tools.push((tool_name, version));
                } else {
                    tracing::debug!("extract_mise_tools_from_command: '{}' is not a mise tool", first);
                }
            }
        }
    }

    tracing::debug!("extract_mise_tools_from_command: extracted {} tools: {:?}", tools.len(), tools);
    tools
}

/// Wrap a command with mise, optionally specifying tool versions.
/// Now that we add mise shims to PATH, we don't need to wrap commands with `mise x`.
/// This function now just returns the command as-is, since mise binaries are available via PATH.
/// `pm_name` is the package manager name (e.g., "yarn", "npm") - `pm_version` is only used when the tool matches `pm_name`
fn wrap_with_mise_versioned(cmdline: &str, _pm_version: Option<&str>, _pm_name: Option<&str>) -> String {
    // Avoid double prefixing if command already starts with mise
    if cmdline.trim_start().starts_with("mise ") {
        return cmdline.to_string();
    }
    
    // Return command as-is - mise shims are now in PATH, so binaries are available automatically
    cmdline.to_string()
}

fn wrap_with_mise(cmdline: &str) -> String {
    wrap_with_mise_versioned(cmdline, None, None)
}

/// Find all node_modules/.bin directories by walking up from the starting directory.
/// Returns paths in order from closest to starting dir first, then parent directories.
/// This matches Vercel's getNodeBinPaths behavior.
/// Also checks for node_modules/bin (without dot) as a fallback for non-standard setups.
fn find_node_modules_bin_paths(starting_dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Canonicalize the starting directory to ensure we have an absolute path
    let canonical_start = if let Ok(canonical) = starting_dir.canonicalize() {
        canonical
    } else {
        // If canonicalization fails (e.g., path doesn't exist), use as-is
        starting_dir.to_path_buf()
    };

    let mut current_dir = Some(canonical_start.as_path());

    while let Some(dir) = current_dir {
        // Check for standard node_modules/.bin
        let node_modules_bin = dir.join("node_modules").join(".bin");
        if node_modules_bin.exists() && node_modules_bin.is_dir() {
            paths.push(node_modules_bin);
        }

        // Also check for node_modules/bin (without dot) as fallback for non-standard setups
        let node_modules_bin_alt = dir.join("node_modules").join("bin");
        if node_modules_bin_alt.exists() && node_modules_bin_alt.is_dir() {
            paths.push(node_modules_bin_alt);
        }

        current_dir = dir.parent();
    }

    paths
}

/// Check if yarn binary exists in node_modules/.bin
fn yarn_exists_in_node_modules(project_path: &Path) -> bool {
    let node_modules_bin_paths = find_node_modules_bin_paths(project_path);
    for bin_path in node_modules_bin_paths {
        #[cfg(target_os = "windows")]
        {
            let yarn_path = bin_path.join("yarn.cmd");
            if yarn_path.exists() {
                return true;
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let yarn_path = bin_path.join("yarn");
            if yarn_path.exists() {
                return true;
            }
        }
    }
    false
}

/// Check if a script contains yarn commands
fn script_uses_yarn(script: &str) -> bool {
    // Check for yarn commands - look for "yarn" as a standalone command
    // This matches patterns like "yarn generate-json", "yarn build", "yarn && something", etc.
    let script_lower = script.to_lowercase();
    
    // Split by common separators and check if any token is exactly "yarn"
    script_lower
        .split(&[' ', '\t', '&', '|', '\n', '\r'][..])
        .any(|token| token.trim() == "yarn")
}

/// Check if any scripts in package.json use yarn
fn scripts_use_yarn(package_manager: &Option<detectors::PackageManagerInfo>) -> bool {
    if let Some(ref pm) = package_manager {
        // Check all scripts for yarn usage
        if let Some(ref dev_script) = pm.dev_script {
            if script_uses_yarn(dev_script) {
                return true;
            }
        }
        if let Some(ref install_script) = pm.install_script {
            if script_uses_yarn(install_script) {
                return true;
            }
        }
        if let Some(ref build_script) = pm.build_script {
            if script_uses_yarn(build_script) {
                return true;
            }
        }
    }
    false
}

/// Check package.json directly for yarn usage in scripts (fallback when package_manager is None)
fn check_package_json_for_yarn(project_path: &Path) -> bool {
    let package_json_path = project_path.join("package.json");
    if !package_json_path.exists() {
        return false;
    }

    if let Ok(content) = std::fs::read_to_string(&package_json_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) {
                for (_, value) in scripts {
                    if let Some(script_str) = value.as_str() {
                        if script_uses_yarn(script_str) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Install yarn using npm if it's the detected package manager or used in scripts, and not already installed
pub async fn ensure_yarn_installed(
    ctx: &Context,
    package_manager: &Option<detectors::PackageManagerInfo>,
    project_path: &Path,
) -> Result<()> {
    // Check if yarn is the package manager OR if any scripts use yarn
    let needs_yarn = if let Some(ref pm) = package_manager {
        pm.manager == "yarn" || scripts_use_yarn(package_manager)
    } else {
        // If no package manager detected, check package.json directly
        check_package_json_for_yarn(project_path)
    };

    if needs_yarn {
        // Check if yarn already exists in node_modules/.bin
        if !yarn_exists_in_node_modules(project_path) {
            use tracing::info;
            info!("Installing yarn...");
            
            // Build install command with version if available
            let install_cmd = if let Some(ref pm) = package_manager {
                if let Some(ref version) = pm.version {
                    format!("npm install yarn@{}", version)
                } else {
                    "npm install yarn".to_string()
                }
            } else {
                "npm install yarn".to_string()
            };
            
            let opts = RunOptions {
                cwd: Some(project_path.to_path_buf()),
                env: None,
                show_output: true,
                package_manager: None, // Use npm, not yarn
                tool_info: None,
            };
            
            run_local_with(ctx, &install_cmd, opts).await?;
            info!("✓ yarn installed");
        }
    }
    Ok(())
}

pub fn run_local(cmd: &str) -> Result<()> {
    // Let Command crate handle shell wrapping automatically
    // It will detect the shell and wrap the command appropriately
    // Use interactive execution to preserve stdin/stdout/stderr for prompts
    let wrapped = wrap_with_mise(cmd);
    let mut command = Command::new(&wrapped);

    // Add mise shims and node_modules/.bin to PATH (session-only, non-persistent)
    if let Ok(current_dir) = std::env::current_dir() {
        let node_modules_bin_paths = find_node_modules_bin_paths(&current_dir);
        
        // Add mise shims path first
        if let Some(shims_path) = get_mise_shims_path() {
            if let Ok(shims_pathbuf) = PathBuf::from(&shims_path).canonicalize() {
                command.prepend_paths([shims_pathbuf]);
            }
        }
        
        // Then add node_modules/.bin paths
        if !node_modules_bin_paths.is_empty() {
            command.prepend_paths(node_modules_bin_paths);
        }
    }

    let status = command
        .exec_interactive()
        .map_err(|e| miette::miette!("Command execution failed: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err(miette::miette!(
            "Command failed with exit code: {:?}",
            status.code()
        ))
    }
}

/// Tool version info for mise-managed tools (non-Node tools like Hugo)
#[derive(Default, Clone, Debug)]
pub struct ToolVersionInfo {
    /// Tool name (e.g., "hugo")
    pub tool: String,
    /// Version requirement (e.g., "0.83.0" or "latest")
    pub version: Option<String>,
    /// For Hugo: whether extended version is required (for SCSS/SASS support)
    pub extended: bool,
}

#[derive(Default, Clone)]
pub struct RunOptions {
    pub cwd: Option<PathBuf>,
    pub env: Option<HashMap<String, String>>, // additional env
    pub show_output: bool,
    pub package_manager: Option<detectors::PackageManagerInfo>,
    /// Tool version info for non-Node mise-managed tools (e.g., Hugo)
    pub tool_info: Option<ToolVersionInfo>,
}

/// Build PATH string from path parts with platform-specific separator
fn build_path_string(path_parts: &[String]) -> String {
    if path_parts.is_empty() {
        String::new()
    } else {
        #[cfg(target_os = "windows")]
        {
            path_parts.join(";")
        }
        #[cfg(not(target_os = "windows"))]
        {
            path_parts.join(":")
        }
    }
}

/// Convert node_modules/.bin paths to string vector
fn node_modules_paths_to_strings(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|p| p.to_str().map(|s| s.to_string()))
        .collect()
}

/// Get the mise shims directory path
fn get_mise_shims_path() -> Option<String> {
    if let Ok(home) = std::env::var("HOME") {
        let shims_path = format!("{}/.local/share/mise/shims", home);
        // Check if the path exists
        if Path::new(&shims_path).exists() {
            return Some(shims_path);
        }
    }
    None
}

/// Build PATH with priority: mise shims > mise PATH > node_modules/.bin > existing PATH
fn build_merged_path(
    mise_path: Option<&str>,
    node_modules_bin_paths: &[PathBuf],
    existing_path: &str,
) -> String {
    let mut path_parts = Vec::new();

    // Add mise shims path first (highest priority)
    if let Some(shims) = get_mise_shims_path() {
        path_parts.push(shims);
    }

    // Add mise PATH second (if available from mise env)
    if let Some(mise) = mise_path {
        path_parts.push(mise.to_string());
    }

    // Add node_modules/.bin paths
    path_parts.extend(node_modules_paths_to_strings(node_modules_bin_paths));

    // Add existing PATH last
    if !existing_path.is_empty() {
        path_parts.push(existing_path.to_string());
    }

    build_path_string(&path_parts)
}

fn merged_env(base: &Context, extra: &Option<HashMap<String, String>>) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();

    // Find node_modules/.bin paths from working directory
    let node_modules_bin_paths = if let Some(working_path) = base.working_path() {
        find_node_modules_bin_paths(working_path)
    } else if let Ok(current_dir) = std::env::current_dir() {
        find_node_modules_bin_paths(&current_dir)
    } else {
        Vec::new()
    };

    let current_path = env.get("PATH").cloned().unwrap_or_default();

    // Load mise environment variables from Context
    // mise env JSON is stored in _mise_env_json
    if let Some(mise_env_json) = base.get("_mise_env_json") {
        if let Ok(env_json) = serde_json::from_str::<serde_json::Value>(&mise_env_json) {
            if let serde_json::Value::Object(env_map) = env_json {
                // Extract mise PATH if present
                let mise_path = env_map.get("PATH").and_then(|v| v.as_str());

                // Build merged PATH: mise PATH > node_modules/.bin > existing PATH
                let new_path = build_merged_path(mise_path, &node_modules_bin_paths, &current_path);
                env.insert("PATH".to_string(), new_path);

                // Add all other mise environment variables
                for (key, value) in env_map {
                    if key != "PATH" {
                        if let serde_json::Value::String(val_str) = value {
                            env.insert(key, val_str);
                        }
                    }
                }
            }
        }
    } else {
        // No mise env JSON, but still add mise shims and node_modules/.bin paths to PATH
        let new_path = build_merged_path(None, &node_modules_bin_paths, &current_path);
        env.insert("PATH".to_string(), new_path);
    }

    // load dotenv into base env map if configured
    let mut base_env = base.env().clone();
    if base.dotenv().is_some() {
        // best-effort: a copy of context with dotenv loaded
        let mut tmp = Context::new();
        tmp.set_dotenv(base.dotenv().unwrap().to_string());
        tmp.load_dotenv_into_env();
        base_env.extend(tmp.env().clone());
    }
    for (k, v) in base_env {
        env.insert(k, v);
    }
    if let Some(more) = extra {
        for (k, v) in more {
            env.insert(k.clone(), v.clone());
        }
    }
    env
}

pub async fn run_local_with(ctx: &Context, cmd: &str, opts: RunOptions) -> Result<()> {
    let cwd = opts.cwd.or_else(|| ctx.working_path().cloned());
    let env = merged_env(ctx, &opts.env);

    // Determine the working directory for detection
    let current_dir = std::env::current_dir().ok();
    let search_dir = if let Some(ref cwd_path) = cwd {
        Some(cwd_path.as_path())
    } else if let Some(ctx_path) = ctx.working_path() {
        Some(ctx_path.as_path())
    } else {
        current_dir.as_deref()
    };

    // Check if command already starts with a package manager
    let cmd_parts: Vec<&str> = cmd.split_whitespace().collect();
    let has_pm_prefix = cmd_parts
        .first()
        .map(|first| {
            matches!(
                *first,
                "npm" | "pnpm" | "yarn" | "bun" | "npx" | "pnpx" | "yarnpkg" | "bunx"
            )
        })
        .unwrap_or(false);

    // Check if command is a shell script
    // On Windows, always return an error for shell scripts so caller can fallback to framework command
    let is_sh_script = is_shell_script(cmd);

    #[cfg(target_os = "windows")]
    {
        if is_sh_script {
            // Shell script detected on Windows - return error immediately for fallback to framework command
            // This prevents the command from being wrapped with package managers (like bun x) which would
            // try to resolve it as an npm package
            return Err(miette::miette!(
                "Shell script detected. Cannot execute shell scripts on Windows."
            ));
        }
    }

    // Use provided package manager info if available, otherwise detect it
    let pm_info = if let Some(pm) = opts.package_manager {
        Some(pm)
    } else {
        // Create filesystem detector for package manager detection
        use detectors::{detect_package_manager, StdFilesystem};
        use std::path::PathBuf;
        use std::sync::Arc;

        let fs: Arc<dyn detectors::DetectorFilesystem> =
            if let Some(dir) = search_dir {
                Arc::new(StdFilesystem::new(Some(dir.to_path_buf())))
            } else if let Ok(current_dir) = std::env::current_dir() {
                Arc::new(StdFilesystem::new(Some(current_dir)))
            } else {
                Arc::new(StdFilesystem::new(None::<PathBuf>))
            };

        detect_package_manager(&fs).await.ok().flatten()
    };

    // Build the command normally
    // For shell scripts, don't wrap with package managers - just run as-is
    // Extract version and name from package manager info for mise versioned execution
    let pm_version = pm_info.as_ref().and_then(|pm| pm.version.as_deref());
    let pm_name = pm_info.as_ref().map(|pm| pm.manager.as_str());

    // Check if we have tool info for non-Node tools (e.g., Hugo)
    let _tool_mise_version = opts.tool_info.as_ref().map(|info| {
        let version = info.version.as_deref().unwrap_or("latest");
        if info.extended {
            // Hugo extended version format: extended_0.83.0 or extended_latest
            format!("extended_{}", version)
        } else {
            version.to_string()
        }
    });

    // Check if command starts with a binary tool that mise can install via GitHub
    let cmd_parts: Vec<&str> = cmd.split_whitespace().collect();
    let first_token = cmd_parts.first().copied().unwrap_or("");
    let is_binary_tool = matches!(first_token, "zola" | "mdbook");

    // Check if command starts with jekyll (not bundle exec jekyll) and if Gemfile exists
    let needs_bundle_exec = if first_token == "jekyll" && !cmd.trim_start().starts_with("bundle exec") {
        // Check if Gemfile exists in the project directory
        if let Some(dir) = search_dir {
            dir.join("Gemfile").exists()
        } else {
            false
        }
    } else {
        false
    };

    let final_cmd = if is_sh_script {
        // Shell script - don't wrap with package managers, just use the command as-is
        cmd.to_string()
    } else if needs_bundle_exec {
        // Jekyll command with Gemfile - wrap with bundle exec
        format!("bundle exec {}", cmd)
    } else if opts.tool_info.is_some() {
        // Non-Node tool with version info (e.g., Hugo)
        // Since mise shims are in PATH, just use the command as-is
        cmd.to_string()
    } else if is_binary_tool && has_mise() {
        // Binary tools that mise can install via GitHub backend
        // Since mise shims are in PATH, just use the command as-is
        cmd.to_string()
    } else if has_pm_prefix {
        // User already specified a package manager, use command as-is (with mise wrapper if needed)
        // Pass version and name so mise can auto-install the correct version (e.g., yarn@3.6.3)
        wrap_with_mise_versioned(cmd, pm_version, pm_name)
    } else if let Some(pm_info) = &pm_info {
        // Use detected package manager to run the command
        match pm_info.manager.as_str() {
            "bun" if command_exists("bun") => {
                // If project uses Bun, use `bun x` to run the command
                // `bun x` runs binaries/scripts and handles Bun's binary remapping correctly
                format!("bun x {}", cmd)
            }
            "npm" | "pnpm" | "yarn" => {
                // For npm/pnpm/yarn, run command directly (binaries in node_modules/.bin)
                // PATH will be set up with node_modules/.bin, so binaries can be found
                // Use mise with version to ensure the right tool versions are available
                wrap_with_mise_versioned(cmd, pm_version, pm_name)
            }
            _ => {
                // Unknown package manager, use mise as fallback
                wrap_with_mise_versioned(cmd, pm_version, pm_name)
            }
        }
    } else {
        // No package manager detected, use mise if command is a node tool
        wrap_with_mise(cmd)
    };

    // Clone final_cmd early to avoid borrow issues
    let final_cmd_clone = final_cmd.clone();
    tracing::debug!("run_local_with: original cmd={:?}", cmd);
    tracing::debug!("run_local_with: final_cmd={:?}", final_cmd);

    // Add node_modules/.bin to PATH (session-only, non-persistent)
    // Only add if not using Bun (Bun handles its own binary resolution)
    // Use detected package manager info instead of separate detection
    let is_bun = pm_info
        .as_ref()
        .map(|pm| pm.manager == "bun")
        .unwrap_or(false);
    tracing::debug!("run_local_with: is_bun={}", is_bun);

    // For WSL scripts, we need to modify the command to export PATH
    // Collect node_modules paths first to determine if we need to wrap the command
    // For WSL scripts, we always need to add node_modules/.bin to PATH even for Bun
    // because Bun's binary resolution doesn't work inside WSL
    let node_modules_bin_paths = if let Some(dir) = search_dir {
        find_node_modules_bin_paths(dir)
    } else if let Ok(current_dir) = std::env::current_dir() {
        find_node_modules_bin_paths(&current_dir)
    } else {
        Vec::new()
    };
    tracing::debug!("run_local_with: node_modules_bin_paths={:?}", node_modules_bin_paths);

    // Vercel's approach: Set PATH as an environment variable, don't modify the command string
    // This matches Vercel's runPackageJsonScript behavior where they prepend node_modules/.bin
    // to PATH as an env var, which gets merged with the command's environment (including mise's PATH)
    tracing::debug!("run_local_with: final_cmd={:?}", final_cmd_clone);

    // Let Command crate handle shell wrapping automatically
    // It will detect the shell and wrap the command appropriately
    let mut command = Command::new(&final_cmd_clone);
    
    // Debug: Show current PATH
    if let Ok(current_path) = std::env::var("PATH") {
        tracing::debug!("run_local_with: current PATH={}", current_path);
    }

    // Add mise shims path first (highest priority)
    if let Some(shims_path) = get_mise_shims_path() {
        if let Ok(shims_pathbuf) = PathBuf::from(&shims_path).canonicalize() {
            tracing::debug!("run_local_with: prepending mise shims to PATH: {:?}", shims_pathbuf);
            command.prepend_paths([shims_pathbuf]);
        }
    }

    // Vercel's approach: Always prepend node_modules/.bin to PATH as an environment variable
    // This works because mise shims are now in PATH, so binaries are available automatically
    // This matches Vercel's runPackageJsonScript which sets PATH as an env var
    if !is_bun && !node_modules_bin_paths.is_empty() {
        tracing::debug!("run_local_with: prepending node_modules/.bin to PATH");
        command.prepend_paths(node_modules_bin_paths);
    } else {
        tracing::debug!("run_local_with: skipping prepend_paths (is_bun={}, has_paths={})", is_bun, !node_modules_bin_paths.is_empty());
    }

    // Set working directory
    if let Some(dir) = cwd {
        command.cwd(dir.as_os_str());
    }

    // Set environment variables
    for (k, v) in env {
        command.env(k, v);
    }

    // Print the clean command (without internal wrappers) if show_output is true
    if opts.show_output {
        println!("Running {}", cmd);
        // Don't print the wrapped command - we already printed the clean version
        command.set_print_command(false);
    }

    let status = command
        .exec_interactive()
        .map_err(|e| miette::miette!("Command execution failed: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err(miette::miette!(
            "Command failed with exit code: {:?}",
            status.code()
        ))
    }
}

pub fn test_local(cmd: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        // On Windows, test by running the command and checking exit code
        let mut command = Command::new(cmd);
        command.set_error_on_nonzero(false);
        command.exec().map(|o| o.status.success()).unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        // On Unix, wrap in a conditional test
        let test_cmd = format!("if {cmd}; then echo +true; fi");
        let mut command = Command::new(&test_cmd);
        command.set_error_on_nonzero(false);
        command
            .exec()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "+true")
            .unwrap_or(false)
    }
}

pub fn which(name: &str) -> Result<String> {
    Ok(which::which(name)
        .into_diagnostic()?
        .to_string_lossy()
        .to_string())
}

pub fn command_exists(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

/// Check if a command is a shell script (.sh file)
pub fn is_shell_script(cmd: &str) -> bool {
    // Check if command ends with .sh or starts with ./ and contains .sh
    cmd.trim().ends_with(".sh") || (cmd.trim().starts_with("./") && cmd.contains(".sh"))
}

/// Check if WSL (Windows Subsystem for Linux) is available
pub fn has_wsl() -> bool {
    #[cfg(target_os = "windows")]
    {
        command_exists("wsl")
    }
    #[cfg(not(target_os = "windows"))]
    {
        false // WSL is only on Windows
    }
}

/// Convert Windows path to WSL path format using wslpath
/// This uses the official WSL utility for reliable path conversion
#[cfg(target_os = "windows")]
fn windows_path_to_wsl(path: &Path) -> String {
    // First, resolve to absolute path if it's relative
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        // Make it absolute relative to current directory
        if let Ok(current_dir) = std::env::current_dir() {
            current_dir.join(path)
        } else {
            path.to_path_buf()
        }
    };

    // Convert to string and normalize
    let mut path_str = absolute_path.to_string_lossy().to_string();
    // Remove \\?\ prefix if present (extended-length path format)
    // Handle both \\?\ and //?/ formats
    if path_str.starts_with("\\\\?\\") {
        path_str = path_str[4..].to_string();
    } else if path_str.starts_with("//?/") {
        path_str = path_str[4..].to_string();
    }
    // Replace backslashes with forward slashes for wslpath
    let path_str = path_str.replace('\\', "/");

    // Use wsl wslpath -u to convert the path
    if let Ok(output) = std::process::Command::new("wsl")
        .arg("wslpath")
        .arg("-u")
        .arg(&path_str)
        .output()
    {
        if output.status.success() {
            let wsl_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !wsl_path.is_empty() && !wsl_path.contains("wslpath:") {
                return wsl_path;
            }
        }
    }

    // Fallback to manual conversion if wslpath fails
    if let Some(colon_pos) = path_str.find(':') {
        if colon_pos == 1 && path_str.len() > 2 {
            let drive_letter = path_str.chars().next().unwrap().to_lowercase();
            format!("/mnt/{}{}", drive_letter, &path_str[2..])
        } else {
            path_str
        }
    } else {
        path_str
    }
}

pub fn timestamp_utc_iso8601() -> String {
    use chrono::{DateTime, Utc};
    let now: DateTime<Utc> = SystemTime::now().into();
    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// Local transfer utilities (Phase 3 minimal parity)
pub fn copy_path_recursive(src: &Path, dst: &Path) -> Result<()> {
    if src.is_file() {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).into_diagnostic()?;
        }
        fs::copy(src, dst).into_diagnostic()?;
        return Ok(());
    }
    if src.is_dir() {
        fs::create_dir_all(dst).into_diagnostic()?;
        for entry in fs::read_dir(src).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let from = entry.path();
            let to = dst.join(entry.file_name());
            if entry.file_type().into_diagnostic()?.is_dir() {
                copy_path_recursive(&from, &to)?;
            } else {
                if let Some(parent) = to.parent() {
                    fs::create_dir_all(parent).into_diagnostic()?;
                }
                fs::copy(&from, &to).into_diagnostic()?;
            }
        }
    }
    Ok(())
}

// Execute a command by splitting arguments with shell-words (no shell wrapping)
pub fn run_local_words(cmdline: &str) -> Result<()> {
    let parts = shell_words::split(cmdline)
        .map_err(|e| miette::miette!("Failed to parse command: {}", e))?;
    if parts.is_empty() {
        return Ok(());
    }
    let prog = &parts[0];
    let args = &parts[1..];

    let mut command = Command::new(prog);
    command.args(args);
    command.without_shell(); // Direct execution, no shell wrapping
    command
        .run()
        .map_err(|e| miette::miette!("Command execution failed: {}", e))
}
