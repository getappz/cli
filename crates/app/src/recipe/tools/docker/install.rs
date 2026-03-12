use super::common::ensure_container_runtime;
use crate::{
    log::{info, warning},
    shell,
};

pub fn install_unix() -> miette::Result<()> {
    // macOS: Install Podman via Homebrew
    #[cfg(target_os = "macos")]
    {
        if shell::command_exists("brew") {
            info("Installing Podman via Homebrew");
            let res = shell::run_local("brew install podman");
            if res.is_ok() || shell::command_exists("podman") {
                // Initialize and start podman machine on macOS
                info("Setting up Podman machine...");
                let _ = shell::run_local("podman machine init");
                let _ = shell::run_local("podman machine start");
                // Create docker alias if shell supports it
                setup_docker_alias()?;
                return Ok(());
            }
        }
    }

    // Linux: Debian/Ubuntu via apt
    if shell::command_exists("apt-get") || shell::command_exists("apt") {
        info("Installing Podman via apt");
        let res = shell::run_local("sudo apt-get update && sudo apt-get install -y podman");
        if res.is_ok() || shell::command_exists("podman") {
            setup_docker_alias()?;
            return Ok(());
        }
    }

    // Linux: Fedora/RedHat via dnf/yum (Podman is often pre-installed)
    if shell::command_exists("dnf") {
        info("Installing Podman via dnf");
        let res = shell::run_local("sudo dnf install -y podman");
        if res.is_ok() || shell::command_exists("podman") {
            setup_docker_alias()?;
            return Ok(());
        }
    }

    // Linux: Arch via pacman
    if shell::command_exists("pacman") {
        info("Installing Podman via pacman");
        // Try AUR helpers first
        if shell::command_exists("yay") {
            let _ = shell::run_local("yay -S --noconfirm podman");
            if shell::command_exists("podman") {
                setup_docker_alias()?;
                return Ok(());
            }
        }
        if shell::command_exists("paru") {
            let _ = shell::run_local("paru -S --noconfirm podman");
            if shell::command_exists("podman") {
                setup_docker_alias()?;
                return Ok(());
            }
        }
        // Fallback to pacman
        let res = shell::run_local("sudo pacman -S --noconfirm podman");
        if res.is_ok() || shell::command_exists("podman") {
            setup_docker_alias()?;
            return Ok(());
        }
    }

    // Linux: Alpine via apk
    if shell::command_exists("apk") {
        info("Installing Podman via apk");
        let res = shell::run_local("sudo apk add podman");
        if res.is_ok() || shell::command_exists("podman") {
            setup_docker_alias()?;
            return Ok(());
        }
    }

    // Linux: OpenSUSE via zypper
    if shell::command_exists("zypper") {
        info("Installing Podman via zypper");
        let res = shell::run_local("sudo zypper install -y podman");
        if res.is_ok() || shell::command_exists("podman") {
            setup_docker_alias()?;
            return Ok(());
        }
    }

    // Fallback: Try installing from official repositories or build from source
    warning(
        "Could not find a supported package manager. Podman may need to be installed manually.",
    );
    warning("Visit https://podman.io/getting-started/installation for installation instructions.");
    Err(miette::miette!(
        "Failed to install Podman via any supported method"
    ))
}

pub fn install_windows() -> miette::Result<()> {
    // Windows: Try winget first, then scoop
    if shell::command_exists("winget") {
        info("Installing Podman via winget");
        let res = shell::run_local("winget install --id RedHat.Podman -e --accept-source-agreements --accept-package-agreements");
        if res.is_ok() || shell::command_exists("podman") {
            info(
                "Podman installed. Please initialize the podman machine with: podman machine init",
            );
            info("Then start it with: podman machine start");
            setup_docker_alias()?;
            return Ok(());
        }
    }
    if shell::command_exists("scoop") {
        info("Installing Podman via scoop");
        let res = shell::run_local("scoop install podman");
        if res.is_ok() || shell::command_exists("podman") {
            info(
                "Podman installed. Please initialize the podman machine with: podman machine init",
            );
            info("Then start it with: podman machine start");
            setup_docker_alias()?;
            return Ok(());
        }
    }

    // Windows: Check if WSL2 is available and guide user
    if shell::command_exists("wsl") {
        warning("Podman on Windows is best installed via WSL2. Detected WSL is available.");
        warning("Please run 'wsl' and install Podman inside your Linux distribution using the Linux installation method.");
        warning("Alternatively, install winget or scoop and try again.");
        return Err(miette::miette!(
			"Podman installation on Windows requires WSL2 or a package manager (winget/scoop). Please install Podman inside your WSL2 distribution or install winget/scoop."
		));
    }
    warning("No supported Windows package manager (winget/scoop) or WSL2 found.");
    warning("Please install Podman manually from https://podman.io/getting-started/installation/");
    Err(miette::miette!(
		"No supported Windows package manager found. Please install Podman manually or set up WSL2."
	))
}

