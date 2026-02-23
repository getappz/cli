//! Deploy prebuilt output to Appz.
//!
//! Flow: create deployment with file list -> if missing_files, upload each ->
//! continue deployment.

use api::models::{Deployment, DeploymentCreateRequest, DeploymentCreateResult};
use api::{Client, ClientExt};
use futures_util::stream::{self, StreamExt};
use miette::{miette, Result};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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
        created_at: i64,
    },
    Processing,
    Ready {
        url: String,
        inspect_url: Option<String>,
        is_production: bool,
        created_at: i64,
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
pub async fn deploy_prebuilt_stream<F>(client: Arc<Client>, ctx: DeployContext, mut on_event: F) -> Result<DeployOutput>
where
    F: FnMut(DeployEvent) + Send + 'static,
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
            let created_at = d.createdAt;
            on_event(DeployEvent::Created {
                deployment_id: d.id.clone(),
                url: url.clone(),
                inspect_url: None,
                is_production,
                created_at,
            });
            on_event(DeployEvent::Processing);
            on_event(DeployEvent::Ready {
                url,
                inspect_url: None,
                is_production,
                created_at,
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

            // Channel for events from parallel upload tasks (progress runs in upload context).
            let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
            let handle = tokio::spawn(async move {
                while let Some(ev) = rx.recv().await {
                    on_event(ev);
                }
            });

            let _ = tx
                .send(DeployEvent::FileCount {
                    total: files_by_sha.len(),
                    missing: missing.len(),
                    total_bytes,
                })
                .await;

            // Reduce concurrency for local wrangler to avoid "Broken pipe" (workerd disconnects under load)
            let concurrency = std::env::var("APPZ_DEPLOY_UPLOAD_CONCURRENCY")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|n| *n > 0)
                .unwrap_or(50);
            let client = client.clone();
            let uploaded = std::sync::Arc::new(AtomicU64::new(0));
            let uploaded_for_progress = std::sync::Arc::clone(&uploaded);
            let tx_progress = tx.clone();
            let progress = std::sync::Arc::new(move |delta: u64| {
                let prev = uploaded_for_progress.fetch_add(delta, Ordering::Relaxed);
                let new_val = prev + delta;
                let _ = tx_progress.try_send(DeployEvent::UploadProgress {
                    uploaded_bytes: new_val,
                    total_bytes,
                });
            });

            let upload_futures = missing.iter().filter_map(|sha| {
                let file_ref = files_by_sha.get(sha)?;
                let path_str = file_ref.path.to_string_lossy().replace('\\', "/");
                let size = file_ref.data.len() as u64;
                let client = client.clone();
                let deployment_id = deployment_id.clone();
                let sha = sha.clone();
                let data = file_ref.data.clone();
                let progress = std::sync::Arc::clone(&progress);
                let tx = tx.clone();
                Some(async move {
                    let res = client
                        .deployments()
                        .upload_file_with_progress(&deployment_id, &sha, data, progress)
                        .await
                        .map(|_| (path_str, size))
                        .map_err(|e| miette!("Upload file {} failed: {}", sha, e));
                    (res, tx)
                })
            });

            let results: Vec<_> = stream::iter(upload_futures)
                .buffer_unordered(concurrency)
                .collect()
                .await;

            let mut upload_error = None;
            for (result, _tx) in results {
                match result {
                    Ok((path_str, size)) => {
                        let _ = tx
                            .send(DeployEvent::FileUploaded {
                                path: path_str,
                                bytes: size,
                            })
                            .await;
                        let _ = tx
                            .send(DeployEvent::UploadProgress {
                                uploaded_bytes: uploaded.load(Ordering::Relaxed),
                                total_bytes,
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = tx.send(DeployEvent::Error(e.to_string())).await;
                        upload_error = Some(e);
                        break;
                    }
                }
            }

            if let Some(e) = upload_error {
                drop(tx);
                let _ = handle.await;
                return Err(e);
            }

            let _ = tx.send(DeployEvent::Processing).await;

            let continue_result = match client
                .deployments()
                .continue_deployment(&deployment_id, prepared)
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx
                        .send(DeployEvent::Error(format!(
                            "Continue deployment failed: {}",
                            e
                        )))
                        .await;
                    drop(tx);
                    let _ = handle.await;
                    return Err(miette!("Continue deployment failed: {}", e));
                }
            };

            let out = match continue_result {
                DeploymentCreateResult::Created(d) => {
                    let is_production = ctx.target == "production" || ctx.target == "prod";
                    let url = d
                        .url
                        .clone()
                        .unwrap_or_else(|| format!("https://{}", d.id));
                    let created_at = d.createdAt;
                    let _ = tx
                        .send(DeployEvent::Created {
                            deployment_id: d.id.clone(),
                            url: url.clone(),
                            inspect_url: None,
                            is_production,
                            created_at,
                        })
                        .await;
                    let _ = tx
                        .send(DeployEvent::Ready {
                            url,
                            inspect_url: None,
                            is_production,
                            created_at,
                        })
                        .await;
                    Ok(extract_output(&d)?)
                }
                DeploymentCreateResult::MissingFiles { missing: m, .. } => {
                    let _ = tx
                        .send(DeployEvent::Error(format!(
                            "Continue still requested {} files after upload.",
                            m.len()
                        )))
                        .await;
                    drop(tx);
                    let _ = handle.await;
                    return Err(miette!(
                        "Continue still requested {} files after upload. Retry or check API.",
                        m.len()
                    ));
                }
            };

            drop(tx);
            let _ = handle.await;
            out
        }
    }
}

/// Deploy prebuilt output to Appz.
///
/// Thin wrapper around `deploy_prebuilt_stream` with a no-op callback.
pub async fn deploy_prebuilt(client: Arc<Client>, ctx: DeployContext) -> Result<DeployOutput> {
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
