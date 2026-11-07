//! AWS S3 deploy provider.
//!
//! ## Deployment model
//! `aws s3 sync` mirrors the build output into an S3 bucket, deleting bucket
//! objects that are not in the output (`--delete`). The deployment URL is the
//! bucket's real REST endpoint (`https://<bucket>.s3.<region>.amazonaws.com`)
//! derived from config — never a guessed dir-name URL.
//!
//! ## Authentication
//! `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` (env or vault). Neither is
//! a CLI flag: both are injected into the child process environment.
//!
//! ## Detection
//! S3 has no marker file, so this provider is never auto-detected: the user
//! passes `--target aws-s3` and sets `deploy.targets.aws-s3.bucket` (plus
//! optional `region`, `prefix`) in appz.jsonc.

use std::path::Path;
use std::time::Instant;

use crate::context::DeployContext;
use crate::error::{DeployError, DeployResult};
use crate::output::{AuthState, DeployOutput, DeployStatus, DetectedConfig, PrerequisiteStatus};
use crate::provider::{DEPLOY_TIMEOUT, DeployProvider, QUERY_TIMEOUT};

pub struct AwsS3Provider;

impl AwsS3Provider {
    fn deploy_internal(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        let start = Instant::now();
        let (bucket, region, prefix) = target_config(ctx)?;

        let output = ctx.project_dir.join(&ctx.output_dir);
        match has_files(&output) {
            Ok(true) => {}
            Ok(false) => {
                return Err(DeployError::DeployFailed {
                    provider: self.name(),
                    reason: format!(
                        "build output '{}' is missing or empty: build the project first (appz build)",
                        ctx.output_dir
                    ),
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(DeployError::DeployFailed {
                    provider: self.name(),
                    reason: format!(
                        "build output '{}' is missing or empty: build the project first (appz build)",
                        ctx.output_dir
                    ),
                });
            }
            Err(e) => {
                return Err(DeployError::DeployFailed {
                    provider: self.name(),
                    reason: format!("cannot read build output '{}': {e}", ctx.output_dir),
                });
            }
        }

        if ctx.dry_run {
            return Ok(DeployOutput::dry_run(self.slug(), false));
        }
        let dest = if prefix.is_empty() {
            format!("s3://{bucket}/")
        } else {
            format!("s3://{bucket}/{prefix}/")
        };
        let output_arg = output.display().to_string();
        let mut args = vec!["s3", "sync", &output_arg, &dest];
        // No configured region: let the aws CLI use its profile / AWS_REGION.
        if !region.is_empty() {
            args.extend(["--region", &region]);
        }
        args.extend(["--delete", "--no-follow-symlinks"]);
        let out = ctx
            .exec_live("aws", &args, DEPLOY_TIMEOUT)
            .map_err(|reason| DeployError::DeployFailed {
                provider: self.name(),
                reason,
            })?;
        if !out.success {
            return Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("aws s3 sync failed:\n{}\n{}", out.stdout, out.stderr),
            });
        }

        let url = s3_url(&bucket, &region, &prefix);
        Ok(DeployOutput {
            provider: self.slug().to_string(),
            url: Some(url),
            is_preview: false,
            // A successful sync doesn't prove that the bucket serves a public website.
            status: DeployStatus::Unknown,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
            auth: None,
            setup_files: Vec::new(),
            cli_missing: false,
            link_needed: false,
            unchanged: false,
        })
    }
}

