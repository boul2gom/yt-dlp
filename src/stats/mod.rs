//! Statistics and analytics for download and metadata fetch operations.
//!
//! When the `statistics` feature is enabled, the library automatically tracks
//! aggregate metrics for every download and metadata fetch. The [`StatisticsTracker`]
//! subscribes to the [`crate::events::EventBus`] in a background task and maintains
//! running counters, so no polling or manual bookkeeping is required.
//!
//! Access the tracker through [`crate::Downloader::statistics`] and call
//! [`StatisticsTracker::snapshot`] to obtain a point-in-time [`GlobalSnapshot`].
//!
//! # Example
//!
//! ```rust,no_run
//! use std::path::PathBuf;
//!
//! use yt_dlp::Downloader;
//! use yt_dlp::client::deps::Libraries;
//!
//! #[tokio::main]
//! async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
//!     let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
//!     let downloader = Downloader::builder(libraries, "output").build().await?;
//!
//!     // ... perform downloads and fetches ...
//!
//!     let snapshot = downloader.statistics().snapshot().await;
//!     println!("Completed:        {}", snapshot.downloads.completed);
//!     println!("Total bytes:      {}", snapshot.downloads.total_bytes);
//!     println!(
//!         "Avg speed (B/s):  {:?}",
//!         snapshot.downloads.avg_speed_bytes_per_sec
//!     );
//!     println!("Fetch success %:  {:?}", snapshot.fetches.success_rate);
//!     Ok(())
//! }
//! ```

mod config;
mod inner;
mod snapshot;
mod tracker;

pub use config::TrackerConfig;
pub use snapshot::{
    ActiveDownloadSnapshot, DownloadOutcomeSnapshot, DownloadSnapshot, DownloadStats, FetchStats, GlobalSnapshot,
    PlaylistStats, PostProcessStats,
};
pub use tracker::StatisticsTracker;
