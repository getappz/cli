use super::common::{ensure_ddev, ensure_mkcert_configured};
use crate::{
    log::{info, warning},
    shell,
};

pub fn install_unix() -> miette::Result<()> {
    // macOS: Prefer Homebrew
    #[cfg(target_os = "macos")]
    {
        if shell::command_exists("brew") {
            info("Installing ddev via Homebrew");
            let res = shell::run_local("brew install ddev/ddev/ddev");
            if res.is_ok() || shell::command_exists("ddev") {
                let _ = ensure_mkcert_configured();
                return Ok(());
            }
        }
    }

    // Linux: Debian/Ubuntu via apt
    if shell::command_exists("apt-get") || shell::command_exists("apt") {
        info("Installing ddev via apt repository");
        // Add DDEV's GPG key and repository (order matters)
        let _ = shell::run_local("sudo install -m 0755 -d /etc/apt/keyrings");
        let _ = shell::run_local("curl -fsSL https://pkg.ddev.com/apt/gpg.key | gpg --dearmor | sudo tee /etc/apt/keyrings/ddev.gpg > /dev/null");
        let _ = shell::run_local("sudo chmod a+r /etc/apt/keyrings/ddev.gpg");
        let _ = shell::run_local("echo \"deb [signed-by=/etc/apt/keyrings/ddev.gpg] https://pkg.ddev.com/apt/ * *\" | sudo tee /etc/apt/sources.list.d/ddev.list >/dev/null");
        if shell::run_local("sudo apt-get update && sudo apt-get install -y ddev").is_ok()
            || shell::command_exists("ddev")
        {
            let _ = ensure_mkcert_configured();
            return Ok(());
        }
    }

    // Linux: Fedora/RedHat via dnf/yum
    if shell::command_exists("dnf") {
        info("Installing ddev via dnf repository");
        let _ = shell::run_local(
			"echo '[ddev]\nname=ddev\nbaseurl=https://pkg.ddev.com/yum/\ngpgcheck=0\nenabled=1' | sudo tee /etc/yum.repos.d/ddev.repo >/dev/null",
		);
        if shell::run_local("sudo dnf install --refresh ddev").is_ok()
            || shell::command_exists("ddev")
        {
            let _ = ensure_mkcert_configured();
            return Ok(());
        }
    }

    // Linux: Arch via AUR (yay, paru, etc.) or pacman
    if shell::command_exists("pacman") {
        info("Installing ddev via AUR/pacman");
        // Try yay first (most common AUR helper)
        if shell::command_exists("yay") {
            let _ = shell::run_local("yay -S --noconfirm ddev-bin");
            if shell::command_exists("ddev") {
                let _ = ensure_mkcert_configured();
                return Ok(());
            }
        }
        // Try paru
        if shell::command_exists("paru") {
            let _ = shell::run_local("paru -S --noconfirm ddev-bin");
            if shell::command_exists("ddev") {
                let _ = ensure_mkcert_configured();
                return Ok(());
            }
        }
    }

    // Fallback: Official install script (works on all Unix systems)
    info("Installing ddev via official installer script");
    if shell::run_local("curl -fsSL https://ddev.com/install.sh | bash").is_ok()
        || shell::command_exists("ddev")
    {
        // Note: mkcert installation is handled separately via tools:ddev:install_mkcert task
        // The install script may include mkcert, but we don't force it here
        if shell::command_exists("mkcert") {
            // Run mkcert -install if available
            let _ = shell::run_local("mkcert -install");
        }
        return Ok(());
    }

    Err(miette::miette!(
        "Failed to install ddev via any supported method"
    ))
}

pub fn install_windows() -> miette::Result<()> {
    // Windows: Try scoop first (more common for dev tools), then winget, then WSL2 guidance
    if shell::command_exists("scoop") {
        info("Attempting to install ddev via scoop...");
        let res = shell::run_local("scoop install ddev");
        if res.is_ok() || shell::command_exists("ddev") {
            if shell::command_exists("ddev") {
                info("ddev installed successfully via scoop");
            } else {
                info("scoop command completed. Please restart your shell and verify ddev is available.");
            }
            let _ = ensure_mkcert_configured();
            return Ok(());
        } else {
            warning("scoop installation failed, trying alternative methods...");
        }
    } else {
        info("scoop not found, checking for winget...");
    }

    if shell::command_exists("winget") {
        info("Attempting to install ddev via winget...");
        let res = shell::run_local("winget install --id DDEV.DDEV -e --accept-source-agreements --accept-package-agreements");
        if res.is_ok() || shell::command_exists("ddev") {
            if shell::command_exists("ddev") {
                info("ddev installed successfully via winget");
            } else {
                info("winget command completed. Please restart your shell and verify ddev is available.");
            }
            let _ = ensure_mkcert_configured();
            return Ok(());
        } else {
            warning("winget installation failed, trying alternative methods...");
        }
    } else {
        info("winget not found");
    }

    // Windows: Check if WSL2 is available and guide user
    if shell::command_exists("wsl") {
        warning("DDEV on Windows is best installed via WSL2. Detected WSL is available.");
        warning("Please run 'wsl' and install DDEV inside your Linux distribution using the Linux installation method.");
        warning("Alternatively, install winget or scoop and try again.");
        return Err(miette::miette!(
			"DDEV installation on Windows requires WSL2 or a package manager (winget/scoop). Please install DDEV inside your WSL2 distribution or install winget/scoop."
		));
    }

    warning("No supported Windows package manager (winget/scoop) or WSL2 found.");
    warning("Please install DDEV manually from https://docs.ddev.com/en/stable/users/install/ddev-installation/");
    Err(miette::miette!(
        "No supported Windows package manager found. Please install DDEV manually or set up WSL2."
    ))
}

pub fn install() -> miette::Result<()> {
    #[cfg(target_os = "windows")]
    {
        ensure_ddev(install_windows)
    }
    #[cfg(not(target_os = "windows"))]
    {
        ensure_ddev(install_unix)
    }
}
