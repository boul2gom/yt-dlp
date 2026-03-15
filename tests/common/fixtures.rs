use std::path::{Path, PathBuf};

use yt_dlp::model::Video;
use yt_dlp::model::chapter::Chapter;
use yt_dlp::model::playlist::Playlist;

/// Returns the path to the test fixtures directory.
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Returns the path to a JSON fixture file.
pub fn json_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("json").join(name)
}

/// Returns the path to a media fixture file.
pub fn media_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("media").join(name)
}

/// Returns the path to an HLS fixture file.
pub fn hls_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("hls").join(name)
}

/// Returns the path to a subtitle fixture file.
pub fn subtitle_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("subtitles").join(name)
}

/// Loads the raw JSON string for a fixture file.
pub fn load_json_string(name: &str) -> String {
    std::fs::read_to_string(json_fixture(name)).unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", name, e))
}

/// Loads a Video from the video.json fixture.
pub fn load_video_fixture() -> Video {
    let json = load_json_string("video.json");
    serde_json::from_str(&json).expect("Failed to deserialize video fixture")
}

/// Loads any JSON fixture, replaces `{{MOCK_SERVER}}` with `base_url`, and deserialises it.
///
/// Use this instead of duplicating the `load_json_string + replace + from_str` pattern.
pub fn load_fixture_with_url<T: serde::de::DeserializeOwned>(name: &str, base_url: &str) -> T {
    let json = load_json_string(name).replace("{{MOCK_SERVER}}", base_url);
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("Failed to deserialize fixture {name}: {e}"))
}

/// Loads a Video from the video.json fixture with mock server URLs replaced.
pub fn load_video_with_mock_urls(mock_server_url: &str) -> Video {
    load_fixture_with_url("video.json", mock_server_url)
}

/// Loads a Video from the live_video.json fixture.
pub fn load_live_video_fixture() -> Video {
    let json = load_json_string("live_video.json");
    serde_json::from_str(&json).expect("Failed to deserialize live video fixture")
}

/// Loads a Video from the short_video.json fixture.
pub fn load_short_video_fixture() -> Video {
    let json = load_json_string("short_video.json");
    serde_json::from_str(&json).expect("Failed to deserialize short video fixture")
}

/// Loads a Video from the twitch_live.json fixture.
pub fn load_twitch_live_fixture() -> Video {
    let json = load_json_string("twitch_live.json");
    serde_json::from_str(&json).expect("Failed to deserialize twitch live fixture")
}

/// Loads a Video from the drm_video.json fixture.
pub fn load_drm_video_fixture() -> Video {
    let json = load_json_string("drm_video.json");
    serde_json::from_str(&json).expect("Failed to deserialize DRM video fixture")
}

/// Loads a Playlist from the playlist.json fixture.
pub fn load_playlist_fixture() -> Playlist {
    let json = load_json_string("playlist.json");
    serde_json::from_str(&json).expect("Failed to deserialize playlist fixture")
}

/// Loads chapters from the chapters.json fixture.
pub fn load_chapters_fixture() -> Vec<Chapter> {
    let json = load_json_string("chapters.json");
    serde_json::from_str(&json).expect("Failed to deserialize chapters fixture")
}

/// Loads a Video from the reel.json fixture.
pub fn load_reel_fixture() -> Video {
    let json = load_json_string("reel.json");
    serde_json::from_str(&json).expect("Failed to deserialize reel fixture")
}

/// Loads the raw string content of a subtitle fixture.
pub fn load_subtitle_string(name: &str) -> String {
    std::fs::read_to_string(subtitle_fixture(name))
        .unwrap_or_else(|e| panic!("Failed to read subtitle fixture {}: {}", name, e))
}

/// Loads an HLS fixture and replaces the mock server URL placeholder.
pub fn load_hls_fixture(name: &str, mock_server_url: &str) -> String {
    let content = std::fs::read_to_string(hls_fixture(name))
        .unwrap_or_else(|e| panic!("Failed to read HLS fixture {}: {}", name, e));
    content.replace("{{MOCK_SERVER}}", mock_server_url)
}

/// Loads the raw bytes of a media fixture.
pub fn load_media_bytes(name: &str) -> Vec<u8> {
    std::fs::read(media_fixture(name)).unwrap_or_else(|e| panic!("Failed to read media fixture {}: {}", name, e))
}

/// Creates a temporary directory for test output files.
pub fn temp_test_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("Failed to create temporary directory")
}
