use miette::miette;

use crate::{
    log::{info, warning},
    shell,
};
use command::Command;
use regex::Regex;
use task::{Task, TaskRegistry, TaskResult};

/// Print version detected from `mise --version` output
fn print_detected_mise_version() {
    let mut cmd = Command::new("mise");
    cmd.arg("--version");
    cmd.set_error_on_nonzero(false);
    if let Ok(out) = cmd.exec() {
        let output = String::from_utf8_lossy(&out.stdout);
        if let Some(ver) = extract_version_from_text(&output) {
            info(&format!("Detected Mise {} installation", ver));
        } else {
            info("Detected Mise installation");
        }
    } else {
        info("Detected Mise installation");
    }
}

fn extract_version_from_text(text: &str) -> Option<String> {
    let re = Regex::new(r"\d{4}\.\d{2}\.\d{2}").unwrap();
    if let Some(cap) = re.find(text) {
        return Some(cap.as_str().to_string());
    }
    None
}

/// Run the provided installer if `mise` is not found (async)
pub async fn ensure_mise() -> TaskResult {
    if shell::command_exists("mise") {
        print_detected_mise_version();
        return Ok(());
    }

    let res = {
        #[cfg(target_os = "windows")]
        {
            install_mise_windows().await
        }
        #[cfg(not(target_os = "windows"))]
        {
            install_mise_unix().await
        }
    };

    if res.is_err() && shell::command_exists("mise") {
        print_detected_mise_version();
        return Ok(());
    }
    if res.is_ok() && shell::command_exists("mise") {
        print_detected_mise_version();
    }
    res
}

async fn install_mise_unix() -> TaskResult {
    // Prefer Homebrew on macOS if available, otherwise use official install script
    if shell::command_exists("brew") {
        info("Installing mise via Homebrew");
        shell::run_local("brew install mise")
            .map_err(|e| miette!("Failed to install mise via brew: {}", e))
    } else if shell::command_exists("apk") {
        info("Installing mise via apk");
        shell::run_local("apk add mise")
            .map_err(|e| miette!("Failed to install mise via apk: {}", e))
    } else if shell::command_exists("apt") || shell::command_exists("apt-get") {
        info("Installing mise via apt repository");
        // Minimal path: use official script for portability instead of repo wiring
        shell::run_local("curl https://mise.run | sh")
            .map_err(|e| miette!("Failed to install mise via curl script: {}", e))
    } else if shell::command_exists("pacman") {
        info("Installing mise via pacman");
        shell::run_local("sudo pacman -S --noconfirm mise")
            .map_err(|e| miette!("Failed to install mise via pacman: {}", e))
    } else if shell::command_exists("dnf") {
        info("Installing mise via dnf (copr)");
        shell::run_local("dnf copr enable -y jdxcode/mise && dnf install -y mise")
            .map_err(|e| miette!("Failed to install mise via dnf: {}", e))
    } else if shell::command_exists("zypper") {
        info("Installing mise via zypper");
        shell::run_local("sudo wget https://mise.jdx.dev/rpm/mise.repo -O /etc/zypp/repos.d/mise.repo && sudo zypper refresh && sudo zypper install -y mise").map_err(|e| miette!("Failed to install mise via zypper: {}", e))
    } else if shell::command_exists("snap") {
        info("Installing mise via snap (beta)");
        shell::run_local("sudo snap install mise --classic --beta")
            .map_err(|e| miette!("Failed to install mise via snap: {}", e))
    } else if shell::command_exists("nix-env") {
        info("Installing mise via nix-env");
        shell::run_local("nix-env -iA mise")
            .map_err(|e| miette!("Failed to install mise via nix-env: {}", e))
    } else {
        info("Installing mise via official installer script");
        shell::run_local("curl https://mise.run | sh")
            .map_err(|e| miette!("Failed to install mise via curl script: {}", e))
    }
}

async fn install_mise_windows() -> TaskResult {
    // Try winget, then scoop. Fall back to manual warning
    if shell::command_exists("winget") {
        info("Installing mise via winget");
        let res = shell::run_local("winget install --id jdx.mise -e --accept-source-agreements --accept-package-agreements").map_err(|e| miette!("Failed to install mise via winget: {}", e));
        if res.is_err() && shell::command_exists("mise") {
            // winget may return non-zero if already installed/no upgrade; treat as success if mise exists
            return Ok(());
        }
        res
    } else if shell::command_exists("scoop") {
        info("Installing mise via scoop");
        shell::run_local("scoop install mise")
            .map_err(|e| miette!("Failed to install mise via scoop: {}", e))
    } else {
        warning("winget/scoop not found. Please install one of them or install mise manually from GitHub Releases.");
        Err(miette!("No supported Windows package manager found"))
    }
}

pub fn register_mise_tools(reg: &mut TaskRegistry) {
    // tools:mise:install — fully async
    reg.register(
        Task::new(
            "tools:mise:install",
            task::task_fn_async!(|_ctx: std::sync::Arc<task::Context>| async move {
                ensure_mise().await
            }),
        )
        .desc("Installs mise across supported platforms"),
    );

    // tools:mise:verify — check version (non-fatal)
    reg.register(
        Task::new(
            "tools:mise:verify",
            task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| {
                if !shell::command_exists("mise") {
                    warning("mise not found on PATH. Run tools:mise:install to install it.");
                    return Ok(());
                }
                info("mise is installed:");
                // Best-effort: print version without failing the task
                let mut cmd = Command::new("mise");
                cmd.arg("version");
                cmd.set_error_on_nonzero(false);
                if let Ok(out) = cmd.exec() {
                    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !v.is_empty() {
                        info(&v);
                    }
                }
                Ok(())
            }),
        )
        .desc("Verifies mise installation"),
    );

    // tools:mise:sync — install tools from .tool-versions/mise.toml
    reg.register(
        Task::new(
            "tools:mise:sync",
            task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| {
                if !shell::command_exists("mise") {
                    warning("mise not found on PATH. Run tools:mise:install first.");
                    return Ok(());
                }
                shell::run_local("mise install").map_err(|e| miette!("mise install failed: {}", e))
            }),
        )
        .desc("Sync tools defined in .tool-versions / mise.toml"),
    );

    // tools:mise:use_node — ensure a global node toolchain is active
    reg.register(
        Task::new(
            "tools:mise:use_node",
            task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| {
                if !shell::command_exists("mise") {
                    warning("mise not found on PATH. Run tools:mise:install first.");
                    return Ok(());
                }
                // Let mise resolve default from config if present
                shell::run_local("mise use -g node")
                    .map_err(|e| miette!("mise use -g node failed: {}", e))
            }),
        )
        .desc("Ensure Node toolchain is active via mise"),
    );

    // node:install_deps — default to bun, users may explicitly use npm/pnpm/yarn in recipes
    reg.register(
        Task::new(
            "node:install_deps",
            task::task_fn_async!(|ctx: std::sync::Arc<task::Context>| async move {
                let cwd = ctx.working_path().cloned();
                let opts = shell::RunOptions {
                    cwd,
                    env: None,
                    show_output: true,
                    package_manager: None,
                };
                // Default to bun install; our shell wrapper routes through `mise x --` when available
                shell::run_local_with(&ctx, "bun install", opts).await
            }),
        )
        .desc("Install Node dependencies using bun by default (mise-managed)"),
    );
}
