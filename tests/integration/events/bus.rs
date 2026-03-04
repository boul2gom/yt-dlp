use std::sync::Arc;
use std::time::Duration;

use tokio_stream::StreamExt;
use yt_dlp::events::{DownloadEvent, EventBus};

// ---------------------------------------------------------------------------
// Multi-subscriber broadcast (integration-only)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multiple_subscribers_receive_same_event() {
    let bus = EventBus::with_default_capacity();
    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();
    let mut rx3 = bus.subscribe();

    assert_eq!(bus.subscriber_count(), 3);
    assert!(bus.has_subscribers());

    let event = DownloadEvent::DownloadCompleted {
        download_id: 7,
        url: "https://example.com/file.mp4".into(),
        output_path: "/tmp/file.mp4".into(),
        duration: Duration::from_secs(10),
        total_bytes: 4096,
    };

    let count = bus.emit(event);
    assert_eq!(count, 3);

    for rx in [&mut rx1, &mut rx2, &mut rx3] {
        let ev = rx.recv().await.expect("recv failed");
        assert_eq!(ev.download_id(), Some(7));
    }
}

// ---------------------------------------------------------------------------
// Subscriber lifecycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn subscriber_count_tracks_drops() {
    let bus = EventBus::with_default_capacity();
    let rx1 = bus.subscribe();
    assert_eq!(bus.subscriber_count(), 1);

    let rx2 = bus.subscribe();
    assert_eq!(bus.subscriber_count(), 2);

    drop(rx1);
    assert_eq!(bus.subscriber_count(), 1);

    drop(rx2);
    assert_eq!(bus.subscriber_count(), 0);
    assert!(!bus.has_subscribers());
}

// ---------------------------------------------------------------------------
// Stream API
// ---------------------------------------------------------------------------

#[tokio::test]
async fn stream_receives_events() {
    let bus = EventBus::with_default_capacity();
    let mut stream = bus.stream();

    bus.emit(DownloadEvent::DownloadStarted {
        download_id: 10,
        url: "https://example.com".into(),
        total_bytes: 100,
        format_id: None,
    });

    bus.emit(DownloadEvent::DownloadCompleted {
        download_id: 10,
        url: "https://example.com".into(),
        output_path: "/tmp/out.mp4".into(),
        duration: Duration::from_secs(1),
        total_bytes: 100,
    });

    let first = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .expect("timed out")
        .expect("stream ended")
        .expect("recv error");
    assert_eq!(first.download_id(), Some(10));
    assert_eq!(first.event_type(), "download_started");

    let second = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .expect("timed out")
        .expect("stream ended")
        .expect("recv error");
    assert!(second.is_terminal());
}

// ---------------------------------------------------------------------------
// Backpressure (lag)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn backpressure_does_not_panic() {
    let bus = EventBus::new(4); // tiny capacity
    let _rx = bus.subscribe();

    // Emit more events than capacity — should not panic
    for i in 0..20u64 {
        bus.emit(DownloadEvent::DownloadProgress {
            download_id: i,
            downloaded_bytes: i * 100,
            total_bytes: 2000,
            speed_bytes_per_sec: 100.0,
            eta_seconds: None,
        });
    }
}

// ---------------------------------------------------------------------------
// Arc wrapper
// ---------------------------------------------------------------------------

#[tokio::test]
async fn emit_accepts_arc_event() {
    let bus = EventBus::with_default_capacity();
    let mut rx = bus.subscribe();

    let event = Arc::new(DownloadEvent::DownloadPaused {
        download_id: 5,
        reason: "user request".into(),
    });

    bus.emit(event.clone());

    let received = rx.recv().await.expect("recv failed");
    assert_eq!(received.download_id(), Some(5));
}

// ---------------------------------------------------------------------------
// Display / Debug
// ---------------------------------------------------------------------------

#[tokio::test]
async fn display_and_debug() {
    let bus = EventBus::with_default_capacity();
    let display = format!("{bus}");
    let debug = format!("{bus:?}");
    assert!(!display.is_empty());
    assert!(!debug.is_empty());
}
