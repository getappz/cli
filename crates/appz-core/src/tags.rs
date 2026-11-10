//! Project-level tags describing the languages, tools and test setups in use.
//!
//! These are informational: they do not drive installation or deployment, but
//! give agents/skills a cheap, stable summary of a project (e.g. "rust",
//! "docker", "vitest") without re-scanning manifests.

use std::path::Path;

use serde::Serialize;

use crate::fs::DetectorFilesystem;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TagCategory {
    Language,
    Tool,
    Testing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Tag {
    pub id: &'static str,
    pub category: TagCategory,
}

/// Marker files (or directories) that identify a language. A marker may be a
/// glob (contains `*`).
const LANGUAGE_MARKERS: &[(&[&str], &str)] = &[
    (&["Cargo.toml"], "rust"),
    (&["tsconfig.json"], "typescript"),
    (&["package.json"], "javascript"),
    (
        &["pyproject.toml", "requirements.txt", "setup.py", "Pipfile"],
        "python",
    ),
    (&["go.mod"], "go"),
    (&["Gemfile"], "ruby"),
    (&["pom.xml", "build.gradle", "build.gradle.kts"], "java"),
    (&["composer.json"], "php"),
    (&["mix.exs"], "elixir"),
    (&["*.csproj", "*.fsproj"], "dotnet"),
];

/// Marker files/directories that identify a build or infrastructure tool.
const TOOL_MARKERS: &[(&[&str], &str)] = &[
    (
        &[
            "Dockerfile",
            "Containerfile",
            "docker-compose.yml",
            "docker-compose.yaml",
            "compose.yml",
            "compose.yaml",
        ],
        "docker",
    ),
    (&[".github/workflows"], "github-actions"),
    (&[".gitlab-ci.yml"], "gitlab-ci"),
    (&["turbo.json"], "turborepo"),
    (&["nx.json"], "nx"),
    (&["flake.nix", "default.nix"], "nix"),
    (&["Makefile", "makefile", "GNUmakefile"], "make"),
    (&["justfile", "Justfile"], "just"),
    (&["mise.toml", ".mise.toml"], "mise"),
    (&["main.tf", "terraform.tf"], "terraform"),
    (&["Chart.yaml"], "helm"),
    (&["kustomization.yaml", "kustomization.yml"], "kustomize"),
];

/// Manifest + substring pairs that identify a test runner.
const TESTING_MARKERS: &[(&[&str], &str, &str)] = &[
    (&["package.json"], "\"jest\"", "jest"),
    (&["package.json"], "vitest", "vitest"),
    (&["package.json"], "@playwright/test", "playwright"),
    (&["package.json"], "\"cypress\"", "cypress"),
    (&["package.json"], "\"mocha\"", "mocha"),
    (
        &["pyproject.toml", "requirements.txt", "Pipfile"],
        "pytest",
        "pytest",
    ),
    (&["Gemfile"], "rspec", "rspec"),
    (&["composer.json"], "phpunit", "phpunit"),
    (&["Cargo.toml"], "[dev-dependencies]", "cargo-test"),
];

fn marker_present(fs: &DetectorFilesystem, markers: &[&str]) -> bool {
    markers.iter().any(|marker| {
        if marker.contains('*') {
            fs.has_glob(marker)
        } else {
            fs.has_path(marker)
        }
    })
}

/// Detect every tag that applies to `root`.
pub fn detect_tags(root: &Path) -> Vec<Tag> {
    let fs = DetectorFilesystem::new(root.to_path_buf());
    let mut tags = Vec::new();

    for (markers, id) in LANGUAGE_MARKERS {
        if marker_present(&fs, markers) {
            tags.push(Tag {
                id,
                category: TagCategory::Language,
            });
        }
    }
    for (markers, id) in TOOL_MARKERS {
        if marker_present(&fs, markers) {
            tags.push(Tag {
                id,
                category: TagCategory::Tool,
            });
        }
    }
    for (files, needle, id) in TESTING_MARKERS {
        if files
            .iter()
            .filter_map(|f| fs.read_file(f))
            .any(|content| content.contains(needle))
        {
            tags.push(Tag {
                id,
                category: TagCategory::Testing,
            });
        }
    }

    tags
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("appz-tags-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn ids(tags: &[Tag]) -> Vec<&'static str> {
        tags.iter().map(|t| t.id).collect()
    }

    #[test]
    fn detects_languages_from_manifests() {
        let dir = test_dir("languages");
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        fs::write(dir.join("tsconfig.json"), "{}").unwrap();
        fs::write(dir.join("go.mod"), "module x").unwrap();

        let tags = detect_tags(&dir);
        let ids = ids(&tags);
        assert!(ids.contains(&"rust"), "{ids:?}");
        assert!(ids.contains(&"typescript"), "{ids:?}");
        assert!(ids.contains(&"go"), "{ids:?}");
        assert!(tags.iter().all(|t| t.category == TagCategory::Language));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detects_glob_language_marker() {
        let dir = test_dir("dotnet");
        fs::write(dir.join("App.csproj"), "<Project />").unwrap();

        assert!(ids(&detect_tags(&dir)).contains(&"dotnet"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detects_tools_from_files_and_directories() {
        let dir = test_dir("tools");
        fs::write(dir.join("Dockerfile"), "FROM alpine").unwrap();
        fs::write(dir.join("turbo.json"), "{}").unwrap();
        fs::create_dir_all(dir.join(".github").join("workflows")).unwrap();

        let ids = ids(&detect_tags(&dir));
        assert!(ids.contains(&"docker"), "{ids:?}");
        assert!(ids.contains(&"turborepo"), "{ids:?}");
        assert!(ids.contains(&"github-actions"), "{ids:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detects_test_runners_from_manifest_content() {
        let dir = test_dir("testing");
        fs::write(
            dir.join("package.json"),
            r#"{"devDependencies":{"vitest":"^1","@playwright/test":"^1"}}"#,
        )
        .unwrap();
        fs::write(dir.join("pyproject.toml"), "pytest = \"^8\"").unwrap();

        let tags = detect_tags(&dir);
        let ids = ids(&tags);
        assert!(ids.contains(&"vitest"), "{ids:?}");
        assert!(ids.contains(&"playwright"), "{ids:?}");
        assert!(ids.contains(&"pytest"), "{ids:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_project_has_no_tags() {
        let dir = test_dir("empty");
        assert!(detect_tags(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
