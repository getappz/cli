//! Security helpers for plugin handshake.
//!
//! Plugins compiled with `appz_pdk` use this module to compute the HMAC
//! handshake response, proving they were built against a genuine PDK.

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::types::{PluginHandshakeChallenge, PluginHandshakeResponse};

/// The same public key bytes used by the CLI for signing verification.
/// This must match `plugin-manager/src/signing_key.pub`.
const SIGNING_PUBLIC_KEY: &[u8; 32] = include_bytes!("../../plugin-manager/src/signing_key.pub");

/// Derive the HMAC key from the signing public key + salt.
/// Must produce the same key as the host side.
fn derive_hmac_key() -> Vec<u8> {
    use sha2::Digest;
    let mut key = Vec::with_capacity(64);
    key.extend_from_slice(SIGNING_PUBLIC_KEY);
    key.extend_from_slice(b"appz-plugin-handshake-v1");
    let hash = Sha256::digest(&key);
    hash.to_vec()
}

/// Compute the handshake HMAC response for a given challenge.
///
/// This function is called from `appz_plugin_handshake()` in plugin code.
pub fn compute_handshake(challenge: &PluginHandshakeChallenge) -> PluginHandshakeResponse {
    let key = derive_hmac_key();
    let mut mac = Hmac::<Sha256>::new_from_slice(&key).expect("HMAC can take key of any size");

    // Message = nonce + "|" + cli_version
    mac.update(challenge.nonce.as_bytes());
    mac.update(b"|");
    mac.update(challenge.cli_version.as_bytes());

    let result = mac.finalize();
    let hmac_hex = hex::encode(result.into_bytes());

    PluginHandshakeResponse { hmac: hmac_hex }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_deterministic() {
        let challenge = PluginHandshakeChallenge {
            nonce: "test-nonce".to_string(),
            cli_version: "0.1.0".to_string(),
        };

        let r1 = compute_handshake(&challenge);
        let r2 = compute_handshake(&challenge);

        assert_eq!(r1.hmac, r2.hmac);
        assert!(!r1.hmac.is_empty());
    }
}
