use crate::{
    log::{info, warning},
    shell,
};

/// Check if Docker is available and working
pub fn docker_available() -> bool {
    if !shell::command_exists("docker") {
        return false;
    }
    // Check if docker daemon is running by trying a simple command
    // Use shell::test_local which handles cross-platform redirects
    #[cfg(target_os = "windows")]
    {
        shell::test_local("docker info >nul 2>&1")
    }
    #[cfg(not(target_os = "windows"))]
    {
        shell::test_local("docker info > /dev/null 2>&1")
    }
}

/// Check if Podman is available and working
pub fn podman_available() -> bool {
    if !shell::command_exists("podman") {
        return false;
    }
    // Check if podman is working by trying a simple command
    #[cfg(target_os = "windows")]
    {
        shell::test_local("podman info >nul 2>&1")
    }
    #[cfg(not(target_os = "windows"))]
    {
        shell::test_local("podman info > /dev/null 2>&1")
    }
}

/// Run the provided installer if Docker/Podman is not found.
/// After running, treat installation as success if the command appears on PATH,
/// even when the installer returned an error (e.g., already installed).
pub fn ensure_container_runtime<F>(install_fn: F) -> miette::Result<()>
where
    F: FnOnce() -> miette::Result<()>,
{
    // First check if Docker is available
    if docker_available() {
        info("Docker is already installed and running");
        let _ = shell::run_local("docker --version");
        return Ok(());
    }
    // If Docker not found, try Podman
    if podman_available() {
        info("Podman is already installed and running");
        let _ = shell::run_local("podman --version");
        return Ok(());
    }

    info("Docker/Podman not found, attempting to install Podman...");
    let res = install_fn();

    // Check if installation succeeded even if command returned error
    if docker_available() {
        info("Docker installation successful!");
        let _ = shell::run_local("docker --version");
        return Ok(());
    }
    if podman_available() {
        info("Podman installation successful!");
        let _ = shell::run_local("podman --version");
        return Ok(());
    }

    // If we get here, installation failed
    if let Err(e) = &res {
        warning(&format!("Installation attempt failed: {}", e));
    }
    res
}
