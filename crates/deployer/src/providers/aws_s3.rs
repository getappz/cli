//! AWS S3 deploy provider.
//!
//! Deploys static sites to an S3 bucket with optional CloudFront distribution.
//!
//! ## Detection
//!
//! - `s3-deploy.json` or `samconfig.toml`
//!
//! ## Authentication
//!
//! - `AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY` (CI/CD)
//! - AWS CLI profile (local)

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use sandbox::SandboxProvider;

use crate::config::{DeployContext, SetupContext};
use crate::error::{DeployError, DeployResult};
use crate::output::{
    DeployOutput, DeployStatus, DetectedConfig, PrerequisiteStatus,
};
use crate::provider::DeployProvider;
use crate::providers::helpers::{combined_output, has_env_var};

/// AWS S3 provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AwsS3Config {
    /// S3 bucket name.
    pub bucket: Option<String>,
    /// AWS region.
    pub region: Option<String>,
    /// CloudFront distribution ID (for cache invalidation).
    pub cloudfront_distribution_id: Option<String>,
}

/// AWS S3 deploy provider.
pub struct AwsS3Provider;

#[async_trait]
impl DeployProvider for AwsS3Provider {
    fn name(&self) -> &str {
        "AWS S3"
    }

    fn slug(&self) -> &str {
        "aws-s3"
    }

    fn description(&self) -> &str {
        "Deploy to AWS S3 with optional CloudFront CDN"
    }

    fn cli_tool(&self) -> &str {
        "aws"
    }

    fn auth_env_var(&self) -> &str {
        "AWS_ACCESS_KEY_ID"
    }

    async fn check_prerequisites(
        &self,
        sandbox: Arc<dyn SandboxProvider>,
    ) -> DeployResult<PrerequisiteStatus> {
        let cli_ok = sandbox.exec("aws --version").await.map(|o| o.success()).unwrap_or(false);
        let auth_ok = has_env_var("AWS_ACCESS_KEY_ID") && has_env_var("AWS_SECRET_ACCESS_KEY");

        match (cli_ok, auth_ok) {
            (true, true) => Ok(PrerequisiteStatus::Ready),
            (false, _) => Ok(PrerequisiteStatus::CliMissing {
                tool: "aws".into(),
                install_hint: "Install AWS CLI: https://aws.amazon.com/cli/".into(),
            }),
            (true, false) => Ok(PrerequisiteStatus::AuthMissing {
                env_var: "AWS_ACCESS_KEY_ID".into(),
                login_hint: "aws configure".into(),
            }),
        }
    }

    async fn detect_config(&self, project_dir: PathBuf) -> DeployResult<Option<DetectedConfig>> {
        for name in &["s3-deploy.json", "samconfig.toml"] {
            if project_dir.join(name).exists() {
                return Ok(Some(DetectedConfig {
                    config_file: name.to_string(),
                    is_linked: false,
                    project_name: None,
                    team: None,
                }));
            }
        }
        Ok(None)
    }

    async fn setup(&self, ctx: &mut SetupContext) -> DeployResult<serde_json::Value> {
        if ctx.is_ci {
            return Err(DeployError::CiMissingConfig);
        }

        let _ = ui::layout::section_title("Setting up AWS S3 deployment");
        let _ = ui::status::info("Configure your S3 bucket name and region in appz.json.");

        let config = AwsS3Config {
            region: Some("us-east-1".into()),
            ..Default::default()
        };

        serde_json::to_value(config).map_err(|e| DeployError::JsonError {
            reason: e.to_string(),
        })
    }

    async fn deploy(&self, ctx: DeployContext) -> DeployResult<DeployOutput> {
        let start = std::time::Instant::now();

        if ctx.dry_run {
            return Ok(DeployOutput {
                provider: "aws-s3".into(),
                url: "https://dry-run.s3.amazonaws.com".into(),
                additional_urls: vec![],
                deployment_id: None,
                is_preview: false,
                status: DeployStatus::Ready,
                created_at: Some(chrono::Utc::now()),
                duration_ms: Some(0),
            });
        }

        let s3_config = ctx.deploy_config
            .get_target_config::<AwsS3Config>("aws-s3")?
            .unwrap_or_default();

        let bucket = s3_config.bucket.ok_or(DeployError::MissingConfig {
            provider: "AWS S3 (bucket name required)".into(),
        })?;

        let output_path = ctx.output_path();
        let output_str = output_path.to_string_lossy();
        let s3_uri = format!("s3://{}/", bucket);

        let cmd = format!("aws s3 sync {} {} --delete", output_str, s3_uri);

        crate::ui::info(&ctx, &format!("Syncing to S3 bucket: {}...", bucket));

        let result = ctx.exec(&cmd).await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "AWS S3".into(),
                reason: combined_output(&result),
            });
        }

        // Invalidate CloudFront cache if configured
        if let Some(ref dist_id) = s3_config.cloudfront_distribution_id {
            crate::ui::info(&ctx, "Invalidating CloudFront cache...");
            let invalidate_cmd = format!(
                "aws cloudfront create-invalidation --distribution-id {} --paths '/*'",
                dist_id
            );
            let _ = ctx.exec(&invalidate_cmd).await;
        }

        let url = format!("http://{}.s3-website.amazonaws.com", bucket);

        Ok(DeployOutput {
            provider: "aws-s3".into(),
            url,
            additional_urls: vec![],
            deployment_id: None,
            is_preview: false,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
        })
    }

    async fn deploy_preview(&self, _ctx: DeployContext) -> DeployResult<DeployOutput> {
        Err(DeployError::Unsupported {
            provider: "AWS S3".into(),
            operation: "preview deployments (use a separate bucket for staging)".into(),
        })
    }
}
