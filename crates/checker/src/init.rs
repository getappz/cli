//! Best-practice config file generation for `appz check --init`.
//!
//! Generates starter configuration files for detected frameworks:
//! - JS/TS: `biome.json` with recommended rules
//! - Python: `[tool.ruff]` section in `pyproject.toml`
//! - Rust: `.clippy.toml` if custom config needed
//! - CSS: `.stylelintrc.json`
//! - All: `.gitleaks.toml` baseline

use std::path::Path;

use crate::detect::{analyse_project, DetectedLanguage};
use crate::error::CheckResult;

/// Run the init process: analyse project and generate config files.
pub fn run_init(project_dir: &Path) -> CheckResult<Vec<String>> {
    let analysis = analyse_project(project_dir);
    let mut created = Vec::new();

    // JS/TS: Generate biome.json if not present.
    if analysis.has_js_ts() && !analysis.has_biome_config && !analysis.has_eslint_config {
        let biome_path = project_dir.join("biome.json");
        if !biome_path.exists() {
            std::fs::write(&biome_path, BIOME_CONFIG)?;
            created.push("biome.json".to_string());
        }
    }

    // Python: Add ruff config to pyproject.toml or create ruff.toml.
    if analysis.languages.contains(&DetectedLanguage::Python) {
        let ruff_toml = project_dir.join("ruff.toml");
        let dot_ruff_toml = project_dir.join(".ruff.toml");
        let pyproject = project_dir.join("pyproject.toml");

        if !ruff_toml.exists() && !dot_ruff_toml.exists() {
            if pyproject.exists() {
                // Check if ruff config already exists in pyproject.toml.
                let content = std::fs::read_to_string(&pyproject).unwrap_or_default();
                if !content.contains("[tool.ruff]") {
                    let mut new_content = content;
                    new_content.push_str(RUFF_PYPROJECT_SECTION);
                    std::fs::write(&pyproject, new_content)?;
                    created.push("pyproject.toml (added [tool.ruff])".to_string());
                }
            } else {
                std::fs::write(&ruff_toml, RUFF_CONFIG)?;
                created.push("ruff.toml".to_string());
            }
        }
    }

    // CSS: Generate .stylelintrc.json if CSS files exist but no config.
    if analysis.languages.contains(&DetectedLanguage::Css) && !analysis.has_stylelint_config {
        let stylelint_path = project_dir.join(".stylelintrc.json");
        if !stylelint_path.exists() {
            std::fs::write(&stylelint_path, STYLELINT_CONFIG)?;
            created.push(".stylelintrc.json".to_string());
        }
    }

    // Secret scanning: Generate .gitleaks.toml baseline.
    if analysis.is_git_repo {
        let gitleaks_path = project_dir.join(".gitleaks.toml");
        if !gitleaks_path.exists() {
            std::fs::write(&gitleaks_path, GITLEAKS_CONFIG)?;
            created.push(".gitleaks.toml".to_string());
        }
    }

    Ok(created)
}

// ---------------------------------------------------------------------------
// Config templates
// ---------------------------------------------------------------------------

const BIOME_CONFIG: &str = r#"{
  "$schema": "https://biomejs.dev/schemas/1.9.4/schema.json",
  "vcs": {
    "enabled": true,
    "clientKind": "git",
    "useIgnoreFile": true
  },
  "organizeImports": {
    "enabled": true
  },
  "linter": {
    "enabled": true,
    "rules": {
      "recommended": true,
      "complexity": {
        "noExtraBooleanCast": "error",
        "noMultipleSpacesInRegularExpressionLiterals": "error",
        "noUselessCatch": "error",
        "noUselessTypeConstraint": "error"
      },
      "correctness": {
        "noConstAssign": "error",
        "noConstantCondition": "warn",
        "noEmptyCharacterClassInRegex": "error",
        "noEmptyPattern": "error",
        "noGlobalObjectCalls": "error",
        "noInvalidUseBeforeDeclaration": "error",
        "noUndeclaredVariables": "error",
        "noUnusedImports": "warn",
        "noUnusedVariables": "warn",
        "useArrayLiterals": "error",
        "useExhaustiveDependencies": "warn",
        "useHookAtTopLevel": "error"
      },
      "suspicious": {
        "noDebugger": "error",
        "noDoubleEquals": "warn",
        "noDuplicateCase": "error",
        "noDuplicateObjectKeys": "error",
        "noFallthroughSwitchClause": "error",
        "noRedeclare": "error"
      }
    }
  },
  "formatter": {
    "enabled": true,
    "indentStyle": "space",
    "indentWidth": 2,
    "lineEnding": "lf",
    "lineWidth": 100
  },
  "javascript": {
    "formatter": {
      "quoteStyle": "single",
      "trailingCommas": "all",
      "semicolons": "always"
    }
  }
}
"#;

const RUFF_CONFIG: &str = r#"# Ruff configuration — https://docs.astral.sh/ruff/configuration/
target-version = "py311"
line-length = 100

[lint]
# Enable recommended + additional rules.
select = [
    "E",     # pycodestyle errors
    "W",     # pycodestyle warnings
    "F",     # pyflakes
    "I",     # isort
    "N",     # pep8-naming
    "UP",    # pyupgrade
    "B",     # flake8-bugbear
    "S",     # flake8-bandit (security)
    "C4",    # flake8-comprehensions
    "SIM",   # flake8-simplify
    "TCH",   # flake8-type-checking
    "RUF",   # ruff-specific rules
]
ignore = [
    "E501",  # line-too-long (handled by formatter)
    "S101",  # assert usage (common in tests)
]

[lint.per-file-ignores]
"tests/**/*.py" = ["S101", "S106"]

[format]
quote-style = "double"
indent-style = "space"
"#;

const RUFF_PYPROJECT_SECTION: &str = r#"

[tool.ruff]
target-version = "py311"
line-length = 100

[tool.ruff.lint]
select = ["E", "W", "F", "I", "N", "UP", "B", "S", "C4", "SIM", "TCH", "RUF"]
ignore = ["E501", "S101"]

[tool.ruff.lint.per-file-ignores]
"tests/**/*.py" = ["S101", "S106"]

[tool.ruff.format]
quote-style = "double"
indent-style = "space"
"#;

const STYLELINT_CONFIG: &str = r#"{
  "extends": ["stylelint-config-standard"],
  "rules": {
    "color-no-invalid-hex": true,
    "font-family-no-duplicate-names": true,
    "function-calc-no-unspaced-operator": true,
    "unit-no-unknown": true,
    "declaration-block-no-duplicate-properties": true,
    "selector-pseudo-class-no-unknown": true,
    "selector-pseudo-element-no-unknown": true,
    "selector-type-no-unknown": true,
    "no-duplicate-selectors": true,
    "no-empty-source": true,
    "no-invalid-double-slash-comments": true
  }
}
"#;

const GITLEAKS_CONFIG: &str = r#"# Gitleaks configuration — https://github.com/gitleaks/gitleaks
title = "Gitleaks config"

[allowlist]
description = "Allowlist for known false positives"
paths = [
    '''^\.gitleaks\.toml$''',
    '''(.*?)(png|jpg|gif|svg|ico|woff|woff2|ttf|eot)$''',
    '''node_modules''',
    '''vendor''',
    '''\.lock$''',
    '''pnpm-lock\.yaml$''',
    '''package-lock\.json$''',
]
"#;
