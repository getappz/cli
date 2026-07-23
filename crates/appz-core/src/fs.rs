use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use glob::glob;
use regex::Regex;

/// Filesystem stat entry, adopted from Vercel's `DetectorFilesystemStat`.
#[derive(Debug, Clone)]
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub is_file: bool,
}

/// Cached filesystem probe, adopted from Vercel's `DetectorFilesystem`.
///
/// Caches all probe results so multiple detectors de-dup reads.
pub struct DetectorFilesystem {
    root: PathBuf,
    path_cache: Mutex<HashMap<String, bool>>,
    file_cache: Mutex<HashMap<String, bool>>,
    read_cache: Mutex<HashMap<String, Option<String>>>,
    readdir_cache: Mutex<HashMap<String, Vec<FsEntry>>>,
}

impl DetectorFilesystem {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            path_cache: Mutex::new(HashMap::new()),
            file_cache: Mutex::new(HashMap::new()),
            read_cache: Mutex::new(HashMap::new()),
            readdir_cache: Mutex::new(HashMap::new()),
        }
    }

    fn abs_path(&self, name: &str) -> PathBuf {
        if let Some(stripped) = name.strip_prefix('/') {
            self.root.join(stripped)
        } else {
            self.root.join(name)
        }
    }

    pub fn has_path(&self, path: &str) -> bool {
        let mut cache = self.path_cache.lock().unwrap();
        *cache
            .entry(path.to_string())
            .or_insert_with(|| self.abs_path(path).exists())
    }

    pub fn is_file(&self, name: &str) -> bool {
        let mut cache = self.file_cache.lock().unwrap();
        *cache
            .entry(name.to_string())
            .or_insert_with(|| self.abs_path(name).is_file())
    }

    pub fn read_file(&self, name: &str) -> Option<String> {
        let mut cache = self.read_cache.lock().unwrap();
        cache
            .entry(name.to_string())
            .or_insert_with(|| fs::read_to_string(self.abs_path(name)).ok())
            .clone()
    }

    pub fn readdir(&self, dir_path: &str) -> Vec<FsEntry> {
        let mut cache = self.readdir_cache.lock().unwrap();
        cache
            .entry(dir_path.to_string())
            .or_insert_with(|| {
                let p = if dir_path.is_empty() || dir_path == "." {
                    self.root.clone()
                } else {
                    self.abs_path(dir_path)
                };
                fs::read_dir(&p)
                    .ok()
                    .into_iter()
                    .flat_map(|entries| {
                        entries
                            .filter_map(|e| e.ok())
                            .map(|e| {
                                let name = e.file_name().to_string_lossy().to_string();
                                let path = format!("{}/{}", dir_path.trim_end_matches('/'), name);
                                let is_file = e.file_type().map(|t| t.is_file()).unwrap_or(false);
                                FsEntry {
                                    name,
                                    path,
                                    is_file,
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect()
            })
            .clone()
    }

    /// Check if a glob pattern matches any files.
    pub fn has_glob(&self, pattern: &str) -> bool {
        let full = self.root.join(pattern);
        let s = full.to_string_lossy();
        glob(&s)
            .ok()
            .is_some_and(|entries| entries.filter_map(|e| e.ok()).any(|p| p.is_file()))
    }

    /// Check a detector: exact path or glob, with optional content regex and package match.
    pub fn check_detector(
        &self,
        path: &str,
        is_glob: bool,
        match_content: Option<&str>,
        match_package: Option<&str>,
    ) -> bool {
        // match_package: check if a dependency exists in package.json
        if let Some(pkg) = match_package {
            let content = match self.read_file("package.json") {
                Some(c) => c,
                None => return false,
            };
            return crate::pkg::has_dependency(&content, pkg);
        }

        if is_glob {
            let full = self.root.join(path);
            let s = full.to_string_lossy();
            let entries: Vec<_> = glob(&s)
                .ok()
                .into_iter()
                .flat_map(|e| e.filter_map(|p| p.ok()))
                .filter(|p| p.is_file())
                .collect();

            if entries.is_empty() {
                return false;
            }

            match match_content {
                None => true,
                Some(re_str) => {
                    let re = Regex::new(re_str).ok();
                    let re = match re {
                        Some(r) => r,
                        None => return false,
                    };
                    entries
                        .iter()
                        .any(|p| fs::read_to_string(p).ok().is_some_and(|c| re.is_match(&c)))
                }
            }
        } else {
            if !self.is_file(path) {
                return false;
            }
            match match_content {
                None => true,
                Some(re_str) => {
                    let content = self.read_file(path);
                    content
                        .is_some_and(|c| Regex::new(re_str).ok().is_some_and(|re| re.is_match(&c)))
                }
            }
        }
    }
}
