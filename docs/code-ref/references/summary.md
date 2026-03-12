This file is a merged representation of a subset of the codebase, containing specifically included files and files not matching ignore patterns, combined into a single document by Repomix.

# Summary

## Purpose

This is a reference codebase organized into multiple files for AI consumption.
It is designed to be easily searchable using grep and other text-based tools.

## File Structure

This skill contains the following reference files:

| File | Contents |
|------|----------|
| `project-structure.md` | Directory tree with line counts per file |
| `files.md` | All file contents (search with `## File: <path>`) |
| `tech-stack.md` | Languages, frameworks, and dependencies |
| `summary.md` | This file - purpose and format explanation |

## Usage Guidelines

- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.

## Notes

- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Only files matching these patterns are included: **/*.ts, **/*.tsx, **/*.astro, **/*.js, **/*.jsx, **/*.rs, **/*.py, **/*.go, **/*.md, **/*.mdx, **/astro.config.*, **/tailwind.config.*, **/vite.config.*, **/*.config.*, **/Cargo.toml, **/package.json
- Files matching these patterns are excluded: .claude/**, .cursor/**, .codex/**, .aider/**, .continue/**, .github/copilot/**, **/node_modules/**
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)

## Statistics

1094 files | 249,119 lines

| Language | Files | Lines |
|----------|------:|------:|
| Rust | 489 | 84,207 |
| TypeScript | 273 | 29,024 |
| JavaScript | 167 | 25,295 |
| TypeScript (TSX) | 63 | 6,206 |
| Markdown | 45 | 102,309 |
| TOML | 42 | 1,221 |
| ASTRO | 9 | 508 |
| JSON | 5 | 208 |
| Go | 1 | 141 |

**Largest files:**
- `crates/appzcrawl/repomix-output.md` (94,348 lines)
- `crates/appzcrawl/api/dist/db/schema.d.ts` (3,175 lines)
- `crates/appzcrawl/containers/appzcrawl/crates/native/src/html.rs` (1,643 lines)
- `crates/crawl-core/src/html.rs` (1,643 lines)
- `crates/appzcrawl/api/dist/scraper/scrapeURL/transformers/llmExtract.js` (1,152 lines)
- `crates/sandbox/src/scoped_fs.rs` (1,082 lines)
- `crates/appzcrawl/containers/appzcrawl/crates/native/src/document/providers/docx.rs` (1,074 lines)
- `crates/appzcrawl/containers/appzcrawl/crates/native/src/crawler.rs` (1,057 lines)
- `crates/crawl-core/src/crawler.rs` (1,057 lines)
- `crates/app/src/importer.rs` (1,049 lines)