//! Remove pack scripts and rules from project.

use std::path::Path;

use miette::IntoDiagnostic;

pub fn uninstall(cwd: &Path) -> miette::Result<()> {
    // Remove .cursor/rules/pack/ and .cursor/rules/hypermix/
    for dir in [".cursor/rules/pack", ".cursor/rules/hypermix"] {
        let rule_dir = cwd.join(dir);
        if rule_dir.exists() {
            std::fs::remove_dir_all(&rule_dir).into_diagnostic()?;
        }
    }

    // Remove pack script from package.json
    let pkg_path = cwd.join("package.json");
    if pkg_path.exists() {
        let content = std::fs::read_to_string(&pkg_path).into_diagnostic()?;
        if let Ok(mut pkg) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(scripts) = pkg.get_mut("scripts").and_then(|s| s.as_object_mut()) {
                scripts.remove("pack");
                std::fs::write(&pkg_path, serde_json::to_string_pretty(&pkg).into_diagnostic()?)
                    .into_diagnostic()?;
            }
        }
    }

    // Remove pack task from deno.json
    for name in ["deno.json", "deno.jsonc"] {
        let path = cwd.join(name);
        if path.exists() {
            let content = std::fs::read_to_string(&path).into_diagnostic()?;
            let mut deno: serde_json::Value = if path.extension().map_or(false, |e| e == "jsonc") {
                let stripped = json_comments::StripComments::new(content.as_bytes());
                serde_json::from_reader(stripped)
                    .map_err(|_| miette::miette!("Invalid JSONC"))?
            } else {
                serde_json::from_str(&content).into_diagnostic()?
            };
            if let Some(tasks) = deno.get_mut("tasks").and_then(|t| t.as_object_mut()) {
                tasks.remove("pack");
                std::fs::write(&path, serde_json::to_string_pretty(&deno).into_diagnostic()?)
                    .into_diagnostic()?;
            }
        }
    }

    Ok(())
}
