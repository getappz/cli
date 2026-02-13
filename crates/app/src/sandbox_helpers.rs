//! Shared helpers for sandbox-based command execution (init, dev, build, etc.).

use crate::detectors::PackageManagerInfo;
use crate::shell::ToolVersionInfo;
use sandbox::SandboxSettings;

/// Build sandbox settings for running commands in a project.
///
/// Maps package manager and optional framework-specific tools (e.g. Hugo) to
/// mise tool specs for consistent execution.
pub fn mise_tools_for_execution(
    pm: &Option<PackageManagerInfo>,
    tool_info: Option<&ToolVersionInfo>,
) -> SandboxSettings {
    let mut s = SandboxSettings::default();

    // Package manager tools (node, pnpm, yarn, bun)
    match pm.as_ref().map(|p| p.manager.as_str()) {
        Some("bun") => s = s.with_tool("bun", None::<String>),
        Some("pnpm") => s = s.with_tool("node", Some("22")).with_tool("pnpm", None::<String>),
        Some("yarn") => s = s.with_tool("node", Some("22")).with_tool("yarn", None::<String>),
        _ => s = s.with_tool("node", Some("22")), // npm or unknown
    }

    // Framework-specific tools (e.g. Hugo extended)
    if let Some(info) = tool_info {
        let version = info.version.as_deref().unwrap_or("latest");
        let mise_version = if info.extended {
            format!("extended_{}", version)
        } else {
            version.to_string()
        };
        s = s.with_tool(&info.tool, Some(mise_version));
    }

    s
}
