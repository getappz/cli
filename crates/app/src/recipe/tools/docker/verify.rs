use super::common::{docker_available, podman_available};
use crate::{
    log::{info, warning},
    shell,
};

pub fn verify() -> miette::Result<()> {
    if docker_available() {
        info("Docker is installed and running:");
        let _ = shell::run_local("docker --version");
        let _ = shell::run_local("docker info");
        return Ok(());
    }
    if podman_available() {
        info("Podman is installed and running:");
        let _ = shell::run_local("podman --version");
        let _ = shell::run_local("podman info");
        // Check if docker alias exists
        if shell::command_exists("docker") {
            info("Docker alias is configured");
        } else {
            warning("Docker alias not found. You may want to create one for compatibility.");
            info("On Unix, add 'alias docker=podman' to your shell config (.bashrc, .zshrc, etc.)");
        }
        return Ok(());
    }
    Err(miette::miette!(
		"Neither Docker nor Podman is installed and running. Run 'tools:docker:install' to install Podman."
	))
}
