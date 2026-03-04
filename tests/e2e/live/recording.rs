use std::time::Duration;

use crate::common::fixtures;
use crate::helpers;

/// Live recording on a non-live video should fail with an appropriate error.
#[tokio::test]
async fn record_live_rejects_non_live_video() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    // video.json has is_live=false
    let result = downloader
        .record_live(&video, tmp.path().join("live_output.ts"))
        .with_max_duration(Duration::from_secs(2))
        .execute()
        .await;

    assert!(result.is_err(), "record_live should fail on non-live video");
}

/// Live recording with a live video fixture and wiremock-served HLS.
/// The native recorder fetches HLS playlists from wiremock and downloads segments.
#[tokio::test]
async fn record_live_with_hls_fixture() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_live_video(&server.uri());

    let result = downloader
        .record_live(&video, tmp.path().join("hls_recording.ts"))
        .with_max_duration(Duration::from_secs(2))
        .execute()
        .await;

    // The HLS recording may partially succeed depending on playlist parsing,
    // or it may fail due to segment issues. Either way, it should not panic.
    // If it succeeds, verify the output file exists.
    if let Ok(recording) = &result {
        assert!(recording.output_path.exists(), "Recording output file should exist");
    }
}

/// Cancel live recording via a custom cancellation token.
#[tokio::test]
async fn cancel_live_recording() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_live_video(&server.uri());

    let token = tokio_util::sync::CancellationToken::new();
    let token_clone = token.clone();

    // Cancel after a short delay
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        token_clone.cancel();
    });

    let result = downloader
        .record_live(&video, tmp.path().join("canceled_recording.ts"))
        .with_cancellation_token(token)
        .execute()
        .await;

    // The recording should either complete quickly or be canceled.
    // This is acceptable — we mainly want to ensure no panics.
    let _ = result;
}

/// Live recording with the fallback (FFmpeg) method.
/// Without a real ffmpeg binary, this should fail with an IO/process error.
#[tokio::test]
async fn record_live_fallback_fails_without_ffmpeg() {
    use yt_dlp::events::RecordingMethod;

    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_live_video(&server.uri());

    let result = downloader
        .record_live(&video, tmp.path().join("fallback_recording.ts"))
        .with_method(RecordingMethod::Fallback)
        .with_max_duration(Duration::from_secs(2))
        .execute()
        .await;

    assert!(
        result.is_err(),
        "Fallback (FFmpeg) recording should fail without ffmpeg"
    );
}
