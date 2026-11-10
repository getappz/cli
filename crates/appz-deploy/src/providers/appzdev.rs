//! AppzDev / Cloudflare Edge native deploy provider.
//!
//! Direct edge-native CAS deployment to the Cloudflare runtime (designflare/worker/serve).
//! Features:
//! - Multi-threaded SHA-256 Merkle tree calculation
//! - Delta-check: only missing hashes are uploaded (zero duplicate transfer)
//! - Atomic immutable deployment to R2 MANIFESTS
//! - Sub-second instant rollback via KV HOSTS pointer mutation

use std::path::Path;
use std::time::Instant;

use crate::context::DeployContext;
use crate::error::{DeployError, DeployResult};
use crate::output::{
    DeployOutput, DeployStatus, DeploymentInfo, DeploymentInspect, DetectedConfig,
    PrerequisiteStatus,
};
use crate::provider::DeployProvider;
use crate::static_cas::scan_output_dir;

pub struct AppzdevProvider;

const DEFAULT_API_URL: &str = "https://appz.to";

impl AppzdevProvider {
    fn api_base(&self, ctx: &DeployContext) -> String {
        if let Some(url) = ctx.run_env.get("APPZ_API_URL") {
            if !url.is_empty() {
                return url.clone();
            }
        }
        std::env::var("APPZ_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_string())
    }

    fn resolve_project_id(&self, ctx: &DeployContext) -> String {
        // 1. Check .appz/project.json
        let project_json = ctx.project_dir.join(".appz").join("project.json");
        if let Ok(content) = std::fs::read_to_string(&project_json) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(id) = v.get("projectId").and_then(|s| s.as_str()) {
                    return id.to_string();
                }
            }
        }

        // 2. Check APPZ_PROJECT_ID env var
        if let Some(id) = ctx.run_env.get("APPZ_PROJECT_ID") {
            if !id.is_empty() {
                return id.clone();
            }
        }
        if let Ok(id) = std::env::var("APPZ_PROJECT_ID") {
            if !id.is_empty() {
                return id;
            }
        }

