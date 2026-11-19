//! Content-Addressable Storage (CAS) scanner & Merkle tree builder.
//!
//! New manifests use tagged BLAKE3 blob IDs; legacy manifests have untagged SHA-256 IDs.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

pub const MAX_FILE_SIZE: usize = 25 * 1024 * 1024; // 25 MB edge upload limit

const DEFAULT_IGNORES: &[&str] = &[
    ".git",
    ".gitmodules",
    ".gitignore",
    ".gitattributes",
    ".cache",
    ".next",
    ".vercel",
    ".appz",
    "node_modules",
    ".DS_Store",
    "Thumbs.db",
    ".appzignore",
    ".vercelignore",
];

/// Hidden directories that must be served (ACME challenges, security.txt, ...).
const HIDDEN_DIR_ALLOWLIST: &[&str] = &[".well-known"];

/// One line of an `.appzignore` / `.vercelignore` file (gitignore-style).
struct IgnoreRule {
    pattern: glob::Pattern,
    negated: bool,
    dir_only: bool,
    /// Contains a `/`: matched against the whole path from the scan root
    /// instead of against single path components.
    anchored: bool,
}

/// User ignore rules read from the scan root: `.appzignore`, else `.vercelignore`.
#[derive(Default)]
struct IgnoreRules(Vec<IgnoreRule>);

