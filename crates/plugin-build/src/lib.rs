//! Plugin build library: WASM header injection, signing, packaging, CDN upload.

mod wasm_header;

use ed25519_dalek::Signer;
use pkcs8::DecodePrivateKey;

use miette::{Context, IntoDiagnostic, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Plugin definition from config.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginDef {
    /// Crate name (e.g. check-plugin)
    pub crate_name: String,
    /// Plugin ID for appz_header (e.g. check, ssg-migrator)
    pub plugin_id: String,
    /// Human-readable name
    pub name: String,
    /// Short description
    pub description: String,
    /// Subscription tier (free, pro, enterprise)
    #[serde(default = "default_tier")]
    pub tier: String,
    /// Commands this plugin provides
    pub commands: Vec<String>,
    /// Minimum CLI version
    #[serde(default = "default_min_cli")]
    pub min_cli_version: String,
}

fn default_tier() -> String {
    "free".to_string()
}

fn default_min_cli() -> String {
    "0.1.0".to_string()
}

/// Build configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Plugins to build
    #[serde(default)]
    pub plugins: Vec<PluginDef>,
    /// CDN base URL (e.g. https://cdn.appz.dev/plugins/v1)
    #[serde(default = "default_cdn_base")]
    pub cdn_base_url: String,
    /// S3/R2 bucket for upload (when using publish)
    pub s3_bucket: Option<String>,
    /// S3 region (optional, for R2 use custom endpoint)
    pub s3_region: Option<String>,
    /// S3/R2 endpoint override (for R2: https://<account_id>.r2.cloudflarestorage.com)
    pub s3_endpoint: Option<String>,
    /// Run wasm-opt (Binaryen) to optimize WASM size. Set to false if Binaryen is not installed.
    #[serde(default = "default_wasm_opt")]
    pub wasm_opt: bool,
}

fn default_wasm_opt() -> bool {
    true
}

fn default_cdn_base() -> String {
    "https://get.appz.dev/plugins/v1".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            plugins: Vec::new(),
            cdn_base_url: default_cdn_base(),
            s3_bucket: std::env::var("APPZ_PLUGIN_S3_BUCKET").ok(),
            s3_region: std::env::var("APPZ_PLUGIN_S3_REGION").ok(),
            s3_endpoint: std::env::var("APPZ_PLUGIN_S3_ENDPOINT").ok(),
            wasm_opt: default_wasm_opt(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path).into_diagnostic()?;
        let config: Config = toml::from_str(&content).into_diagnostic()
            .context("Failed to parse plugins config")?;
        Ok(config)
    }
}

