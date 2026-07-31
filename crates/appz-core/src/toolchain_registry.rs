use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::registry_cache::{CachedPayload, load_cached};
use crate::toolchains::{
    DetectionConfidence, DetectionCriteria, Detector, Framework, LifecycleCommands,
};

const REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/getappz/cli/main/registry/toolchains.toml";
const CACHE_TTL_SECONDS: u64 = 24 * 3600;
const CACHE_FILE: &str = "toolchains-registry.json";

pub fn cache_path() -> PathBuf {
    crate::format_defaults::appz_home_dir().join(CACHE_FILE)
}

static FRAMEWORKS_CACHE: OnceLock<Vec<Framework>> = OnceLock::new();

pub fn get_frameworks() -> &'static [Framework] {
    FRAMEWORKS_CACHE.get_or_init(|| load_frameworks_registry(true))
}

pub fn load_frameworks_registry(offline: bool) -> Vec<Framework> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let registry: Vec<Framework> = match load_cached::<Vec<RegistryFrameworkSpec>>(
        &cache_path(),
        CACHE_TTL_SECONDS,
        offline,
        now,
        fetch_registry,
        parse_registry_toml,
    ) {
        Ok(specs) => specs.into_iter().map(convert_to_framework).collect(),
        Err(e) => {
            eprintln!("warning: toolchains registry unavailable — using built-in core only: {e}");
            vec![]
        }
    };

    merge_frameworks(embedded_frameworks(), registry)
}

pub fn parse_registry_toml(raw: &str) -> Result<Vec<RegistryFrameworkSpec>, String> {
    let file: RegistryFile = toml::from_str(raw).map_err(|e| format!("TOML parse error: {e}"))?;
    Ok(file.frameworks)
}

pub fn fetch_registry() -> Result<String, String> {
    let resp = ureq::get(REGISTRY_URL)
        .call()
        .map_err(|e| format!("HTTP fetch error: {e}"))?;
    resp.into_string()
        .map_err(|e| format!("HTTP read error: {e}"))
}

pub fn refresh_registry_cache() -> Result<usize, String> {
    let raw = fetch_registry()?;
    let specs = parse_registry_toml(&raw)?;
    let count = specs.len();
    let payload = CachedPayload {
        fetched_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        data: specs,
    };
    let json = serde_json::to_string(&payload).map_err(|e| format!("serialize error: {e}"))?;
    let path = cache_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cache dir error: {e}"))?;
    }
    std::fs::write(&path, &json).map_err(|e| format!("cache write error: {e}"))?;
    Ok(count)
}

fn merge_frameworks(mut embedded: Vec<Framework>, registry: Vec<Framework>) -> Vec<Framework> {
    for rfw in registry {
        let slug_matches: Vec<usize> = embedded
            .iter()
            .enumerate()
            .filter(|(_, e)| e.slug == rfw.slug)
            .map(|(i, _)| i)
            .collect();

        // Some slugs are shared by more than one embedded entry (e.g. "bun"
        // covers both the runtime and the package manager) — a bare slug
        // match would silently overwrite whichever one comes first. When
        // that happens, only replace the entry whose name also matches;
        // otherwise fall through to appending, rather than guessing.
        let pos = match slug_matches.as_slice() {
            [] => None,
            [i] => Some(*i),
            multiple => multiple
                .iter()
                .copied()
                .find(|&i| embedded[i].name == rfw.name),
        };

        match pos {
            Some(i) => embedded[i] = rfw,
            None => embedded.push(rfw),
        }
    }
    embedded
}

