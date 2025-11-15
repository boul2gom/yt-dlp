//! Progress tracking module.
//!
//! This module provides stream-based progress tracking for downloads.

use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

/// Progress information for a download
#[derive(Debug, Clone, Copy)]
pub struct ProgressInfo {
    /// Downloaded bytes
    pub downloaded: u64,
    /// Total bytes
    pub total: u64,
}

impl ProgressInfo {
    /// Creates a new progress info
    pub fn new(downloaded: u64, total: u64) -> Self {
        Self { downloaded, total }
    }

    /// Returns the progress as a percentage (0.0 to 1.0)
    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.downloaded as f64 / self.total as f64
        }
    }
}

/// Progress tracker for downloads
#[derive(Debug)]
pub struct ProgressTracker {
    tx: broadcast::Sender<ProgressInfo>,
}

impl ProgressTracker {
    /// Creates a new progress tracker
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self { tx }
    }

    /// Updates the progress
    pub fn update(&self, downloaded: u64, total: u64) {
        let _ = self.tx.send(ProgressInfo::new(downloaded, total));
    }

    /// Creates a stream of progress updates
    pub fn stream(&self) -> BroadcastStream<ProgressInfo> {
        BroadcastStream::new(self.tx.subscribe())
    }

    /// Creates a callback function for progress updates
    pub fn callback(&self) -> impl Fn(u64, u64) + Send + Sync + 'static {
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