impl IgnoreRules {
    fn load(root: &Path) -> Result<(Self, Option<&'static str>), String> {
        let mut available = None;
        for name in [".appzignore", ".vercelignore"] {
            let path = root.join(name);
            match std::fs::symlink_metadata(&path) {
                Ok(metadata) if !metadata.is_file() => continue,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    available.get_or_insert(name);
                    continue;
                }
                Err(e) => return Err(format!("Failed to inspect {}: {e}", path.display())),
                Ok(_) => {}
            }
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            return Ok((Self::parse(name, &text)?, Some(name)));
        }
        Ok((Self::default(), available))
    }

    fn parse(name: &str, text: &str) -> Result<Self, String> {
        let mut rules = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let negated = line.starts_with('!');
            let line = line.strip_prefix('!').unwrap_or(line);
            let dir_only = line.ends_with('/');
            let line = line.strip_suffix('/').unwrap_or(line);
            let anchored = line.contains('/');
            let line = line.trim_start_matches('/');
            let number = index + 1;
            if line.is_empty() {
                return Err(format!("{name}:{number}: empty ignore pattern"));
            }
            if line.contains("**") {
                return Err(format!(
                    "{name}:{number}: recursive wildcard ** is unsupported"
                ));
            }
            let pattern = glob::Pattern::new(line).map_err(|e| format!("{name}:{number}: {e}"))?;
            rules.push(IgnoreRule {
                pattern,
                negated,
                dir_only,
                anchored,
            });
        }
        Ok(Self(rules))
    }

    /// `Some(true)` = ignore, `Some(false)` = explicitly kept (negation),
    /// `None` = no rule matched. The last matching rule wins, as in gitignore.
    fn decide(&self, rel: &str) -> Option<bool> {
        let parts: Vec<&str> = rel.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
        let opts = glob::MatchOptions {
            require_literal_separator: true,
            ..glob::MatchOptions::new()
        };
        let mut decision = None;
        for rule in &self.0 {
            let hit = (1..=parts.len()).any(|n| {
                let is_dir = n < parts.len();
                if rule.dir_only && !is_dir {
                    return false;
                }
                if rule.anchored {
                    rule.pattern.matches_with(&parts[..n].join("/"), opts)
                } else {
                    rule.pattern.matches_with(parts[n - 1], opts)
                }
            });
            if hit {
                decision = Some(!rule.negated);
            }
        }
        decision
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScannedFileEntry {
    pub path: String,
    pub hash: String,
    pub size: usize,
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub immutable: Option<bool>,
}

/// A unique file content, located on disk. Its bytes are read again only if
/// the server asks for the blob, so a large site is never held in memory.
#[derive(Debug, Clone)]
pub struct LocalBlob {
    pub hash: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ScannedTree {
    pub files: Vec<ScannedFileEntry>,
    pub blobs_by_hash: HashMap<String, LocalBlob>,
    pub total_bytes: usize,
}

pub(crate) fn blob_id(data: &[u8]) -> String {
    format!("b3:{}", blake3::hash(data))
}

#[inline]
pub fn guess_mime(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path);
    let ext = name.rsplit_once('.').map_or("", |(_, ext)| ext);
    if ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm") {
        "text/html; charset=utf-8"
    } else if ext.eq_ignore_ascii_case("css") {
        "text/css; charset=utf-8"
    } else if ext.eq_ignore_ascii_case("js") || ext.eq_ignore_ascii_case("mjs") {
        "application/javascript; charset=utf-8"
    } else if ext.eq_ignore_ascii_case("json") || ext.eq_ignore_ascii_case("map") {
        "application/json; charset=utf-8"
    } else if ext.eq_ignore_ascii_case("svg") {
        "image/svg+xml"
    } else if ext.eq_ignore_ascii_case("png") {
        "image/png"
    } else if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") {
        "image/jpeg"
    } else if ext.eq_ignore_ascii_case("webp") {
        "image/webp"
    } else if ext.eq_ignore_ascii_case("gif") {
        "image/gif"
    } else if ext.eq_ignore_ascii_case("ico") {
        "image/x-icon"
    } else if ext.eq_ignore_ascii_case("woff") {
        "font/woff"
    } else if ext.eq_ignore_ascii_case("woff2") {
        "font/woff2"
    } else if ext.eq_ignore_ascii_case("ttf") {
        "font/ttf"
    } else if ext.eq_ignore_ascii_case("txt") {
        "text/plain; charset=utf-8"
    } else if ext.eq_ignore_ascii_case("xml") {
        "application/xml; charset=utf-8"
    } else if ext.eq_ignore_ascii_case("wasm") {
        "application/wasm"
    } else if ext.eq_ignore_ascii_case("avif") {
        "image/avif"
    } else if ext.eq_ignore_ascii_case("otf") {
        "font/otf"
    } else if ext.eq_ignore_ascii_case("pdf") {
        "application/pdf"
    } else if ext.eq_ignore_ascii_case("webmanifest") {
        "application/manifest+json"
    } else if ext.eq_ignore_ascii_case("mp4") {
        "video/mp4"
    } else if ext.eq_ignore_ascii_case("webm") {
        "video/webm"
    } else if ext.eq_ignore_ascii_case("mp3") {
        "audio/mpeg"
    } else {
        "application/octet-stream"
    }
}

/// Whether a file can be cached forever: its URL changes whenever its content
/// does. Bundler-only folders (`/_astro/`, `/_next/static/`) always qualify;
/// in the shared `/assets/` and `/static/` folders only a file whose name
/// carries a content hash does — a hand-written `/assets/style.css` is edited
/// in place, and marking it immutable would serve stale copies after a deploy.
pub fn is_immutable_asset(path: &str) -> bool {
    if path.starts_with("/_astro/") || path.starts_with("/_next/static/") || path.contains(".hash.")
    {
        return true;
    }
    (path.starts_with("/assets/") || path.starts_with("/static/")) && has_content_hash(path)
}

/// A name like `index-B1x2c3D4.js` or `main.073c9b0a.chunk.css`: some segment
/// after the first is a hash (8+ characters: mixed-case or hex with digits,
/// so ordinary words or version labels like `version2024` don't count).
pub(crate) fn has_content_hash(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    stem.split(['.', '-']).skip(1).any(|seg| {
        if seg.len() < 8 {
            return false;
        }
        let mut has_digit = false;
        let mut has_upper = false;
        let mut has_lower = false;
        let mut all_hex = true;
        for &b in seg.as_bytes() {
            if b.is_ascii_digit() {
                has_digit = true;
            } else if b.is_ascii_uppercase() {
                has_upper = true;
            } else if b.is_ascii_lowercase() {
                has_lower = true;
            } else if b != b'_' {
                return false;
            }
            if !b.is_ascii_hexdigit() {
                all_hex = false;
            }
        }
        (has_upper && has_lower) || (all_hex && has_digit)
    })
}