fn embedded_frameworks() -> Vec<Framework> {
    use crate::toolchains::{
        BUN_PM, BUN_RUNTIME, GO_RUNTIME, JAVA_RUNTIME, NODE_RUNTIME, NPM_PM, PNPM_PM, PYTHON,
        RUBY_RUNTIME, RUST_RUNTIME, YARN_PM,
    };
    vec![
        PYTHON,
        RUBY_RUNTIME,
        RUST_RUNTIME,
        BUN_RUNTIME,
        NODE_RUNTIME,
        GO_RUNTIME,
        JAVA_RUNTIME,
        NPM_PM,
        PNPM_PM,
        BUN_PM,
        YARN_PM,
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryDetector {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub is_glob: Option<bool>,
    #[serde(default)]
    pub match_content: Option<String>,
    #[serde(default)]
    pub match_package: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryDetectionCriteria {
    #[serde(default)]
    pub every: Vec<RegistryDetector>,
    #[serde(default)]
    pub some: Vec<RegistryDetector>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryLifecycleCommands {
    pub build: Option<String>,
    pub install: Option<String>,
    pub dev: Option<String>,
    pub test: Option<String>,
    pub lint: Option<String>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryFrameworkSpec {
    pub name: String,
    pub slug: String,
    pub mise_plugin: String,
    #[serde(default)]
    pub version_files: Vec<String>,
    pub default_version: String,
    pub detectors: RegistryDetectionCriteria,
    pub commands: RegistryLifecycleCommands,
    #[serde(default)]
    pub supersedes: Vec<String>,
    pub detection_confidence: String,
    pub output_directory: Option<String>,
    pub env_prefix: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistryFile {
    pub version: u32,
    pub frameworks: Vec<RegistryFrameworkSpec>,
}

pub fn convert_to_framework(spec: RegistryFrameworkSpec) -> Framework {
    let output_directory = spec.output_directory.map(leak_str);
    let env_prefix = spec.env_prefix.map(leak_str);

    Framework {
        name: leak_str(spec.name),
        slug: leak_str(spec.slug),
        mise_plugin: leak_str(spec.mise_plugin),
        version_files: leak_slice(spec.version_files),
        default_version: leak_str(spec.default_version),
        detectors: DetectionCriteria {
            every: leak_detectors(spec.detectors.every),
            some: leak_detectors(spec.detectors.some),
        },
        commands: LifecycleCommands {
            build: spec.commands.build.map(leak_str),
            install: spec.commands.install.map(leak_str),
            dev: spec.commands.dev.map(leak_str),
            test: spec.commands.test.map(leak_str),
            lint: spec.commands.lint.map(leak_str),
            format: spec.commands.format.map(leak_str),
        },
        supersedes: leak_slice(spec.supersedes),
        detection_confidence: match spec.detection_confidence.as_str() {
            "weak" => DetectionConfidence::Weak,
            _ => DetectionConfidence::Strong,
        },
        output_directory,
        env_prefix,
    }
}

pub fn framework_to_spec(fw: &Framework) -> RegistryFrameworkSpec {
    RegistryFrameworkSpec {
        name: fw.name.to_string(),
        slug: fw.slug.to_string(),
        mise_plugin: fw.mise_plugin.to_string(),
        version_files: fw.version_files.iter().map(|s| (*s).to_string()).collect(),
        default_version: fw.default_version.to_string(),
        detectors: RegistryDetectionCriteria {
            every: fw
                .detectors
                .every
                .iter()
                .map(detector_to_registry)
                .collect(),
            some: fw.detectors.some.iter().map(detector_to_registry).collect(),
        },
        commands: RegistryLifecycleCommands {
            build: fw.commands.build.map(|s| s.to_string()),
            install: fw.commands.install.map(|s| s.to_string()),
            dev: fw.commands.dev.map(|s| s.to_string()),
            test: fw.commands.test.map(|s| s.to_string()),
            lint: fw.commands.lint.map(|s| s.to_string()),
            format: fw.commands.format.map(|s| s.to_string()),
        },
        supersedes: fw.supersedes.iter().map(|s| (*s).to_string()).collect(),
        detection_confidence: match fw.detection_confidence {
            DetectionConfidence::Weak => "weak".to_string(),
            DetectionConfidence::Strong => "strong".to_string(),
        },
        output_directory: fw.output_directory.map(|s| s.to_string()),
        env_prefix: fw.env_prefix.map(|s| s.to_string()),
    }
}

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn leak_slice(v: Vec<String>) -> &'static [&'static str] {
    Box::leak(
        v.into_iter()
            .map(leak_str)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

fn leak_detectors(v: Vec<RegistryDetector>) -> &'static [Detector] {
    Box::leak(
        v.into_iter()
            .map(|d| Detector {
                path: d.path.map(leak_str).unwrap_or(""),
                is_glob: d.is_glob.unwrap_or(false),
                match_content: d.match_content.map(leak_str),
                match_package: d.match_package.map(leak_str),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

fn detector_to_registry(d: &Detector) -> RegistryDetector {
    RegistryDetector {
        path: if d.path.is_empty() {
            None
        } else {
            Some(d.path.to_string())
        },
        is_glob: if d.is_glob { Some(true) } else { None },
        match_content: d.match_content.map(ToString::to_string),
        match_package: d.match_package.map(ToString::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nextjs_toml() {
        let toml_str = r#"
version = 1

[[frameworks]]
name = "Next.js"
slug = "nextjs"
mise_plugin = "node"
version_files = []
default_version = "lts"
detection_confidence = "strong"
output_directory = ".next"
env_prefix = "NEXT_PUBLIC_"
supersedes = []

[frameworks.detectors]
every = []
some = [{ match_package = "next" }]

[frameworks.commands]
build = "next build"
install = "npm install"
dev = "next dev --port $PORT"
"#;

        let file: RegistryFile = toml::from_str(toml_str).unwrap();
        assert_eq!(file.version, 1);
        assert_eq!(file.frameworks.len(), 1);

        let fw = &file.frameworks[0];
        assert_eq!(fw.name, "Next.js");
        assert_eq!(fw.slug, "nextjs");
        assert_eq!(fw.mise_plugin, "node");
        assert!(fw.version_files.is_empty());
        assert_eq!(fw.default_version, "lts");
        assert_eq!(fw.detection_confidence, "strong");
        assert_eq!(fw.output_directory.as_deref(), Some(".next"));
        assert_eq!(fw.env_prefix.as_deref(), Some("NEXT_PUBLIC_"));
        assert!(fw.supersedes.is_empty());

        assert!(fw.detectors.every.is_empty());
        assert_eq!(fw.detectors.some.len(), 1);
        assert_eq!(fw.detectors.some[0].match_package.as_deref(), Some("next"));

        assert_eq!(fw.commands.build.as_deref(), Some("next build"));
        assert_eq!(fw.commands.install.as_deref(), Some("npm install"));
        assert_eq!(fw.commands.dev.as_deref(), Some("next dev --port $PORT"));
        assert!(fw.commands.test.is_none());
        assert!(fw.commands.lint.is_none());
        assert!(fw.commands.format.is_none());
    }

    #[test]
    fn test_parse_django_toml() {
        let toml_str = r#"
version = 1

[[frameworks]]
name = "Django"
slug = "django"
mise_plugin = "python"
version_files = []
default_version = "latest"
detection_confidence = "strong"

[frameworks.detectors]
every = []
some = [{ match_package = "django" }]

[frameworks.commands]
build = "python manage.py collectstatic"
install = "pip install -r requirements.txt"
dev = "python manage.py runserver"
"#;

        let file: RegistryFile = toml::from_str(toml_str).unwrap();
        assert_eq!(file.frameworks.len(), 1);

        let fw = &file.frameworks[0];
        assert_eq!(fw.name, "Django");
        assert_eq!(fw.slug, "django");
        assert_eq!(fw.output_directory, None);
        assert_eq!(fw.env_prefix, None);
    }

    #[test]
    fn test_parse_multiple_frameworks() {
        let toml_str = r#"
version = 1

[[frameworks]]
name = "Next.js"
slug = "nextjs"
mise_plugin = "node"
version_files = []
default_version = "lts"
detection_confidence = "strong"

[frameworks.detectors]
every = []
some = [{ match_package = "next" }]

[frameworks.commands]
build = "next build"
install = "npm install"
dev = "next dev --port $PORT"

[[frameworks]]
name = "Django"
slug = "django"
mise_plugin = "python"
version_files = []
default_version = "latest"
detection_confidence = "strong"

[frameworks.detectors]
every = []
some = [{ match_package = "django" }]

[frameworks.commands]
build = "python manage.py collectstatic"
install = "pip install -r requirements.txt"
dev = "python manage.py runserver"

[[frameworks]]
name = "Express"
slug = "express"
mise_plugin = "node"
version_files = []
default_version = "lts"
detection_confidence = "strong"

[frameworks.detectors]
every = []
some = [{ match_package = "express" }]

[frameworks.commands]
build = "express build"
install = "npm install"
dev = "node index.js"
"#;

        let file: RegistryFile = toml::from_str(toml_str).unwrap();
        assert_eq!(file.frameworks.len(), 3);
        assert_eq!(file.frameworks[0].slug, "nextjs");
        assert_eq!(file.frameworks[1].slug, "django");
        assert_eq!(file.frameworks[2].slug, "express");
    }

    #[test]
    fn test_round_trip_nextjs() {
        let spec = RegistryFrameworkSpec {
            name: "Next.js".to_string(),
            slug: "nextjs".to_string(),
            mise_plugin: "node".to_string(),
            version_files: vec![],
            default_version: "lts".to_string(),
            detectors: RegistryDetectionCriteria {
                every: vec![],
                some: vec![RegistryDetector {
                    path: None,
                    is_glob: None,
                    match_content: None,
                    match_package: Some("next".to_string()),
                }],
            },
            commands: RegistryLifecycleCommands {
                build: Some("next build".to_string()),
                install: Some("npm install".to_string()),
                dev: Some("next dev --port $PORT".to_string()),
                test: None,
                lint: None,
                format: None,
            },
            supersedes: vec![],
            detection_confidence: "strong".to_string(),
            output_directory: Some(".next".to_string()),
            env_prefix: Some("NEXT_PUBLIC_".to_string()),
        };

        let fw = convert_to_framework(spec.clone());
        let back = framework_to_spec(&fw);

        assert_eq!(back.name, spec.name);
        assert_eq!(back.slug, spec.slug);
        assert_eq!(back.mise_plugin, spec.mise_plugin);
        assert_eq!(back.default_version, spec.default_version);
        assert_eq!(back.detection_confidence, spec.detection_confidence);
        assert_eq!(back.output_directory, spec.output_directory);
        assert_eq!(back.env_prefix, spec.env_prefix);
        assert_eq!(back.commands.build, spec.commands.build);
        assert_eq!(back.commands.install, spec.commands.install);
        assert_eq!(back.commands.dev, spec.commands.dev);
        assert_eq!(back.detectors.some.len(), spec.detectors.some.len());
        assert_eq!(
            back.detectors.some[0].match_package,
            spec.detectors.some[0].match_package
        );
    }

    #[test]
    fn test_merge_embedded_and_registry() {
        let spec = RegistryFrameworkSpec {
            name: "NewFramework".to_string(),
            slug: "newfw".to_string(),
            mise_plugin: "node".to_string(),
            version_files: vec![],
            default_version: "lts".to_string(),
            detectors: RegistryDetectionCriteria {
                every: vec![],
                some: vec![RegistryDetector {
                    path: None,
                    is_glob: None,
                    match_content: None,
                    match_package: Some("newfw".to_string()),
                }],
            },
            commands: RegistryLifecycleCommands {
                build: Some("newfw build".to_string()),
                install: Some("npm install".to_string()),
                dev: Some("newfw dev".to_string()),
                test: None,
                lint: None,
                format: None,
            },
            supersedes: vec![],
            detection_confidence: "strong".to_string(),
            output_directory: None,
            env_prefix: None,
        };

        let registry_fw = convert_to_framework(spec);
        let merged = merge_frameworks(embedded_frameworks(), vec![registry_fw]);

        assert_eq!(merged.len(), 12);
        assert!(merged.iter().any(|f| f.slug == "newfw"));
        assert!(merged.iter().any(|f| f.slug == "node"));
    }

    #[test]
    fn test_merge_registry_overrides_embedded() {
        let override_slug = embedded_frameworks()[0].slug;
        let spec = RegistryFrameworkSpec {
            name: "Override".to_string(),
            slug: override_slug.to_string(),
            mise_plugin: "override".to_string(),
            version_files: vec![],
            default_version: "latest".to_string(),
            detectors: RegistryDetectionCriteria {
                every: vec![],
                some: vec![],
            },
            commands: RegistryLifecycleCommands {
                build: None,
                install: None,
                dev: None,
                test: None,
                lint: None,
                format: None,
            },
            supersedes: vec![],
            detection_confidence: "weak".to_string(),
            output_directory: None,
            env_prefix: None,
        };

        let registry_fw = convert_to_framework(spec);
        let merged = merge_frameworks(embedded_frameworks(), vec![registry_fw]);

        assert_eq!(merged.len(), 11, "override should replace, not append");
        let overridden = merged.iter().find(|f| f.slug == override_slug).unwrap();
        assert_eq!(overridden.name, "Override");
        assert_eq!(overridden.mise_plugin, "override");
    }

    #[test]
    fn test_merge_disambiguates_duplicate_slug_by_name() {
        // "bun" is shared by BUN_RUNTIME (name "Bun") and BUN_PM (name
        // "bun") — the registry override must only replace the package
        // manager entry, leaving the runtime entry untouched.
        let spec = RegistryFrameworkSpec {
            name: "bun".to_string(),
            slug: "bun".to_string(),
            mise_plugin: "bun".to_string(),
            version_files: vec![],
            default_version: "latest".to_string(),
            detectors: RegistryDetectionCriteria {
                every: vec![],
                some: vec![],
            },
            commands: RegistryLifecycleCommands {
                build: None,
                install: Some("bun install --frozen-lockfile".to_string()),
                dev: None,
                test: None,
                lint: None,
                format: None,
            },
            supersedes: vec![],
            detection_confidence: "strong".to_string(),
            output_directory: None,
            env_prefix: None,
        };

        let registry_fw = convert_to_framework(spec);
        let merged = merge_frameworks(embedded_frameworks(), vec![registry_fw]);

        assert_eq!(merged.len(), 11, "override should replace, not append");
        let bun_entries: Vec<_> = merged.iter().filter(|f| f.slug == "bun").collect();
        assert_eq!(bun_entries.len(), 2, "both bun entries should still exist");

        let runtime = bun_entries.iter().find(|f| f.name == "Bun").unwrap();
        assert!(
            runtime.commands.install.is_none(),
            "BUN_RUNTIME must be untouched by a BUN_PM-named override"
        );

        let pm = bun_entries.iter().find(|f| f.name == "bun").unwrap();
        assert_eq!(
            pm.commands.install,
            Some("bun install --frozen-lockfile"),
            "BUN_PM should be overridden"
        );
    }

    #[test]
    fn test_golden_embedded_roundtrip() {
        let embedded = embedded_frameworks();
        assert_eq!(embedded.len(), 11, "expected 11 embedded core frameworks");

        for orig in &embedded {
            let spec = framework_to_spec(orig);
            let roundtripped = convert_to_framework(spec);

            assert_eq!(
                roundtripped.name, orig.name,
                "name mismatch for {}",
                orig.slug
            );
            assert_eq!(roundtripped.slug, orig.slug, "slug mismatch");
            assert_eq!(
                roundtripped.mise_plugin, orig.mise_plugin,
                "mise_plugin mismatch for {}",
                orig.slug
            );
            assert_eq!(
                roundtripped.default_version, orig.default_version,
                "default_version mismatch for {}",
                orig.slug
            );
            assert_eq!(
                roundtripped.output_directory, orig.output_directory,
                "output_directory mismatch for {}",
                orig.slug
            );
            assert_eq!(
                roundtripped.env_prefix, orig.env_prefix,
                "env_prefix mismatch for {}",
                orig.slug
            );
            assert_eq!(
                roundtripped.supersedes, orig.supersedes,
                "supersedes mismatch for {}",
                orig.slug
            );

            assert_eq!(
                std::mem::discriminant(&roundtripped.detection_confidence),
                std::mem::discriminant(&orig.detection_confidence),
                "detection_confidence mismatch for {}",
                orig.slug
            );

            assert_eq!(
                roundtripped.commands.build, orig.commands.build,
                "build cmd mismatch for {}",
                orig.slug
            );
            assert_eq!(
                roundtripped.commands.install, orig.commands.install,
                "install cmd mismatch for {}",
                orig.slug
            );
            assert_eq!(
                roundtripped.commands.dev, orig.commands.dev,
                "dev cmd mismatch for {}",
                orig.slug
            );
        }
    }

    #[test]
    fn test_parse_detector_with_all_fields() {
        let toml_str = r#"
version = 1

[[frameworks]]
name = "Custom"
slug = "custom"
mise_plugin = "node"
version_files = []
default_version = "lts"
detection_confidence = "strong"

[frameworks.detectors]
every = [{ path = "Dockerfile", is_glob = false, match_content = "FROM node" }]
some = [{ path = "src/**/*.ts", is_glob = true, match_content = "express" }, { match_package = "express" }]

[frameworks.commands]
build = "npm run build"
"#;

        let file: RegistryFile = toml::from_str(toml_str).unwrap();
        assert_eq!(file.frameworks.len(), 1);

        let fw = &file.frameworks[0];
        assert_eq!(fw.detectors.every.len(), 1);
        assert_eq!(fw.detectors.some.len(), 2);

        let every = &fw.detectors.every[0];
        assert_eq!(every.path.as_deref(), Some("Dockerfile"));
        assert!(!every.is_glob.unwrap_or(false));
        assert_eq!(every.match_content.as_deref(), Some("FROM node"));

        let some_glob = &fw.detectors.some[0];
        assert_eq!(some_glob.path.as_deref(), Some("src/**/*.ts"));
        assert!(some_glob.is_glob.unwrap_or(false));
        assert_eq!(some_glob.match_content.as_deref(), Some("express"));

        let some_pkg = &fw.detectors.some[1];
        assert_eq!(some_pkg.match_package.as_deref(), Some("express"));
    }
}
