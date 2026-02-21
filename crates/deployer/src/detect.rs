//! Platform auto-detection.
//!
//! Scans the project directory for platform-specific configuration files
//! to determine which hosting providers are already configured.
//!
//! Detection is performed in priority order:
//! 1. `appz.json` deploy targets (highest confidence).
//! 2. Platform state/link files (e.g. `.vercel/project.json`).
//! 3. Platform config files (e.g. `vercel.json`, `netlify.toml`).

use std::path::Path;

use crate::config::DeployConfig;
use crate::error::DeployResult;
use crate::output::{DetectedPlatform, DetectionConfidence};

/// Detection rule for a single platform.
struct DetectionRule {
    slug: &'static str,
    name: &'static str,
    /// Files whose presence indicates the platform is configured.
    config_files: &'static [&'static str],
    /// Files that indicate the project is linked/connected to the platform.
    /// These give higher confidence than config files.
    state_files: &'static [&'static str],
}

/// All platform detection rules.
const DETECTION_RULES: &[DetectionRule] = &[
    DetectionRule {
        slug: "vercel",
        name: "Vercel",
        config_files: &["vercel.json"],
        state_files: &[".vercel/project.json"],
    },
    DetectionRule {
        slug: "netlify",
        name: "Netlify",
        config_files: &["netlify.toml"],
        state_files: &[".netlify/state.json"],
    },
    DetectionRule {
        slug: "cloudflare-pages",
        name: "Cloudflare Pages",
        config_files: &["wrangler.toml", "wrangler.json", "wrangler.jsonc"],
        state_files: &[],
    },
    DetectionRule {
        slug: "firebase",
        name: "Firebase Hosting",
        config_files: &["firebase.json"],
        state_files: &[".firebaserc"],
    },
    DetectionRule {
        slug: "fly",
        name: "Fly.io",
        config_files: &["fly.toml"],
        state_files: &[],
    },
    DetectionRule {
        slug: "render",
        name: "Render",
        config_files: &["render.yaml", "render.yml"],
        state_files: &[],
    },
    DetectionRule {
        slug: "azure-static",
        name: "Azure Static Web Apps",
        config_files: &["staticwebapp.config.json", "swa-cli.config.json"],
        state_files: &[],
    },
    DetectionRule {
        slug: "aws-s3",
        name: "AWS S3",
        config_files: &["s3-deploy.json", "samconfig.toml"],
        state_files: &[],
    },
    // Note: github-pages and surge have special detection logic
    // (no unique config file), handled separately below.
];

/// Detect all configured platforms in the project directory.
///
/// Returns detected platforms sorted by confidence (highest first).
/// Takes PathBuf to avoid capturing &Path in Send futures (starbase spawn).
pub async fn detect_all(project_dir: std::path::PathBuf) -> DeployResult<Vec<DetectedPlatform>> {
    let mut detected = Vec::new();

    // 1. Check appz.json deploy targets (highest confidence)
    if let Some(deploy_config) = crate::config::read_deploy_config(&project_dir)? {
        detect_from_config(&deploy_config, &mut detected);
    }

    // 2. Check platform-specific files
    detect_from_files(&project_dir, &mut detected);

    // 3. Special-case detections
    detect_github_pages(&project_dir, &mut detected);
    detect_surge(&project_dir, &mut detected);

    // Sort by confidence (highest first), then alphabetically
    detected.sort_by(|a, b| {
        b.confidence
            .cmp(&a.confidence)
            .then_with(|| a.slug.cmp(&b.slug))
    });

    // Deduplicate by slug (keep highest confidence entry)
    let mut seen = std::collections::HashSet::new();
    detected.retain(|p| seen.insert(p.slug.clone()));

    Ok(detected)
}

/// Detect a specific platform by slug.
pub async fn detect(project_dir: &Path, slug: &str) -> DeployResult<Option<DetectedPlatform>> {
    let all = detect_all(project_dir.to_path_buf()).await?;
    Ok(all.into_iter().find(|p| p.slug == slug))
}

