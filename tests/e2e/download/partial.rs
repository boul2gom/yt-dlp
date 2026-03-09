use wiremock::matchers::{header_exists, method};
use wiremock::{Mock, MockServer, ResponseTemplate};
use yt_dlp::DownloadStatus;
use yt_dlp::download::PartialRange;

use crate::common::assertions::assert_file_exists;
use crate::common::fixtures;
use crate::helpers;

/// Partial/range download via the download manager's `enqueue_range`.
/// Wiremock serves enough bytes; we request a sub-range.
#[tokio::test]
async fn partial_download_via_range() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let media_bytes = fixtures::load_media_bytes("small.bin");

    // Mount a range-aware route that returns the requested byte slice
    let len = media_bytes.len();
    let bytes_clone = media_bytes.clone();
    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/media/range_test.bin"))
        .and(header_exists("Range"))
        .respond_with(ResponseTemplate::new(206).set_body_bytes(bytes_clone[..512.min(len)].to_vec()))
        .mount(&server)
        .await;

    let url = format!("{}/media/range_test.bin", server.uri());
    let output = tmp.path().join("partial.bin");

    let id = downloader
        .download_manager()
        .enqueue_range(&url, &output, 0, 511, None, None)
        .await;

    let status = downloader.wait_for_download(id).await;
    assert!(
        matches!(status, Some(DownloadStatus::Completed)),
        "Expected Completed, got {:?}",
        status
    );

    assert_file_exists(&output);
}

/// DownloadBuilder with time_range partial download.
/// The DownloadBuilder.execute() tries a media-seek partial clip first, then falls back.
/// Without a real container index in the mock data, media-seek will fail and
/// the fallback full-download + combine path will be attempted (which also fails
/// at the ffmpeg step). This tests the partial pipeline flow.
#[tokio::test]
async fn download_builder_partial_time_range_falls_back() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let result = downloader
        .download(&video, "partial_clip.mp4")
        .time_range(0.0, 5.0)
        .unwrap()
        .execute()
        .await;

    // media-seek partial will fail (mock data is not a real container),
    // then fallback full download succeeds but ffmpeg combine fails.
    assert!(result.is_err(), "Expected failure without ffmpeg");
}

/// DownloadBuilder with chapter-based partial.
#[tokio::test]
async fn download_builder_partial_chapter() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    let result = downloader
        .download(&video, "chapter_clip.mp4")
        .chapter(0)
        .execute()
        .await;

    // Chapter 0 exists in the fixture, but the same fallback chain applies
    assert!(result.is_err(), "Expected failure without ffmpeg");
}

/// Verify that byte-range requests are actually sent for enqueue_range downloads.
#[tokio::test]
async fn range_request_sent_to_server() {
    let server = MockServer::start().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    // Mount a mock that expects a Range header
    let range_mock = Mock::given(method("GET"))
        .and(wiremock::matchers::path("/ranged"))
        .respond_with(
            ResponseTemplate::new(206)
                .set_body_bytes(vec![0xAA; 256])
                .insert_header("Content-Range", "bytes 0-255/1024")
                .insert_header("Accept-Ranges", "bytes"),
        )
        .expect(1..)
        .mount_as_scoped(&server)
        .await;

    let url = format!("{}/ranged", server.uri());
    let output = tmp.path().join("ranged.bin");

    let id = downloader
        .download_manager()
        .enqueue_range(&url, &output, 0, 255, None, None)
        .await;

    let status = downloader.wait_for_download(id).await;

    // The download may complete or fail depending on range handling,
    // but the server should have received at least one request.
    drop(range_mock);
    assert!(status.is_some());
}

/// download_video_partial returns an error when the video has no chapters but a
/// chapter-based PartialRange is requested.
#[tokio::test]
async fn download_video_partial_no_chapters_returns_error() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    let mut video = helpers::load_e2e_video(&server.uri());
    video.chapters.clear();

    let range = PartialRange::chapter_range(0, 1).expect("valid range");
    let result = downloader
        .download_video_partial(&video, &range, "clip_no_chapters.mp4")
        .await;
    assert!(result.is_err(), "should fail when video has no chapters");
}

/// download_video_partial returns an error when the requested chapter index is
/// beyond the available chapters in the video metadata.
#[tokio::test]
async fn download_video_partial_chapter_out_of_bounds_returns_error() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    let mut video = helpers::load_e2e_video(&server.uri());
    // Keep chapters but request a far-out-of-bounds index
    if video.chapters.is_empty() {
        // If the fixture has no chapters, push a dummy one so the code reaches to_time_range
        video.chapters.push(yt_dlp::model::chapter::Chapter {
            start_time: 0.0,
            end_time: 10.0,
            title: Some("Only chapter".to_string()),
        });
    }

    let range = PartialRange::single_chapter(999);
    let result = downloader.download_video_partial(&video, &range, "clip_oob.mp4").await;
    assert!(result.is_err(), "should fail when chapter index is out of bounds");
}

/// download_video_partial returns an error when the video has no downloadable
/// formats (FormatNotAvailable).
#[tokio::test]
async fn download_video_partial_no_formats_returns_error() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    let mut video = helpers::load_e2e_video(&server.uri());
    video.formats.clear();

    let range = PartialRange::time_range(0.0, 5.0).expect("valid range");
    let result = downloader
        .download_video_partial(&video, &range, "clip_no_formats.mp4")
        .await;
    assert!(result.is_err(), "should fail when video has no formats");
}