/// Whether a file is left out of the deploy. The user's ignore file is checked
/// first (a `!negation` there overrides the defaults); then the built-in names
/// and hidden directories (except `.well-known`). Whole components only: a
/// substring match would also drop `app.cache.js`.
fn is_ignored(rel: &str, rules: &IgnoreRules) -> bool {
    if let Some(decision) = rules.decide(rel) {
        return decision;
    }
    let parts: Vec<&str> = rel.split(['/', '\\']).collect();
    let last = parts.len() - 1;
    parts.iter().enumerate().any(|(i, part)| {
        DEFAULT_IGNORES.contains(part)
            || (i < last && part.starts_with('.') && !HIDDEN_DIR_ALLOWLIST.contains(part))
    })
}

fn file_blob_id(path: &Path) -> std::io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(std::fs::File::open(path)?)?;
    Ok(format!("b3:{}", hasher.finalize()))
}

fn normalize_rel_path(rel: &Path) -> String {
    let rel_str = rel.to_string_lossy();
    let mut norm = String::with_capacity(rel_str.len() + 1);
    if !rel_str.starts_with('/') && !rel_str.starts_with('\\') {
        norm.push('/');
    }
    for c in rel_str.chars() {
        if c == '\\' {
            norm.push('/');
        } else {
            norm.push(c);
        }
    }
    norm
}

struct Candidate {
    abs_path: PathBuf,
    norm_path: String,
    size: usize,
    mime: &'static str,
    immutable: bool,
}

fn oversized_file_error(path: &str, size: usize, ignore_file: Option<&str>) -> String {
    let message =
        format!("File {path:?} exceeds maximum allowed size of 25 MB (size: {size} bytes).");
    if path.contains(['\r', '\n']) {
        return format!(
            "{message} Rename the file to remove line breaks or move it outside the output directory."
        );
    }
    let Some(name) = ignore_file else {
        return format!(
            "{message} Move this file outside the output directory, or replace the non-regular .appzignore entry with a regular ignore file."
        );
    };
    let mut rule = glob::Pattern::escape(path);
    if rule.ends_with(char::is_whitespace) {
        let last = rule.pop().unwrap();
        rule.push_str(&format!("[{last}]"));
    }
    format!("{message} Add this rule to {name} to leave it out of the deploy:\n{rule}")
}

