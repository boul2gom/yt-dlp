use crate::common::assertions::assert_file_exists;
use crate::common::fixtures;
use crate::helpers;

/// Download a video-only stream via `download_video_stream()`.
/// This does NOT require ffmpeg — it downloads a single format through the Fetcher.
#[tokio::test]
async fn download_video_stream_via_pipeline() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let result = downloader.download_video_stream(&video, "stream.webm").await;
    assert!(
        result.is_ok(),
        "download_video_stream should succeed: {:?}",
        result.err()
    );

    let path = result.unwrap();
    assert_file_exists(&path);
}

/// Download a video-only stream to an exact path.
#[tokio::test]
async fn download_video_stream_to_path() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let exact = tmp.path().join("custom/stream.webm");
    std::fs::create_dir_all(exact.parent().unwrap()).unwrap();

    let result = downloader.download_video_stream_to_path(&video, &exact).await;
    assert!(
        result.is_ok(),
        "download_video_stream_to_path failed: {:?}",
        result.err()
    );

    assert_file_exists(&exact);
}

/// Download a specific format by reference.
#[tokio::test]
async fn download_format_by_reference() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let audio_format = video
        .formats
        .iter()
        .find(|f| f.format_id == "251")
        .expect("Format 251 should exist");

    let result = downloader.download_format(audio_format, "fmt_251.webm").await;
    assert!(result.is_ok(), "download_format failed: {:?}", result.err());

    let path = result.unwrap();
    assert_file_exists(&path);
}

/// Download a format to a specific path.
#[tokio::test]
async fn download_format_to_exact_path() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let format = video
        .formats
        .iter()
        .find(|f| f.format_id == "140")
        .expect("Format 140 should exist");

    let exact = tmp.path().join("format_output.m4a");
    let result = downloader.download_format_to_path(format, &exact).await;
    assert!(result.is_ok(), "download_format_to_path failed: {:?}", result.err());

    assert_file_exists(&exact);
}

/// Download a thumbnail using the Downloader pipeline.
#[tokio::test]
async fn download_thumbnail() {
    use yt_dlp::model::selector::ThumbnailQuality;

    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let result = downloader
        .download_thumbnail(&video, ThumbnailQuality::Best, tmp.path().join("thumb.jpg"))
        .await;
    assert!(result.is_ok(), "download_thumbnail failed: {:?}", result.err());

    assert_file_exists(&result.unwrap());
}

/// The fluent `download_and_continue` method calls `download_video` which needs ffmpeg.
/// Verify it returns an error gracefully.
#[tokio::test]
async fn download_and_continue_fails_without_ffmpeg() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let result = downloader.download_and_continue(&video, "pipeline.mp4").await;
    assert!(result.is_err(), "download_and_continue should fail without ffmpeg");
}

/// postprocess_video requires ffmpeg — verify appropriate error.
#[tokio::test]
async fn postprocess_video_fails_without_ffmpeg() {
    use yt_dlp::download::PostProcessConfig;

    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    // Create a dummy input file so the IO part is not the failure point
    let input = tmp.path().join("input.mp4");
    std::fs::write(&input, b"fake video content").unwrap();

    // Config must have at least one option set, otherwise postprocess returns input as-is
    let config = PostProcessConfig::new().with_audio_bitrate("128k");
    let result = downloader.postprocess_video(&input, "processed.mp4", config).await;
    assert!(result.is_err(), "postprocess_video should fail without ffmpeg");
}

/// Verify that event_stream and event_subscriber_count work during pipeline operations.
#[tokio::test]
async fn event_stream_accessible() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    // Capture baseline — statistics tracker may already be subscribed
    let baseline = downloader.event_subscriber_count();

    // Create a subscription
    let _rx = downloader.subscribe_events();
    assert_eq!(downloader.event_subscriber_count(), baseline + 1);

    // Create another
    let _rx2 = downloader.subscribe_events();
    assert_eq!(downloader.event_subscriber_count(), baseline + 2);
}
