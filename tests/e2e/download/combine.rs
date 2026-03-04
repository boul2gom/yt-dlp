use yt_dlp::DownloadStatus;

use crate::common::assertions::assert_file_exists;
use crate::common::fixtures;
use crate::helpers;

/// The DownloadBuilder.execute() flow selects separate audio+video, enqueues both,
/// waits for completion, then calls combine_audio_and_video (needs ffmpeg).
/// With fake binaries, the download portion succeeds but combine fails.
/// This test verifies the entire pipeline surfaces an appropriate error.
#[tokio::test]
async fn download_builder_execute_fails_without_ffmpeg() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let result = downloader.download(&video, "combined.mp4").execute().await;

    // The download of the individual streams might succeed, but the ffmpeg combine step
    // will fail since we use a fake ffmpeg binary.
    assert!(
        result.is_err(),
        "Expected DownloadBuilder.execute() to fail without ffmpeg, got {:?}",
        result
    );
}

/// Separately download audio + video via the manager, verifying both complete.
/// This is the "download" portion of the combine pipeline, without the ffmpeg step.
#[tokio::test]
async fn download_audio_and_video_separately() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    // Select best video and audio formats
    let video_format = video
        .formats
        .iter()
        .find(|f| f.format_type() == yt_dlp::model::format::FormatType::Video)
        .expect("No video format");
    let audio_format = video
        .formats
        .iter()
        .find(|f| f.format_type() == yt_dlp::model::format::FormatType::Audio)
        .expect("No audio format");

    let video_url = video_format.url().unwrap();
    let audio_url = audio_format.url().unwrap();

    let video_path = tmp.path().join("video.webm");
    let audio_path = tmp.path().join("audio.webm");

    let vid_id = downloader
        .download_manager()
        .enqueue(video_url, &video_path, None)
        .await;
    let aud_id = downloader
        .download_manager()
        .enqueue(audio_url, &audio_path, None)
        .await;

    let (vid_status, aud_status) = tokio::join!(
        downloader.wait_for_download(vid_id),
        downloader.wait_for_download(aud_id),
    );

    assert!(
        matches!(vid_status, Some(DownloadStatus::Completed)),
        "Video download should complete, got {:?}",
        vid_status
    );
    assert!(
        matches!(aud_status, Some(DownloadStatus::Completed)),
        "Audio download should complete, got {:?}",
        aud_status
    );

    assert_file_exists(&video_path);
    assert_file_exists(&audio_path);
}

/// DownloadBuilder with codec preferences selects matching formats.
#[tokio::test]
async fn download_builder_with_quality_preferences_fails_without_ffmpeg() {
    use yt_dlp::model::selector::{AudioQuality, VideoCodecPreference, VideoQuality};

    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let result = downloader
        .download(&video, "quality.mp4")
        .video_quality(VideoQuality::Best)
        .audio_quality(AudioQuality::Best)
        .video_codec(VideoCodecPreference::VP9)
        .execute()
        .await;

    // Downloads succeed but combine fails (no ffmpeg)
    assert!(result.is_err());
}

/// DownloadBuilder with high priority.
#[tokio::test]
async fn download_builder_with_priority() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let result = downloader
        .download(&video, "priority.mp4")
        .priority(yt_dlp::DownloadPriority::Critical)
        .execute()
        .await;

    // Downloads complete but combine fails
    assert!(result.is_err());
}
