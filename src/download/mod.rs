//! Download orchestration module.
//!
//! This module handles all download operations including HTTP fetching,
//! parallel segment downloads, and progress tracking.

pub mod fetcher;
pub mod manager;
pub mod progress;
pub mod segment;

pub use fetcher::Fetcher;
pub use manager::{DownloadManager, DownloadPriority, DownloadStatus, ManagerConfig};
pub use progress::ProgressTracker;
