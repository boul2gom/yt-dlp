//! Segment download module.
//!
//! This module handles downloading individual segments of a file in parallel.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;

/// Context for segment download operations
pub struct SegmentContext {
    pub file: Arc<Mutex<tokio::fs::File>>,
    pub downloaded_bytes: Arc<AtomicU64>,
    pub progress_callback: Option<Arc<dyn Fn(u64, u64) + Send + Sync>>,
    pub total_bytes: u64,
}

impl SegmentContext {
    /// Creates a new segment context
    pub fn new(
        file: Arc<Mutex<tokio::fs::File>>,
        total_bytes: u64,
        progress_callback: Option<Arc<dyn Fn(u64, u64) + Send + Sync>>,
    ) -> Self {
        Self {
            file,
            downloaded_bytes: Arc::new(AtomicU64::new(0)),
            progress_callback,
            total_bytes,
        }
    }

    /// Updates the progress
    pub fn update_progress(&self, bytes: u64) {
        let downloaded = self.downloaded_bytes.fetch_add(bytes, Ordering::Relaxed);
        if let Some(callback) = &self.progress_callback {
            callback(downloaded + bytes, self.total_bytes);
        }
    }
}
