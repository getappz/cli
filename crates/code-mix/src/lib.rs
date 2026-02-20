//! Pack codebase for AI context using Repomix with pack-style pre-filters.
//!
//! Uses Repomix for packing; adds Rust pre-filters for content search,
//! git modes, bundles, and workspaces when needed. All operations run
//! through the sandbox crate (ripgrep, git, repomix).

mod cache;
mod prefilter;
mod repomix;
mod store;
pub mod templates;
mod types;
pub mod workspace;

pub use cache::{list_cached, remove_cached, CacheEntry};
pub use repomix::{run_repomix, RepomixError};
pub use types::PackOptions;

use std::path::Path;

use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};

/// Run pack: create sandbox, apply pre-filters, then invoke Repomix.
pub async fn pack(workdir: &Path, mut options: PackOptions) -> Result<(), RepomixError> {
    if let Some(ref name) = options.template {
        if let Some(content) = templates::get(name) {
            let path = workdir.join(".appz-pack-instruction.txt");
            tokio::fs::write(&path, content)
                .await
                .map_err(|e| RepomixError(format!("Failed to write template: {}", e)))?;
            options.instruction = Some(path);
        } else {
            return Err(RepomixError(format!(
                "Unknown template '{}'. Use --list-templates to see available templates.",
                name
            )));
        }
    }
    if let Some(ref name) = options.workspace {
        if let Some(ws) = workspace::resolve_workspace(workdir, name).await? {
            let include_pattern = format!("{}/**", ws.relative_path);
            if options.include.is_empty() {
                options.include = vec![include_pattern];
            } else {
                options.include.insert(0, include_pattern);
            }
        } else {
            return Err(RepomixError(format!(
                "Workspace '{}' not found. Run with --workspaces to list.",
                name
            )));
        }
    }

    let config = SandboxConfig::new(workdir)
        .with_settings(SandboxSettings::default().with_tool("node", Some("22")));
    let sandbox = create_sandbox(config)
        .await
        .map_err(|e| RepomixError(format!("Failed to create sandbox: {}", e)))?;

    let stdin_paths = prefilter::get_stdin_paths(sandbox.as_ref(), &options).await?;

    if let Some(ref paths) = stdin_paths {
        if let Some(cached) = cache::get_cached_output(workdir, &options, paths).await? {
            return cache::apply_cached_output(&options, &cached).await;
        }

        let input_key = cache::compute_input_key(workdir, &options, paths).await?;
        let temp_out = tempfile::NamedTempFile::new()
            .map_err(|e| RepomixError(format!("Failed to create temp file: {}", e)))?;
        let temp_path = temp_out.path().to_path_buf();

        run_repomix(
            sandbox.as_ref(),
            &options,
            Some(paths),
            Some(&temp_path),
        )
        .await?;

        let content = tokio::fs::read(&temp_path)
            .await
            .map_err(|e| RepomixError(format!("Failed to read repomix output: {}", e)))?;

        let meta = store::PackMetadata {
            workdir: Some(workdir.to_string_lossy().into_owned()),
            style: options.style.clone(),
            file_count: Some(paths.len() as i64),
            workspace: options.workspace.clone(),
        };
        cache::put_cached_output(&input_key, &content, &meta).await?;
        return cache::apply_cached_output(&options, &temp_path).await;
    }

    run_repomix(
        sandbox.as_ref(),
        &options,
        stdin_paths.as_deref(),
        None,
    )
    .await
}
