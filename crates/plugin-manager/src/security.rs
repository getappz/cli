//! Plugin security: Ed25519 signature verification, WASM header validation,
//! and runtime challenge-response handshake.
//!
//! Three security layers ensure plugins only work inside the appz CLI:
//! 1. Ed25519 signature — verifies the WASM was signed by the appz build pipeline
//! 2. WASM custom header — validates magic bytes and plugin metadata
//! 3. Runtime handshake — HMAC challenge-response proves both sides are genuine

use crate::error::{PluginError, PluginResult};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use hmac::{Hmac, Mac};
use rand::Rng;
use sha2::Sha256;
use std::path::Path;

/// Magic bytes identifying an appz plugin WASM custom section.
pub const APPZ_PLUGIN_MAGIC: &[u8] = b"APPZ_PLUGIN_V1";

/// Name of the WASM custom section containing plugin metadata.
const APPZ_HEADER_SECTION: &str = "appz_header";

/// Ed25519 public key for verifying plugin signatures.
/// In production this would be embedded via `include_bytes!` from a key file
/// generated during CI. For now we use a placeholder that will be replaced
/// during the build pipeline setup.
const SIGNING_PUBLIC_KEY: &[u8; 32] = include_bytes!("signing_key.pub");

/// Shared HMAC secret used for handshake. In production this is derived from
/// the signing public key + a compile-time salt. Both the CLI and the PDK
/// embed the same derivation so they agree on the key.
fn derive_hmac_key() -> Vec<u8> {
    let mut key = Vec::with_capacity(64);
    key.extend_from_slice(SIGNING_PUBLIC_KEY);
    key.extend_from_slice(b"appz-plugin-handshake-v1");
    // Use SHA-256 to produce a fixed-length key
    use sha2::Digest;
    let hash = Sha256::digest(&key);
    hash.to_vec()
}

/// Parsed content of the appz_header custom WASM section.
#[derive(Debug, Clone)]
pub struct AppzWasmHeader {
    pub plugin_id: String,
    pub min_cli_version: String,
}

/// Handshake challenge sent from host to plugin.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HandshakeChallenge {
    pub nonce: String,
    pub cli_version: String,
}

/// Handshake response from plugin to host.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HandshakeResponse {
    pub hmac: String,
}

pub struct PluginSecurity;

impl PluginSecurity {
    /// Verify the Ed25519 signature of a WASM binary.
    ///
    /// Reads `{wasm_path}.sig` for the detached signature.
    pub fn verify_signature(wasm_path: &Path, plugin_name: &str) -> PluginResult<()> {
        let sig_path = wasm_path.with_extension("wasm.sig");

        let wasm_bytes = std::fs::read(wasm_path)?;
        let sig_bytes = std::fs::read(&sig_path).map_err(|_| PluginError::SignatureInvalid {
            plugin: plugin_name.to_string(),
        })?;

        let signature = Signature::from_slice(&sig_bytes).map_err(|_| {
            PluginError::SignatureInvalid {
                plugin: plugin_name.to_string(),
            }
        })?;

        let verifying_key =
            VerifyingKey::from_bytes(SIGNING_PUBLIC_KEY).map_err(|_| {
                PluginError::SignatureInvalid {
                    plugin: plugin_name.to_string(),
                }
            })?;

        verifying_key
            .verify(&wasm_bytes, &signature)
            .map_err(|_| PluginError::SignatureInvalid {
                plugin: plugin_name.to_string(),
            })?;

        tracing::debug!("Plugin '{}' signature verified successfully", plugin_name);
        Ok(())
    }

    /// Verify the SHA-256 checksum of a WASM binary.
    pub fn verify_checksum(
        wasm_path: &Path,
        expected: &str,
        plugin_name: &str,
    ) -> PluginResult<()> {
        use sha2::Digest;

        let wasm_bytes = std::fs::read(wasm_path)?;
        let hash = Sha256::digest(&wasm_bytes);
        let actual = hex::encode(hash);

        // Expected may have "sha256:" prefix
        let expected_hex = expected.strip_prefix("sha256:").unwrap_or(expected);

        if actual != expected_hex {
            return Err(PluginError::ChecksumMismatch {
                plugin: plugin_name.to_string(),
                expected: expected_hex.to_string(),
                actual,
            });
        }

        tracing::debug!("Plugin '{}' checksum verified", plugin_name);
        Ok(())
    }

