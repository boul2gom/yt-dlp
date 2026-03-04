use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use yt_dlp::download::manager::{DownloadManager, DownloadPriority, DownloadStatus, ManagerConfig};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn setup_file_server() -> MockServer {
    let server = MockServer::start().await;
    let body = vec![0xCA; 4096]; // 4KB fake media

    Mock::given(method("GET"))
        .and(path("/file.mp4"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(body)
                .insert_header("Content-Type", "application/octet-stream"),
        )
        .mount(&server)
        .await;

    server
}

// ---------------------------------------------------------------------------
// Basic enqueue + completion
// ---------------------------------------------------------------------------

#[tokio::test]
async fn enqueue_and_wait_for_completion() {
    let server = setup_file_server().await;
    let dir = tempfile::tempdir().expect("tempdir failed");
    let dest = dir.path().join("output.mp4");

    let manager = DownloadManager::new();
    let id = manager.enqueue(format!("{}/file.mp4", server.uri()), &dest, None).await;

    let status = manager.wait_for_completion(id).await;
    assert!(
        matches!(status, Some(DownloadStatus::Completed)),
        "expected Completed, got {:?}",
        status
    );

    // tokio::fs::File drop is async — give it a moment to close
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(dest.exists(), "downloaded file should exist");
    let content = std::fs::read(&dest).unwrap();
    assert!(!content.is_empty(), "downloaded file should not be empty");
}

// ---------------------------------------------------------------------------
// Priority ordering
// ---------------------------------------------------------------------------

#[tokio::test]
async fn priority_ordering() {
    // Verify integer ordering via discriminant values
    assert_eq!(DownloadPriority::Critical as i32, 3);
    assert_eq!(DownloadPriority::High as i32, 2);
    assert_eq!(DownloadPriority::Normal as i32, 1);
    assert_eq!(DownloadPriority::Low as i32, 0);

    // from_i32 round-trip
    assert_eq!(DownloadPriority::from_i32(3), DownloadPriority::Critical);
    assert_eq!(DownloadPriority::from_i32(99), DownloadPriority::Normal);
}

// ---------------------------------------------------------------------------
// Status transitions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn status_transitions() {
    let server = setup_file_server().await;
    let dir = tempfile::tempdir().expect("tempdir failed");
    let dest = dir.path().join("status.mp4");

    let manager = DownloadManager::new();
    let id = manager.enqueue(format!("{}/file.mp4", server.uri()), &dest, None).await;

    // Should have a status (Queued initially, or already Downloading/Completed)
    let status = manager.get_status(id).await;
    assert!(status.is_some());

    // Wait for completion
    manager.wait_for_completion(id).await;
    let final_status = manager.get_status(id).await;
    assert!(
        matches!(final_status, Some(DownloadStatus::Completed)),
        "expected Completed, got {:?}",
        final_status
    );
}

// ---------------------------------------------------------------------------
// Progress callback
// ---------------------------------------------------------------------------

#[tokio::test]
async fn enqueue_with_progress_callback() {
    let server = setup_file_server().await;
    let dir = tempfile::tempdir().expect("tempdir failed");
    let dest = dir.path().join("progress.mp4");

    let progress_called = Arc::new(AtomicU64::new(0));
    let pc = progress_called.clone();

    let manager = DownloadManager::new();
    let id = manager
        .enqueue_with_progress(
            format!("{}/file.mp4", server.uri()),
            &dest,
            None,
            move |_downloaded, _total| {
                pc.fetch_add(1, Ordering::SeqCst);
            },
        )
        .await;

    manager.wait_for_completion(id).await;
    // Progress callback should have been called at least once
    assert!(progress_called.load(Ordering::SeqCst) >= 1);
}

// ---------------------------------------------------------------------------
// Cancellation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cancel_download() {
    let server = MockServer::start().await;
    // Slow response with delay to give us time to cancel
    let body = vec![0; 1024 * 1024]; // 1MB
    Mock::given(method("GET"))
        .and(path("/slow.mp4"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(body)
                .insert_header("Content-Type", "application/octet-stream")
                .set_delay(Duration::from_secs(5)),
        )
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().expect("tempdir failed");
    let dest = dir.path().join("cancel.mp4");

    let manager = DownloadManager::new();
    let id = manager.enqueue(format!("{}/slow.mp4", server.uri()), &dest, None).await;

    // Allow some time for task to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    let cancelled = manager.cancel(id).await;
    // May or may not cancel depending on timing
    if cancelled {
        let status = manager.get_status(id).await;
        assert!(
            matches!(
                status,
                Some(DownloadStatus::Canceled) | Some(DownloadStatus::Failed { .. })
            ),
            "expected Canceled or Failed, got {:?}",
            status
        );
    }
}

// ---------------------------------------------------------------------------
// Concurrent downloads
// ---------------------------------------------------------------------------

#[tokio::test]
async fn concurrent_downloads() {
    let server = setup_file_server().await;
    let dir = tempfile::tempdir().expect("tempdir failed");

    let config = ManagerConfig::default();
    let manager = DownloadManager::with_config(config);

    let mut ids = Vec::new();
    for i in 0..5u32 {
        let dest = dir.path().join(format!("concurrent_{i}.mp4"));
        let id = manager.enqueue(format!("{}/file.mp4", server.uri()), &dest, None).await;
        ids.push(id);
    }

    // Wait for all
    for id in &ids {
        manager.wait_for_completion(*id).await;
    }

    // All should be completed
    for id in &ids {
        let status = manager.get_status(*id).await;
        assert!(
            matches!(status, Some(DownloadStatus::Completed)),
            "download {id} expected Completed, got {:?}",
            status
        );
    }

    // All files should exist
    for i in 0..5u32 {
        let path = dir.path().join(format!("concurrent_{i}.mp4"));
        assert!(path.exists(), "file concurrent_{i}.mp4 should exist");
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn config_getters() {
    let manager = DownloadManager::new();
    assert!(manager.parallel_segments() > 0);
    assert!(manager.segment_size() > 0);
    assert!(manager.retry_attempts() > 0);
}

// ---------------------------------------------------------------------------
// Cleanup finished
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cleanup_finished_does_not_error() {
    let server = setup_file_server().await;
    let dir = tempfile::tempdir().expect("tempdir failed");

    let manager = DownloadManager::new();
    let dest = dir.path().join("cleanup.mp4");
    let id = manager.enqueue(format!("{}/file.mp4", server.uri()), &dest, None).await;

    manager.wait_for_completion(id).await;
    manager.cleanup_finished().await;
}

// ---------------------------------------------------------------------------
// With event bus
// ---------------------------------------------------------------------------

#[tokio::test]
async fn with_event_bus_emits_events() {
    let bus = yt_dlp::events::EventBus::with_default_capacity();
    let mut rx = bus.subscribe();

    let server = setup_file_server().await;
    let dir = tempfile::tempdir().expect("tempdir failed");
    let dest = dir.path().join("events.mp4");

    let config = ManagerConfig::default();
    let manager = DownloadManager::with_config_and_event_bus(config, Some(bus));

    let id = manager.enqueue(format!("{}/file.mp4", server.uri()), &dest, None).await;

    manager.wait_for_completion(id).await;

    // Should have received at least queued and completed/started events
    let mut event_types = Vec::new();
    while let Ok(event) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
        if let Ok(ev) = event {
            event_types.push(ev.event_type().to_string());
        }
    }

    assert!(
        event_types.iter().any(|t| t == "download_queued"),
        "expected download_queued event, got: {:?}",
        event_types
    );
}
