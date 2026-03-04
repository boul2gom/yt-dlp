use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use yt_dlp::events::{DownloadEvent, EventBus, EventFilter, HookRegistry};
use yt_dlp::simple_hook;

// ---------------------------------------------------------------------------
// Basic registration and execution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn register_and_execute_hook() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let registry = HookRegistry::new();
    registry
        .register(simple_hook!("test_hook", EventFilter::all(), move |_event| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }))
        .await;

    assert_eq!(registry.count().await, 1);

    let event = DownloadEvent::DownloadResumed { download_id: 1 };
    registry.execute(&event).await;

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

// ---------------------------------------------------------------------------
// Multiple hooks all execute
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multiple_hooks_all_execute() {
    let counter = Arc::new(AtomicU32::new(0));

    let registry = HookRegistry::new();

    for _i in 0..5 {
        let c = counter.clone();
        // simple_hook! names must be &'static str; we use a fixed name here
        registry
            .register(simple_hook!("multi_hook", EventFilter::all(), move |_event| {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }))
            .await;
    }

    assert_eq!(registry.count().await, 5);

    let event = DownloadEvent::DownloadResumed { download_id: 42 };
    registry.execute(&event).await;

    assert_eq!(counter.load(Ordering::SeqCst), 5);
}

// ---------------------------------------------------------------------------
// Filtered hooks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn hook_with_filter_only_fires_for_matching_events() {
    let completed_count = Arc::new(AtomicU32::new(0));
    let cc = completed_count.clone();

    let registry = HookRegistry::new();
    registry
        .register(simple_hook!(
            "completed_only",
            EventFilter::only_completed(),
            move |_event| {
                cc.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        ))
        .await;

    // Non-matching event
    registry
        .execute(&DownloadEvent::DownloadResumed { download_id: 1 })
        .await;
    assert_eq!(completed_count.load(Ordering::SeqCst), 0);

    // Matching event
    registry
        .execute(&DownloadEvent::DownloadCompleted {
            download_id: 1,
            url: "https://example.com".into(),
            output_path: "/tmp/out.mp4".into(),
            duration: Duration::from_secs(5),
            total_bytes: 1024,
        })
        .await;
    assert_eq!(completed_count.load(Ordering::SeqCst), 1);
}

// ---------------------------------------------------------------------------
// Hook with download_id filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn hook_download_id_filter() {
    let counter = Arc::new(AtomicU32::new(0));
    let c = counter.clone();

    let registry = HookRegistry::new();
    registry
        .register(simple_hook!("id_filter", EventFilter::download_id(42), move |_event| {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }))
        .await;

    // Wrong ID
    registry
        .execute(&DownloadEvent::DownloadResumed { download_id: 99 })
        .await;
    assert_eq!(counter.load(Ordering::SeqCst), 0);

    // Correct ID
    registry
        .execute(&DownloadEvent::DownloadResumed { download_id: 42 })
        .await;
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

// ---------------------------------------------------------------------------
// Clear hooks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn clear_removes_all_hooks() {
    let registry = HookRegistry::new();
    registry
        .register(simple_hook!("hook_a", EventFilter::all(), |_event| Ok(())))
        .await;
    registry
        .register(simple_hook!("hook_b", EventFilter::all(), |_event| Ok(())))
        .await;

    assert_eq!(registry.count().await, 2);
    registry.clear().await;
    assert_eq!(registry.count().await, 0);
}

// ---------------------------------------------------------------------------
// Hook integrated with EventBus
// ---------------------------------------------------------------------------

#[tokio::test]
async fn hooks_integrated_with_event_bus() {
    let bus = EventBus::with_default_capacity();
    let counter = Arc::new(AtomicU32::new(0));
    let c = counter.clone();

    let registry = HookRegistry::new();
    registry
        .register(simple_hook!("bus_hook", EventFilter::all(), move |_event| {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }))
        .await;

    // Emit event on bus
    let mut rx = bus.subscribe();
    bus.emit(DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "https://example.com".into(),
        total_bytes: 500,
        format_id: None,
    });

    // Simulate what Downloader does: receive event, execute hooks
    let event = rx.recv().await.expect("recv failed");
    registry.execute(&event).await;

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

// ---------------------------------------------------------------------------
// Display / Debug
// ---------------------------------------------------------------------------

#[tokio::test]
async fn display_and_debug() {
    let registry = HookRegistry::new();
    let display = format!("{registry}");
    let debug = format!("{registry:?}");
    assert!(!display.is_empty());
    assert!(!debug.is_empty());
}
