//! Progress indicators for async operations.

use indicatif::{ProgressBar, ProgressStyle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::LazyLock as Lazy;
use std::time::Duration;

/// Tick interval for spinner (250ms, matching mise)
const TICK_INTERVAL: Duration = Duration::from_millis(250);

/// Spinner template (matching mise's pattern)
static SPINNER_TEMPLATE: Lazy<ProgressStyle> = Lazy::new(|| {
    ProgressStyle::with_template("{spinner:.blue} {msg} {elapsed:>3.dim.italic}").unwrap()
});

/// Progress bar template (matching mise's pattern)
static PROGRESS_TEMPLATE: Lazy<ProgressStyle> = Lazy::new(|| {
    ProgressStyle::with_template(
        "{msg} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})",
    )
    .unwrap()
    .progress_chars("=> ")
});

/// A spinner for indicating ongoing async operations.
pub struct SpinnerHandle {
    spinner: Arc<ProgressBar>,
}

impl SpinnerHandle {
    /// Create a new spinner with a message.
    ///
    /// # Arguments
    /// * `message` - Message to display with the spinner
    ///
    /// # Returns
    /// A `SpinnerHandle` that can be finished or dropped
    pub fn new(message: &str) -> Self {
        let spinner = Arc::new(ProgressBar::new_spinner());
        spinner.set_message(message.to_string());
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        spinner.enable_steady_tick(Duration::from_millis(100));

        Self { spinner }
    }

    /// Update the spinner message.
    ///
    /// # Arguments
    /// * `message` - New message to display
    pub fn set_message(&self, message: &str) {
        self.spinner.set_message(message.to_string());
    }

    /// Finish the spinner with a success message.
    ///
    /// # Arguments
    /// * `message` - Final message to display
    pub fn finish_with_message(&self, message: &str) {
        self.spinner.finish_with_message(message.to_string());
    }

    /// Finish the spinner.
    pub fn finish(&self) {
        self.spinner.finish();
    }
}

impl Drop for SpinnerHandle {
    fn drop(&mut self) {
        self.spinner.finish();
    }
}

/// Create a new spinner with a message.
///
/// # Arguments
/// * `message` - Message to display with the spinner
///
/// # Returns
/// A `SpinnerHandle` that can be finished or dropped
pub fn spinner(message: &str) -> SpinnerHandle {
    SpinnerHandle::new(message)
}

/// A progress bar for long-running operations.
/// Starts as a spinner when total is 0 or unknown, switches to progress bar when length is set.
pub struct ProgressBarHandle {
    pb: Arc<ProgressBar>,
    is_spinner: Arc<AtomicBool>,
}

impl ProgressBarHandle {
    /// Create a new progress bar.
    /// If `total` is 0, starts as a spinner and switches to progress bar when `set_length` is called.
    ///
    /// # Arguments
    /// * `total` - Total number of items/steps (0 for unknown/spinner mode)
    /// * `message` - Message to display
    ///
    /// # Returns
    /// A `ProgressBarHandle` for updating progress
    pub fn new(total: u64, message: &str) -> Self {
        let is_spinner = Arc::new(AtomicBool::new(total == 0));

        // Always create a ProgressBar (not new_spinner), matching mise's pattern
        // Use a placeholder length of 100 for spinner mode
        let pb = Arc::new(ProgressBar::new(if total == 0 { 100 } else { total }));
        pb.set_message(message.to_string());

        if total == 0 {
            // Start as spinner (unknown total) - use spinner template and enable tick
            pb.set_style(SPINNER_TEMPLATE.clone());
            pb.enable_steady_tick(TICK_INTERVAL);
        } else {
            // Start as progress bar (known total)
            pb.set_style(PROGRESS_TEMPLATE.clone());
        }

        Self { pb, is_spinner }
    }

    /// Increment the progress by 1.
    pub fn inc(&self) {
        self.pb.inc(1);
    }

    /// Increment the progress by a specific amount.
    ///
    /// # Arguments
    /// * `n` - Amount to increment
    pub fn inc_by(&self, n: u64) {
        self.pb.inc(n);
    }

    /// Set the current position.
    ///
    /// # Arguments
    /// * `pos` - Current position
    pub fn set_position(&self, pos: u64) {
        self.pb.set_position(pos);
    }

    /// Set the total length of the progress bar.
    /// If currently a spinner, switches to progress bar mode.
    ///
    /// # Arguments
    /// * `len` - Total length
    pub fn set_length(&self, len: u64) {
        // If we're currently a spinner, switch to progress bar
        if self
            .is_spinner
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            // Disable steady tick (spinner mode)
            self.pb.disable_steady_tick();

            // Switch to progress bar template
            self.pb.set_style(PROGRESS_TEMPLATE.clone());

            // Reset position to 0 (matching mise's behavior)
            self.pb.set_position(0);
        }

        // Set the length
        self.pb.set_length(len);
    }

    /// Update the message.
    ///
    /// # Arguments
    /// * `message` - New message
    pub fn set_message(&self, message: &str) {
        self.pb.set_message(message.to_string());
    }

    /// Finish the progress bar with a message.
    ///
    /// # Arguments
    /// * `message` - Final message
    pub fn finish_with_message(&self, message: &str) {
        self.pb.finish_with_message(message.to_string());
    }

    /// Finish the progress bar.
    pub fn finish(&self) {
        self.pb.finish();
    }
}

impl grab::Progress for ProgressBarHandle {
    fn set_length(&self, len: u64) {
        ProgressBarHandle::set_length(self, len);
    }

    fn inc(&self, n: u64) {
        self.inc_by(n);
    }

    fn finish(&self) {
        ProgressBarHandle::finish(self);
    }
}

impl Drop for ProgressBarHandle {
    fn drop(&mut self) {
        self.pb.finish();
    }
}

/// Create a new progress bar.
/// If `total` is 0, starts as a spinner and switches to progress bar when `set_length` is called.
///
/// # Arguments
/// * `total` - Total number of items/steps (0 for unknown/spinner mode)
/// * `message` - Message to display
///
/// # Returns
/// A `ProgressBarHandle` for updating progress
pub fn progress_bar(total: u64, message: &str) -> ProgressBarHandle {
    ProgressBarHandle::new(total, message)
}
