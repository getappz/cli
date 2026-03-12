//! FFI bindings to Go html-to-markdown C shared library.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

extern "C" {
  fn ConvertHTMLToMarkdown(html: *const c_char) -> *mut c_char;
  fn FreeCString(s: *mut c_char);
}

/// Convert HTML to markdown using the Go library.
pub fn convert_html_to_markdown(html: &str) -> String {
  let c_html = match CString::new(html) {
    Ok(s) => s,
    Err(_) => return String::new(),
  };
  let ptr = unsafe { ConvertHTMLToMarkdown(c_html.as_ptr()) };
  if ptr.is_null() {
    return String::new();
  }
  let result = unsafe {
    let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
    FreeCString(ptr);
    s
  };
  result
}
