//! Progress reporting for downloads.
//!
//! Implement this trait to report download progress (e.g. from `ui::ProgressBarHandle`).

/// Progress callback for download operations.
/// Implement for progress bars or other UI; pass `None` for no progress.
pub trait Progress: Send + Sync {
    /// Set total length when known (e.g. from Content-Length).
    fn set_length(&self, len: u64);

    /// Increment progress by `n` bytes.
    fn inc(&self, n: u64);

    /// Called when download is finished (success or not).
    fn finish(&self);
}