        // 3. Fallback to folder name
        ctx.project_dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "default-project".to_string())
    }

    fn deploy_internal(&self, ctx: &DeployContext, is_preview: bool) -> DeployResult<DeployOutput> {
        let start = Instant::now();
        if ctx.dry_run {
            return Ok(DeployOutput::dry_run(self.slug(), is_preview));
        }

        let output_path = ctx.output_path();
        let scanned = scan_output_dir(&output_path).map_err(|e| DeployError::DeployFailed {
            provider: self.name(),
            reason: e,
        })?;

        if scanned.files.is_empty() {
            return Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: format!(
                    "No static files found in output directory: {}",
                    output_path.display()
                ),
            });
        }

        let project_id = self.resolve_project_id(ctx);
        let api_base = self.api_base(ctx);
        let target = if is_preview { "preview" } else { "production" };

        let payload = serde_json::json!({
            "projectId": project_id,
            "target": target,
            "files": scanned.files,
        });

        // Step 1: POST /api/v0/deployments - Handshake & missing blob diff
        let create_url = format!("{}/api/v0/deployments", api_base);
        let mut req = ureq::post(&create_url);
        if let Ok(token) = std::env::var("APPZ_TOKEN") {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let resp: serde_json::Value = req
            .send_json(payload)
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Failed to initiate deployment to {create_url}: {e}"),
            })?
            .into_json()
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Invalid JSON response from {create_url}: {e}"),
            })?;

        let deployment_id = resp["deploymentId"]
            .as_str()
            .ok_or_else(|| DeployError::DeployFailed {
                provider: self.name(),
                reason: "Missing deploymentId in response".into(),
            })?;

        let missing = resp["missing"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Step 2: Upload missing blobs
        for hash in &missing {
            if let Some(blob) = scanned.blobs_by_hash.get(hash) {
                let upload_url = format!(
                    "{}/api/v0/deployments/{}/files/{}",
                    api_base, deployment_id, hash
                );
                let mut up_req = ureq::put(&upload_url)
                    .set("Content-Type", "application/octet-stream");
                if let Ok(token) = std::env::var("APPZ_TOKEN") {
                    up_req = up_req.set("Authorization", &format!("Bearer {token}"));
                }
                up_req
                    .send_bytes(&blob.data)
                    .map_err(|e| DeployError::DeployFailed {
                        provider: self.name(),
                        reason: format!("Failed to upload blob {hash}: {e}"),
                    })?;
            }
        }

        // Step 3: Commit deployment if blobs were uploaded
        let final_url = if !missing.is_empty() {
            let commit_url = format!("{}/api/v0/deployments/{}/commit", api_base, deployment_id);
            let mut commit_req = ureq::post(&commit_url);
            if let Ok(token) = std::env::var("APPZ_TOKEN") {
                commit_req = commit_req.set("Authorization", &format!("Bearer {token}"));
            }
            let commit_resp: serde_json::Value = commit_req
                .send_json(serde_json::json!({}))
                .map_err(|e| DeployError::DeployFailed {
                    provider: self.name(),
                    reason: format!("Failed to commit deployment: {e}"),
                })?
                .into_json()
                .map_err(|e| DeployError::DeployFailed {
                    provider: self.name(),
                    reason: format!("Invalid commit response: {e}"),
                })?;

            if !is_preview && commit_resp["productionUrl"].is_string() {
                commit_resp["productionUrl"].as_str().map(String::from)
            } else {
                commit_resp["url"].as_str().map(String::from)
            }
        } else if !is_preview && resp["productionUrl"].is_string() {
            resp["productionUrl"].as_str().map(String::from)
        } else {
            resp["url"].as_str().map(String::from)
        };

        Ok(DeployOutput {
            provider: self.slug().to_string(),
            url: final_url,
            is_preview,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
            auth: None,
            setup_files: Vec::new(),
            cli_missing: false,
            link_needed: false,
            unchanged: missing.is_empty(),
        })
    }
}

