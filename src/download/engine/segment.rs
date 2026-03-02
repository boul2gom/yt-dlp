//! Segment download module.
//!
//! This module handles downloading individual segments of a file in parallel.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::download::types::ProgressCallback;

/// Context for segment download operations
///
/// Provides shared state for parallel segment downloads including file handle,
/// progress tracking, and callback notification
pub struct SegmentContext {
    /// Shared file handle for positional (lock-free) segment writes.
    pub file: Arc<std::fs::File>,
    /// Atomic counter for total downloaded bytes across all segments
    pub downloaded_bytes: Arc<AtomicU64>,
    /// Optional callback for progress notifications
    pub progress_callback: Option<ProgressCallback>,
    /// Total size of the file in bytes
    pub total_bytes: u64,
    /// Byte offset subtracted from URL-absolute segment positions to obtain file-write positions.
    ///
    /// For regular full-file downloads this is `0`. For range downloads it equals `byte_start`
    /// so that segment data is always written from offset 0 in the destination file.
    pub file_offset_base: u64,
    /// Whether this download is resuming a partial file.
    ///
    /// When `false`, `is_segment_downloaded` checks are skipped — the file is freshly
    /// created and cannot have any pre-existing segment data.
    pub is_resuming: bool,
}

impl std::fmt::Debug for SegmentContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SegmentContext")
            .field("downloaded_bytes", &self.downloaded_bytes.load(Ordering::Relaxed))
            .field("total_bytes", &self.total_bytes)
            .field("has_callback", &self.progress_callback.is_some())
            .finish()
    }
}

impl std::fmt::Display for SegmentContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SegmentContext(downloaded={}, total={})",
            self.downloaded_bytes.load(Ordering::Relaxed),
            self.total_bytes
        )
    }
}

impl SegmentContext {
    /// Creates a new segment context
    ///
    /// # Arguments
    ///
    /// * `file` - Shared file handle for writing segments
    /// * `total_bytes` - Total size of the file in bytes
    /// * `progress_callback` - Optional callback for progress updates
    ///
    /// # Returns
    ///
    /// A new SegmentContext instance
    pub fn new(file: Arc<std::fs::File>, total_bytes: u64, progress_callback: Option<ProgressCallback>) -> Self {
        tracing::debug!(
            total_bytes = total_bytes,
            has_callback = progress_callback.is_some(),
            "⚙️ Created new segment context"
        );

        Self {
            file,
            downloaded_bytes: Arc::new(AtomicU64::new(0)),
            progress_callback,
            total_bytes,
            file_offset_base: 0,
            is_resuming: false,
        }
    }

    /// Updates the progress
    ///
    /// # Arguments
    ///
    /// * `bytes` - Number of bytes just downloaded
    pub fn update_progress(&self, bytes: u64) {
        let downloaded = self.downloaded_bytes.fetch_add(bytes, Ordering::Relaxed);
        let new_total = downloaded + bytes;

        let percentage = (new_total as f64 / self.total_bytes as f64) * 100.0;
        tracing::debug!(
            bytes_downloaded = bytes,
            total_downloaded = new_total,
            total_bytes = self.total_bytes,
            percentage = percentage,
            "📥 Segment progress updated"
        );

        if let Some(callback) = &self.progress_callback {
            callback(new_total, self.total_bytes);
        }
    }
}
