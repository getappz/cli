//! C-ABI exports for the wasm32 build (no wasm-bindgen, no imports).
//!
//! The host copies input into module memory, calls [`md_convert`], reads the
//! framed result, and frees every buffer:
//!
//! ```text
//! p = md_alloc(len)            // host writes `len` bytes at p
//! r = md_convert(html_ptr, html_len, origin_ptr, origin_len)
//! md_free(html_ptr, html_len)  // free inputs with the length they were allocated with
//! ```
//!
//! `md_convert` returns `0` on any failure (bad UTF-8, bad origin, empty
//! result, over the size cap, panic is `abort`). On success it returns a
//! pointer to one allocation laid out as:
//!
//! | bytes  | field                                   |
//! |--------|-----------------------------------------|
//! | 0..4   | `total_len`: u32 LE, whole allocation   |
//! | 4..8   | `tokens`: u32 LE                        |
//! | 8      | mode: `0` minimal, `1` fallback         |
//! | 9..    | Markdown, UTF-8 (`total_len - 9` bytes) |
//!
//! Free it with `md_free(ptr, total_len)` — exactly the value in bytes 0..4.

#![allow(unsafe_code)]

use crate::{Mode, convert_str};

const HEADER: usize = 9;

/// Allocate `len` zeroed bytes in module memory for the host to fill.
#[unsafe(no_mangle)]
pub extern "C" fn md_alloc(len: u32) -> *mut u8 {
    Box::into_raw(vec![0u8; len as usize].into_boxed_slice()).cast()
}

/// Free a buffer from [`md_alloc`] or [`md_convert`]; `len` must be the
/// original allocation length (for results: the `total_len` header field).
///
/// # Safety
/// `ptr` must come from this module with exactly `len` bytes, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn md_free(ptr: *mut u8, len: u32) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller contract above; allocations are exact boxed slices.
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len as usize)) });
}

/// Convert HTML at `html_ptr` (fetched from `origin_ptr`) to Markdown.
/// Returns a framed result pointer (see module docs) or `0`.
///
/// # Safety
/// Both ranges must be readable module memory of the given lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn md_convert(
    html_ptr: u32,
    html_len: u32,
    origin_ptr: u32,
    origin_len: u32,
) -> u32 {
    // SAFETY: caller contract above.
    let (html, origin) = unsafe {
        (
            std::slice::from_raw_parts(html_ptr as *const u8, html_len as usize),
            std::slice::from_raw_parts(origin_ptr as *const u8, origin_len as usize),
        )
    };
    // An origin must be an absolute http(s) URL, else relative links would be garbage.
    let Some(origin) = std::str::from_utf8(origin)
        .ok()
        .filter(|s| s.starts_with("http://") || s.starts_with("https://"))
    else {
        return 0;
    };
    let Ok(done) = convert_str(html, origin) else {
        return 0;
    };

    let total = HEADER + done.markdown.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(done.tokens as u32).to_le_bytes());
    out.push(match done.mode {
        Mode::Minimal => 0,
        Mode::Fallback => 1,
    });
    out.extend_from_slice(done.markdown.as_bytes());
    Box::into_raw(out.into_boxed_slice()).cast::<u8>() as u32
}
