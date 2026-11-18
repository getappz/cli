use std::path::Path;

use crate::detect::{self, DetectedToolchain, MonorepoConfig, detect_monorepo, detect_toolchains};

#[derive(serde::Serialize)]
pub struct DoctorReport {
    pub root: String,
    pub toolchains: Vec<DetectedToolchain>,
    pub monorepo: MonorepoConfig,
    pub package_manager: Option<String>,
    pub has_appz_jsonc: bool,
    pub has_claude_md: bool,
    pub has_agents_md: bool,
    pub has_mise_config: bool,
    pub has_state: bool,
    pub suggestions: Vec<String>,
}

pub fn run_doctor(root: &Path) -> DoctorReport {
    let toolchains = detect_toolchains(root).unwrap_or_default();
    let monorepo = detect_monorepo(root);
    let pm = detect::detect_package_manager(root).map(|s| s.to_string());

    let mut suggestions: Vec<String> = Vec::new();

    for tc in &toolchains {
        if tc.build_command.is_some() || tc.dev_command.is_some() {
            suggestions.push(format!("{} is ready for appz build/dev", tc.name));
        }
        if let Some(out) = tc.output_directory {
            suggestions.push(format!("{} outputs to '{}'", tc.name, out));
        }
    }

    if monorepo.manager.is_some() {
        suggestions.push(format!(
            "monorepo ({}) detected — appz manages per-workspace state",
            monorepo.manager.as_deref().unwrap_or("unknown")
        ));
    }

    if let Some(ref pm) = pm {
        let install_cmd = detect::pm_install_cmd(pm);
        suggestions.push(format!(
            "package manager: {} — install via `{}`",
            pm, install_cmd
        ));
    }

    if root.join("package.json").exists() {
        suggestions.push("run `appz install` to set up mise tools + dependencies".to_string());
    }

    DoctorReport {
        root: root.to_string_lossy().to_string(),
        toolchains,
        monorepo,
        package_manager: pm,
        has_appz_jsonc: root.join("appz.jsonc").exists(),
        has_claude_md: root.join("CLAUDE.md").exists(),
        has_agents_md: root.join("AGENTS.md").exists(),
        has_mise_config: crate::storage::mise_config_path(root).exists(),
        has_state: crate::storage::state_path(root).exists(),
        suggestions,
    }
}
