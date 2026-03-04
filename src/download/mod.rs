//! Download orchestration module.
//!
//! This module handles all download operations including HTTP fetching,
//! parallel segment downloads, and progress tracking.

mod api;
pub mod config;
pub mod engine;
pub mod manager;
pub(crate) mod types;
mod worker;

pub use config::postprocess::{
    AudioCodec, EncodingPreset, FfmpegFilter, PostProcessConfig, Resolution, VideoCodec, WatermarkPosition,
};
pub use config::progress::ProgressTracker;
pub use config::speed_profile::SpeedProfile;
pub use engine::fetcher::Fetcher;
pub use engine::partial::PartialRange;
pub use engine::range_fetcher::HttpRangeFetcher;
pub use manager::{DownloadManager, DownloadPriority, DownloadStatus, ManagerConfig};
