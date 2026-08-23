use std::path::Path;

/// Check if `mise` is on PATH. Returns true if available.
pub fn mise_on_path() -> bool {
    command::Command::new("mise")
        .arg("--version")
        .without_shell()
        .set_error_on_nonzero(false)
        .exec()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Install mise using the platform-native method.
/// Windows → winget, macOS → brew, Linux → curl | sh
pub fn install_mise() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if command::Command::new("winget")
            .args(["install", "jdx.mise"])
            .without_shell()
            .exec_interactive()
            .map_err(|e| format!("failed to launch winget: {e}"))?
            .success()
        {
            return Ok(());
        }
        if command::interrupted() {
            return Err("mise install cancelled".to_string());
        }
        if command::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "irm https://mise.jdx.dev/install.ps1 | iex",
            ])
            .without_shell()
            .exec_interactive()
            .map_err(|e| format!("failed to launch powershell: {e}"))?
            .success()
        {
            return Ok(());
        }
        Err("winget and PowerShell install both failed".to_string())
    }

    #[cfg(target_os = "macos")]
    {
        if command::Command::new("brew")
            .args(["install", "mise"])
            .without_shell()
            .exec_interactive()
            .map_err(|e| format!("failed to launch brew: {e}"))?
            .success()
        {
            return Ok(());
        }
        if command::Command::new("sh")
            .args(["-c", "curl -fsSL https://mise.jdx.dev/install.sh | sh"])
            .without_shell()
            .exec_interactive()
            .map_err(|e| format!("failed to run curl install: {e}"))?
            .success()
        {
            return Ok(());
        }
        Err("brew and curl install both failed".to_string())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if command::Command::new("sh")
            .args(["-c", "curl -fsSL https://mise.jdx.dev/install.sh | sh"])
            .without_shell()
            .exec_interactive()
            .map_err(|e| format!("failed to run curl install: {e}"))?
            .success()
        {
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
    command::Command::new("mise")
        .arg("trust")
        .cwd(root)
        .without_shell()
        .run()
        .map_err(|e| format!("failed to run 'mise trust': {e}"))
}
