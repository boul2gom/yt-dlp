use std::path::{Path, PathBuf};

use yt_dlp::client::deps::Libraries;
use yt_dlp::{Downloader, DownloaderBuilder};

/// Builds a test `Downloader` with fake binary paths and output directed to a temporary directory.
///
/// The binary paths are non-existent placeholders — this `Downloader` is
/// suitable only for operations that do NOT spawn yt-dlp or ffmpeg processes
/// (e.g. format selection, event emission, cache operations).
pub async fn build_test_downloader(mock_server_uri: &str, output_dir: &Path) -> Downloader {
    let libraries = Libraries::new(PathBuf::from("/tmp/fake-yt-dlp"), PathBuf::from("/tmp/fake-ffmpeg"));

    DownloaderBuilder::new(libraries, output_dir)
        .with_args(vec!["--no-playlist".to_string(), "--no-check-certificates".to_string()])
        .with_user_agent("yt-dlp-test-agent/1.0")
        .build()
        .await
        .unwrap_or_else(|e| panic!("Failed to build test downloader (mock={}): {}", mock_server_uri, e))
}