pub fn scan_output_dir(output_dir: &Path) -> Result<ScannedTree, String> {
    if !output_dir.is_dir() {
        return Err(format!(
            "Output directory not found: {}",
            output_dir.display()
        ));
    }

    let mut candidates = Vec::new();
    let (rules, ignore_file) = IgnoreRules::load(output_dir)?;

    for entry in WalkDir::new(output_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let abs_path = entry.path();
        let rel = match abs_path.strip_prefix(output_dir) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let rel_lossy = rel.to_string_lossy();
        if is_ignored(&rel_lossy, &rules) {
            continue;
        }

        let norm_path = normalize_rel_path(rel);

        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to read metadata for {}: {e}", abs_path.display()))?;
        let size = metadata.len() as usize;
        if size > MAX_FILE_SIZE {
            return Err(oversized_file_error(&norm_path, size, ignore_file));
        }

        let mime = guess_mime(&norm_path);
        let immutable = is_immutable_asset(&norm_path);

        candidates.push(Candidate {
            abs_path: abs_path.to_path_buf(),
            norm_path,
            size,
            mime,
            immutable,
        });
    }

    if candidates.is_empty() {
        return Ok(ScannedTree {
            files: Vec::new(),
            blobs_by_hash: HashMap::new(),
            total_bytes: 0,
        });
    }

    let count = candidates.len();
    let mut files = Vec::with_capacity(count);
    let mut blobs_by_hash = HashMap::with_capacity(count);
    let mut total_bytes = 0;

    if count == 1 {
        let c = &candidates[0];
        let hash = file_blob_id(&c.abs_path)
            .map_err(|e| format!("Failed to read {}: {e}", c.abs_path.display()))?;
        total_bytes += c.size;
        files.push(ScannedFileEntry {
            path: c.norm_path.clone(),
            hash: hash.clone(),
            size: c.size,
            r#type: c.mime.to_string(),
            immutable: if c.immutable { Some(true) } else { None },
        });
        blobs_by_hash.insert(
            hash.clone(),
            LocalBlob {
                hash,
                path: c.abs_path.clone(),
            },
        );
        return Ok(ScannedTree {
            files,
            blobs_by_hash,
            total_bytes,
        });
    }

    // Multi-threaded Merkle tree calculation across available cores
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 8)
        .min(count);

    let next_idx = AtomicUsize::new(0);
    let error_slot = Mutex::new(None::<String>);
    let hashes: Vec<Mutex<Option<String>>> = (0..count).map(|_| Mutex::new(None)).collect();

    std::thread::scope(|s| {
        for _ in 0..num_threads {
            s.spawn(|| {
                loop {
                    let idx = next_idx.fetch_add(1, Ordering::Relaxed);
                    if idx >= count {
                        break;
                    }
                    if error_slot.lock().unwrap().is_some() {
                        break;
                    }

                    let c = &candidates[idx];
                    match file_blob_id(&c.abs_path) {
                        Ok(hash) => {
                            *hashes[idx].lock().unwrap() = Some(hash);
                        }
                        Err(e) => {
                            let mut lock = error_slot.lock().unwrap();
                            if lock.is_none() {
                                *lock =
                                    Some(format!("Failed to read {}: {e}", c.abs_path.display()));
                            }
                            break;
                        }
                    }
                }
            });
        }
    });

    if let Some(err) = error_slot.into_inner().unwrap() {
        return Err(err);
    }

    for (c, hash_mutex) in candidates.into_iter().zip(hashes) {
        let hash = hash_mutex
            .into_inner()
            .unwrap()
            .ok_or_else(|| "Missing computed hash".to_string())?;
        total_bytes += c.size;

        files.push(ScannedFileEntry {
            path: c.norm_path,
            hash: hash.clone(),
            size: c.size,
            r#type: c.mime.to_string(),
            immutable: if c.immutable { Some(true) } else { None },
        });

        blobs_by_hash
            .entry(hash.clone())
            .or_insert_with(|| LocalBlob {
                hash,
                path: c.abs_path,
            });
    }

    Ok(ScannedTree {
        files,
        blobs_by_hash,
        total_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn blob_id_matches_the_worker_wire_format() {
        assert_eq!(
            blob_id(b"abc"),
            "b3:6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
        );
    }

    #[test]
    fn test_scan_output_dir_basic() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<h1>Hello</h1>").unwrap();
        let sub = dir.path().join("assets");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("style.css"), "body { color: red; }").unwrap();

        let scanned = scan_output_dir(dir.path()).unwrap();
        assert_eq!(scanned.files.len(), 2);
        assert_eq!(scanned.blobs_by_hash.len(), 2);

        let index_entry = scanned
            .files
            .iter()
            .find(|f| f.path == "/index.html")
            .unwrap();
        assert_eq!(index_entry.r#type, "text/html; charset=utf-8");
        assert_eq!(index_entry.immutable, None);

        let css_entry = scanned
            .files
            .iter()
            .find(|f| f.path == "/assets/style.css")
            .unwrap();
        assert_eq!(css_entry.r#type, "text/css; charset=utf-8");
        // Not content-hashed: edited in place, so never cached forever.
        assert_eq!(css_entry.immutable, None);
        assert_eq!(css_entry.hash, blob_id(b"body { color: red; }"));
        let blob = &scanned.blobs_by_hash[&css_entry.hash];
        assert_eq!(std::fs::read(&blob.path).unwrap(), b"body { color: red; }");
    }

    #[test]
    fn only_content_hashed_files_are_immutable() {
        for path in [
            "/assets/index-B1x2c3D4.js",
            "/assets/index-DkIpYxHs.css",
            "/static/js/main.1a2b3c4d.js",
            "/static/css/787.073c9b0a.chunk.css",
            "/_astro/page.B1x2c3D4.css",
            "/_next/static/chunks/app.js",
        ] {
            assert!(is_immutable_asset(path), "{path}");
        }
        for path in [
            "/assets/style.css",
            "/assets/main-navigation.js",
            "/assets/jquery-3.6.0.min.js",
            "/assets/app-version2024.js",
            "/static/logo.png",
            "/index.html",
        ] {
            assert!(!is_immutable_asset(path), "{path}");
        }
    }

    #[test]
    fn ignores_whole_path_components_only() {
        let none = IgnoreRules::default();
        assert!(is_ignored(".git/config", &none));
        assert!(is_ignored("sub\\node_modules\\x.js", &none));
        assert!(is_ignored(".DS_Store", &none));
        assert!(is_ignored(".gitbook/assets/a.png", &none));
        assert!(is_ignored(".github/workflows/x.yml", &none));
        assert!(is_ignored(".backups/site.zip", &none));
        assert!(!is_ignored("js/app.cache.js", &none));
        assert!(!is_ignored("blog/next-steps.html", &none));
        // Hidden files (not directories) are still uploaded.
        assert!(!is_ignored(".nojekyll", &none));
        assert!(is_ignored(".appzignore", &none));
        assert!(is_ignored(".vercelignore", &none));
    }

    #[test]
    fn well_known_is_kept() {
        assert!(!is_ignored(
            ".well-known/security.txt",
            &IgnoreRules::default()
        ));
    }

    #[test]
    fn negation_overrides_hidden_default() {
        let rules = IgnoreRules::parse(
            ".appzignore",
            "# c\n!.hallmark\n*.log\ndrafts/\n/private/*.md\n",
        )
        .unwrap();
        assert!(!is_ignored(".hallmark/a.json", &rules));
        assert!(is_ignored("x/debug.log", &rules));
        assert!(is_ignored("drafts/a.html", &rules));
        assert!(!is_ignored("drafts", &rules));
        assert!(is_ignored("private/a.md", &rules));
        assert!(!is_ignored("Private/a.md", &rules));
        assert!(!is_ignored("other/private/a.md", &rules));
        assert!(is_ignored(".wrangler/x", &rules));
    }

    #[test]
    fn appzignore_falls_back_to_vercelignore() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "hi").unwrap();
        std::fs::write(dir.path().join("secret.txt"), "x").unwrap();
        std::fs::write(dir.path().join(".vercelignore"), "secret.txt\n").unwrap();
        let paths = |d: &Path| -> Vec<String> {
            scan_output_dir(d)
                .unwrap()
                .files
                .into_iter()
                .map(|f| f.path)
                .collect()
        };
        assert_eq!(paths(dir.path()), vec!["/index.html"]);

        let appzignore = dir.path().join(".appzignore");
        std::fs::create_dir(&appzignore).unwrap();
        assert_eq!(paths(dir.path()), vec!["/index.html"]);
        std::fs::remove_dir(&appzignore).unwrap();

        // .appzignore wins over .vercelignore.
        std::fs::write(appzignore, "index.html\n").unwrap();
        assert_eq!(paths(dir.path()), vec!["/secret.txt"]);
    }

    #[test]
    fn invalid_ignore_file_aborts_the_scan() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".vercelignore"), "*.txt").unwrap();
        let file = dir.path().join(".appzignore");
        std::fs::write(&file, [0xff]).unwrap();
        assert!(
            scan_output_dir(dir.path())
                .unwrap_err()
                .contains(".appzignore")
        );
        for invalid in ["!/private/**/report.txt", "[unterminated"] {
            std::fs::write(&file, format!("/private/\n{invalid}\n")).unwrap();
            let err = scan_output_dir(dir.path()).unwrap_err();
            assert!(err.contains(".appzignore:2"), "{err}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_ignore_file_is_skipped() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("outside-rules"), "index.html").unwrap();
        std::os::unix::fs::symlink("outside-rules", dir.path().join(".appzignore")).unwrap();
        std::fs::write(dir.path().join(".vercelignore"), "outside-rules").unwrap();
        std::fs::write(dir.path().join("index.html"), "hi").unwrap();
        let scan = scan_output_dir(dir.path()).unwrap();
        assert_eq!(scan.files.len(), 1);
        assert_eq!(scan.files[0].path, "/index.html");
    }

    #[test]
    fn mime_types_ignore_extension_case() {
        assert_eq!(guess_mime("/IMG.PNG"), "image/png");
        assert_eq!(guess_mime("/photo.avif"), "image/avif");
        assert_eq!(guess_mime("/v1.2/LICENSE"), "application/octet-stream");
    }

    #[test]
    fn oversized_rule_preserves_whitespace_and_handles_multiline_paths() {
        for path in ["/[draft]  ", "/draft\t", "/draft\u{a0}"] {
            let err = oversized_file_error(path, MAX_FILE_SIZE + 1, Some(".appzignore"));
            let rule = err.lines().last().unwrap();
            let parsed = IgnoreRules::parse(".appzignore", rule).unwrap();
            assert!(is_ignored(path, &parsed));
            assert!(!is_ignored(path.trim_end(), &parsed));
        }
        for path in ["/draft\n.bin", "/draft\r.bin"] {
            let err = oversized_file_error(path, MAX_FILE_SIZE + 1, Some(".appzignore"));
            assert!(err.contains("Rename"));
            assert_eq!(err.lines().count(), 1);
        }
    }

    #[test]
    fn test_scan_output_dir_rejects_oversized_file() {
        let dir = tempdir().unwrap();
        let large_file = dir.path().join("[draft].bin");
        // Create sparse or truncated file simulating > 25 MB
        let f = std::fs::File::create(&large_file).unwrap();
        f.set_len((MAX_FILE_SIZE + 1024) as u64).unwrap();

        let err = scan_output_dir(dir.path()).unwrap_err();
        assert!(err.contains("exceeds maximum allowed size of 25 MB"));
        assert!(err.contains("/[draft].bin"));
        assert!(err.contains(".appzignore"));

        let active = dir.path().join(".vercelignore");
        std::fs::create_dir(dir.path().join(".appzignore")).unwrap();
        let err = scan_output_dir(dir.path()).unwrap_err();
        assert!(err.contains("Add this rule to .vercelignore"));
        std::fs::create_dir(&active).unwrap();
        let err = scan_output_dir(dir.path()).unwrap_err();
        assert!(err.contains("replace the non-regular .appzignore"));
        std::fs::remove_dir(&active).unwrap();
        std::fs::write(&active, "private.txt\n").unwrap();
        std::fs::write(dir.path().join("private.txt"), "private").unwrap();
        std::fs::write(dir.path().join("index.html"), "hi").unwrap();
        let err = scan_output_dir(dir.path()).unwrap_err();
        assert!(err.contains(".vercelignore"));
        let rule = err.lines().last().unwrap();
        std::fs::write(&active, format!("private.txt\n{rule}\n")).unwrap();
        let scan = scan_output_dir(dir.path()).unwrap();
        assert_eq!(scan.files.len(), 1);
        assert_eq!(scan.files[0].path, "/index.html");
    }
}
