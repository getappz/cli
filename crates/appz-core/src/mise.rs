use std::path::Path;
use std::process::Command;

/// Check if `mise` is on PATH. Returns true if available.
pub fn mise_on_path() -> bool {
    Command::new("mise")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Install mise using the platform-native method.
/// Windows → winget, macOS → brew, Linux → curl | sh
pub fn install_mise() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        // winget is native on Win11+ / Win10 with App Installer
        let winget = Command::new("winget")
            .args(["install", "jdx.mise"])
            .status()
            .map_err(|e| format!("failed to launch winget: {e}"))?;
        if winget.success() {
            return Ok(());
        }
        // Fallback: PowerShell script
        let ps = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "irm https://mise.jdx.dev/install.ps1 | iex",
            ])
            .status()
            .map_err(|e| format!("failed to launch powershell: {e}"))?;
        if ps.success() {
            return Ok(());
        }
        Err("winget and PowerShell install both failed".to_string())
    }

    #[cfg(target_os = "macos")]
    {
        let brew = Command::new("brew")
            .args(["install", "mise"])
            .status()
            .map_err(|e| format!("failed to launch brew: {e}"))?;
        if brew.success() {
            return Ok(());
        }
        // Fallback: curl script
        let curl = Command::new("sh")
            .args(["-c", "curl -fsSL https://mise.jdx.dev/install.sh | sh"])
            .status()
            .map_err(|e| format!("failed to run curl install: {e}"))?;
        if curl.success() {
            return Ok(());
        }
        Err("brew and curl install both failed".to_string())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let curl = Command::new("sh")
            .args(["-c", "curl -fsSL https://mise.jdx.dev/install.sh | sh"])
            .status()
            .map_err(|e| format!("failed to run curl install: {e}"))?;
        if curl.success() {
            return Ok(());
        }
        Err("mise install failed — install manually from https://mise.jdx.dev".to_string())
    }
}

/// Ensure mise is installed, installing if missing.
/// Returns Ok if available after check/install.
pub fn ensure_mise(_root: &Path) -> Result<(), String> {
    if mise_on_path() {
        return Ok(());
    }

    eprintln!("mise not found — installing via platform-native method...");
    install_mise()?;

    // Verify install succeeded
    if mise_on_path() {
        Ok(())
    } else {
        Err("mise installed but not on PATH — restart your terminal or add it manually".to_string())
    }
}

/// Mark a project's mise.toml as trusted so mise will auto-activate its
/// tool versions. Best-effort — appz generated the file itself, so a
/// failure here is a warning for the caller to surface, not a hard error.
pub fn trust(root: &Path) -> Result<(), String> {
    let status = Command::new("mise")
        .arg("trust")
        .current_dir(root)
        .status()
        .map_err(|e| format!("failed to run 'mise trust': {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("'mise trust' exited with status {status}"))
    }
}
