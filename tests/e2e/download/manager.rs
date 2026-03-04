use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use yt_dlp::DownloadStatus;

use crate::common::assertions::assert_file_exists;
use crate::common::fixtures;
use crate::helpers;

/// Enqueue multiple downloads with different priorities and verify all complete.
#[tokio::test]
async fn enqueue_multiple_with_priorities() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    let priorities = [
        yt_dlp::DownloadPriority::Low,
        yt_dlp::DownloadPriority::Normal,
        yt_dlp::DownloadPriority::High,
        yt_dlp::DownloadPriority::Critical,
    ];

    let mut ids = Vec::new();
    for (i, priority) in priorities.iter().enumerate() {
        let url = format!("{}/media/priority_{}.bin", server.uri(), i);
        let output = tmp.path().join(format!("priority_{}.bin", i));
        let id = downloader
            .download_manager()
            .enqueue(&url, &output, Some(*priority))
            .await;
        ids.push((id, output));
    }

    // Wait for all
    for (id, output) in &ids {
        let status = downloader.wait_for_download(*id).await;
        assert!(
            matches!(status, Some(DownloadStatus::Completed)),
            "Download {} should complete, got {:?}",
            id,
            status
        );
        assert_file_exists(output);
    }
}

/// Enqueue with progress callback and verify concurrent downloads.
#[tokio::test]
async fn concurrent_downloads_with_progress() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    let total_progress = Arc::new(AtomicU64::new(0));
    let mut ids = Vec::new();

    for i in 0..3 {
        let url = format!("{}/media/concurrent_{}.bin", server.uri(), i);
        let output = tmp.path().join(format!("concurrent_{}.bin", i));

        let progress = Arc::clone(&total_progress);
        let id = downloader
            .download_manager()
            .enqueue_with_progress(&url, &output, None, move |downloaded, _total| {
                progress.fetch_add(downloaded, Ordering::Relaxed);
            })
            .await;
        ids.push(id);
    }

    // Wait for all downloads
    for id in &ids {
        let _ = downloader.wait_for_download(*id).await;
    }

    // Progress should have been reported
    assert!(
        total_progress.load(Ordering::Relaxed) > 0,
        "Some progress should have been reported"
    );
}

/// Check download status transitions.
#[tokio::test]
async fn download_status_transitions() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    let url = format!("{}/media/status_test.bin", server.uri());
    let output = tmp.path().join("status_test.bin");

    let id = downloader.download_manager().enqueue(&url, &output, None).await;

    // Right after enqueue, status should be Queued (or already Downloading if fast)
    let initial_status = downloader.get_download_status(id).await;
    assert!(
        initial_status.is_some(),
        "Download should have a status immediately after enqueue"
    );

    // Wait for completion
    let final_status = downloader.wait_for_download(id).await;
    assert!(
        matches!(final_status, Some(DownloadStatus::Completed)),
        "Final status should be Completed, got {:?}",
        final_status
    );
}

/// Verify events are emitted for all concurrent downloads.
#[tokio::test]
async fn events_for_concurrent_downloads() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    let rx = downloader.subscribe_events();

    let mut ids = Vec::new();
    for i in 0..3 {
        let url = format!("{}/media/event_concurrent_{}.bin", server.uri(), i);
        let output = tmp.path().join(format!("event_concurrent_{}.bin", i));
        let id = downloader.download_manager().enqueue(&url, &output, None).await;
        ids.push(id);
    }

    for id in &ids {
        let _ = downloader.wait_for_download(*id).await;
    }

    let events = helpers::collect_events(rx, Duration::from_millis(500)).await;

    // Each download should have generated at least a DownloadQueued event
    for id in &ids {
        let has_queued = events.iter().any(|e| {
            matches!(
                e.as_ref(),
                yt_dlp::events::DownloadEvent::DownloadQueued { download_id, .. } if *download_id == *id
            )
        });
        assert!(has_queued, "Download {} should have a DownloadQueued event", id);
    }
}

/// Cancel a download and verify it reports the canceled status.
#[tokio::test]
async fn cancel_download() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    // Mount a slow response to give us time to cancel
    Mock::given(method("GET"))
        .and(path("/slow"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0u8; 1024 * 1024])
                .set_delay(Duration::from_secs(10)),
        )
        .mount(&server)
        .await;

    let url = format!("{}/slow", server.uri());
    let output = tmp.path().join("canceled.bin");

    let id = downloader.download_manager().enqueue(&url, &output, None).await;

    // Give some time for the download to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Cancel the download
    let canceled = downloader.cancel_download(id).await;
    assert!(canceled, "Should be able to cancel the download");

    // The status should be Canceled
    let status = downloader.get_download_status(id).await;
    assert!(
        matches!(status, Some(DownloadStatus::Canceled)),
        "Status after cancel should be Canceled, got {:?}",
        status
    );
}

/// Download IDs are unique and incremental.
#[tokio::test]
async fn download_ids_are_unique() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    let url = format!("{}/media/small.bin", server.uri());
    let mut ids = Vec::new();
    for i in 0..5 {
        let output = tmp.path().join(format!("unique_{}.bin", i));
        let id = downloader.download_manager().enqueue(&url, &output, None).await;
        ids.push(id);
    }

    // All IDs should be unique
    let mut unique_ids = ids.clone();
    unique_ids.sort();
    unique_ids.dedup();
    assert_eq!(ids.len(), unique_ids.len(), "All download IDs should be unique");

    // IDs should be sequential (monotonically increasing)
    for window in ids.windows(2) {
        assert!(window[1] > window[0], "IDs should be monotonically increasing");
    }
}
