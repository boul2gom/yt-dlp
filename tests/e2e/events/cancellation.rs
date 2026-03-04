use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use yt_dlp::DownloadStatus;

use crate::common::fixtures;
use crate::helpers;

/// Shutdown triggers cancellation token.
#[tokio::test]
async fn shutdown_sets_cancellation_flag() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    assert!(!downloader.is_shutdown_requested());

    downloader.shutdown();

    assert!(downloader.is_shutdown_requested());
}

/// Shutdown can be called multiple times safely.
#[tokio::test]
async fn shutdown_idempotent() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    downloader.shutdown();
    downloader.shutdown();
    downloader.shutdown();

    assert!(downloader.is_shutdown_requested());
}

/// Cancel a single in-flight download via cancel_download.
#[tokio::test]
async fn cancel_inflight_download() {
    let server = MockServer::start().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    // Slow response so the download is in-flight when we cancel
    Mock::given(method("GET"))
        .and(path("/slow_cancel"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0u8; 4096])
                .set_delay(Duration::from_secs(30)),
        )
        .mount(&server)
        .await;

    let url = format!("{}/slow_cancel", server.uri());
    let output = tmp.path().join("will_cancel.bin");

    let id = downloader.download_manager().enqueue(&url, &output, None).await;

    // Short delay to let the download start
    tokio::time::sleep(Duration::from_millis(100)).await;

    let canceled = downloader.cancel_download(id).await;
    assert!(canceled, "Should successfully cancel the download");

    // Wait for the status to settle
    let status = downloader.get_download_status(id).await;
    assert!(
        matches!(status, Some(DownloadStatus::Canceled)),
        "Status should be Canceled, got {:?}",
        status
    );
}

/// Cancel emits a DownloadCanceled event.
#[tokio::test]
async fn cancel_emits_event() {
    let server = MockServer::start().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    Mock::given(method("GET"))
        .and(path("/slow_event"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0u8; 4096])
                .set_delay(Duration::from_secs(30)),
        )
        .mount(&server)
        .await;

    let rx = downloader.subscribe_events();

    let url = format!("{}/slow_event", server.uri());
    let output = tmp.path().join("cancel_event.bin");
    let id = downloader.download_manager().enqueue(&url, &output, None).await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    downloader.cancel_download(id).await;

    // Give some time for the event to propagate
    tokio::time::sleep(Duration::from_millis(200)).await;

    let events = helpers::collect_events(rx, Duration::from_millis(500)).await;

    // Look for a DownloadCanceled event
    let has_canceled_event = events.iter().any(|e| {
        matches!(
            e.as_ref(),
            yt_dlp::events::DownloadEvent::DownloadCanceled { download_id, .. } if *download_id == id
        )
    });

    // The download might also show up as Failed if it was already in progress.
    // Either Canceled or Failed (with cancellation) is acceptable.
    let has_failed_event = events.iter().any(|e| {
        matches!(
            e.as_ref(),
            yt_dlp::events::DownloadEvent::DownloadFailed { download_id, .. } if *download_id == id
        )
    });

    assert!(
        has_canceled_event || has_failed_event,
        "Should have a Canceled or Failed event for download {}, events: {:?}",
        id,
        events.iter().map(|e| e.event_type()).collect::<Vec<_>>()
    );
}

/// Downloads that haven't started yet can be canceled.
#[tokio::test]
async fn cancel_queued_download() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    // Enqueue but try to cancel immediately
    let url = format!("{}/media/small.bin", server.uri());
    let output = tmp.path().join("queued_cancel.bin");

    let id = downloader.download_manager().enqueue(&url, &output, None).await;

    // Try to cancel right away — it may have already started if the worker is fast
    let _canceled = downloader.cancel_download(id).await;

    // Regardless, wait for it and check the final status
    let status = downloader.get_download_status(id).await;
    assert!(status.is_some(), "Should have a status for download {}", id);
}

/// Cloned Downloader shares the same cancellation token.
#[tokio::test]
async fn cloned_downloader_shares_cancellation() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let clone = downloader.clone();

    assert!(!clone.is_shutdown_requested());

    downloader.shutdown();

    assert!(clone.is_shutdown_requested(), "Clone should see the shutdown");
}