/// Compile plugins for wasm32-wasi.
pub fn build(
    config: &Config,
    output_dir: &Path,
    plugin_filter: Option<&str>,
    skip_wasm_opt: bool,
) -> Result<()> {
    let workspace_root = find_workspace_root()?;
    std::fs::create_dir_all(output_dir).into_diagnostic()?;

    let plugins = filter_plugins(&config.plugins, plugin_filter);
    if plugins.is_empty() {
        println!("No plugins to build");
        return Ok(());
    }

    // Use wasm32-wasip1 (current) or wasm32-wasi (legacy)
    let wasm_target = std::env::var("APPZ_WASM_TARGET")
        .unwrap_or_else(|_| "wasm32-wasip1".to_string());

    let status = Command::new("rustup")
        .args(["target", "add", &wasm_target])
        .current_dir(&workspace_root)
        .status()
        .into_diagnostic()?;
    if !status.success() {
        return Err(miette::miette!(
            "Failed to add {} target. Is rustup installed? Try: rustup target add {}",
            wasm_target,
            wasm_target
        ));
    }

    let skip_wasm_opt =
        skip_wasm_opt || std::env::var("APPZ_SKIP_WASM_OPT").as_deref() == Ok("1");
    let use_wasm_opt = config.wasm_opt && !skip_wasm_opt;

    for plugin in &plugins {
        println!("Building {}...", plugin.crate_name);
        let status = Command::new("cargo")
            .args([
                "build",
                "--profile",
                "release-wasm",
                "--target",
                &wasm_target,
                "-p",
                &plugin.crate_name,
            ])
            .current_dir(&workspace_root)
            .status()
            .into_diagnostic()?;

        if !status.success() {
            return Err(miette::miette!("Failed to build plugin: {}", plugin.crate_name));
        }

        let wasm_name = plugin.crate_name.replace('-', "_");
        let src = workspace_root
            .join("target")
            .join(&wasm_target)
            .join("release-wasm")
            .join(format!("{}.wasm", wasm_name));
        let dest_dir = output_dir.join(&plugin.plugin_id).join(&get_version()?);
        std::fs::create_dir_all(&dest_dir).into_diagnostic()?;
        let dest = dest_dir.join("plugin.wasm");

        if !src.exists() {
            return Err(miette::miette!(
                "Build succeeded but WASM not found at {}",
                src.display()
            ));
        }

        if use_wasm_opt {
            match run_wasm_opt(&src, &dest) {
                Ok(()) => println!("  -> {} (wasm-opt)", dest.display()),
                Err(e) => {
                    println!("  Warning: wasm-opt failed ({}), copying unoptimized", e);
                    std::fs::copy(&src, &dest).into_diagnostic()?;
                    println!("  -> {}", dest.display());
                }
            }
        } else {
            std::fs::copy(&src, &dest).into_diagnostic()?;
            println!("  -> {}", dest.display());
        }
    }

    Ok(())
}

/// Run wasm-opt (Binaryen) to optimize WASM size.
///
/// Uses -Oz for maximum size reduction and --strip-debug to remove debug sections.
/// Requires Binaryen: https://github.com/WebAssembly/binaryen
/// Install: apt install binaryen, brew install binaryen, or download from GitHub releases.
fn run_wasm_opt(input: &Path, output: &Path) -> Result<()> {
    let status = Command::new("wasm-opt")
        .arg(input)
        .arg("-o")
        .arg(output)
        .arg("-Oz")
        .arg("--strip-debug")
        .status()
        .into_diagnostic()
        .context("wasm-opt not found. Install Binaryen (apt install binaryen / brew install binaryen) or set wasm_opt = false in config / APPZ_SKIP_WASM_OPT=1")?;

    if !status.success() {
        return Err(miette::miette!(
            "wasm-opt exited with {:?}",
            status.code()
        ));
    }
    Ok(())
}

/// Inject appz_header custom section into WASM.
pub fn inject_header(
    input: &Path,
    output: &Path,
    plugin_id: &str,
    min_cli_version: &str,
) -> Result<()> {
    wasm_header::inject(input, output, plugin_id, min_cli_version)
}

/// Sign WASM with Ed25519, producing plugin.wasm.sig alongside.
pub fn sign(input: &Path, key_path: Option<&Path>) -> Result<()> {
    let key_path = key_path.ok_or_else(|| {
        miette::miette!(
            "Signing key not found. Set APPZ_SIGNING_KEY env or pass --key. \
             Generate with: openssl genpkey -algorithm ed25519 -out signing_key.key"
        )
    })?;

    let wasm_bytes = std::fs::read(input).into_diagnostic()?;
    let key_pem = std::fs::read_to_string(key_path).into_diagnostic()?;

    let signing_key = ed25519_dalek::SigningKey::from_pkcs8_pem(&key_pem)
        .map_err(|e| miette::miette!("Invalid signing key (expected Ed25519 PEM): {}", e))?;

    let signature = signing_key.sign(&wasm_bytes);
    let sig_path = input.with_extension("wasm.sig");
    std::fs::write(&sig_path, signature.to_bytes()).into_diagnostic()?;

    Ok(())
}

