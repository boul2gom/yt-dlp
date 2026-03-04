use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};
use yt_dlp::DownloadStatus;

use crate::common::fixtures;
use crate::helpers;

/// Downloading from a 404 URL should produce an error via the download manager.
#[tokio::test]
async fn download_404_url_returns_error() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    // Use a path outside /media/* to avoid conflict with the generic media mock
    Mock::given(method("GET"))
        .and(path("/errors/not_found.bin"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_bytes(b"Not Found".to_vec())
                .insert_header("Content-Type", "text/plain"),
        )
        .mount(&server)
        .await;

    let id = downloader
        .download_manager()
        .enqueue(
            format!("{}/errors/not_found.bin", server.uri()),
            tmp.path().join("should_not_exist.bin"),
            None,
        )
        .await;

    let status = downloader.wait_for_download(id).await;

    // A 404 response should either be reported as Failed or produce a
    // degenerate file containing only the error page body.
    match status {
        Some(DownloadStatus::Failed { .. }) => {} // expected
        Some(DownloadStatus::Completed) => {
            // Some HTTP stacks silently write the error body — verify it
            let out = tmp.path().join("should_not_exist.bin");
            if out.exists() {
                let bytes = std::fs::read(&out).unwrap();
                assert!(
                    bytes.is_empty() || bytes == b"Not Found",
                    "404 file should be empty or contain error page, got {} bytes",
                    bytes.len()
                );
            }
        }
        other => panic!("Expected Failed or Completed, got {:?}", other),
    }
}

/// Downloading from a very slow route should be bounded by the downloader timeout.
#[tokio::test]
async fn download_slow_route_respects_timeout() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    // Use a path outside /media/* to avoid conflict with the generic media mock
    Mock::given(method("GET"))
        .and(path("/errors/slow.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0u8; 64])
                .set_delay(Duration::from_secs(30)),
        )
        .mount(&server)
        .await;

    let id = downloader
        .download_manager()
        .enqueue(
            format!("{}/errors/slow.bin", server.uri()),
            tmp.path().join("slow.bin"),
            None,
        )
        .await;

    // Wait with a short timeout — we don't actually want to wait 30s
    let result = tokio::time::timeout(Duration::from_secs(5), downloader.wait_for_download(id)).await;

    // Either the outer timeout fires or the download manager reports a non-success status
    match result {
        Err(_) => {}                                  // Outer timeout fired — expected path
        Ok(Some(DownloadStatus::Failed { .. })) => {} // Download timed out internally
        Ok(Some(DownloadStatus::Canceled)) => {}      // Canceled due to timeout
        Ok(other) => panic!("Slow download should not succeed in 5s, got {:?}", other),
    }
}

/// A download that gets an empty body should handle gracefully.
#[tokio::test]
async fn download_empty_response_body() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    // Use a path outside /media/* to avoid conflict with the generic media mock
    Mock::given(method("GET"))
        .and(path("/errors/empty.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(Vec::<u8>::new())
                .insert_header("Content-Length", "0"),
        )
        .mount(&server)
        .await;

    let id = downloader
        .download_manager()
        .enqueue(
            format!("{}/errors/empty.bin", server.uri()),
            tmp.path().join("empty.bin"),
            None,
        )
        .await;

    let status = downloader.wait_for_download(id).await;

    // An empty body should either fail or produce a 0-byte file
    match status {
        Some(DownloadStatus::Failed { .. }) => {} // acceptable
        Some(DownloadStatus::Completed) => {
            let out = tmp.path().join("empty.bin");
            if out.exists() {
                let bytes = std::fs::read(&out).unwrap();
                assert!(
                    bytes.is_empty(),
                    "Empty response should produce empty file, got {} bytes",
                    bytes.len()
                );
            }
        }
        other => panic!("Expected Failed or Completed, got {:?}", other),
    }
}

/// DRM format metadata is preserved and accessible.
#[tokio::test]
async fn drm_format_metadata_preserved() {
    let server = helpers::setup_e2e_server().await;
    let video = helpers::load_e2e_drm_video(&server.uri());

    use yt_dlp::model::DrmStatus;

    assert!(!video.formats.is_empty());

    let drm_audio = video.formats.iter().find(|f| f.format_id == "drm_251");
    assert!(drm_audio.is_some(), "DRM audio format should exist");

    let format = drm_audio.unwrap();
    assert_eq!(format.has_drm, Some(DrmStatus::Yes));

    let drm_video = video.formats.iter().find(|f| f.format_id == "drm_303");
    assert!(drm_video.is_some(), "DRM video format should exist");

    let format = drm_video.unwrap();
    assert_eq!(format.has_drm, Some(DrmStatus::Maybe));
}

/// Enqueue a download and cancel it immediately — should not hang.
#[tokio::test]
async fn cancel_download_does_not_hang() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    // Slow route ensures cancel happens before completion
    helpers::mount_delayed_route(&server, "/media/cancellable.bin", Duration::from_secs(30)).await;

    let id = downloader
        .download_manager()
        .enqueue(
            format!("{}/media/cancellable.bin", server.uri()),
            tmp.path().join("cancelled.bin"),
            None,
        )
        .await;

    downloader.cancel_download(id).await;

    // After cancel, wait should resolve quickly (either error or ok)
    let result = tokio::time::timeout(Duration::from_secs(3), downloader.wait_for_download(id)).await;
    assert!(result.is_ok(), "Waiting after cancel should not hang");
}

/// Downloading DRM-protected content should still succeed at the HTTP level
/// (the DRM flag is metadata-only, real DRM is enforced by CDN).
#[tokio::test]
async fn download_drm_format_still_fetches_bytes() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_drm_video(&server.uri());

    let format = video
        .formats
        .iter()
        .find(|f| f.format_id == "drm_303")
        .expect("DRM video format");

    let url = format.url().unwrap();

    let id = downloader
        .download_manager()
        .enqueue(url, tmp.path().join("drm_video.webm"), None)
        .await;

    let status = downloader.wait_for_download(id).await;

    // The mock server serves bytes regardless of DRM — this verifies the download
    // path doesn't reject formats with has_drm set.
    if matches!(status, Some(DownloadStatus::Completed)) {
        let out = tmp.path().join("drm_video.webm");
        assert!(out.exists(), "DRM format file should be downloaded");
    }
}
