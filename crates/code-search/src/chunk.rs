//! Chunk files for embedding. ~512 tokens per chunk with overlap.

use crate::error::CodeSearchError;
use crate::parse::ParsedFile;
use tracing::instrument;

const CHUNK_TOKENS: usize = 512;
const OVERLAP_TOKENS: usize = 64;
const CHARS_PER_TOKEN: usize = 4;

/// Round down to the nearest UTF-8 character boundary so slicing doesn't split multi-byte chars.
fn floor_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = idx.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[derive(Debug, Clone)]
pub struct CodeChunk {
    pub path: String,
    pub content: String,
    pub line_start: usize,
}

/// Split files into chunks. Uses character-based approximation for token count.
#[instrument(skip_all)]
pub fn chunk(files: &[ParsedFile]) -> Result<Vec<CodeChunk>, CodeSearchError> {
    let chunk_chars = CHUNK_TOKENS * CHARS_PER_TOKEN;
    let overlap_chars = OVERLAP_TOKENS * CHARS_PER_TOKEN;
    let _step = chunk_chars.saturating_sub(overlap_chars).max(1);

    let mut chunks = Vec::new();
    for file in files {
        if file.content.trim().is_empty() {
            continue;
        }

        let mut pos = 0;
        while pos < file.content.len() {
            let end = floor_char_boundary(&file.content, (pos + chunk_chars).min(file.content.len()));
            let chunk_content = file.content[pos..end].to_string();
            let line_start = file.content[..pos].lines().count() + 1;
            pos = floor_char_boundary(&file.content, end.saturating_sub(overlap_chars));

            if !chunk_content.trim().is_empty() {
                chunks.push(CodeChunk {
                    path: file.path.clone(),
                    content: chunk_content,
                    line_start,
                });
            }
            if end >= file.content.len() {
                break;
            }
        }
    }

    Ok(chunks)
}
