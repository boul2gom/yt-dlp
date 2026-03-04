use std::time::Duration;

use yt_dlp::events::{DownloadEvent, EventBus};
use yt_dlp::stats::StatisticsTracker;

// ---------------------------------------------------------------------------
// Basic snapshot after events
// ---------------------------------------------------------------------------

#[tokio::test]
async fn snapshot_after_download_events() {
    let bus = EventBus::with_default_capacity();
    let tracker = StatisticsTracker::new(&bus);

    // Emit a full download lifecycle
    bus.emit(DownloadEvent::DownloadQueued {
        download_id: 1,
        url: "https://example.com/file.mp4".into(),
        priority: yt_dlp::download::DownloadPriority::Normal,
        output_path: "/tmp/out.mp4".into(),
    });
    bus.emit(DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "https://example.com/file.mp4".into(),
        total_bytes: 10000,
        format_id: None,
    });
    bus.emit(DownloadEvent::DownloadCompleted {
        download_id: 1,
        url: "https://example.com/file.mp4".into(),
        output_path: "/tmp/out.mp4".into(),
        duration: Duration::from_secs(5),
        total_bytes: 10000,
    });

    // Give the background task time to process events
    tokio::time::sleep(Duration::from_millis(200)).await;

    let snap = tracker.snapshot().await;
    assert_eq!(snap.downloads.completed, 1);
    assert_eq!(snap.downloads.total_bytes, 10000);
}

// ---------------------------------------------------------------------------
// Failed downloads tracked
// ---------------------------------------------------------------------------

#[tokio::test]
async fn snapshot_tracks_failures() {
    let bus = EventBus::with_default_capacity();
    let tracker = StatisticsTracker::new(&bus);

    bus.emit(DownloadEvent::DownloadQueued {
        download_id: 1,
        url: "https://example.com/fail.mp4".into(),
        priority: yt_dlp::download::DownloadPriority::Normal,
        output_path: "/tmp/fail.mp4".into(),
    });
    bus.emit(DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "https://example.com/fail.mp4".into(),
        total_bytes: 5000,
        format_id: None,
    });
    bus.emit(DownloadEvent::DownloadFailed {
        download_id: 1,
        url: "https://example.com/fail.mp4".into(),
        error: "network timeout".into(),
        retry_count: 0,
    });

    tokio::time::sleep(Duration::from_millis(200)).await;

    let snap = tracker.snapshot().await;
    assert_eq!(snap.downloads.failed, 1);
}

// ---------------------------------------------------------------------------
// Fetch events tracked
// ---------------------------------------------------------------------------

#[tokio::test]
async fn snapshot_tracks_fetches() {
    let bus = EventBus::with_default_capacity();
    let tracker = StatisticsTracker::new(&bus);

    bus.emit(DownloadEvent::VideoFetched {
        url: "https://youtube.com/watch?v=abc".into(),
        video: Box::new(crate::common::fixtures::load_video_fixture()),
        duration: Duration::from_millis(250),
    });
    bus.emit(DownloadEvent::VideoFetchFailed {
        url: "https://youtube.com/watch?v=def".into(),
        error: "not found".into(),
        duration: Duration::from_millis(100),
    });

    tokio::time::sleep(Duration::from_millis(200)).await;

    let snap = tracker.snapshot().await;
    assert_eq!(snap.fetches.succeeded, 1);
    assert_eq!(snap.fetches.failed, 1);
    assert_eq!(snap.fetches.attempted, 2);
}

// ---------------------------------------------------------------------------
// Concurrent events
// ---------------------------------------------------------------------------

#[tokio::test]
async fn concurrent_events_counted_correctly() {
    let bus = EventBus::with_default_capacity();
    let tracker = StatisticsTracker::new(&bus);

    let mut handles = Vec::new();
    for i in 0..10u64 {
        let bus_clone = bus.clone();
        handles.push(tokio::spawn(async move {
            bus_clone.emit(DownloadEvent::DownloadQueued {
                download_id: i,
                url: format!("https://example.com/{i}.mp4"),
                priority: yt_dlp::download::DownloadPriority::Normal,
                output_path: format!("/tmp/{i}.mp4").into(),
            });
            bus_clone.emit(DownloadEvent::DownloadStarted {
                download_id: i,
                url: format!("https://example.com/{i}.mp4"),
                total_bytes: 1000,
                format_id: None,
            });
            bus_clone.emit(DownloadEvent::DownloadCompleted {
                download_id: i,
                url: format!("https://example.com/{i}.mp4"),
                output_path: format!("/tmp/{i}.mp4").into(),
                duration: Duration::from_secs(1),
                total_bytes: 1000,
            });
        }));
    }

    for handle in handles {
        handle.await.expect("task failed");
    }

    tokio::time::sleep(Duration::from_millis(500)).await;

    let snap = tracker.snapshot().await;
    assert_eq!(snap.downloads.completed, 10);
    assert_eq!(snap.downloads.total_bytes, 10000);
}

// ---------------------------------------------------------------------------
// Reset clears counters
// ---------------------------------------------------------------------------

#[tokio::test]
async fn reset_clears_counters() {
    let bus = EventBus::with_default_capacity();
    let tracker = StatisticsTracker::new(&bus);

    bus.emit(DownloadEvent::DownloadQueued {
        download_id: 1,
        url: "https://example.com/file.mp4".into(),
        priority: yt_dlp::download::DownloadPriority::Normal,
        output_path: "/tmp/out.mp4".into(),
    });
    bus.emit(DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "https://example.com/file.mp4".into(),
        total_bytes: 5000,
        format_id: None,
    });
    bus.emit(DownloadEvent::DownloadCompleted {
        download_id: 1,
        url: "https://example.com/file.mp4".into(),
        output_path: "/tmp/out.mp4".into(),
        duration: Duration::from_secs(2),
        total_bytes: 5000,
    });

    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(tracker.completed_count().await, 1);

    tracker.reset().await;

    let snap = tracker.snapshot().await;
    assert_eq!(snap.downloads.completed, 0);
    assert_eq!(snap.downloads.total_bytes, 0);
}

// ---------------------------------------------------------------------------
// Display / Debug
// ---------------------------------------------------------------------------

#[tokio::test]
async fn display_and_debug() {
    let bus = EventBus::with_default_capacity();
    let tracker = StatisticsTracker::new(&bus);
    let display = format!("{tracker}");
    let debug = format!("{tracker:?}");
    assert!(!display.is_empty());
    assert!(!debug.is_empty());
}
