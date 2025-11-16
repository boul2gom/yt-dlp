//! Download orchestration module.
//!
//! This module handles all download operations including HTTP fetching,
//! parallel segment downloads, and progress tracking.

pub mod fetcher;
pub mod manager;
pub mod partial;
pub mod postprocess;
pub mod progress;
pub mod segment;
pub mod speed_profile;

pub use fetcher::Fetcher;
pub use manager::{DownloadManager, DownloadPriority, DownloadStatus, ManagerConfig};
pub use partial::PartialRange;
pub use postprocess::{
    AudioCodec, EncodingPreset, FfmpegFilter, PostProcessConfig, Resolution, VideoCodec,
    WatermarkPosition,
};
pub use progress::ProgressTracker;
pub use speed_profile::SpeedProfile;
