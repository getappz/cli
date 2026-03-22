use crate::{
    log::{info, warning},
    shell,
};

/// Run the provided installer if `ddev` is not found.
/// After running, treat installation as success if `ddev` is now on PATH,
/// even when the installer returned an error (e.g., already installed).
pub fn ensure_ddev<F>(install_fn: F) -> miette::Result<()>
where
    F: FnOnce() -> miette::Result<()>,
{
    if shell::command_exists("ddev") {
        info("ddev is already installed");
        // Show version if available
        let _ = shell::run_local("ddev --version");
        return Ok(());
    }

    info("ddev not found, attempting to install...");
    let res = install_fn();

    // Check if installation succeeded even if command returned error
    if shell::command_exists("ddev") {
        info("ddev installation successful!");
        let _ = shell::run_local("ddev --version");
        return Ok(());
    }

    // If we get here, installation failed
    if let Err(e) = &res {
        warning(&format!("Installation attempt failed: {}", e));
    }
    res
}

/// Ensures mkcert is installed.
/// Returns Ok(()) if mkcert is already installed or successfully installed.
pub fn ensure_mkcert_installed() -> miette::Result<()> {
    if shell::command_exists("mkcert") {
        return Ok(());
    }
    let os = std::env::consts::OS;
    #[cfg(target_os = "macos")]
    {
        if os == "macos" && shell::command_exists("brew") {
            info("Installing mkcert via Homebrew");
            shell::run_local("brew install mkcert")
                .map_err(|e| miette::miette!("Failed to install mkcert: {}", e))?;
        }
    }
    #[cfg(target_os = "linux")]
    {
        if os == "linux" {
            if shell::command_exists("apt-get") || shell::command_exists("apt") {
                shell::run_local("sudo apt-get install -y mkcert")
                    .map_err(|e| miette::miette!("Failed to install mkcert: {}", e))?;
            } else if shell::command_exists("dnf") {
                shell::run_local("sudo dnf install -y mkcert")
                    .map_err(|e| miette::miette!("Failed to install mkcert: {}", e))?;
            } else if shell::command_exists("yay") {
                shell::run_local("yay -S --noconfirm mkcert")
                    .map_err(|e| miette::miette!("Failed to install mkcert: {}", e))?;
            } else if shell::command_exists("paru") {
                shell::run_local("paru -S --noconfirm mkcert")
                    .map_err(|e| miette::miette!("Failed to install mkcert: {}", e))?;
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if os == "windows" {
            if shell::command_exists("winget") {
                shell::run_local("winget install --id Filippo.Mkcert -e --accept-source-agreements --accept-package-agreements")
                    .map_err(|e| miette::miette!("Failed to install mkcert: {}", e))?;
            } else if shell::command_exists("scoop") {
                shell::run_local("scoop install mkcert")
                    .map_err(|e| miette::miette!("Failed to install mkcert: {}", e))?;
            }
        }
    }
    Ok(())
}

/// Installs mkcert certificate authority.
pub fn install_mkcert_ca() -> miette::Result<()> {
    if shell::command_exists("mkcert") {
        info("Installing mkcert certificate authority");
        shell::run_local("mkcert -install")
            .map_err(|e| miette::miette!("Failed to install mkcert CA: {}", e))?;
        Ok(())
    } else {
        Err(miette::miette!("mkcert is not installed"))
    }
}

/// Ensures mkcert is installed and CA is configured.
pub fn ensure_mkcert_configured() -> miette::Result<()> {
    ensure_mkcert_installed()?;
    install_mkcert_ca()?;
    Ok(())
}

/// Gets the home directory path (works on Unix and Windows).
pub fn get_home_dir() -> Option<std::path::PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(std::path::PathBuf::from)
}
