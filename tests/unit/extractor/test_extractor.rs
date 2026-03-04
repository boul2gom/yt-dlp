use yt_dlp::extractor::{ExtractorName, VideoExtractor};

use crate::common::test_extractor::TestExtractor;

#[test]
fn test_extractor_youtube_name() {
    let extractor = TestExtractor::youtube();
    assert_eq!(extractor.name(), ExtractorName::Youtube);
}

#[test]
fn test_extractor_generic_name() {
    let extractor = TestExtractor::generic("vimeo");
    assert_eq!(extractor.name(), ExtractorName::Generic(Some("vimeo".to_string())));
}

#[test]
fn test_extractor_supports_youtube_urls() {
    let extractor = TestExtractor::youtube();
    assert!(extractor.supports_url("https://www.youtube.com/watch?v=test"));
    assert!(extractor.supports_url("https://youtu.be/test"));
    assert!(!extractor.supports_url("https://vimeo.com/12345"));
}

#[test]
fn test_extractor_generic_supports_all_urls() {
    let extractor = TestExtractor::generic("vimeo");
    assert!(extractor.supports_url("https://vimeo.com/12345"));
    assert!(extractor.supports_url("https://example.com/video"));
}

#[test]
fn test_extractor_debug_format() {
    let extractor = TestExtractor::youtube();
    let debug = format!("{:?}", extractor);
    assert!(debug.contains("TestExtractor"));
    assert!(debug.contains("Youtube"));
}

#[tokio::test]
async fn test_extractor_fetch_video_returns_fixture() {
    let extractor = TestExtractor::youtube();
    let video = extractor
        .fetch_video("https://www.youtube.com/watch?v=test")
        .await
        .expect("fetch_video should succeed");
    assert!(!video.id.is_empty());
    assert!(!video.title.is_empty());
}

#[tokio::test]
async fn test_extractor_fetch_playlist_returns_fixture() {
    let extractor = TestExtractor::youtube();
    let playlist = extractor
        .fetch_playlist("https://www.youtube.com/playlist?list=test")
        .await
        .expect("fetch_playlist should succeed");
    assert!(!playlist.id.is_empty());
}

#[tokio::test]
async fn test_extractor_with_mock_server_rewrites_urls() {
    let extractor = TestExtractor::youtube().with_mock_server("http://localhost:8080");
    let video = extractor
        .fetch_video("https://www.youtube.com/watch?v=test")
        .await
        .expect("fetch_video should succeed");

    // Verify that at least one format URL was rewritten
    let has_mock_url = video
        .formats
        .iter()
        .any(|f| f.url().ok().is_some_and(|u| u.contains("localhost:8080")));

    assert!(has_mock_url, "mock server URL should be present in format URLs");
}
