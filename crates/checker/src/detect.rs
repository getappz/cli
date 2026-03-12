//! Language and framework detection for provider selection.
//!
//! Uses file-existence heuristics and the `frameworks` crate to determine
//! which check providers should run on a project. Multiple providers can
//! match simultaneously (e.g. a Next.js project gets Biome + tsc + gitleaks).

use std::path::Path;

/// Detected language/stack in the project.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DetectedLanguage {
    JavaScript,
    TypeScript,
    Python,
    Rust,
    Php,
    Css,
}

/// Result of project analysis for checker detection.
#[derive(Debug, Clone)]
pub struct ProjectAnalysis {
    /// Languages detected in the project.
    pub languages: Vec<DetectedLanguage>,
    /// Framework slugs from the `frameworks` crate.
    pub frameworks: Vec<String>,
    /// Whether the project has an existing Biome config.
    pub has_biome_config: bool,
    /// Whether the project has an existing ESLint config.
    pub has_eslint_config: bool,
    /// Whether the project has a tsconfig.json.
    pub has_tsconfig: bool,
    /// Whether the project has Stylelint config.
    pub has_stylelint_config: bool,
    /// Whether the project is a git repository.
    pub is_git_repo: bool,
}

impl ProjectAnalysis {
    /// Whether any JS/TS language was detected.
    pub fn has_js_ts(&self) -> bool {
        self.languages.contains(&DetectedLanguage::JavaScript)
            || self.languages.contains(&DetectedLanguage::TypeScript)
    }
}

/// Analyse a project directory to determine languages and tooling.
///
/// This is a fast, synchronous scan using file-existence checks.
/// It does NOT read file contents — just checks for marker files.
pub fn analyse_project(project_dir: &Path) -> ProjectAnalysis {
    let mut languages = Vec::new();
    let mut frameworks = Vec::new();

    // -- JavaScript / TypeScript --
    let has_package_json = project_dir.join("package.json").exists();
    let has_tsconfig = project_dir.join("tsconfig.json").exists();
    let has_jsconfig = project_dir.join("jsconfig.json").exists();

    if has_package_json || has_jsconfig {
        languages.push(DetectedLanguage::JavaScript);
    }
    if has_tsconfig {
        languages.push(DetectedLanguage::TypeScript);
    }

    // -- Python --
    let has_pyproject = project_dir.join("pyproject.toml").exists();
    let has_requirements = project_dir.join("requirements.txt").exists();
    let has_setup_py = project_dir.join("setup.py").exists();
    let has_pipfile = project_dir.join("Pipfile").exists();

    if has_pyproject || has_requirements || has_setup_py || has_pipfile {
        languages.push(DetectedLanguage::Python);
    }

    // -- Rust --
    let has_cargo_toml = project_dir.join("Cargo.toml").exists();
    if has_cargo_toml {
        languages.push(DetectedLanguage::Rust);
    }

    // -- PHP --
    let has_composer = project_dir.join("composer.json").exists();
    if has_composer {
        languages.push(DetectedLanguage::Php);
    }

    // -- CSS/SCSS --
    // Check for common CSS indicators beyond just having a package.json.
    let has_css_files = project_dir.join("src").exists(); // Rough heuristic; CSS exists in JS projects.
    let has_stylelint_config = project_dir.join(".stylelintrc").exists()
        || project_dir.join(".stylelintrc.json").exists()
        || project_dir.join(".stylelintrc.yml").exists()
        || project_dir.join("stylelint.config.js").exists()
        || project_dir.join("stylelint.config.mjs").exists()
        || project_dir.join("stylelint.config.cjs").exists();
    if has_stylelint_config || (has_package_json && has_css_files) {
        languages.push(DetectedLanguage::Css);
    }

    // -- Tool configs --
    let has_biome_config = project_dir.join("biome.json").exists()
        || project_dir.join("biome.jsonc").exists();

    let has_eslint_config = project_dir.join(".eslintrc").exists()
        || project_dir.join(".eslintrc.js").exists()
        || project_dir.join(".eslintrc.cjs").exists()
        || project_dir.join(".eslintrc.json").exists()
        || project_dir.join(".eslintrc.yml").exists()
        || project_dir.join("eslint.config.js").exists()
        || project_dir.join("eslint.config.mjs").exists()
        || project_dir.join("eslint.config.cjs").exists()
        || project_dir.join("eslint.config.ts").exists();

    // -- Git --
    let is_git_repo = project_dir.join(".git").exists();

    // -- Framework detection via frameworks crate --
    let framework_list = frameworks::frameworks();
    for fw in framework_list {
        // Simple slug match: if the framework has detectors and the project
        // has a package.json, check for the dependency name.
        if let Some(slug) = fw.slug {
            if let Some(detectors) = &fw.detectors {
                let mut matches = false;

                // Check "every" detectors.
                if let Some(every) = detectors.every {
                    matches = every.iter().all(|d| {
                        use frameworks::DetectorStatic;
                        match d {
                            DetectorStatic::MatchPackage { match_package } => {
                                has_package_json
                                    && check_package_dependency(project_dir, match_package)
                            }
                            DetectorStatic::Path { path } => project_dir.join(path).exists(),
                            DetectorStatic::MatchContent { .. } => false, // Skip content matching for speed.
                            DetectorStatic::MatchComposerPackage {
                                match_composer_package,
                            } => {
                                has_composer
                                    && check_composer_dependency(
                                        project_dir,
                                        match_composer_package,
                                    )
                            }
                        }
                    });
                }

                // Check "some" detectors.
                if !matches {
                    if let Some(some) = detectors.some {
                        matches = some.iter().any(|d| {
                            use frameworks::DetectorStatic;
                            match d {
                                DetectorStatic::MatchPackage { match_package } => {
                                    has_package_json
                                        && check_package_dependency(project_dir, match_package)
                                }
                                DetectorStatic::Path { path } => {
                                    project_dir.join(path).exists()
                                }
                                DetectorStatic::MatchContent { .. } => false,
                                DetectorStatic::MatchComposerPackage {
                                    match_composer_package,
                                } => {
                                    has_composer
                                        && check_composer_dependency(
                                            project_dir,
                                            match_composer_package,
                                        )
                                }
                            }
                        });
                    }
                }

                if matches {
                    frameworks.push(slug.to_string());
                }
            }
        }
    }

    ProjectAnalysis {
        languages,
        frameworks,
        has_biome_config,
        has_eslint_config,
        has_tsconfig,
        has_stylelint_config,
        is_git_repo,
    }
}

/// Check if a package exists in package.json dependencies.
///
/// Fast check: reads the file and does a simple string search.
fn check_package_dependency(project_dir: &Path, package_name: &str) -> bool {
    let pkg_path = project_dir.join("package.json");
    if let Ok(content) = std::fs::read_to_string(pkg_path) {
        // Simple string search — faster than full JSON parsing.
        content.contains(&format!("\"{}\"", package_name))
    } else {
        false
    }
}

/// Check if a package exists in composer.json require/require-dev.
fn check_composer_dependency(project_dir: &Path, package_name: &str) -> bool {
    let composer_path = project_dir.join("composer.json");
    if let Ok(content) = std::fs::read_to_string(composer_path) {
        content.contains(&format!("\"{}\"", package_name))
    } else {
        false
    }
}