/// Sets up a docker alias to podman for compatibility
fn setup_docker_alias() -> miette::Result<()> {
    // On Unix systems, try to create a docker alias in common shell config files
    #[cfg(not(target_os = "windows"))]
    {
        // Try to add alias to shell config
        let home = std::env::var("HOME").ok();
        if let Some(home) = home {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            let config_file = if shell.contains("zsh") {
                format!("{}/.zshrc", home)
            } else if shell.contains("fish") {
                format!("{}/.config/fish/config.fish", home)
            } else {
                format!("{}/.bashrc", home)
            };
            let alias_line = "alias docker=podman\n";
            // Check if alias already exists
            if let Ok(content) = starbase_utils::fs::read_file(&config_file) {
                if content.contains("alias docker=podman") {
                    info("Docker alias to Podman already exists in shell config");
                    return Ok(());
                }
            }
            // Try to append alias (best effort)
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&config_file)
            {
                use std::io::Write;
                let _ = writeln!(file, "\n# Docker alias for Podman (added by appz)");
                let _ = writeln!(file, "{}", alias_line);
                info(&format!("Added docker alias to Podman in {}", config_file));
                info(&format!(
                    "Please restart your shell or run: source {}",
                    config_file
                ));
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        setup_windows_docker_shim()?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn setup_windows_docker_shim() -> miette::Result<()> {
    use std::fs;
    use std::path::PathBuf;

    // Step 1: Detect Podman
    info("Detecting podman.exe...");
    let podman_path = match shell::which("podman") {
        Ok(path) => path,
        Err(_) => {
            miette::bail!("Podman is not installed or not in PATH. Please install Podman first.");
        }
    };
    info(&format!("Found podman at: {}", podman_path));

    // Step 2: Create shim directory
    let shim_dir = PathBuf::from("C:\\docker-shim");
    info(&format!("Creating shim directory: {}", shim_dir.display()));
    fs::create_dir_all(&shim_dir).map_err(AppError::from)?;

    // Step 3: Create docker.cmd
    info("Creating docker.cmd shim...");
    let cmd_content = format!("@echo off\r\n\"{}\" %*", podman_path);
    fs::write(shim_dir.join("docker.cmd"), cmd_content.as_bytes()).map_err(AppError::from)?;

    // Step 4: Create docker.ps1
    info("Creating docker.ps1 shim...");
    let ps1_content = format!("& \"{}\" @args", podman_path);
    fs::write(shim_dir.join("docker.ps1"), ps1_content.as_bytes()).map_err(AppError::from)?;

    // Step 5: Create docker.exe using PowerShell Add-Type
    info("Creating docker.exe shim...");
    let podman_path_escaped = podman_path.replace('\\', "\\\\");
    let file_name_line = format!(r#"@"{}""#, podman_path_escaped);
    let join_separator = "\" \"";
    let exe_stub = format!(
        r"using System;using System.Diagnostics;public class Program {{    public static int Main(string[] args) {{        var psi = new ProcessStartInfo();        psi.FileName = {};        psi.Arguments = String.Join({}, args);        psi.UseShellExecute = false;        psi.RedirectStandardInput = false;        psi.RedirectStandardOutput = false;        psi.RedirectStandardError = false;        var p = Process.Start(psi);        p.WaitForExit();        return p.ExitCode;    }}}}",
        file_name_line, join_separator
    );
    let exe_output_path = shim_dir.join("docker.exe");
    let temp_cs_file = shim_dir.join("docker_shim_temp.cs");
    fs::write(&temp_cs_file, exe_stub.as_bytes()).map_err(AppError::from)?;
    let temp_cs_path = temp_cs_file
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace("'", "''");
    let exe_output_path_escaped = exe_output_path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace("'", "''");
    let ps_command = format!(
		"$code = Get-Content -Raw '{}'; Add-Type -OutputAssembly '{}' -TypeDefinition $code -ErrorAction Stop",
		temp_cs_path, exe_output_path_escaped
	);
    let mut ps_cmd = Command::new("powershell.exe");
    ps_cmd.arg("-Command");
    ps_cmd.arg(&ps_command);
    let exe_created = ps_cmd.exec().is_ok();
    let _ = fs::remove_file(&temp_cs_file);
    if !exe_created {
        warning("Failed to create docker.exe shim. docker.cmd and docker.ps1 are still available.");
    }

    // Step 6: Update PATH (User scope, no admin needed)
    info("Updating PATH...");
    let shim_dir_str = shim_dir.to_string_lossy().to_string();
    let mut check_cmd = Command::new("powershell.exe");
    check_cmd.arg("-Command");
    check_cmd.arg(format!(
        r#"[Environment]::GetEnvironmentVariable('PATH', 'User') -like '*{}*'"#,
        shim_dir_str.replace('\\', "\\\\")
    ));
    check_cmd.set_error_on_nonzero(false);
    let path_exists = check_cmd
        .exec()
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.trim().eq_ignore_ascii_case("true")
        })
        .unwrap_or(false);
    if !path_exists {
        let update_cmd = format!(
            r#"powershell.exe -Command "$currentPath = [Environment]::GetEnvironmentVariable('PATH', 'User'); if ($currentPath) {{ $newPath = '{};' + $currentPath }} else {{ $newPath = '{}' }}; [Environment]::SetEnvironmentVariable('PATH', $newPath, 'User')"#,
            shim_dir_str, shim_dir_str
        );
        match shell::run_local(&update_cmd) {
            Ok(_) => {
                info("PATH updated successfully in user profile. You may need to restart your terminal for changes to take effect.");
            }
            Err(e) => {
                warning(&format!("Failed to update user PATH: {}.", e));
                warning(&format!(
                    "Please manually add {} to your PATH environment variable.",
                    shim_dir_str
                ));
            }
        }
    } else {
        info("PATH already contains shim directory.");
    }
    info("✅ Docker shim installation complete!");
    info("Try: docker --version");
    Ok(())
}

pub fn install() -> miette::Result<()> {
    ensure_container_runtime(|| {
        #[cfg(target_os = "windows")]
        {
            install_windows()
        }
        #[cfg(not(target_os = "windows"))]
        {
            install_unix()
        }
    })
}
