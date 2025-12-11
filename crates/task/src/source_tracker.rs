//! Source and output tracking for task execution.
//!
//! This module implements mise-style source/output tracking to skip task execution
//! when sources haven't changed since the last run.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use glob::glob;
use miette::{miette, Result};
use serde::{Deserialize, Serialize};
use starbase_utils::{fs, json};

/// Cache entry for a task's execution metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskCacheEntry {
    /// Timestamp of last successful execution (Unix timestamp in seconds)
    last_run: u64,
    /// Hash of source file modification times (for change detection)
    sources_hash: String,
    /// Hash of output file modification times (for change detection)
    outputs_hash: String,
}

/// Cache structure for all tasks
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct TaskCache {
    tasks: HashMap<String, TaskCacheEntry>,
}

/// Tracks source and output files for task execution optimization
pub struct SourceTracker {
    cache_path: PathBuf,
    working_dir: PathBuf,
    cache: TaskCache,
}

impl SourceTracker {
    /// Create a new SourceTracker
    pub fn new(working_dir: PathBuf) -> Self {
        let cache_dir = working_dir.join(".appz").join("cache");
        let cache_path = cache_dir.join("tasks.json");

        // Load existing cache if it exists
        let cache = if cache_path.exists() {
            json::read_file(&cache_path).unwrap_or_default()
        } else {
            TaskCache::default()
        };

        Self {
            cache_path,
            working_dir,
            cache,
        }
    }

    /// Check if a task should be skipped based on source/output timestamps
    /// Returns `true` if task should be skipped (sources are up-to-date)
    /// Returns `false` if task should run (sources changed or outputs missing)
    pub fn should_skip_task(
        &self,
        task_name: &str,
        sources: &[String],
        outputs: &[String],
    ) -> Result<bool> {
        // If no sources or outputs defined, always run
        if sources.is_empty() && outputs.is_empty() {
            return Ok(false);
        }

        // If sources are empty but outputs exist, check if outputs are newer than last run
        if sources.is_empty() {
            return self.check_outputs_only(task_name, outputs);
        }

        // If outputs are empty but sources exist, always run (can't verify)
        if outputs.is_empty() {
            return Ok(false);
        }

        // Get modification times for all source files
        let source_times = self.get_file_modification_times(sources)?;

        // Get modification times for all output files
        let output_times = self.get_file_modification_times(outputs)?;

        // If no source files found, run the task
        if source_times.is_empty() {
            return Ok(false);
        }

        // If no output files found, run the task
        if output_times.is_empty() {
            return Ok(false);
        }

        // Find the newest source file and oldest output file
        let newest_source = source_times.values().max().copied();
        let oldest_output = output_times.values().min().copied();

        match (newest_source, oldest_output) {
            (Some(newest_src), Some(oldest_out)) => {
                // Skip if newest source is older than oldest output
                Ok(newest_src < oldest_out)
            }
            _ => Ok(false),
        }
    }

    /// Check if outputs-only task should be skipped
    fn check_outputs_only(&self, task_name: &str, outputs: &[String]) -> Result<bool> {
        // Get cache entry for this task
        let cache_entry = match self.cache.tasks.get(task_name) {
            Some(entry) => entry,
            None => return Ok(false), // Never run, so run it
        };

        // Get current output modification times
        let output_times = self.get_file_modification_times(outputs)?;

        if output_times.is_empty() {
            return Ok(false); // No outputs, run task
        }

        // Check if outputs are newer than last run
        let last_run_time =
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(cache_entry.last_run);
        let oldest_output = output_times.values().min().copied();

        match oldest_output {
            Some(out_time) => {
                // Skip if outputs are newer than last run
                Ok(out_time > last_run_time)
            }
            None => Ok(false),
        }
    }

    /// Get modification times for files matching glob patterns
    fn get_file_modification_times(
        &self,
        patterns: &[String],
    ) -> Result<HashMap<PathBuf, SystemTime>> {
        let mut times = HashMap::new();

        for pattern in patterns {
            // Resolve glob pattern relative to working directory
            let full_pattern = if Path::new(pattern).is_absolute() {
                pattern.clone()
            } else {
                // Convert to string for glob matching
                self.working_dir.join(pattern).to_string_lossy().to_string()
            };

            // Use glob crate for pattern matching
            match glob(&full_pattern) {
                Ok(paths) => {
                    for entry in paths.flatten() {
                        if entry.is_file() {
                            if let Ok(metadata) = std::fs::metadata(&entry) {
                                if let Ok(modified) = metadata.modified() {
                                    times.insert(entry, modified);
                                }
                            }
                        }
                    }
                }
                Err(_e) => {
                    // If glob pattern is invalid, try treating it as a direct path
                    let path = PathBuf::from(&full_pattern);
                    if path.exists() && path.is_file() {
                        if let Ok(metadata) = std::fs::metadata(&path) {
                            if let Ok(modified) = metadata.modified() {
                                times.insert(path, modified);
                            }
                        }
                    } else {
                        // Pattern doesn't match anything - that's ok, just continue
                        // (No files matched - this is expected for some patterns)
                    }
                }
            }
        }

        Ok(times)
    }

    /// Record that a task was executed successfully
    pub fn record_task_execution(
        &mut self,
        task_name: &str,
        sources: &[String],
        outputs: &[String],
    ) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|e| miette!("Failed to get current time: {}", e))?
            .as_secs();

        // Calculate hashes for sources and outputs
        let source_times = self.get_file_modification_times(sources)?;
        let output_times = self.get_file_modification_times(outputs)?;

        let sources_hash = self.hash_times(&source_times);
        let outputs_hash = self.hash_times(&output_times);

        // Update cache
        self.cache.tasks.insert(
            task_name.to_string(),
            TaskCacheEntry {
                last_run: now,
                sources_hash,
                outputs_hash,
            },
        );

        // Ensure cache directory exists
        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| miette!("Failed to create cache directory: {}", e))?;
        }

        // Write cache file
        json::write_file(&self.cache_path, &self.cache, true)
            .map_err(|e| miette!("Failed to write cache file: {}", e))?;

        Ok(())
    }

    /// Check if a task has changed sources (for --changed flag)
    pub fn has_changed_sources(&self, task_name: &str, sources: &[String]) -> Result<bool> {
        if sources.is_empty() {
            return Ok(false);
        }

        // Get current source modification times
        let source_times = self.get_file_modification_times(sources)?;

        if source_times.is_empty() {
            return Ok(false);
        }

        // Get cached entry
        let cache_entry = match self.cache.tasks.get(task_name) {
            Some(entry) => entry,
            None => return Ok(true), // Never run, so changed
        };

        // Compare current hash with cached hash
        let current_hash = self.hash_times(&source_times);
        Ok(current_hash != cache_entry.sources_hash)
    }

    /// Create a hash from modification times
    fn hash_times(&self, times: &HashMap<PathBuf, SystemTime>) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        let mut sorted_paths: Vec<_> = times.keys().collect();
        sorted_paths.sort();

        for path in sorted_paths {
            path.hash(&mut hasher);
            if let Some(time) = times.get(path) {
                time.duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .hash(&mut hasher);
            }
        }

        format!("{:x}", hasher.finish())
    }
}
