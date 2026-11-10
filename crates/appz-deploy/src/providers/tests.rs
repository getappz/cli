use std::path::PathBuf;

use super::{
    aws_s3::AwsS3Provider, cloudflare::CloudflareProvider, netlify::NetlifyProvider,
    render::RenderProvider, vercel::VercelProvider,
};
use crate::{DeployContext, DeployError, DeployProvider, DeployStatus};

/// Exercise real process execution without credentials or platform side effects.
fn cli(label: &str, bin: &str, response: &str) -> (DeployContext, PathBuf) {
    let dir = std::env::temp_dir().join(format!("appz-provider-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("commands.txt");
    std::fs::write(&log, "").unwrap();
    std::fs::write(dir.join("response.json"), response).unwrap();
    #[cfg(windows)]
    std::fs::write(
        dir.join(format!("{bin}.cmd")),
        "@echo off\r\necho %*>>commands.txt\r\ntype response.json\r\nexit /b 0\r\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file = dir.join(bin);
        std::fs::write(
            &file,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> commands.txt\ncat response.json\n",
        )
        .unwrap();
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut ctx = DeployContext::new(dir.clone(), "build output".into());
    let path = std::env::var_os("PATH").unwrap_or_default();
    let paths = std::iter::once(dir.clone()).chain(std::env::split_paths(&path));
    ctx.run_env.insert(
        "PATH".into(),
        std::env::join_paths(paths)
            .unwrap()
            .to_string_lossy()
            .into_owned(),
    );
    ctx.json_output = true;
    (ctx, log)
}

#[test]
fn vercel_lists_json_metadata_instead_of_scanning_a_table() {
    let (ctx, log) = cli(
        "vercel-list",
        "vercel",
        r#"{"deployments":[{"id":"dpl_one","url":"one.vercel.app","state":"READY","createdAt":1700000000000}]}"#,
    );
    std::fs::create_dir_all(ctx.project_dir.join(".vercel")).unwrap();
    std::fs::write(ctx.project_dir.join(".vercel/project.json"), "{}").unwrap();
    let deployments = VercelProvider.list_deployments(&ctx).unwrap();
    assert_eq!(deployments.len(), 1);
    assert_eq!(deployments[0].id, "dpl_one");
    assert_eq!(
        deployments[0].url.as_deref(),
        Some("https://one.vercel.app")
    );
    assert_eq!(deployments[0].status, DeployStatus::Ready);
    assert!(deployments[0].created_at.is_some());
    assert!(
        std::fs::read_to_string(log)
            .unwrap()
            .contains("--format json")
    );
}

#[test]
fn netlify_lists_deploys_for_the_linked_site_and_restores_by_id() {
    let (mut ctx, log) = cli(
        "netlify-list",
        "netlify",
        r#"[{"id":"deploy-one","deploy_ssl_url":"https://one--site.netlify.app","state":"ready","created_at":"2026-01-01T00:00:00Z"}]"#,
    );
    ctx.run_env
        .insert("NETLIFY_SITE_ID".into(), "site-one".into());
    let deployments = NetlifyProvider.list_deployments(&ctx).unwrap();
    assert_eq!(deployments[0].id, "deploy-one");
    assert_eq!(deployments[0].status, DeployStatus::Ready);
    assert!(deployments[0].created_at.is_some());
    let args = std::fs::read_to_string(&log).unwrap();
    assert!(
        args.contains("api listSiteDeploys") && args.contains("site-one"),
        "{args}"
    );
    std::fs::write(
        ctx.project_dir.join("response.json"),
        r#"{"id":"deploy-one","ssl_url":"https://site.example","state":"ready"}"#,
    )
    .unwrap();
    let result = NetlifyProvider.rollback(&ctx, "deploy-one").unwrap();
    assert_eq!(result.url.as_deref(), Some("https://site.example"));
    assert!(
        std::fs::read_to_string(log)
            .unwrap()
            .contains("api restoreSiteDeploy")
    );
}

#[test]
fn cloudflare_rolls_back_the_version_from_a_deployment() {
    let (ctx, log) = cli(
        "cloudflare-list",
        "wrangler",
        r#"[{"id":"deployment-one","created_on":"2026-01-01T00:00:00Z","versions":[{"version_id":"version-one","percentage":100}]}]"#,
    );
    let deployments = CloudflareProvider.list_deployments(&ctx).unwrap();
    assert_eq!(deployments[0].id, "deployment-one");
    assert!(deployments[0].is_current);
    CloudflareProvider.rollback(&ctx, "deployment-one").unwrap();
    let args = std::fs::read_to_string(log).unwrap();
    assert!(args.contains("deployments list --json"), "{args}");
    assert!(args.contains("rollback version-one --message"), "{args}");
    assert!(!args.contains("versions rollback"), "{args}");
}

#[test]
fn malformed_deployment_json_is_an_error_not_an_empty_list() {
    for (label, bin, provider) in [
        (
            "bad-vercel",
            "vercel",
            Box::new(VercelProvider) as Box<dyn DeployProvider>,
        ),
        ("bad-netlify", "netlify", Box::new(NetlifyProvider)),
        ("bad-cloudflare", "wrangler", Box::new(CloudflareProvider)),
        ("bad-render", "render", Box::new(RenderProvider)),
    ] {
        let (mut ctx, _) = cli(label, bin, "this is not json");
        ctx.run_env
            .insert("NETLIFY_SITE_ID".into(), "site-one".into());
        ctx.deploy_config
            .targets
            .insert("render".into(), serde_json::json!({"service": "srv-one"}));
        std::fs::create_dir_all(ctx.project_dir.join(".vercel")).unwrap();
        std::fs::write(ctx.project_dir.join(".vercel/project.json"), "{}").unwrap();
        assert!(provider.list_deployments(&ctx).is_err(), "{bin}");
    }
}

#[test]
fn rollback_dry_runs_never_invoke_platform_commands() {
    for (label, bin, provider) in [
        (
            "dry-vercel",
            "vercel",
            Box::new(VercelProvider) as Box<dyn DeployProvider>,
        ),
        ("dry-netlify", "netlify", Box::new(NetlifyProvider)),
        ("dry-cloudflare", "wrangler", Box::new(CloudflareProvider)),
    ] {
        let (mut ctx, log) = cli(label, bin, "{}");
        ctx.dry_run = true;
        assert_eq!(
            provider.rollback(&ctx, "deployment-one").unwrap().status,
            DeployStatus::Unknown
        );
        assert!(std::fs::read_to_string(log).unwrap().is_empty(), "{bin}");
    }
}

#[test]
fn rollback_rejects_flag_like_ids_before_executing() {
    for (label, bin, provider) in [
        (
            "flag-vercel",
            "vercel",
            Box::new(VercelProvider) as Box<dyn DeployProvider>,
        ),
        ("flag-netlify", "netlify", Box::new(NetlifyProvider)),
        ("flag-cloudflare", "wrangler", Box::new(CloudflareProvider)),
    ] {
        let (ctx, log) = cli(label, bin, "{}");
        for id in ["", "--help", "bad\nvalue"] {
            assert!(provider.rollback(&ctx, id).is_err(), "{bin}: {id}");
        }
        assert!(std::fs::read_to_string(log).unwrap().is_empty(), "{bin}");
    }
}

#[test]
fn render_uses_the_platform_cli_and_the_real_service_url() {
    let response =
        r#"[{"service":{"id":"srv-one","serviceDetails":{"url":"https://actual.example"}}}]"#;
    let (mut ctx, log) = cli("render-deploy", "render", response);
    ctx.deploy_config
        .targets
        .insert("render".into(), serde_json::json!({"service": "srv-one"}));
    assert!(!RenderProvider.needs_prebuild(&[]));
    let result = RenderProvider.deploy(&ctx).unwrap();
    assert_eq!(result.url.as_deref(), Some("https://actual.example"));
    let args = std::fs::read_to_string(log).unwrap();
    assert!(
        args.contains("deploys create srv-one --wait --output json --confirm"),
        "{args}"
    );
    assert!(args.contains("services --output json --confirm"), "{args}");
    std::fs::write(
        ctx.project_dir.join("response.json"),
        r#"[{"id":"dep-one","status":"live","createdAt":"2026-01-01T00:00:00Z"}]"#,
    )
    .unwrap();
    let deployments = RenderProvider.list_deployments(&ctx).unwrap();
    assert_eq!(deployments[0].id, "dep-one");
    assert_eq!(deployments[0].status, DeployStatus::Ready);
    assert!(matches!(
        RenderProvider.rollback(&ctx, "dep-one"),
        Err(DeployError::Unsupported { .. })
    ));
}

#[test]
fn github_pages_respects_the_output_cname() {
    let (ctx, _) = cli("pages-cname", "gh-pages", "git@github.com:acme/my-site.git");
    cli("pages-cname", "git", "git@github.com:acme/my-site.git");
    std::fs::create_dir_all(ctx.output_path()).unwrap();
    std::fs::write(ctx.output_path().join("CNAME"), "www.example.com\n").unwrap();
    let result = super::github_pages::GithubPagesProvider
        .deploy(&ctx)
        .unwrap();
    assert_eq!(result.url.as_deref(), Some("https://www.example.com"));
    assert_eq!(result.status, DeployStatus::Unknown);
}

#[test]
fn cloudflare_refuses_to_flatten_a_split_deployment() {
    let response = r#"[{"id":"deployment-one","created_on":"2026-01-01T00:00:00Z","versions":[{"version_id":"a","percentage":50},{"version_id":"b","percentage":50}]}]"#;
    let (ctx, log) = cli("cloudflare-split", "wrangler", response);
    assert!(CloudflareProvider.rollback(&ctx, "deployment-one").is_err());
    assert!(!std::fs::read_to_string(log).unwrap().contains("rollback"));
}

#[test]
fn s3_passes_region_and_refuses_empty_output_before_sync_delete() {
    let (mut ctx, log) = cli("s3-deploy", "aws", "{}");
    ctx.deploy_config.targets.insert(
        "aws-s3".into(),
        serde_json::json!({"bucket":"my-bucket","region":"eu-west-1","prefix":"site/v1"}),
    );
    let err = AwsS3Provider.deploy(&ctx).unwrap_err().to_string();
    assert!(err.contains("missing or empty"), "{err}");
    std::fs::create_dir_all(ctx.output_path()).unwrap();
    assert!(AwsS3Provider.deploy(&ctx).is_err());
    assert!(std::fs::read_to_string(&log).unwrap().is_empty());
    std::fs::create_dir_all(ctx.output_path().join("assets")).unwrap();
    assert!(
        AwsS3Provider.deploy(&ctx).is_err(),
        "empty nested folders must never reach sync --delete"
    );
    assert!(std::fs::read_to_string(&log).unwrap().is_empty());
    std::fs::write(ctx.output_path().join("assets/index.html"), "hello").unwrap();
    let output = AwsS3Provider.deploy(&ctx).unwrap();
    assert!(output.url.as_deref().unwrap().contains("eu-west-1"));
    let args = std::fs::read_to_string(log).unwrap();
    assert!(
        args.contains("s3 sync")
            && args.contains("--region eu-west-1")
            && args.contains("--delete"),
        "{args}"
    );
}
