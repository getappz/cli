//! Language detection for skills recommendations.

use crate::filesystem::DetectorFilesystem;
use std::collections::HashMap;
use std::sync::Arc;

struct LanguagePattern {
    name: &'static str,
    files: &'static [&'static str],
    dependencies: &'static [&'static str],
}

const LANGUAGE_PATTERNS: &[LanguagePattern] = &[
    LanguagePattern {
        name: "typescript",
        files: &["tsconfig.json", "tsconfig.base.json"],
        dependencies: &["typescript"],
    },
    LanguagePattern {
        name: "javascript",
        files: &["package.json", "jsconfig.json"],
        dependencies: &[],
    },
    LanguagePattern {
        name: "python",
        files: &["requirements.txt", "pyproject.toml", "setup.py", "Pipfile", "poetry.lock"],
        dependencies: &[],
    },
    LanguagePattern {
        name: "ruby",
        files: &["Gemfile", "Gemfile.lock", ".ruby-version"],
        dependencies: &[],
    },
    LanguagePattern {
        name: "go",
        files: &["go.mod", "go.sum"],
        dependencies: &[],
    },
    LanguagePattern {
        name: "rust",
        files: &["Cargo.toml", "Cargo.lock"],
        dependencies: &[],
    },
    LanguagePattern {
        name: "java",
        files: &["pom.xml", "build.gradle"],
        dependencies: &[],
    },
    LanguagePattern {
        name: "kotlin",
        files: &["build.gradle.kts"],
        dependencies: &[],
    },
    LanguagePattern {
        name: "php",
        files: &["composer.json", "composer.lock"],
        dependencies: &[],
    },
    LanguagePattern {
        name: "elixir",
        files: &["mix.exs", "mix.lock"],
        dependencies: &[],
    },
    LanguagePattern {
        name: "scala",
        files: &["build.sbt"],
        dependencies: &[],
    },
    LanguagePattern {
        name: "clojure",
        files: &["project.clj", "deps.edn"],
        dependencies: &[],
    },
    LanguagePattern {
        name: "haskell",
        files: &["stack.yaml", "cabal.project"],
        dependencies: &[],
    },
    LanguagePattern {
        name: "zig",
        files: &["build.zig"],
        dependencies: &[],
    },
];

/// Detect programming languages in the project.
pub async fn detect_languages(
    fs: &Arc<dyn DetectorFilesystem>,
    all_dependencies: &HashMap<String, String>,
) -> Vec<String> {
    let mut detected = Vec::new();

    for pattern in LANGUAGE_PATTERNS {
        if matches_language_pattern(fs, all_dependencies, pattern).await {
            detected.push(pattern.name.to_string());
        }
    }

    detected
}

async fn matches_language_pattern(
    fs: &Arc<dyn DetectorFilesystem>,
    all_dependencies: &HashMap<String, String>,
    pattern: &LanguagePattern,
) -> bool {
    for file in pattern.files {
        if file.contains('*') {
            continue;
        }
        if fs.has_path(file).await {
            return true;
        }
    }

    for dep in pattern.dependencies {
        if all_dependencies.contains_key(*dep) {
            return true;
        }
    }

    false
}
