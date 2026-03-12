//! Inject appz_header custom section into WASM binary.
//!
//! WASM custom section format:
//!   section_id(0) + size(LEB128) + name_len(LEB128) + name + payload
//!
//! Our payload format:
//!   magic(14 bytes) + plugin_id_len(2 BE) + plugin_id + min_ver_len(2 BE) + min_ver

use miette::{IntoDiagnostic, Result};
use std::path::Path;

const APPZ_PLUGIN_MAGIC: &[u8] = b"APPZ_PLUGIN_V1";
const APPZ_HEADER_SECTION: &str = "appz_header";

/// Append LEB128-encoded u32 to a byte vector.
fn write_leb128(buf: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
}

/// Build the appz_header payload.
fn build_payload(plugin_id: &str, min_cli_version: &str) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(APPZ_PLUGIN_MAGIC);
    payload.extend_from_slice(&(plugin_id.len() as u16).to_be_bytes());
    payload.extend_from_slice(plugin_id.as_bytes());
    payload.extend_from_slice(&(min_cli_version.len() as u16).to_be_bytes());
    payload.extend_from_slice(min_cli_version.as_bytes());
    payload
}

/// Inject the appz_header custom section into WASM.
/// Inserts it after the WASM preamble (magic + version).
pub fn inject(
    input: &Path,
    output: &Path,
    plugin_id: &str,
    min_cli_version: &str,
) -> Result<()> {
    let wasm = std::fs::read(input).into_diagnostic()?;

    if wasm.len() < 8 {
        return Err(miette::miette!("Invalid WASM: too short"));
    }
    if &wasm[0..4] != b"\0asm" {
        return Err(miette::miette!("Invalid WASM: bad magic"));
    }

    let payload = build_payload(plugin_id, min_cli_version);
    let name = APPZ_HEADER_SECTION.as_bytes();

    // Custom section: id(0) + size + name_len + name + payload
    let mut section_content = Vec::new();
    write_leb128(&mut section_content, name.len() as u32);
    section_content.extend_from_slice(name);
    section_content.extend_from_slice(&payload);

    let section_size = section_content.len() as u32;
    let mut section_header = Vec::new();
    section_header.push(0); // custom section id
    write_leb128(&mut section_header, section_size);

    let mut result = Vec::with_capacity(wasm.len() + section_header.len() + section_content.len());
    result.extend_from_slice(&wasm);
    result.extend_from_slice(&section_header);
    result.extend_from_slice(&section_content);

    std::fs::write(output, &result).into_diagnostic()?;
    Ok(())
}
