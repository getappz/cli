//! Add a GitHub repo to pack config.

use std::path::Path;

use miette::IntoDiagnostic;
use reqwest::blocking::Client;

use crate::load_config;
use crate::MixConfig;

/// Validate GitHub repo and return owner/repo shorthand.
pub fn validate_github_repo(repo_input: &str) -> miette::Result<String> {
    let (shorthand, url) = if repo_input.starts_with("https://github.com/") {
        let m = regex::Regex::new(r"github\.com/([^/]+/[^/]+)")
            .unwrap()
            .captures(repo_input)
            .ok_or_else(|| miette::miette!("Invalid GitHub URL"))?;
        (m[1].to_string(), repo_input.to_string())
    } else if repo_input.contains('/') {
        (
            repo_input.to_string(),
            format!("https://github.com/{}", repo_input),
        )
    } else {
        return Err(miette::miette!("Use owner/repo or full GitHub URL"));
    };

    let client = Client::new();
    let resp = client
        .head(&url)
        .send()
        .map_err(|e| miette::miette!("{}", e))?;
    if !resp.status().is_success() {
        return Err(miette::miette!(
            "Repo {} not found or not accessible",
            shorthand
        ));
    }

    Ok(shorthand)
}

/// Append mix to config and save.
pub fn add_repo(cwd: &Path, config_path: Option<&Path>, repo: &str) -> miette::Result<()> {
    let shorthand = validate_github_repo(repo)?;

    let (path, mut config) = load_config(config_path, cwd)?;
    if config
        .mixes
        .iter()
        .any(|m| m.remote.as_deref() == Some(&shorthand))
    {
        return Err(miette::miette!("Repository {} already in config", shorthand));
    }

    let new_mix = MixConfig {
        remote: Some(shorthand.clone()),
        include: Some(vec![
            "*.ts".to_string(),
            "*.js".to_string(),
            "*.md".to_string(),
        ]),
        ignore: None,
        output: Some(format!("{}.xml", shorthand.replace('/', "-"))),
        repomix_config: None,
        extra_flags: None,
    };
    config.mixes.push(new_mix);

    let json = serde_json::to_string_pretty(&config).into_diagnostic()?;
    std::fs::write(&path, json).into_diagnostic()?;

    Ok(())
}
