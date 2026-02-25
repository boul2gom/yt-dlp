//! Progress tracking module.
//!
//! This module provides stream-based progress tracking for downloads.

use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

/// Progress information for a download
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressInfo {
    /// Downloaded bytes
    pub downloaded: u64,
    /// Total bytes
    pub total: u64,
}

impl ProgressInfo {
    /// Creates a new progress info
    ///
    /// # Arguments
    ///
    /// * `downloaded` - Number of bytes downloaded
    /// * `total` - Total number of bytes
    ///
    /// # Returns
    ///
    /// A new ProgressInfo instance
    pub fn new(downloaded: u64, total: u64) -> Self {
        Self { downloaded, total }
    }

    /// Returns the progress as a percentage (0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// Progress as a percentage from 0.0 to 1.0
    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.downloaded as f64 / self.total as f64
        }
    }
}

impl std::fmt::Display for ProgressInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ProgressInfo(downloaded={}, total={}, percent={:.1}%)",
            self.downloaded,
            self.total,
            self.percentage() * 100.0
        )
    }
}

/// Progress tracker for downloads
#[derive(Debug)]
pub struct ProgressTracker {
    tx: broadcast::Sender<ProgressInfo>,
}

impl ProgressTracker {
    /// Creates a new progress tracker
    ///
    /// # Returns
    ///
    /// A new ProgressTracker instance with a broadcast channel
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);

        tracing::debug!(capacity = 100, "⚙️ Created new progress tracker");

        Self { tx }
    }

    /// Updates the progress
    ///
    /// # Arguments
    ///
    /// * `downloaded` - Number of bytes downloaded
    /// * `total` - Total number of bytes
    pub fn update(&self, downloaded: u64, total: u64) {
        let percentage = if total > 0 {
            (downloaded as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        tracing::debug!(
            downloaded = downloaded,
            total = total,
            percentage = percentage,
            "📥 Progress updated"
        );

        let _ = self.tx.send(ProgressInfo::new(downloaded, total));
    }

    /// Creates a stream of progress updates
    ///
    /// # Returns
    ///
    /// A BroadcastStream that receives progress updates
    pub fn stream(&self) -> BroadcastStream<ProgressInfo> {
        tracing::debug!("📥 Creating progress stream");

        BroadcastStream::new(self.tx.subscribe())
    }

    /// Creates a callback function for progress updates
    ///
    /// # Returns
    ///
    /// A callback function that can be used to update progress
    pub fn callback(&self) -> impl Fn(u64, u64) + Send + Sync + 'static {
        tracing::debug!("⚙️ Creating progress callback");

        let tx = self.tx.clone();
        move |downloaded, total| {
            let _ = tx.send(ProgressInfo::new(downloaded, total));
        }
    }
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}
