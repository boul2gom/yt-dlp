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
#[allow(dead_code)]
pub fn load_e2e_live_video(base_url: &str) -> yt_dlp::model::Video {
    fixtures::load_fixture_with_url("live_video.json", base_url)
}

/// Loads the short video fixture with `{{MOCK_SERVER}}` replaced.
#[allow(dead_code)]
pub fn load_e2e_short_video(base_url: &str) -> yt_dlp::model::Video {
    fixtures::load_fixture_with_url("short_video.json", base_url)
}

/// Loads the Twitch live fixture with `{{MOCK_SERVER}}` replaced.
#[allow(dead_code)]
pub fn load_e2e_twitch_live(base_url: &str) -> yt_dlp::model::Video {
    fixtures::load_fixture_with_url("twitch_live.json", base_url)
}

/// Loads the DRM video fixture with `{{MOCK_SERVER}}` replaced.
pub fn load_e2e_drm_video(base_url: &str) -> yt_dlp::model::Video {
    fixtures::load_fixture_with_url("drm_video.json", base_url)
}

/// Loads the playlist fixture (note: entry URLs are not mock-server-relative).
pub fn load_e2e_playlist() -> yt_dlp::model::playlist::Playlist {
    fixtures::load_playlist_fixture()
}

/// Loads the reel fixture with `{{MOCK_SERVER}}` replaced.
#[allow(dead_code)]
pub fn load_e2e_reel(base_url: &str) -> yt_dlp::model::Video {
    fixtures::load_fixture_with_url("reel.json", base_url)
}

/// Mounts an additional route on `server` that responds with `status` for `GET <path>`.
#[allow(dead_code)]
pub async fn mount_custom_route(server: &MockServer, url_path: &str, status: u16, body: Vec<u8>, content_type: &str) {
    Mock::given(method("GET"))
        .and(path(url_path))
        .respond_with(
            ResponseTemplate::new(status)
                .set_body_bytes(body)
                .insert_header("Content-Type", content_type),
        )
        .mount(server)
        .await;
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

/// Mounts a webhook receiver route that captures POSTed bodies.
///
/// Returns an `Arc<tokio::sync::Mutex<Vec<String>>>` where each POST body is appended.
#[allow(dead_code)]
pub async fn mount_webhook_receiver(server: &MockServer, webhook_path: &str) -> Arc<tokio::sync::Mutex<Vec<String>>> {
    let bodies: Arc<tokio::sync::Mutex<Vec<String>>> = Arc::new(tokio::sync::Mutex::new(Vec::new()));

    Mock::given(method("POST"))
        .and(path(webhook_path))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;

    bodies
}

/// Mounts a 404 route for a specific path.
#[allow(dead_code)]
pub async fn mount_not_found(server: &MockServer, url_path: &str) {
    Mock::given(method("GET"))
        .and(path(url_path))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
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
