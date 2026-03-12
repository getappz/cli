//! Run Repomix for a single mix (remote or local) via sandbox.

use std::path::Path;

use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};

use crate::MixConfig;

/// Result of running Repomix for one mix.
pub struct RepomixResult {
    pub success: bool,
    pub repo_name: String,
    pub output_path: std::path::PathBuf,
}

/// Run Repomix for one mix. Uses sandbox with Node 22.
pub async fn run_mix(
    workdir: &Path,
    mix: &MixConfig,
    output_dir: &Path,
) -> miette::Result<Option<RepomixResult>> {
    let config = SandboxConfig::new(workdir)
        .with_settings(SandboxSettings::default().with_tool("node", Some("22")));
    let sandbox = create_sandbox(config)
        .await
        .map_err(|e| miette::miette!("Failed to create sandbox: {}", e))?;

    let mut args: Vec<String> = vec!["npx repomix@latest".to_string()];

    if let Some(ref remote) = mix.remote {
        let url = if remote.starts_with("http") {
            remote.clone()
        } else {
            format!("https://github.com/{}", remote)
        };
        args.push("--remote".to_string());
        args.push(url);
    }

    let includes: Vec<String> = mix
        .include
        .clone()
        .unwrap_or_else(|| vec!["**/*".to_string()]);
    args.push("--include".to_string());
    args.push(includes.join(","));

    if let Some(ref ign) = mix.ignore {
        if !ign.is_empty() {
            args.push("-i".to_string());
            args.push(ign.join(","));
        }
    }

    if let Some(ref cfg) = mix.repomix_config {
        args.push("--config".to_string());
        args.push(cfg.clone());
    }

    args.push("--remove-empty-lines".to_string());
    args.push("--compress".to_string());
    args.push("--quiet".to_string());
    args.push("--parsable-style".to_string());

    if let Some(ref flags) = mix.extra_flags {
        args.extend(flags.clone());
    }

    let output_name: String = mix.output.clone().unwrap_or_else(|| {
        mix.remote
            .as_ref()
            .map(|r| format!("{}.xml", r.replace('/', "-")))
            .unwrap_or_else(|| "codebase.xml".to_string())
    });
    let output_file = output_dir.join(&output_name);
    std::fs::create_dir_all(output_dir)
        .map_err(|e| miette::miette!("Failed to create output dir: {}", e))?;
    args.push("-o".to_string());
    args.push(output_file.display().to_string());

    let cmd = args.join(" ");
    let status = sandbox
        .exec_interactive(&cmd)
        .await
        .map_err(|e| miette::miette!("{}", e))?;

    if !status.success() {
        return Ok(None);
    }

    let repo_name = mix
        .remote
        .as_ref()
        .and_then(|r| r.split('/').last().map(String::from))
        .unwrap_or_else(|| "local".to_string());

    Ok(Some(RepomixResult {
        success: true,
        repo_name,
        output_path: output_file,
    }))
}
