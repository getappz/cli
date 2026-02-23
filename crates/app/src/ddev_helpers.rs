//! DDEV integration — PHP/CMS local development via Docker.
//!
//! DDEV supports WordPress, Drupal, Laravel, TYPO3, Backdrop, CakePHP, Magento,
//! Symfony, CodeIgniter, and generic PHP projects. See
//! <https://docs.ddev.com/en/stable/users/quickstart/>.

use std::path::Path;

use crate::shell::command_exists;

/// DDEV web container name from project config (e.g. `ddev-wp-demo-web`).
/// Falls back to directory name when `name` is not in config.
pub fn ddev_web_container_name(project_path: &Path) -> Option<String> {
    let config_path = project_path.join(".ddev").join("config.yaml");
    if !config_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&config_path).ok()?;
    let config: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
    let name = config
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| project_path.file_name().and_then(|n| n.to_str()).map(str::to_string))?;
    Some(format!("ddev-{}-web", name))
}

/// DDEV project type for `ddev config --project-type=X`.
/// Maps framework slugs to DDEV's --project-type values.
const DDEV_PROJECT_TYPES: &[(&str, &str, Option<&str>)] = &[
    ("wordpress", "wordpress", None),
    ("drupal", "drupal10", None),
    ("drupal9", "drupal9", None),
    ("drupal10", "drupal10", None),
    ("laravel", "laravel", Some("public")),
    ("typo3", "typo3", Some("public")),
    ("backdrop", "backdrop", None),
    ("cakephp", "cakephp", Some("webroot")),
    ("magento", "magento2", None),
    ("magento2", "magento2", None),
    ("symfony", "symfony", Some("public")),
    ("codeigniter", "codeigniter", Some("public")),
    // Generic PHP (Jigsaw, Sculpin, Spress, Kirby, Statamic, etc.)
    ("jigsaw", "php", Some("build_local")),
    ("sculpin", "php", Some("output_prod")),
    ("spress", "php", Some("build")),
    ("kirby", "php", None),
    ("statamic", "php", Some("public")),
];

/// Check if DDEV is available on the system PATH.
pub fn is_ddev_available() -> bool {
    command_exists("ddev")
}

/// Return the DDEV project type and optional docroot for a framework slug.
pub fn ddev_project_type_for_framework(slug: &str) -> Option<(/* project_type */ &'static str, Option<&'static str>)> {
    DDEV_PROJECT_TYPES
        .iter()
        .find(|(s, _, _)| *s == slug)
        .map(|(_, pt, docroot)| (*pt, *docroot))
}

/// Check if the given framework slug is supported by DDEV.
pub fn is_ddev_supported_framework(slug: &str) -> bool {
    ddev_project_type_for_framework(slug).is_some()
}

/// Check if the project has an existing DDEV configuration.
pub fn has_ddev_config(project_path: &Path) -> bool {
    project_path.join(".ddev").join("config.yaml").exists()
}

/// Build the `ddev config` command for a framework.
/// Returns e.g. `ddev config --project-type=wordpress` or
/// `ddev config --project-type=php --docroot=build_local`.
pub fn ddev_config_command(project_type: &str, docroot: Option<&str>) -> String {
    let mut cmd = format!("ddev config --project-type={}", project_type);
    if let Some(dr) = docroot {
        cmd.push_str(&format!(" --docroot={}", dr));
    }
    cmd
}