// ---------------------------------------------------------------------------
// Detection helpers
// ---------------------------------------------------------------------------

fn detect_from_config(config: &DeployConfig, detected: &mut Vec<DetectedPlatform>) {
    for slug in config.target_slugs() {
        let name = slug_to_name(&slug);
        detected.push(DetectedPlatform {
            slug: slug.clone(),
            name,
            config_files: vec!["appz.json".to_string()],
            confidence: DetectionConfidence::High,
        });
    }
}

fn detect_from_files(project_dir: &Path, detected: &mut Vec<DetectedPlatform>) {
    for rule in DETECTION_RULES {
        let mut found_files = Vec::new();
        let mut confidence = DetectionConfidence::Low;

        // Check state files first (higher confidence)
        for state_file in rule.state_files {
            if project_dir.join(state_file).exists() {
                found_files.push(state_file.to_string());
                confidence = DetectionConfidence::Medium;
            }
        }

        // Check config files
        for config_file in rule.config_files {
            if project_dir.join(config_file).exists() {
                found_files.push(config_file.to_string());
                if confidence == DetectionConfidence::Low {
                    confidence = DetectionConfidence::Low;
                }
            }
        }

        if !found_files.is_empty() {
            detected.push(DetectedPlatform {
                slug: rule.slug.to_string(),
                name: rule.name.to_string(),
                config_files: found_files,
                confidence,
            });
        }
    }
}

/// GitHub Pages detection: check for `.github/workflows` with deploy actions
/// or a `CNAME` file in common output directories.
fn detect_github_pages(project_dir: &Path, detected: &mut Vec<DetectedPlatform>) {
    let workflows_dir = project_dir.join(".github/workflows");
    let mut found_files = Vec::new();

    // Check for GitHub Pages deploy workflows
    if workflows_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&workflows_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "yml" || ext == "yaml") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let lower = content.to_lowercase();
                        if lower.contains("actions/deploy-pages")
                            || lower.contains("peaceiris/actions-gh-pages")
                            || (lower.contains("github-pages") && lower.contains("deploy"))
                        {
                            found_files.push(format!(
                                ".github/workflows/{}",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            ));
                        }
                    }
                }
            }
        }
    }

    // Check for CNAME in common output dirs
    for dir in &["public", "docs", "."] {
        let cname = project_dir.join(dir).join("CNAME");
        if cname.exists() {
            found_files.push(format!("{}/CNAME", dir));
        }
    }

    if !found_files.is_empty() {
        detected.push(DetectedPlatform {
            slug: "github-pages".to_string(),
            name: "GitHub Pages".to_string(),
            config_files: found_files,
            confidence: DetectionConfidence::Low,
        });
    }
}

/// Surge detection: check for CNAME file with a `.surge.sh` domain.
fn detect_surge(project_dir: &Path, detected: &mut Vec<DetectedPlatform>) {
    // Check CNAME in project root and common output dirs
    for dir in &[".", "dist", "build", "public", "_site"] {
        let cname = project_dir.join(dir).join("CNAME");
        if let Ok(content) = std::fs::read_to_string(&cname) {
            if content.trim().ends_with(".surge.sh") {
                detected.push(DetectedPlatform {
                    slug: "surge".to_string(),
                    name: "Surge".to_string(),
                    config_files: vec![format!("{}/CNAME", dir)],
                    confidence: DetectionConfidence::Low,
                });
                return;
            }
        }
    }
}

/// Map a provider slug to its human-readable name.
fn slug_to_name(slug: &str) -> String {
    match slug {
        "vercel" => "Vercel".to_string(),
        "netlify" => "Netlify".to_string(),
        "cloudflare-pages" => "Cloudflare Pages".to_string(),
        "github-pages" => "GitHub Pages".to_string(),
        "firebase" => "Firebase Hosting".to_string(),
        "aws-s3" => "AWS S3".to_string(),
        "azure-static" => "Azure Static Web Apps".to_string(),
        "surge" => "Surge".to_string(),
        "fly" => "Fly.io".to_string(),
        "render" => "Render".to_string(),
        other => other.to_string(),
    }
}
