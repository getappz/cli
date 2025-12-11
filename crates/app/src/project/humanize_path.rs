//! Humanize path utility - converts absolute paths to user-friendly relative paths

use std::path::Path;

/// Convert an absolute path to a human-friendly relative path
///
/// Examples:
/// - `C:\Users\shiva\projects\hugoplate` → `hugoplate`
/// - `/home/user/projects/app` → `app`
/// - If path is already relative, returns it as-is
pub fn humanize_path(path: &Path) -> String {
    // If path is already relative, return as-is
    if !path.is_absolute() {
        return path.to_string_lossy().to_string();
    }

    // Try to get the file name (last component)
    if let Some(name) = path.file_name() {
        return name.to_string_lossy().to_string();
    }

    // Fallback: return the path as string
    path.to_string_lossy().to_string()
}
