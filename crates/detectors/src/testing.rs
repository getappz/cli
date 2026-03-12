//! Testing framework detection for skills recommendations.

use crate::filesystem::DetectorFilesystem;
use std::collections::HashMap;
use std::sync::Arc;

struct TestingPattern {
    name: &'static str,
    config_files: &'static [&'static str],
    files: &'static [&'static str],
    dependencies: &'static [&'static str],
}

const TESTING_PATTERNS: &[TestingPattern] = &[
    TestingPattern {
        name: "vitest",
        config_files: &["vitest.config.ts", "vitest.config.js", "vitest.config.mjs"],
        files: &[],
        dependencies: &["vitest"],
    },
    TestingPattern {
        name: "jest",
        config_files: &["jest.config.js", "jest.config.ts", "jest.config.json"],
        files: &[],
        dependencies: &["jest"],
    },
    TestingPattern {
        name: "mocha",
        config_files: &[".mocharc.js", ".mocharc.json", ".mocharc.yaml"],
        files: &[],
        dependencies: &["mocha"],
    },
    TestingPattern {
        name: "ava",
        config_files: &[],
        files: &[],
        dependencies: &["ava"],
    },
    TestingPattern {
        name: "tap",
        config_files: &[],
        files: &[],
        dependencies: &["tap"],
    },
    TestingPattern {
        name: "playwright",
        config_files: &["playwright.config.ts", "playwright.config.js"],
        files: &[],
        dependencies: &["@playwright/test", "playwright"],
    },
    TestingPattern {
        name: "cypress",
        config_files: &["cypress.config.ts", "cypress.config.js", "cypress.json"],
        files: &[],
        dependencies: &["cypress"],
    },
    TestingPattern {
        name: "puppeteer",
        config_files: &[],
        files: &[],
        dependencies: &["puppeteer"],
    },
    TestingPattern {
        name: "selenium",
        config_files: &[],
        files: &[],
        dependencies: &["selenium-webdriver"],
    },
    TestingPattern {
        name: "testing-library",
        config_files: &[],
        files: &[],
        dependencies: &[
            "@testing-library/react",
            "@testing-library/vue",
            "@testing-library/svelte",
            "@testing-library/dom",
        ],
    },
    TestingPattern {
        name: "enzyme",
        config_files: &[],
        files: &[],
        dependencies: &["enzyme"],
    },
    TestingPattern {
        name: "pytest",
        config_files: &["pytest.ini", "pyproject.toml"],
        files: &["tests", "test"],
        dependencies: &[],
    },
    TestingPattern {
        name: "rspec",
        config_files: &[],
        files: &["spec", ".rspec"],
        dependencies: &[],
    },
    TestingPattern {
        name: "minitest",
        config_files: &[],
        files: &["test"],
        dependencies: &[],
    },
];

/// Detect testing frameworks in the project.
pub async fn detect_testing(
    fs: &Arc<dyn DetectorFilesystem>,
    all_dependencies: &HashMap<String, String>,
) -> Vec<String> {
    let mut detected = Vec::new();

    for pattern in TESTING_PATTERNS {
        if matches_testing_pattern(fs, all_dependencies, pattern).await {
            detected.push(pattern.name.to_string());
        }
    }

    detected
}

async fn matches_testing_pattern(
    fs: &Arc<dyn DetectorFilesystem>,
    all_dependencies: &HashMap<String, String>,
    pattern: &TestingPattern,
) -> bool {
    for file in pattern.config_files {
        if fs.has_path(file).await {
            return true;
        }
    }

    for file in pattern.files {
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