    /// Validate the appz_header custom WASM section.
    ///
    /// Parses the WASM binary to find the `appz_header` custom section,
    /// checks the magic bytes, and extracts the plugin ID.
    pub fn validate_header(
        wasm_bytes: &[u8],
        expected_plugin_id: &str,
    ) -> PluginResult<AppzWasmHeader> {
        let header = Self::parse_appz_header(wasm_bytes).ok_or_else(|| {
            PluginError::HeaderInvalid {
                plugin: expected_plugin_id.to_string(),
                reason: "Missing appz_header custom section".to_string(),
            }
        })?;

        if header.plugin_id != expected_plugin_id {
            return Err(PluginError::HeaderInvalid {
                plugin: expected_plugin_id.to_string(),
                reason: format!(
                    "Plugin ID mismatch: header says '{}', expected '{}'",
                    header.plugin_id, expected_plugin_id
                ),
            });
        }

        tracing::debug!(
            "Plugin header validated: id={}, min_cli={}",
            header.plugin_id,
            header.min_cli_version
        );
        Ok(header)
    }

    /// Generate a handshake challenge for a plugin.
    pub fn generate_challenge() -> HandshakeChallenge {
        let mut rng = rand::thread_rng();
        let nonce: String = (0..32).map(|_| rng.gen_range(b'a'..=b'z') as char).collect();

        HandshakeChallenge {
            nonce,
            cli_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Verify a handshake response from a plugin.
    pub fn verify_handshake(
        challenge: &HandshakeChallenge,
        response: &HandshakeResponse,
        plugin_name: &str,
    ) -> PluginResult<()> {
        let expected = Self::compute_handshake_hmac(&challenge.nonce, &challenge.cli_version);

        if response.hmac != expected {
            return Err(PluginError::HandshakeFailed {
                plugin: plugin_name.to_string(),
            });
        }

        tracing::debug!("Plugin '{}' handshake verified", plugin_name);
        Ok(())
    }

    /// Compute the expected HMAC for a handshake.
    /// This is also used by `appz_pdk` on the plugin side.
    pub fn compute_handshake_hmac(nonce: &str, cli_version: &str) -> String {
        let key = derive_hmac_key();
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&key).expect("HMAC can take key of any size");

        // Message = nonce + "|" + cli_version
        mac.update(nonce.as_bytes());
        mac.update(b"|");
        mac.update(cli_version.as_bytes());

        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }

    /// Check that the current CLI version satisfies the plugin's minimum.
    pub fn check_cli_version(
        min_version: &str,
        plugin_name: &str,
    ) -> PluginResult<()> {
        let current: semver::Version = env!("CARGO_PKG_VERSION")
            .parse()
            .unwrap_or_else(|_| semver::Version::new(0, 0, 0));

        let required: semver::Version = min_version
            .parse()
            .unwrap_or_else(|_| semver::Version::new(0, 0, 0));

        if current < required {
            return Err(PluginError::VersionIncompatible {
                plugin: plugin_name.to_string(),
                required: min_version.to_string(),
                current: current.to_string(),
            });
        }

        Ok(())
    }

    // -- Internal helpers --

    /// Parse the `appz_header` custom section from raw WASM bytes.
    ///
    /// WASM custom sections have the format:
    ///   section_id(0) + size + name_len + name + payload
    ///
    /// Our payload format:
    ///   magic(14 bytes) + plugin_id_len(2 bytes BE) + plugin_id + min_ver_len(2 bytes BE) + min_ver
    fn parse_appz_header(wasm_bytes: &[u8]) -> Option<AppzWasmHeader> {
        // Simple WASM custom section parser
        // WASM starts with magic + version (8 bytes)
        if wasm_bytes.len() < 8 {
            return None;
        }

        let mut pos = 8; // skip WASM header

        while pos < wasm_bytes.len() {
            if pos >= wasm_bytes.len() {
                break;
            }

            let section_id = wasm_bytes[pos];
            pos += 1;

            // Read LEB128 section size
            let (section_size, bytes_read) = read_leb128(&wasm_bytes[pos..])?;
            pos += bytes_read;

            let section_end = pos + section_size as usize;
            if section_end > wasm_bytes.len() {
                break;
            }

            if section_id == 0 {
                // Custom section - read name
                let (name_len, name_bytes_read) = read_leb128(&wasm_bytes[pos..])?;
                let name_start = pos + name_bytes_read;
                let name_end = name_start + name_len as usize;

                if name_end > section_end {
                    pos = section_end;
                    continue;
                }

                let name = std::str::from_utf8(&wasm_bytes[name_start..name_end]).ok()?;

                if name == APPZ_HEADER_SECTION {
                    let payload = &wasm_bytes[name_end..section_end];
                    return Self::parse_header_payload(payload);
                }
            }

            pos = section_end;
        }

        None
    }

    /// Parse the payload of the appz_header section.
    fn parse_header_payload(payload: &[u8]) -> Option<AppzWasmHeader> {
        let magic_len = APPZ_PLUGIN_MAGIC.len();
        if payload.len() < magic_len {
            return None;
        }

        // Check magic
        if &payload[..magic_len] != APPZ_PLUGIN_MAGIC {
            return None;
        }

        let mut pos = magic_len;

        // Read plugin_id
        if pos + 2 > payload.len() {
            return None;
        }
        let plugin_id_len = u16::from_be_bytes([payload[pos], payload[pos + 1]]) as usize;
        pos += 2;
        if pos + plugin_id_len > payload.len() {
            return None;
        }
        let plugin_id = std::str::from_utf8(&payload[pos..pos + plugin_id_len])
            .ok()?
            .to_string();
        pos += plugin_id_len;

        // Read min_cli_version
        if pos + 2 > payload.len() {
            return None;
        }
        let min_ver_len = u16::from_be_bytes([payload[pos], payload[pos + 1]]) as usize;
        pos += 2;
        if pos + min_ver_len > payload.len() {
            return None;
        }
        let min_cli_version = std::str::from_utf8(&payload[pos..pos + min_ver_len])
            .ok()?
            .to_string();

        Some(AppzWasmHeader {
            plugin_id,
            min_cli_version,
        })
    }
}

/// Read a LEB128-encoded unsigned integer. Returns (value, bytes_consumed).
fn read_leb128(bytes: &[u8]) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0;
    for (i, &byte) in bytes.iter().enumerate() {
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some((result, i + 1));
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_roundtrip() {
        let challenge = HandshakeChallenge {
            nonce: "test-nonce-123".to_string(),
            cli_version: "0.1.0".to_string(),
        };

        let expected_hmac =
            PluginSecurity::compute_handshake_hmac(&challenge.nonce, &challenge.cli_version);

        let response = HandshakeResponse {
            hmac: expected_hmac,
        };

        assert!(PluginSecurity::verify_handshake(&challenge, &response, "test").is_ok());
    }

    #[test]
    fn test_handshake_wrong_hmac() {
        let challenge = HandshakeChallenge {
            nonce: "test-nonce-123".to_string(),
            cli_version: "0.1.0".to_string(),
        };

        let response = HandshakeResponse {
            hmac: "wrong-hmac".to_string(),
        };

        assert!(PluginSecurity::verify_handshake(&challenge, &response, "test").is_err());
    }

    #[test]
    fn test_leb128_parsing() {
        assert_eq!(read_leb128(&[0x00]), Some((0, 1)));
        assert_eq!(read_leb128(&[0x01]), Some((1, 1)));
        assert_eq!(read_leb128(&[0x7F]), Some((127, 1)));
        assert_eq!(read_leb128(&[0x80, 0x01]), Some((128, 2)));
        assert_eq!(read_leb128(&[0xE5, 0x8E, 0x26]), Some((624485, 3)));
    }

    #[test]
    fn test_cli_version_check() {
        // Current version should satisfy 0.0.0
        assert!(PluginSecurity::check_cli_version("0.0.0", "test").is_ok());
    }
}