impl DeployProvider for AwsS3Provider {
    fn name(&self) -> &'static str {
        "AWS S3"
    }
    fn slug(&self) -> &'static str {
        "aws-s3"
    }
    fn cli_tool(&self) -> &'static str {
        "aws"
    }
    fn install_package(&self) -> &'static str {
        "aws-cli"
    }
    fn mise_tool(&self) -> String {
        "awscli".into()
    }
    fn auth_env_var(&self) -> &'static str {
        "AWS_ACCESS_KEY_ID"
    }
    fn whoami_args(&self) -> &'static [&'static str] {
        &["sts", "get-caller-identity"]
    }
    fn login_args(&self) -> &'static [&'static str] {
        &["configure"]
    }
    fn login_hint(&self) -> String {
        "aws configure (or set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY)".into()
    }

    fn check_prerequisites(&self) -> DeployResult<PrerequisiteStatus> {
        if command::shell::find_command_on_path("aws").is_none() {
            return Ok(PrerequisiteStatus::CliMissing {
                tool: "aws".into(),
                install_hint:
                    "https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html"
                        .into(),
            });
        }
        Ok(PrerequisiteStatus::Ready)
    }

    /// Key pairs skip the probe; profiles, SSO and roles use STS instead.
    fn auth_status(&self, ctx: &DeployContext) -> AuthState {
        let present = |key: &str| {
            ctx.run_env.get(key).map_or_else(
                || std::env::var(key).is_ok_and(|v| !v.is_empty()),
                |v| !v.is_empty(),
            )
        };
        let access = present("AWS_ACCESS_KEY_ID");
        let secret = present("AWS_SECRET_ACCESS_KEY");
        if access && secret {
            return AuthState::Authenticated;
        }
        match ctx.exec(self.cli_tool(), self.whoami_args(), QUERY_TIMEOUT) {
            Ok(out) if out.success => AuthState::Authenticated,
            Ok(_) => AuthState::NotAuthenticated,
            Err(_) => AuthState::Unknown,
        }
    }

    fn detect_config(&self, _project_dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        // S3 has no marker file: never auto-detect.
        Ok(None)
    }

    fn is_linked(&self, _ctx: &DeployContext) -> bool {
        true
    }

    fn link_args(&self, _ctx: &DeployContext, _interactive: bool) -> Option<Vec<String>> {
        None
    }

    fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx)
    }

    fn deploy_preview(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        let _ = ctx;
        Err(DeployError::Unsupported {
            provider: self.name(),
            operation: "preview deploy (use --prod)",
        })
    }

    fn list_deployments(
        &self,
        _ctx: &DeployContext,
    ) -> DeployResult<Vec<crate::output::DeploymentInfo>> {
        Err(DeployError::Unsupported {
            provider: self.name(),
            operation: "list_deployments",
        })
    }

    fn rollback(&self, _ctx: &DeployContext, _deployment_id: &str) -> DeployResult<DeployOutput> {
        Err(DeployError::Unsupported {
            provider: self.name(),
            operation: "rollback",
        })
    }
}

// Match sync's --no-follow-symlinks: empty directories don't upload S3 objects.
fn has_files(dir: &Path) -> std::io::Result<bool> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_file() {
            std::fs::File::open(entry.path())?;
            return Ok(true);
        }
        if kind.is_dir() && has_files(&entry.path())? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `deploy.targets.aws-s3` config: bucket (required), region, prefix.
fn target_config(ctx: &DeployContext) -> DeployResult<(String, String, String)> {
    let obj = ctx
        .deploy_config
        .targets
        .get("aws-s3")
        .and_then(|v| v.as_object())
        .ok_or_else(|| DeployError::DeployFailed {
            provider: "AWS S3",
            reason: "missing config: set deploy.targets.aws-s3 in appz.jsonc".into(),
        })?;
    let bucket = obj
        .get("bucket")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| DeployError::DeployFailed {
            provider: "AWS S3",
            reason: "missing config: set deploy.targets.aws-s3.bucket in appz.jsonc".into(),
        })?;
    if bucket.starts_with('-') || bucket.chars().any(char::is_control) {
        return Err(DeployError::DeployFailed {
            provider: "AWS S3",
            reason: format!(
                "'bucket' must not start with '-' or contain control characters (got '{bucket}')"
            ),
        });
    }
    if !(3..=63).contains(&bucket.len())
        || !bucket
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'.'))
        || !bucket.as_bytes()[0].is_ascii_alphanumeric()
        || !bucket.as_bytes()[bucket.len() - 1].is_ascii_alphanumeric()
        || bucket.contains("..")
    {
        return Err(DeployError::DeployFailed {
            provider: "AWS S3",
            reason: "bucket must be a DNS-compatible S3 bucket name".into(),
        });
    }
    let region = obj
        .get("region")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    if region.starts_with('-')
        || !region
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(DeployError::DeployFailed {
            provider: "AWS S3",
            reason: format!(
                "'region' must not start with '-' or contain control characters (got '{region}')"
            ),
        });
    }
    let prefix = obj
        .get("prefix")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().trim_matches('/'))
        .unwrap_or("");
    if prefix.starts_with('-')
        || prefix.chars().any(char::is_control)
        || prefix
            .split('/')
            .any(|seg| seg == ".." || seg.is_empty() && !prefix.is_empty())
    {
        return Err(DeployError::DeployFailed {
            provider: "AWS S3",
            reason: format!(
                "'prefix' must not start with '-' or contain control characters (got '{prefix}')"
            ),
        });
    }
    Ok((bucket.to_string(), region.to_string(), prefix.to_string()))
}

