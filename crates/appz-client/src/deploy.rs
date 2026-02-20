//! Deploy prebuilt output to Appz.
//!
//! Flow: create deployment with file list -> if missing_files, upload each ->
//! continue deployment.

use api::models::{Deployment, DeploymentCreateRequest, DeploymentCreateResult};
use api::Client;
use miette::{miette, Result};
use std::path::Path;

use crate::file_tree::{build_file_tree, build_hashed_files};
use crate::upload::prepare_files;

/// Context for a prebuilt deployment.
#[derive(Debug, Clone)]
pub struct DeployContext {
    /// Output directory (e.g. .appz/output or dist)
    pub output_dir: std::path::PathBuf,
    /// Project ID (from .appz/project.json)
    pub project_id: String,
    /// Team ID (optional)
    pub team_id: Option<String>,
    /// Target: preview or production
    pub target: String,
    /// Project name (for deployment)
    pub name: Option<String>,
}

/// Result of a successful deployment.
#[derive(Debug, Clone)]
pub struct DeployOutput {
    pub deployment_id: String,
    pub url: String,
    pub status: String,
}

/// Deploy prebuilt output to Appz.
///
/// 1. Build file tree and hashes
/// 2. Create deployment with file list
/// 3. If missing_files, upload each file then call continue
/// 4. Return deployment URL when ready
pub async fn deploy_prebuilt(client: &Client, ctx: &DeployContext) -> Result<DeployOutput> {
    let output_dir = Path::new(&ctx.output_dir);
    let tree = build_file_tree(output_dir, &[])?;
    let files_by_sha = build_hashed_files(output_dir, &tree)?;

    if files_by_sha.is_empty() {
        return Err(miette!(
            "No files to deploy in {}",
            ctx.output_dir.display()
        ));
    }

    let prepared = prepare_files(output_dir, &files_by_sha);
    let payload = DeploymentCreateRequest {
        projectId: ctx.project_id.clone(),
        name: ctx.name.clone(),
        target: Some(ctx.target.clone()),
        meta: None,
        files: Some(prepared.clone()),
    };

    if let Some(team_id) = &ctx.team_id {
        client.set_team_id(Some(team_id.clone())).await;
    }

    let result = client
        .deployments()
        .create_deployment(payload)
        .await
        .map_err(|e| miette!("Create deployment failed: {}", e))?;

    match result {
        DeploymentCreateResult::Created(d) => return extract_output(&d),
        DeploymentCreateResult::MissingFiles {
            deployment_id,
            missing,
        } => {
            if deployment_id.is_empty() {
                return Err(miette!(
                    "Backend returned missing_files but no deploymentId. \
                     API should return deploymentId when files are required."
                ));
            }
            for sha in &missing {
                if let Some(file_ref) = files_by_sha.get(sha) {
                    client
                        .deployments()
                        .upload_file(&deployment_id, sha, file_ref.data.clone())
                        .await
                        .map_err(|e| miette!("Upload file {} failed: {}", sha, e))?;
                }
            }
            let continue_result = client
                .deployments()
                .continue_deployment(&deployment_id, prepared)
                .await
                .map_err(|e| miette!("Continue deployment failed: {}", e))?;

            match continue_result {
                DeploymentCreateResult::Created(d) => extract_output(&d),
                DeploymentCreateResult::MissingFiles { missing: m, .. } => Err(miette!(
                    "Continue still requested {} files after upload. Retry or check API.",
                    m.len()
                )),
            }
        }
    }
}

fn extract_output(d: &Deployment) -> Result<DeployOutput> {
    Ok(DeployOutput {
        deployment_id: d.id.clone(),
        url: d
            .url
            .clone()
            .unwrap_or_else(|| format!("https://{}", d.id)),
        status: d.status.clone().unwrap_or_else(|| "READY".to_string()),
    })
}