/// Full package: build + inject + sign + checksum.
pub fn package(
    config: &Config,
    output_dir: &Path,
    plugin_filter: Option<&str>,
    skip_wasm_opt: bool,
) -> Result<()> {
    build(config, output_dir, plugin_filter, skip_wasm_opt)?;

    let workspace_root = find_workspace_root()?;
    let key_path = std::env::var("APPZ_SIGNING_KEY")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            Some(workspace_root.join("scripts").join("signing_key.key"))
        });

    let plugins = filter_plugins(&config.plugins, plugin_filter);
    let version = get_version()?;

    for plugin in &plugins {
        let plugin_dir = output_dir.join(&plugin.plugin_id).join(&version);
        let wasm_path = plugin_dir.join("plugin.wasm");

        if !wasm_path.exists() {
            continue;
        }

        // Inject header into a temp file, then replace
        let temp_path = plugin_dir.join("plugin_injected.wasm");
        inject_header(
            &wasm_path,
            &temp_path,
            &plugin.plugin_id,
            &plugin.min_cli_version,
        )?;
        std::fs::rename(&temp_path, &wasm_path).into_diagnostic()?;

        // Sign (skip if no key - for local dev)
        if let Some(ref kp) = key_path {
            if kp.exists() {
                sign(&wasm_path, Some(kp.as_path()))?;
            } else {
                println!("  Skipping sign (key not found at {})", kp.display());
            }
        } else {
            println!("  Skipping sign (no APPZ_SIGNING_KEY or scripts/signing_key.key)");
        }

        // Compute checksum
        let wasm_bytes = std::fs::read(&wasm_path).into_diagnostic()?;
        let hash = Sha256::digest(&wasm_bytes);
        let checksum = hex::encode(hash);
        let checksum_path = plugin_dir.join("checksum.txt");
        std::fs::write(&checksum_path, &checksum).into_diagnostic()?;

        // Write manifest entry (sig_url optional when not signed)
        let size_bytes = wasm_bytes.len() as u64;
        let sig_exists = wasm_path.with_extension("wasm.sig").exists();
        let manifest_entry = serde_json::json!({
            "name": plugin.name,
            "description": plugin.description,
            "version": version,
            "min_cli_version": plugin.min_cli_version,
            "tier": plugin.tier,
            "wasm_url": format!("{}/{}/{}/plugin.wasm", config.cdn_base_url, plugin.plugin_id, version),
            "sig_url": if sig_exists { format!("{}/{}/{}/plugin.wasm.sig", config.cdn_base_url, plugin.plugin_id, version) } else { String::new() },
            "checksum": checksum,
            "commands": plugin.commands,
            "size_bytes": size_bytes,
        });
        let manifest_path = plugin_dir.join("manifest_entry.json");
        std::fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest_entry).into_diagnostic()?,
        )
        .into_diagnostic()?;

        println!("Packaged {} v{}", plugin.plugin_id, version);
    }

    Ok(())
}