impl DeployProvider for AppzdevProvider {
    fn name(&self) -> &'static str {
        "AppzDev"
    }

    fn slug(&self) -> &'static str {
        "appzdev"
    }

    fn cli_tool(&self) -> &'static str {
        "appz"
    }

    fn install_package(&self) -> &'static str {
        "@getappz/cli"
    }

    fn auth_env_var(&self) -> &'static str {
        "APPZ_TOKEN"
    }

    fn whoami_args(&self) -> &'static [&'static str] {
        &["whoami"]
    }

    fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        let appz_dir = project_dir.join(".appz");
        let project_json = appz_dir.join("project.json");
        if project_json.is_file() {
            let content = std::fs::read_to_string(&project_json).map_err(|e| {
                DeployError::DeployFailed {
                    provider: self.name(),
                    reason: e.to_string(),
                }
            })?;
            let (name, team) = serde_json::from_str::<serde_json::Value>(&content)
                .ok()
                .map(|v| {
                    (
                        v.get("projectId")
                            .or_else(|| v.get("projectName"))
                            .and_then(|s| s.as_str())
                            .map(String::from),
                        v.get("orgId")
                            .and_then(|s| s.as_str())
                            .map(String::from),
                    )
                })
                .unwrap_or((None, None));
            return Ok(Some(DetectedConfig {
                config_file: ".appz/project.json".into(),
                is_linked: true,
                project_name: name,
                team,
            }));
        }

        if project_dir.join("appz.jsonc").is_file() {
            return Ok(Some(DetectedConfig {
                config_file: "appz.jsonc".into(),
                is_linked: false,
                project_name: None,
                team: None,
            }));
        }

        if project_dir.join("appz.json").is_file() {
            return Ok(Some(DetectedConfig {
                config_file: "appz.json".into(),
                is_linked: false,
                project_name: None,
                team: None,
            }));
        }

        Ok(None)
    }

    fn check_prerequisites(&self) -> DeployResult<PrerequisiteStatus> {
        Ok(PrerequisiteStatus::Ready)
    }

    fn is_linked(&self, ctx: &DeployContext) -> bool {
        ctx.project_dir.join(".appz").join("project.json").is_file()
            || ctx.run_env.contains_key("APPZ_PROJECT_ID")
            || std::env::var("APPZ_PROJECT_ID").is_ok()
    }

    fn link(&self, ctx: &DeployContext, _interactive: bool) -> DeployResult<()> {
        let appz_dir = ctx.project_dir.join(".appz");
        std::fs::create_dir_all(&appz_dir).map_err(|e| DeployError::DeployFailed {
            provider: self.name(),
            reason: format!("Failed to create .appz directory: {e}"),
        })?;

        let project_id = self.resolve_project_id(ctx);
        let project_json = appz_dir.join("project.json");
        let content = serde_json::json!({
            "projectId": project_id,
            "orgId": "team_default",
        });

        std::fs::write(&project_json, serde_json::to_string_pretty(&content).unwrap())
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Failed to write .appz/project.json: {e}"),
            })?;

        Ok(())
    }

    fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, false)
    }

    fn deploy_preview(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, true)
    }

    fn rollback(&self, ctx: &DeployContext, version_id: &str) -> DeployResult<DeployOutput> {
        let start = Instant::now();
        let project_id = self.resolve_project_id(ctx);
        let api_base = self.api_base(ctx);
        let rollback_url = format!("{}/api/v0/projects/{}/rollback", api_base, project_id);

        let mut req = ureq::post(&rollback_url);
        if let Ok(token) = std::env::var("APPZ_TOKEN") {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let resp: serde_json::Value = req
            .send_json(serde_json::json!({ "versionId": version_id }))
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Rollback request failed: {e}"),
            })?
            .into_json()
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Invalid rollback response: {e}"),
            })?;

        let host = resp["host"].as_str().unwrap_or(&project_id);
        let url = format!("https://{}", host);

        Ok(DeployOutput {
            provider: self.slug().to_string(),
            url: Some(url),
            is_preview: false,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
            auth: None,
            setup_files: Vec::new(),
            cli_missing: false,
            link_needed: false,
            unchanged: false,
        })
    }

    fn alias(&self, ctx: &DeployContext, domain: &str) -> DeployResult<String> {
        let project_id = self.resolve_project_id(ctx);
        let api_base = self.api_base(ctx);
        let alias_url = format!("{}/api/v0/projects/{}/alias", api_base, project_id);

        let mut req = ureq::post(&alias_url);
        if let Ok(token) = std::env::var("APPZ_TOKEN") {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let resp: serde_json::Value = req
            .send_json(serde_json::json!({ "domain": domain }))
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Alias request failed: {e}"),
            })?
            .into_json()
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Invalid alias response: {e}"),
            })?;

        let bound_domain = resp["domain"].as_str().unwrap_or(domain);
        Ok(format!("https://{bound_domain}"))
    }

    fn list_deployments(&self, ctx: &DeployContext) -> DeployResult<Vec<DeploymentInfo>> {
        let project_id = self.resolve_project_id(ctx);
        let api_base = self.api_base(ctx);
        let list_url = format!("{}/api/v0/projects/{}/deployments", api_base, project_id);

        let mut req = ureq::get(&list_url);
        if let Ok(token) = std::env::var("APPZ_TOKEN") {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let resp: serde_json::Value = req
            .call()
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Failed to list deployments: {e}"),
            })?
            .into_json()
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Invalid JSON from list deployments: {e}"),
            })?;

        let items = resp
            .as_array()
            .ok_or_else(|| DeployError::DeployFailed {
                provider: self.name(),
                reason: "Expected JSON array of deployments".into(),
            })?;

        let mut out = Vec::new();
        for item in items {
            let id = item["id"].as_str().unwrap_or_default().to_string();
            let version_id = item["versionId"].as_str().map(String::from);
            let url = item["url"].as_str().map(String::from);
            let status_str = item["status"].as_str().unwrap_or("ready");
            let status = match status_str {
                "ready" => DeployStatus::Ready,
                "building" => DeployStatus::Building,
                "error" => DeployStatus::Error,
                _ => DeployStatus::Unknown,
            };
            let created_at = item["createdAt"]
                .as_i64()
                .and_then(|ms| chrono::DateTime::from_timestamp_millis(ms));
            let is_current = item["isCurrent"].as_bool().unwrap_or(false);

            out.push(DeploymentInfo {
                id,
                version_id,
                url,
                status,
                created_at,
                is_current,
            });
        }

        Ok(out)
    }

    fn inspect_deployment(
        &self,
        ctx: &DeployContext,
        id_or_url: &str,
    ) -> DeployResult<DeploymentInspect> {
        let api_base = self.api_base(ctx);
        let trimmed = id_or_url
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        let dpl_id = trimmed
            .split('/')
            .next()
            .unwrap_or(trimmed)
            .split('.')
            .next()
            .unwrap_or(trimmed);

        let inspect_url = format!("{}/api/v0/deployments/{}", api_base, dpl_id);
        let mut req = ureq::get(&inspect_url);
        if let Ok(token) = std::env::var("APPZ_TOKEN") {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let resp: serde_json::Value = req
            .call()
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Failed to inspect deployment {dpl_id}: {e}"),
            })?
            .into_json()
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Invalid JSON from inspect deployment: {e}"),
            })?;

        let inspect: DeploymentInspect =
            serde_json::from_value(resp).map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Failed to parse deployment inspect: {e}"),
            })?;

        Ok(inspect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_resolve_project_id_from_file() {
        let dir = tempdir().unwrap();
        let appz_dir = dir.path().join(".appz");
        std::fs::create_dir_all(&appz_dir).unwrap();
        std::fs::write(
            appz_dir.join("project.json"),
            r#"{"projectId": "prj_file123", "orgId": "team_abc"}"#,
        )
        .unwrap();

        let ctx = DeployContext::new(dir.path().to_path_buf(), "dist".to_string());
        let provider = AppzdevProvider;
        assert_eq!(provider.resolve_project_id(&ctx), "prj_file123");
        assert!(provider.is_linked(&ctx));

        let config = provider.detect_config(dir.path()).unwrap().unwrap();
        assert_eq!(config.project_name.as_deref(), Some("prj_file123"));
        assert_eq!(config.team.as_deref(), Some("team_abc"));
    }

    #[test]
    fn test_resolve_project_id_from_env() {
        let dir = tempdir().unwrap();
        let mut ctx = DeployContext::new(dir.path().to_path_buf(), "dist".to_string());
        ctx.run_env
            .insert("APPZ_PROJECT_ID".to_string(), "prj_env999".to_string());

        let provider = AppzdevProvider;
        assert_eq!(provider.resolve_project_id(&ctx), "prj_env999");
        assert!(provider.is_linked(&ctx));
    }

    #[test]
    fn test_link_writes_clean_project_json() {
        let dir = tempdir().unwrap();
        let mut ctx = DeployContext::new(dir.path().to_path_buf(), "dist".to_string());
        ctx.run_env
            .insert("APPZ_PROJECT_ID".to_string(), "prj_custom_1".to_string());

        let provider = AppzdevProvider;
        provider.link(&ctx, false).unwrap();

        let project_json = dir.path().join(".appz").join("project.json");
        assert!(project_json.is_file());

        let raw = std::fs::read_to_string(&project_json).unwrap();
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["projectId"], "prj_custom_1");
        assert_eq!(val["orgId"], "team_default");
    }
}
