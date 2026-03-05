use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use yt_dlp::DownloadStatus;
use yt_dlp::model::selector::{AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality};

use crate::common::assertions::{assert_file_exists, assert_file_size_gt};
use crate::common::fixtures;
use crate::helpers;

/// Full download pipeline: load fixture → rewrite URLs → enqueue via download manager → verify file.
#[tokio::test]
async fn download_single_format_via_manager() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    let url = format!("{}/media/small.bin", server.uri());
    let output = tmp.path().join("output.bin");

    let id = downloader.download_manager().enqueue(&url, &output, None).await;

    let status = downloader.wait_for_download(id).await;
    assert!(
        matches!(status, Some(DownloadStatus::Completed)),
        "Expected Completed, got {:?}",
        status
    );

    assert_file_exists(&output);
    assert_file_size_gt(&output, 0);
}

/// Download to a specific absolute path.
#[tokio::test]
async fn download_to_exact_path() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    let url = format!("{}/media/small.mp4", server.uri());
    let exact_path = tmp.path().join("subdir").join("exact_output.mp4");
    std::fs::create_dir_all(exact_path.parent().unwrap()).unwrap();

    let id = downloader.download_manager().enqueue(&url, &exact_path, None).await;

    let status = downloader.wait_for_download(id).await;
    assert!(
        matches!(status, Some(DownloadStatus::Completed)),
        "Expected Completed, got {:?}",
        status
    );

    assert_file_exists(&exact_path);
}

/// Download using the video fixture with priority, verifying format selection.
/// The fixture has combined audio+video formats (e.g. format 18), so
/// `best_audio_video_format()` succeeds and the download is enqueued.
#[tokio::test]
async fn download_video_with_priority_selects_combined_format() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let result = downloader
        .download_video_with_priority(&video, "output.mp4", None)
        .await;

    // The fixture has combined audio+video formats, so this should succeed
    assert!(result.is_ok(), "Expected success since combined format exists");

    let download_id = result.unwrap();
    let status = downloader.wait_for_download(download_id).await;
    assert!(
        matches!(status, Some(DownloadStatus::Completed)),
        "Expected Completed, got {:?}",
        status
    );
}

/// The reel fixture has no combined audio+video format, so `best_audio_video_format()`
/// fails with `FormatNotAvailable`; this tests error handling.
#[tokio::test]
async fn download_reel_with_priority_no_combined_format() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let reel = helpers::load_e2e_reel(&server.uri());

    let result = downloader.download_video_with_priority(&reel, "output.mp4", None).await;

    assert!(result.is_err(), "Expected error since reel has no combined format");
}

/// Download an audio-only stream by going through the download manager directly.
#[tokio::test]
async fn download_audio_stream() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    // Select an audio format (format_id=251 is opus audio)
    let audio_format = video
        .formats
        .iter()
        .find(|f| f.format_type() == yt_dlp::model::format::FormatType::Audio)
        .expect("No audio format in fixture");

    let url = audio_format.url().unwrap();
    let output = tmp.path().join("audio.webm");

    let id = downloader.download_manager().enqueue(url, &output, None).await;

    let status = downloader.wait_for_download(id).await;
    assert!(
        matches!(status, Some(DownloadStatus::Completed)),
        "Expected Completed, got {:?}",
        status
    );

    assert_file_exists(&output);
    assert_file_size_gt(&output, 0);
}

/// Download a video-only stream.
#[tokio::test]
async fn download_video_stream() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let video_format = video
        .formats
        .iter()
        .find(|f| f.format_type() == yt_dlp::model::format::FormatType::Video)
        .expect("No video format in fixture");

    let url = video_format.url().unwrap();
    let output = tmp.path().join("video.webm");

    let id = downloader.download_manager().enqueue(url, &output, None).await;

    let status = downloader.wait_for_download(id).await;
    assert!(
        matches!(status, Some(DownloadStatus::Completed)),
        "Expected Completed, got {:?}",
        status
    );

    assert_file_exists(&output);
}

/// Download with progress callback, verify callback is invoked.
#[tokio::test]
async fn download_with_progress_tracking() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    let url = format!("{}/media/small.bin", server.uri());
    let output = tmp.path().join("progress_output.bin");

    let call_count = Arc::new(AtomicU64::new(0));
    let call_count_clone = Arc::clone(&call_count);

    let id = downloader
        .download_manager()
        .enqueue_with_progress(&url, &output, None, move |_downloaded, _total| {
            call_count_clone.fetch_add(1, Ordering::Relaxed);
        })
        .await;

    let status = downloader.wait_for_download(id).await;
    assert!(
        matches!(status, Some(DownloadStatus::Completed)),
        "Expected Completed, got {:?}",
        status
    );

    assert_file_exists(&output);
    // Progress callback should have been called at least once
    assert!(
        call_count.load(Ordering::Relaxed) > 0,
        "Progress callback was never invoked"
    );
}

/// Download a specific format by format_id.
#[tokio::test]
async fn download_specific_format() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    // Pick format 247 (720p vp9 video-only)
    let format = video
        .formats
        .iter()
        .find(|f| f.format_id == "247")
        .expect("Format 247 not found in fixture");

    let url = format.url().unwrap();
    let output = tmp.path().join("format_247.webm");

    let id = downloader.download_manager().enqueue(url, &output, None).await;

    let status = downloader.wait_for_download(id).await;
    assert!(
        matches!(status, Some(DownloadStatus::Completed)),
        "Expected Completed, got {:?}",
        status
    );

    assert_file_exists(&output);
}

// ============================== Quality-based download API ==============================

/// Download video stream with explicit quality and codec preferences.
#[tokio::test]
async fn download_video_stream_with_quality() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let result = downloader
        .download_video_stream_with_quality(
            &video,
            "quality_video.webm",
            VideoQuality::Best,
            VideoCodecPreference::Any,
        )
        .await;

    assert!(
        result.is_ok(),
        "download_video_stream_with_quality failed: {:?}",
        result.err()
    );
    crate::common::assertions::assert_file_exists(&result.unwrap());
}

/// Download audio stream with explicit quality and codec preferences.
#[tokio::test]
async fn download_audio_stream_with_quality() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let result = downloader
        .download_audio_stream_with_quality(
            &video,
            "quality_audio.webm",
            AudioQuality::Best,
            AudioCodecPreference::Any,
        )
        .await;

    assert!(
        result.is_ok(),
        "download_audio_stream_with_quality failed: {:?}",
        result.err()
    );
    crate::common::assertions::assert_file_exists(&result.unwrap());
}

/// download_video_with_quality requires ffmpeg for combining — verify appropriate error.
#[tokio::test]
async fn download_video_with_quality_fails_without_ffmpeg() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let result = downloader
        .download_video_with_quality(
            &video,
            "combined.mp4",
            VideoQuality::Best,
            VideoCodecPreference::Any,
            AudioQuality::Best,
            AudioCodecPreference::Any,
        )
        .await;

    // Without a real ffmpeg binary, combining should fail
    assert!(
        result.is_err(),
        "download_video_with_quality should fail without ffmpeg"
    );
}
