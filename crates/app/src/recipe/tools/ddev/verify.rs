use crate::{
    log::{info, warning},
    shell,
};

pub fn verify() -> miette::Result<()> {
    if !shell::command_exists("ddev") {
        return Err(miette::miette!("ddev not found on PATH"));
    }
    info("ddev is installed:");
    // Best effort: print version
    let _ = shell::run_local("ddev --version");

    // Verify mkcert is installed and configured
    if shell::command_exists("mkcert") {
        info("mkcert is installed");
        // Check if mkcert CA is installed (best effort)
        let _ = shell::run_local("mkcert -CAROOT");
    } else {
        warning("mkcert is not installed. SSL certificates may not work correctly.");
        warning("Install mkcert and run 'mkcert -install' for local development.");
    }
    Ok(())
}
