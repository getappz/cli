//! Upload logic — prepare files for API, content-addressed format.

use api::models::PreparedFile;
use std::collections::HashMap;
use std::path::Path;

use crate::file_tree::FileRef;

/// Prepare files for deployment API (path + sha + size + mode).
///
/// For each unique sha, we emit one PreparedFile per path that has that content.
pub fn prepare_files(
    _output_dir: &Path,
    files_by_sha: &HashMap<String, FileRef>,
) -> Vec<PreparedFile> {
    let mut prepared = Vec::new();
    for (_, file_ref) in files_by_sha {
        let rel_str = file_ref
            .path
            .to_string_lossy()
            .replace('\\', "/");
        prepared.push(PreparedFile {
            file: rel_str,
            sha: Some(file_ref.sha.clone()),
            size: Some(file_ref.data.len() as u64),
            mode: file_ref.mode,
        });
    }
    prepared
}
