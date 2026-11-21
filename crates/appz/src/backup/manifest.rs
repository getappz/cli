//! Snapshot manifest (`appz-manifest.json`) and rustic snapshot tags.
//!
//! Only variable **names** (`env_names`, `build_env_names`) are recorded; values
//! are never stored in manifests or tags.

// Consumed by upcoming backup tasks (create, list, restore).
#![allow(dead_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::select::GitInfo;

/// Filename written into each staged snapshot root.
pub const MANIFEST_NAME: &str = "appz-manifest.json";

/// Metadata stored alongside snapshotted source in `appz-manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub appz_version: String,
    pub created_at: String,
    pub target: Option<String>,
    pub environment: Option<String>,
    pub url: Option<String>,
    pub fingerprint: String,
    pub git: Option<GitInfo>,
    pub tools: BTreeMap<String, String>,
    pub env_names: Vec<String>,
    pub build_env_names: Vec<String>,
    pub files: Vec<String>,
    #[serde(default)]
    pub deleted: super::select::DeletedPaths,
    pub has_bundle: bool,
}

/// Build rustic `--tag` values for a snapshot from `Manifest`, omitting absent fields.
pub fn tags(m: &Manifest) -> Vec<String> {
    let mut out = vec!["appz".to_string()];
    if let Some(target) = m.target.as_deref().filter(|s| !s.is_empty()) {
        out.push(format!("target:{target}"));
    }
    if let Some(env) = m.environment.as_deref().filter(|s| !s.is_empty()) {
        out.push(format!("env:{env}"));
    }
    if let Some(git) = &m.git
        && !git.sha.is_empty()
    {
        out.push(format!("commit:{}", git.sha));
    }
    if !m.fingerprint.is_empty() {
        out.push(format!("fp:{}", m.fingerprint));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> Manifest {
        Manifest {
            appz_version: "0.3.3".to_string(),
            created_at: "2026-11-20T00:00:00Z".to_string(),
            target: Some("vercel".to_string()),
            environment: Some("production".to_string()),
            url: Some("https://app.example.com".to_string()),
            fingerprint: "123".to_string(),
            git: Some(GitInfo {
                sha: "abc".to_string(),
                branch: Some("main".to_string()),
                remote: Some("origin".to_string()),
                ahead: 0,
                prefix: String::new(),
            }),
            tools: BTreeMap::from([("node".to_string(), "22.0.0".to_string())]),
            env_names: vec!["API_KEY".to_string(), "DATABASE_URL".to_string()],
            build_env_names: vec!["PUBLIC_URL".to_string()],
            files: vec!["src/main.rs".to_string()],
            deleted: Default::default(),
            has_bundle: false,
        }
    }

    #[test]
    fn tags_follow_the_spec() {
        let m = sample_manifest();
        assert_eq!(
            tags(&m),
            vec![
                "appz",
                "target:vercel",
                "env:production",
                "commit:abc",
                "fp:123",
            ]
        );

        let minimal = Manifest {
            target: None,
            environment: None,
            git: None,
            fingerprint: String::new(),
            ..m
        };
        assert_eq!(tags(&minimal), vec!["appz"]);
    }

    #[test]
    fn reads_manifests_written_before_deletions_were_recorded() {
        let original = sample_manifest();
        let mut json = serde_json::to_value(&original).unwrap();
        json.as_object_mut().unwrap().remove("deleted");
        assert_eq!(serde_json::from_value::<Manifest>(json).unwrap(), original);
    }

    #[test]
    fn manifest_never_contains_env_values() {
        let m = sample_manifest();
        let json = serde_json::to_string(&m).unwrap();
        assert!(!json.contains('='));
        assert!(json.contains("API_KEY"));
        assert!(json.contains("DATABASE_URL"));
        assert!(json.contains("PUBLIC_URL"));
        let round_trip: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(round_trip, m);
    }
}
