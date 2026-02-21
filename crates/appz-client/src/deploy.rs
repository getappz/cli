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

/// Events emitted during deployment (Vercel-aligned).
#[derive(Debug, Clone)]
pub enum DeployEvent {
    Preparing,
    FileCount {
        total: usize,
        missing: usize,
        total_bytes: u64,
    },
    FileUploaded {
        path: String,
        bytes: u64,
    },
    UploadProgress {
        uploaded_bytes: u64,
        total_bytes: u64,
    },
    Created {
        deployment_id: String,
        url: String,
        inspect_url: Option<String>,
        is_production: bool,
    },
    Processing,
    Ready {
        url: String,
        inspect_url: Option<String>,
        is_production: bool,
    },
    Error(String),
}

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
    /// Arbitrary metadata (KEY=VALUE from -m flag)
    pub meta: Option<serde_json::Map<String, serde_json::Value>>,
    /// Skip automatic domain promotion (from --skip-domain)
    pub skip_domain: bool,
    /// Force new deployment (from -f)
    pub force: bool,
}

/// Result of a successful deployment.
#[derive(Debug, Clone)]
pub struct DeployOutput {
    pub deployment_id: String,
    pub url: String,
    pub status: String,
}

/// Deploy prebuilt output to Appz with event stream.
///
/// Invokes `on_event` for each deployment phase. See `DeployEvent` for variants.
pub async fn deploy_prebuilt_stream<F>(client: &Client, ctx: &DeployContext, mut on_event: F) -> Result<DeployOutput>
where
    F: FnMut(DeployEvent),
{
    let output_dir = Path::new(&ctx.output_dir);
    let tree = build_file_tree(output_dir, &[])?;
    let files_by_sha = build_hashed_files(output_dir, &tree)?;

    if files_by_sha.is_empty() {
        return Err(miette!(
            "No files to deploy in {}",
            ctx.output_dir.display()
        ));
    }

    on_event(DeployEvent::Preparing);

    let prepared = prepare_files(output_dir, &files_by_sha);
    let payload = DeploymentCreateRequest {
        projectId: ctx.project_id.clone(),
        name: ctx.name.clone(),
        target: Some(ctx.target.clone()),
        meta: ctx.meta.clone(),
        skipDomain: Some(ctx.skip_domain),
        force: Some(ctx.force),
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
        DeploymentCreateResult::Created(d) => {
            let is_production = ctx.target == "production" || ctx.target == "prod";
            let url = d
                .url
                .clone()
                .unwrap_or_else(|| format!("https://{}", d.id));
            on_event(DeployEvent::Created {
                deployment_id: d.id.clone(),
                url: url.clone(),
                inspect_url: None,
                is_production,
            });
            on_event(DeployEvent::Processing);
            on_event(DeployEvent::Ready {
                url,
                inspect_url: None,
                is_production,
            });
            extract_output(&d)
        }
        DeploymentCreateResult::MissingFiles {
            deployment_id,
            missing,
        } => {
            if deployment_id.is_empty() {
                on_event(DeployEvent::Error("Backend returned missing_files but no deploymentId.".to_string()));
                return Err(miette!(
                    "Backend returned missing_files but no deploymentId. \
                     API should return deploymentId when files are required."
                ));
            }

            let total_bytes: u64 = missing
                .iter()
                .filter_map(|sha| files_by_sha.get(sha))
                .map(|fr| fr.data.len() as u64)
                .sum();
            on_event(DeployEvent::FileCount {
                total: files_by_sha.len(),
                missing: missing.len(),
                total_bytes,
            });

            let mut uploaded_bytes: u64 = 0;
            for sha in &missing {
                if let Some(file_ref) = files_by_sha.get(sha) {
                    let path_str = file_ref.path.to_string_lossy().replace('\\', "/");
                    let size = file_ref.data.len() as u64;
                    client
                        .deployments()
                        .upload_file(&deployment_id, sha, file_ref.data.clone())
                        .await
                        .map_err(|e| {
                            on_event(DeployEvent::Error(format!("Upload file {} failed: {}", sha, e)));
                            miette!("Upload file {} failed: {}", sha, e)
                        })?;
                    uploaded_bytes += size;
                    on_event(DeployEvent::FileUploaded {
                        path: path_str,
                        bytes: size,
                    });
                    on_event(DeployEvent::UploadProgress {
                        uploaded_bytes,
                        total_bytes,
                    });
                }
            }

            on_event(DeployEvent::Processing);
            let continue_result = client
                .deployments()
                .continue_deployment(&deployment_id, prepared)
                .await
                .map_err(|e| {
                    on_event(DeployEvent::Error(format!("Continue deployment failed: {}", e)));
                    miette!("Continue deployment failed: {}", e)
                })?;

            match continue_result {
                DeploymentCreateResult::Created(d) => {
                    let is_production = ctx.target == "production" || ctx.target == "prod";
                    let url = d
                        .url
                        .clone()
                        .unwrap_or_else(|| format!("https://{}", d.id));
                    on_event(DeployEvent::Created {
                        deployment_id: d.id.clone(),
                        url: url.clone(),
                        inspect_url: None,
                        is_production,
                    });
                    on_event(DeployEvent::Ready {
                        url,
                        inspect_url: None,
                        is_production,
                    });
                    extract_output(&d)
                }
                DeploymentCreateResult::MissingFiles { missing: m, .. } => {
                    on_event(DeployEvent::Error(format!(
                        "Continue still requested {} files after upload.",
                        m.len()
                    )));
                    Err(miette!(
                        "Continue still requested {} files after upload. Retry or check API.",
                        m.len()
                    ))
                }
            }
        }
    }
}

/// Deploy prebuilt output to Appz.
///
/// Thin wrapper around `deploy_prebuilt_stream` with a no-op callback.
pub async fn deploy_prebuilt(client: &Client, ctx: &DeployContext) -> Result<DeployOutput> {
    deploy_prebuilt_stream(client, ctx, |_| {}).await
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
