//! Performance timing for debugging startup and execution.
//!
//! Enable with `APPZ_DEBUG_TIMING=1` (or `true`/`yes`). Timings are printed to stderr.

use std::time::{Duration, Instant};

/// Tracks elapsed time per phase for performance debugging.
pub struct TimingDebug {
    enabled: bool,
    start: Instant,
    last: Instant,
    phases: Vec<(String, Duration)>,
}

impl TimingDebug {
    /// Create a new timer. Checks `APPZ_DEBUG_TIMING` env var.
    pub fn new() -> Self {
        let enabled = std::env::var("APPZ_DEBUG_TIMING")
            .map(|v| {
                let v = v.to_lowercase();
                v == "1" || v == "true" || v == "yes" || v == "on"
            })
            .unwrap_or(false);

        let now = Instant::now();
        Self {
            enabled,
            start: now,
            last: now,
            phases: Vec::new(),
        }
    }

    /// Record a checkpoint. The duration is since the last checkpoint (or start).
    pub fn checkpoint(&mut self, name: &str) {
        if self.enabled {
            let elapsed = self.last.elapsed();
            self.phases.push((name.to_string(), elapsed));
            self.last = Instant::now();
        }
    }

    /// Print all timings to stderr.
    pub fn print(&self) {
        if self.enabled && !self.phases.is_empty() {
            eprintln!("[appz] Performance timings (APPZ_DEBUG_TIMING=1):");
            for (name, d) in &self.phases {
                eprintln!("  {:20} {:>10} ms", name, d.as_millis());
            }
            eprintln!("  {:20} {:>10} ms", "TOTAL", self.start.elapsed().as_millis());
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for TimingDebug {
    fn default() -> Self {
        Self::new()
    }
}