/// The bucket's real REST endpoint, with an optional path prefix.
fn s3_url(bucket: &str, region: &str, prefix: &str) -> String {
    use std::fmt::Write as _;
    let region = if region.is_empty() {
        "us-east-1"
    } else {
        region
    };
    let domain = if region.starts_with("cn-") {
        "amazonaws.com.cn"
    } else {
        "amazonaws.com"
    };
    // S3's wildcard TLS certificate doesn't cover dots in bucket names.
    let base = if bucket.contains('.') {
        format!("https://s3.{region}.{domain}/{bucket}")
    } else {
        format!("https://{bucket}.s3.{region}.{domain}")
    };
    let mut escaped = String::new();
    for b in prefix.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~') {
            escaped.push(char::from(b));
        } else {
            write!(escaped, "%{b:02X}").expect("writing to a String cannot fail");
        }
    }
    if prefix.is_empty() {
        base
    } else {
        format!("{base}/{escaped}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with(bucket: &str, region: &str, prefix: &str) -> DeployContext {
        let mut ctx = DeployContext::new(std::env::temp_dir(), "dist".into());
        ctx.deploy_config.targets.insert(
            "aws-s3".into(),
            serde_json::json!({ "bucket": bucket, "region": region, "prefix": prefix }),
        );
        ctx
    }

    #[test]
    fn target_config_reads_bucket_region_and_prefix() {
        let ctx = ctx_with("my-bucket", "eu-west-1", "site/v1");
        let (b, r, p) = target_config(&ctx).unwrap();
        assert_eq!(b, "my-bucket");
        assert_eq!(r, "eu-west-1");
        assert_eq!(p, "site/v1");
    }

    #[test]
    fn target_config_defaults_region_and_prefix() {
        let ctx = ctx_with("my-bucket", "", "");
        let (_, r, p) = target_config(&ctx).unwrap();
        assert_eq!(r, "");
        assert_eq!(p, "");
    }

    #[test]
    fn target_config_rejects_missing_bucket() {
        let mut ctx = DeployContext::new(std::env::temp_dir(), "dist".into());
        ctx.deploy_config.targets.insert(
            "aws-s3".into(),
            serde_json::json!({ "region": "us-east-1" }),
        );
        let err = target_config(&ctx).unwrap_err().to_string();
        assert!(err.contains("bucket"), "{err}");
    }

    #[test]
    fn target_config_rejects_flag_like_bucket() {
        let mut ctx = DeployContext::new(std::env::temp_dir(), "dist".into());
        ctx.deploy_config
            .targets
            .insert("aws-s3".into(), serde_json::json!({ "bucket": "--delete" }));
        let err = target_config(&ctx).unwrap_err().to_string();
        assert!(err.contains("must not start with '-'"), "{err}");
    }

    #[test]
    fn s3_urls_handle_dotted_buckets_and_escape_object_keys() {
        assert_eq!(
            s3_url("site.example", "eu-west-1", "a b/#file"),
            "https://s3.eu-west-1.amazonaws.com/site.example/a%20b/%23file"
        );
        assert_eq!(
            s3_url("my-bucket", "cn-north-1", ""),
            "https://my-bucket.s3.cn-north-1.amazonaws.com.cn"
        );
        assert!(target_config(&ctx_with("bucket/other", "eu-west-1", "")).is_err());
        assert!(target_config(&ctx_with("my-bucket", "eu/west", "")).is_err());
    }

    #[test]
    fn s3_url_includes_prefix_when_set() {
        assert_eq!(
            s3_url("my-bucket", "us-east-1", ""),
            "https://my-bucket.s3.us-east-1.amazonaws.com"
        );
        assert_eq!(
            s3_url("my-bucket", "eu-west-1", "site/v1"),
            "https://my-bucket.s3.eu-west-1.amazonaws.com/site/v1"
        );
    }

    #[test]
    fn auth_with_both_keys_skips_the_cli_probe() {
        let mut ctx = ctx_with("my-bucket", "", "");
        ctx.run_env
            .insert("AWS_ACCESS_KEY_ID".into(), "test-key".into());
        ctx.run_env
            .insert("AWS_SECRET_ACCESS_KEY".into(), "test-secret".into());
        ctx.run_env.insert("PATH".into(), String::new());
        assert_eq!(AwsS3Provider.auth_status(&ctx), AuthState::Authenticated);
    }
}