/// Upload packaged plugins to CDN and optionally update manifest.
pub async fn publish(
    config: &Config,
    output_dir: &Path,
    plugin_filter: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    let bucket = config
        .s3_bucket
        .clone()
        .or_else(|| std::env::var("APPZ_PLUGIN_S3_BUCKET").ok())
        .filter(|s| !s.is_empty());

    if bucket.is_none() && !dry_run {
        return Err(miette::miette!(
            "S3 bucket not configured. Set APPZ_PLUGIN_S3_BUCKET or s3_bucket in config."
        ));
    }

    if dry_run {
        println!("Dry run - skipping upload");
    }

    let version = get_version()?;
    let mut manifest_plugins: HashMap<String, serde_json::Value> = HashMap::new();

    for entry in std::fs::read_dir(output_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let plugin_id = entry.file_name().to_string_lossy().to_string();

        if let Some(filter) = plugin_filter {
            if plugin_id != filter {
                continue;
            }
        }

        let version_dir = entry.path().join(&version);
        let wasm_path = version_dir.join("plugin.wasm");
        let manifest_path = version_dir.join("manifest_entry.json");

        if !wasm_path.exists() {
            continue;
        }

        if !dry_run {
            if let Some(ref bucket) = bucket {
                let prefix = format!("{}/{}", plugin_id, version);
                upload_to_s3(
                    &wasm_path,
                    bucket,
                    &format!("{}/plugin.wasm", prefix),
                    config,
                )
                .await?;
            }
        }

        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path).into_diagnostic()?;
            let entry: serde_json::Value = serde_json::from_str(&content).into_diagnostic()?;
            manifest_plugins.insert(plugin_id.clone(), entry);
        }
    }

    if !manifest_plugins.is_empty() {
        let manifest = serde_json::json!({
            "version": 1,
            "plugins": manifest_plugins,
        });
        let manifest_path = output_dir.join("plugins.json");
        std::fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).into_diagnostic()?,
        )
        .into_diagnostic()?;
        println!("Updated manifest at {}", manifest_path.display());

        if !dry_run {
            if let Some(ref bucket) = bucket {
                upload_to_s3(&manifest_path, bucket, "plugins.json", config).await?;
            }
        }
    }

    Ok(())
}

async fn upload_to_s3(
    local_path: &Path,
    bucket: &str,
    key: &str,
    config: &Config,
) -> Result<()> {
    use aws_config::environment::credentials::EnvironmentVariableCredentialsProvider;
    use aws_sdk_s3::primitives::ByteStream;

    let endpoint = config
        .s3_endpoint
        .clone()
        .or_else(|| std::env::var("APPZ_PLUGIN_S3_ENDPOINT").ok());
    let region = config
        .s3_region
        .clone()
        .or_else(|| std::env::var("APPZ_PLUGIN_S3_REGION").ok())
        .unwrap_or_else(|| "us-east-1".to_string());

    eprintln!(
        "S3 config: endpoint={} region={} bucket={} key={}",
        endpoint.as_deref().unwrap_or("(default AWS S3)"),
        region,
        bucket,
        key
    );

    // Use env vars only - avoids ProfileFile/credentials-login feature requirements.
    // Set AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY (R2 uses the same for S3-compat).
    let creds = EnvironmentVariableCredentialsProvider::new();
    let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .credentials_provider(creds);

    if let Some(ref ep) = endpoint {
        loader = loader.endpoint_url(ep);
    }

    loader = loader.region(aws_config::Region::new(region));

    let aws_config = loader.load().await;

    let client = aws_sdk_s3::Client::new(&aws_config);
    let body = ByteStream::from(std::fs::read(local_path).into_diagnostic()?);

    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(body)
        .send()
        .await
        .into_diagnostic()?;

    println!("  Uploaded {} -> s3://{}/{}", local_path.display(), bucket, key);
    Ok(())
}

pub fn find_workspace_root() -> Result<PathBuf> {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let cargo = dir.join("Cargo.toml");
        if cargo.exists() {
            let content = std::fs::read_to_string(&cargo).into_diagnostic()?;
            if content.contains("[workspace]") {
                return Ok(dir);
            }
        }
        dir = dir
            .parent()
            .ok_or_else(|| miette::miette!("Could not find workspace root"))?
            .to_path_buf();
    }
}

fn get_version() -> Result<String> {
    let root = find_workspace_root()?;
    let cargo = root.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo).into_diagnostic()?;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("version = ") {
            let version = line
                .trim_start_matches("version = ")
                .trim_matches(|c| c == '"' || c == '\'')
                .to_string();
            return Ok(version);
        }
    }

    Ok("0.1.0".to_string())
}

fn filter_plugins<'a>(
    plugins: &'a [PluginDef],
    filter: Option<&str>,
) -> Vec<&'a PluginDef> {
    plugins
        .iter()
        .filter(|p| {
            filter.map_or(true, |f| {
                p.plugin_id == f || p.crate_name == f
            })
        })
        .collect()
}
