use std::path::Path;
use std::sync::Arc;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common::fixtures;

/// Builds a test Downloader with fake binary paths, having its output directed to `output_dir`.
///
/// The binary paths are non-existent placeholders — operations that spawn yt-dlp or ffmpeg
/// (e.g. `execute()`, `combine_audio_and_video`, `postprocess_video`) will fail.
/// Suitable for download-manager-level testing: enqueue, wait, events, cancellation.
pub async fn build_e2e_downloader(mock_server_uri: &str, output_dir: &Path) -> yt_dlp::Downloader {
    crate::common::downloader::build_test_downloader(mock_server_uri, output_dir).await
}

/// Sets up a fully-stocked mock server that serves media at /media/<name>,
/// thumbnails, storyboards, and HLS playlists.
///
/// Returns the `MockServer` instance whose URI can be fed into fixture URL rewriting.
pub async fn setup_e2e_server() -> MockServer {
    crate::common::server::setup_media_server().await
}

/// Loads the standard `video.json` fixture with `{{MOCK_SERVER}}` replaced by `base_url`.
pub fn load_e2e_video(base_url: &str) -> yt_dlp::model::Video {
    fixtures::load_video_with_mock_urls(base_url)
}

/// Loads the live video fixture with `{{MOCK_SERVER}}` replaced.
pub fn load_e2e_live_video(base_url: &str) -> yt_dlp::model::Video {
    fixtures::load_fixture_with_url("live_video.json", base_url)
}

/// Loads the short video fixture with `{{MOCK_SERVER}}` replaced.
pub fn load_e2e_short_video(base_url: &str) -> yt_dlp::model::Video {
    fixtures::load_fixture_with_url("short_video.json", base_url)
}

/// Loads the reel fixture with `{{MOCK_SERVER}}` replaced.
pub fn load_e2e_reel(base_url: &str) -> yt_dlp::model::Video {
    fixtures::load_fixture_with_url("reel.json", base_url)
}

/// Loads the DRM video fixture with `{{MOCK_SERVER}}` replaced.
pub fn load_e2e_drm_video(base_url: &str) -> yt_dlp::model::Video {
    fixtures::load_fixture_with_url("drm_video.json", base_url)
}

/// Loads the playlist fixture (note: entry URLs are not mock-server-relative).
pub fn load_e2e_playlist() -> yt_dlp::model::playlist::Playlist {
    fixtures::load_playlist_fixture()
}

/// Mounts a route that returns after a `delay`.
pub async fn mount_delayed_route(server: &MockServer, url_path: &str, delay: std::time::Duration) {
    Mock::given(method("GET"))
        .and(path(url_path))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0u8; 64])
                .set_delay(delay),
        )
        .mount(server)
        .await;
}

/// Collects events from a broadcast receiver until `timeout` elapses after the last event.
pub async fn collect_events(
    mut rx: tokio::sync::broadcast::Receiver<Arc<yt_dlp::events::DownloadEvent>>,
    timeout: std::time::Duration,
) -> Vec<Arc<yt_dlp::events::DownloadEvent>> {
    let mut events = Vec::new();
    loop {
        match tokio::time::timeout(timeout, rx.recv()).await {
            Ok(Ok(event)) => events.push(event),
            _ => break,
        }
    }
    events
}
