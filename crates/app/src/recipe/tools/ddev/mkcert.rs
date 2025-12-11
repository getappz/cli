use super::common::{ensure_mkcert_installed, install_mkcert_ca};
use crate::log::info;
pub fn install_mkcert() -> miette::Result<()> {
    ensure_mkcert_installed()?;
    install_mkcert_ca()?;
    info("mkcert is now configured for local development");
    Ok(())
}
