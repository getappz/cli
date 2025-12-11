use super::common::get_home_dir;
use crate::{
    log::{info, warning},
    shell,
};

pub fn uninstall() -> miette::Result<()> {
    info("Starting DDEV uninstall process...");

    // Step 1: Remove all DDEV projects and data
    if shell::command_exists("ddev") {
        info("Removing all DDEV projects and data...");
        // Power off any running DDEV instances first
        let _ = shell::run_local("ddev poweroff");
        // Remove all projects
        let _ = shell::run_local("ddev delete --all --yes");
        // Alternative: ddev clean --all (if delete doesn't work)
        // let _ = shell::run_local("ddev clean --all --yes");
        // Clean up hostname entries
        info("Cleaning up hostname entries...");
        let _ = shell::run_local("ddev hostname --remove-inactive");
    } else {
        info("ddev binary not found, skipping project cleanup");
    }

    // Step 2: Remove global DDEV directories
    if let Some(home) = get_home_dir() {
        let ddev_dir = home.join(".ddev");
        if ddev_dir.exists() {
            info("Removing global .ddev directory...");
            let _ = std::fs::remove_dir_all(&ddev_dir);
        }
        let mutagen_dir = home.join(".ddev_mutagen_data_directory");
        if mutagen_dir.exists() {
            info("Removing Mutagen data directory...");
            let _ = std::fs::remove_dir_all(&mutagen_dir);
        }
    }

    // Step 3: Remove DDEV binary based on installation method
    if shell::command_exists("ddev") {
        info("Removing DDEV binary...");
        // Try to detect installation method and uninstall accordingly
        #[cfg(target_os = "macos")]
        {
            if shell::command_exists("brew") {
                // Try to uninstall via Homebrew (will fail gracefully if not installed via brew)
                info("Attempting to uninstall ddev via Homebrew...");
                let brew_result = shell::run_local("brew uninstall ddev");
                if brew_result.is_ok() {
                    info("Successfully uninstalled ddev via Homebrew");
                } else {
                    info("ddev not found in Homebrew, trying other methods...");
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            // Try apt uninstall (Debian/Ubuntu)
            if shell::command_exists("apt") || shell::command_exists("apt-get") {
                info("Attempting to remove ddev via apt...");
                let _ = shell::run_local("sudo apt-get remove -y ddev");
            }
            // Try dnf/yum uninstall (Fedora/RedHat)
            if shell::command_exists("dnf") {
                info("Attempting to remove ddev via dnf...");
                let _ = shell::run_local("sudo dnf remove -y ddev");
            }
            // Try pacman uninstall (Arch)
            if shell::command_exists("pacman") {
                // Check if installed via AUR
                if shell::command_exists("yay") {
                    let _ = shell::run_local("yay -Rns --noconfirm ddev-bin");
                } else if shell::command_exists("paru") {
                    let _ = shell::run_local("paru -Rns --noconfirm ddev-bin");
                }
            }
        }
        #[cfg(target_os = "windows")]
        {
            // Windows: Try scoop and winget
            if shell::command_exists("scoop") {
                info("Attempting to uninstall ddev via scoop...");
                let _ = shell::run_local("scoop uninstall ddev");
            }
            if shell::command_exists("winget") {
                info("Attempting to uninstall ddev via winget...");
                let _ = shell::run_local("winget uninstall ddev");
            }
        }

        // If ddev still exists, try to find and remove it manually
        if shell::command_exists("ddev") {
            if let Ok(ddev_path) = shell::which("ddev") {
                warning(&format!("ddev binary still found at: {}", ddev_path));
                warning("You may need to manually remove it:");
                warning(&format!("  sudo rm {}", ddev_path));
            }
        }
    } else {
        info("ddev binary not found, skipping binary removal");
    }

    // Step 4: Optional Docker cleanup (informational only, user can do manually)
    if shell::command_exists("docker") {
        warning("Docker is still installed.");
        warning("If you want to remove DDEV-related Docker images and containers:");
        #[cfg(target_os = "windows")]
        {
            warning("  For PowerShell:");
            warning("    docker ps -a | Select-String ddev | ForEach-Object { docker rm $_.Line.Split()[0] }");
            warning("    docker images | Select-String ddev | ForEach-Object { docker rmi $_.Line.Split()[2] }");
            warning("    docker volume ls | Select-String ddev | ForEach-Object { docker volume rm $_.Line.Split()[1] }");
        }
        #[cfg(not(target_os = "windows"))]
        {
            warning("  Run:");
            warning("    docker ps -a | grep ddev | awk '{print $1}' | xargs docker rm");
            warning("    docker images | grep ddev | awk '{print $3}' | xargs docker rmi");
            warning("    docker volume ls | grep ddev | awk '{print $2}' | xargs docker volume rm");
        }
    }

    info("DDEV uninstall completed!");
    info("Note: If Docker was installed solely for DDEV, you may want to uninstall it separately.");
    Ok(())
}
