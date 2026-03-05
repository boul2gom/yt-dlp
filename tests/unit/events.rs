use std::sync::Arc;
use std::time::Duration;

use yt_dlp::events::{DownloadEvent, EventBus, EventFilter, MetadataType, PostProcessOperation};

// ============================== DownloadEvent ==============================

#[test]
fn event_download_id_for_download_events() {
    let event = DownloadEvent::DownloadStarted {
        download_id: 42,
        url: "https://example.com".to_string(),
        total_bytes: 1000,
        format_id: Some("251".to_string()),
    };
    assert_eq!(event.download_id(), Some(42));
}

#[test]
fn event_download_id_for_non_download_events() {
    let event = DownloadEvent::VideoFetched {
        url: "https://example.com".to_string(),
        video: Box::new(
            serde_json::from_str(
                &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/json/video.json"))
                    .unwrap(),
            )
            .unwrap(),
        ),
        duration: Duration::from_secs(1),
    };
    assert_eq!(event.download_id(), None);
}

#[test]
fn event_is_terminal() {
    let completed = DownloadEvent::DownloadCompleted {
        download_id: 1,
        url: "https://example.com".to_string(),
        output_path: "/tmp/test.mp4".into(),
        duration: Duration::from_secs(10),
        total_bytes: 1000,
    };
    assert!(completed.is_terminal());

    let failed = DownloadEvent::DownloadFailed {
        download_id: 1,
        url: "https://example.com".to_string(),
        error: "timeout".to_string(),
        retry_count: 0,
    };
    assert!(failed.is_terminal());

    let canceled = DownloadEvent::DownloadCanceled {
        download_id: 1,
        reason: "user canceled".to_string(),
    };
    assert!(canceled.is_terminal());
}

#[test]
fn event_is_not_terminal() {
    let progress = DownloadEvent::DownloadProgress {
        download_id: 1,
        downloaded_bytes: 500,
        total_bytes: 1000,
        speed_bytes_per_sec: 100.0,
        eta_seconds: Some(5),
    };
    assert!(!progress.is_terminal());
}

#[test]
fn event_is_progress() {
    let progress = DownloadEvent::DownloadProgress {
        download_id: 1,
        downloaded_bytes: 500,
        total_bytes: 1000,
        speed_bytes_per_sec: 100.0,
        eta_seconds: Some(5),
    };
    assert!(progress.is_progress());

    let started = DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "u".to_string(),
        total_bytes: 100,
        format_id: None,
    };
    assert!(!started.is_progress());
}

#[test]
fn event_type_names() {
    let event = DownloadEvent::DownloadQueued {
        download_id: 1,
        url: "u".to_string(),
        priority: yt_dlp::download::DownloadPriority::Normal,
        output_path: "/tmp/test".into(),
    };
    assert_eq!(event.event_type(), "download_queued");

    let event = DownloadEvent::VideoFetchFailed {
        url: "u".to_string(),
        error: "e".to_string(),
        duration: Duration::from_secs(1),
    };
    assert_eq!(event.event_type(), "video_fetch_failed");
}

#[test]
fn event_display_progress() {
    let event = DownloadEvent::DownloadProgress {
        download_id: 42,
        downloaded_bytes: 500,
        total_bytes: 1000,
        speed_bytes_per_sec: 100.0,
        eta_seconds: Some(5),
    };
    let display = format!("{}", event);
    assert!(display.contains("42"));
    assert!(display.contains("500"));
    assert!(display.contains("1000"));
}

#[test]
fn event_display_generic() {
    let event = DownloadEvent::DownloadStarted {
        download_id: 7,
        url: "u".to_string(),
        total_bytes: 100,
        format_id: None,
    };
    let display = format!("{}", event);
    assert!(display.contains("7"));
}

// ============================== MetadataType ==============================

#[test]
fn metadata_type_display() {
    assert_eq!(format!("{}", MetadataType::Mp3), "Mp3");
    assert_eq!(format!("{}", MetadataType::Mp4), "Mp4");
    assert_eq!(format!("{}", MetadataType::Ffmpeg), "Ffmpeg");
}

// ============================== PostProcessOperation ==============================

#[test]
fn post_process_operation_display() {
    let combine = PostProcessOperation::CombineStreams {
        audio_path: "/tmp/audio.mp3".into(),
        video_path: "/tmp/video.mp4".into(),
    };
    assert_eq!(format!("{}", combine), "CombineStreams");

    let convert = PostProcessOperation::ConvertAudio {
        target_format: "mp3".to_string(),
    };
    assert!(format!("{}", convert).contains("mp3"));

    let custom = PostProcessOperation::Custom {
        description: "trim".to_string(),
    };
    assert!(format!("{}", custom).contains("trim"));
}

// ============================== EventFilter ==============================

#[test]
fn event_filter_all_matches_everything() {
    let filter = EventFilter::all();
    let event = DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "u".to_string(),
        total_bytes: 100,
        format_id: None,
    };
    assert!(filter.matches(&event));
}

#[test]
fn event_filter_download_id() {
    let filter = EventFilter::download_id(42);
    let matching = DownloadEvent::DownloadStarted {
        download_id: 42,
        url: "u".to_string(),
        total_bytes: 100,
        format_id: None,
    };
    let non_matching = DownloadEvent::DownloadStarted {
        download_id: 99,
        url: "u".to_string(),
        total_bytes: 100,
        format_id: None,
    };
    assert!(filter.matches(&matching));
    assert!(!filter.matches(&non_matching));
}

#[test]
fn event_filter_only_terminal() {
    let filter = EventFilter::only_terminal();
    let completed = DownloadEvent::DownloadCompleted {
        download_id: 1,
        url: "u".to_string(),
        output_path: "/tmp/test".into(),
        duration: Duration::from_secs(1),
        total_bytes: 100,
    };
    let progress = DownloadEvent::DownloadProgress {
        download_id: 1,
        downloaded_bytes: 50,
        total_bytes: 100,
        speed_bytes_per_sec: 50.0,
        eta_seconds: Some(1),
    };
    assert!(filter.matches(&completed));
    assert!(!filter.matches(&progress));
}

#[test]
fn event_filter_only_progress() {
    let filter = EventFilter::only_progress();
    let progress = DownloadEvent::DownloadProgress {
        download_id: 1,
        downloaded_bytes: 50,
        total_bytes: 100,
        speed_bytes_per_sec: 50.0,
        eta_seconds: None,
    };
    assert!(filter.matches(&progress));
}

#[test]
fn event_filter_only_completed() {
    let filter = EventFilter::only_completed();
    let completed = DownloadEvent::DownloadCompleted {
        download_id: 1,
        url: "u".to_string(),
        output_path: "/tmp/test".into(),
        duration: Duration::from_secs(1),
        total_bytes: 100,
    };
    let failed = DownloadEvent::DownloadFailed {
        download_id: 1,
        url: "u".to_string(),
        error: "err".to_string(),
        retry_count: 0,
    };
    assert!(filter.matches(&completed));
    assert!(!filter.matches(&failed));
}

#[test]
fn event_filter_only_failed() {
    let filter = EventFilter::only_failed();
    let failed = DownloadEvent::DownloadFailed {
        download_id: 1,
        url: "u".to_string(),
        error: "err".to_string(),
        retry_count: 0,
    };
    assert!(filter.matches(&failed));
}

#[test]
fn event_filter_exclude_progress() {
    let filter = EventFilter::all().exclude_progress();
    let progress = DownloadEvent::DownloadProgress {
        download_id: 1,
        downloaded_bytes: 50,
        total_bytes: 100,
        speed_bytes_per_sec: 50.0,
        eta_seconds: None,
    };
    let started = DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "u".to_string(),
        total_bytes: 100,
        format_id: None,
    };
    assert!(!filter.matches(&progress));
    assert!(filter.matches(&started));
}

#[test]
fn event_filter_only_downloads() {
    let filter = EventFilter::all().only_downloads();
    let download_event = DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "u".to_string(),
        total_bytes: 100,
        format_id: None,
    };
    let meta_event = DownloadEvent::MetadataApplied {
        path: "/tmp/test".into(),
        metadata_type: MetadataType::Mp4,
    };
    assert!(filter.matches(&download_event));
    assert!(!filter.matches(&meta_event));
}

#[test]
fn event_filter_and_then_chaining() {
    let filter = EventFilter::all()
        .and_then(|event| event.download_id() == Some(42))
        .and_then(|event| event.is_terminal());
    let matching = DownloadEvent::DownloadCompleted {
        download_id: 42,
        url: "u".to_string(),
        output_path: "/tmp/test".into(),
        duration: Duration::from_secs(1),
        total_bytes: 100,
    };
    let wrong_id = DownloadEvent::DownloadCompleted {
        download_id: 99,
        url: "u".to_string(),
        output_path: "/tmp/test".into(),
        duration: Duration::from_secs(1),
        total_bytes: 100,
    };
    assert!(filter.matches(&matching));
    assert!(!filter.matches(&wrong_id));
}

#[test]
fn event_filter_event_types() {
    let filter = EventFilter::event_types(vec!["download_started", "download_completed"]);
    let started = DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "u".to_string(),
        total_bytes: 100,
        format_id: None,
    };
    let progress = DownloadEvent::DownloadProgress {
        download_id: 1,
        downloaded_bytes: 50,
        total_bytes: 100,
        speed_bytes_per_sec: 50.0,
        eta_seconds: None,
    };
    assert!(filter.matches(&started));
    assert!(!filter.matches(&progress));
}

#[test]
fn event_filter_only_playlist() {
    let filter = EventFilter::only_playlist();
    let playlist_event = DownloadEvent::PlaylistFetched {
        url: "u".to_string(),
        playlist: serde_json::from_str(
            &std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/json/playlist.json"
            ))
            .unwrap(),
        )
        .unwrap(),
        duration: Duration::from_secs(1),
    };
    assert!(filter.matches(&playlist_event));
}

#[test]
fn event_filter_only_metadata() {
    let filter = EventFilter::only_metadata();
    let meta = DownloadEvent::MetadataApplied {
        path: "/tmp/test".into(),
        metadata_type: MetadataType::Mp3,
    };
    assert!(filter.matches(&meta));
}

#[test]
fn event_filter_only_post_process() {
    let filter = EventFilter::only_post_process();
    let pp = DownloadEvent::PostProcessStarted {
        input_path: "/tmp/in".into(),
        operation: PostProcessOperation::ConvertAudio {
            target_format: "mp3".to_string(),
        },
    };
    assert!(filter.matches(&pp));
}

#[test]
fn event_filter_default_is_all() {
    let filter = EventFilter::default();
    let event = DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "u".to_string(),
        total_bytes: 100,
        format_id: None,
    };
    assert!(filter.matches(&event));
}

#[test]
fn event_filter_debug_display() {
    let filter = EventFilter::download_id(42);
    let debug = format!("{:?}", filter);
    assert!(debug.contains("EventFilter"));
    let display = format!("{}", filter);
    assert!(display.contains("EventFilter"));
}

// ============================== EventBus ==============================

#[tokio::test]
async fn event_bus_emit_and_subscribe() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();

    let event = DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "https://example.com".to_string(),
        total_bytes: 1000,
        format_id: Some("251".to_string()),
    };
    let receivers = bus.emit(event);
    assert_eq!(receivers, 1);

    let received = rx.recv().await.unwrap();
    assert_eq!(received.download_id(), Some(1));
}

#[tokio::test]
async fn event_bus_no_subscribers() {
    let bus = EventBus::new(64);
    let event = DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "u".to_string(),
        total_bytes: 100,
        format_id: None,
    };
    let receivers = bus.emit(event);
    assert_eq!(receivers, 0);
}

#[tokio::test]
async fn event_bus_subscriber_count() {
    let bus = EventBus::new(64);
    assert_eq!(bus.subscriber_count(), 0);
    assert!(!bus.has_subscribers());

    let _rx1 = bus.subscribe();
    assert_eq!(bus.subscriber_count(), 1);
    assert!(bus.has_subscribers());

    let _rx2 = bus.subscribe();
    assert_eq!(bus.subscriber_count(), 2);
}

#[tokio::test]
async fn event_bus_emit_if_subscribed() {
    let bus = EventBus::new(64);

    // No subscribers — should not emit
    let event = DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "u".to_string(),
        total_bytes: 100,
        format_id: None,
    };
    assert!(!bus.emit_if_subscribed(event));

    // With a subscriber
    let _rx = bus.subscribe();
    let event = DownloadEvent::DownloadStarted {
        download_id: 2,
        url: "u".to_string(),
        total_bytes: 100,
        format_id: None,
    };
    assert!(bus.emit_if_subscribed(event));
}

#[tokio::test]
async fn event_bus_default_capacity() {
    let bus = EventBus::with_default_capacity();
    assert_eq!(bus.subscriber_count(), 0);
}

#[tokio::test]
async fn event_bus_clone() {
    let bus = EventBus::new(64);
    let bus2 = bus.clone();
    let mut rx = bus2.subscribe();

    bus.emit(DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "u".to_string(),
        total_bytes: 100,
        format_id: None,
    });

    let received = rx.recv().await.unwrap();
    assert_eq!(received.download_id(), Some(1));
}

#[tokio::test]
async fn event_bus_multiple_events() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();

    for i in 0..5 {
        bus.emit(DownloadEvent::DownloadProgress {
            download_id: i,
            downloaded_bytes: i * 100,
            total_bytes: 1000,
            speed_bytes_per_sec: 100.0,
            eta_seconds: None,
        });
    }

    for i in 0..5 {
        let event = rx.recv().await.unwrap();
        assert_eq!(event.download_id(), Some(i));
    }
}

#[tokio::test]
async fn event_bus_arc_event() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();

    let event = Arc::new(DownloadEvent::DownloadStarted {
        download_id: 1,
        url: "u".to_string(),
        total_bytes: 100,
        format_id: None,
    });
    bus.emit(event);

    let received = rx.recv().await.unwrap();
    assert_eq!(received.download_id(), Some(1));
}

#[test]
fn event_bus_debug_display() {
    let bus = EventBus::new(64);
    let debug = format!("{:?}", bus);
    assert!(debug.contains("EventBus"));
    let display = format!("{}", bus);
    assert!(display.contains("EventBus"));
}

// ============================== RetryStrategy (feature = "webhooks") ==============================

#[cfg(feature = "webhooks")]
mod retry_strategy_tests {
    use std::time::Duration;

    use yt_dlp::events::RetryStrategy;

    #[test]
    fn build_exponential_retry_strategy_sets_fields() {
        let s = RetryStrategy::exponential(3, Duration::from_secs(1), Duration::from_secs(30));
        assert_eq!(s.max_attempts, 3);
        assert_eq!(s.initial_delay, Duration::from_secs(1));
        assert_eq!(s.max_delay, Duration::from_secs(30));
        assert!((s.backoff_multiplier - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn build_linear_retry_strategy_sets_constant_delay() {
        let s = RetryStrategy::linear(5, Duration::from_millis(200));
        assert_eq!(s.max_attempts, 5);
        assert_eq!(s.initial_delay, Duration::from_millis(200));
        assert_eq!(s.max_delay, Duration::from_millis(200));
        assert!((s.backoff_multiplier - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn build_none_retry_strategy_sets_zero_attempts() {
        let s = RetryStrategy::none();
        assert_eq!(s.max_attempts, 0);
        assert_eq!(s.initial_delay, Duration::ZERO);
    }

    #[test]
    fn check_retry_strategy_default_is_exponential() {
        let s = RetryStrategy::default();
        assert_eq!(s.max_attempts, 3);
        assert_eq!(s.initial_delay, Duration::from_secs(1));
        assert_eq!(s.max_delay, Duration::from_secs(30));
    }

    #[test]
    fn retry_strategy_delay_for_attempt_0_is_initial() {
        let s = RetryStrategy::exponential(3, Duration::from_secs(1), Duration::from_secs(60));
        let delay = s.delay_for_attempt(0);
        assert_eq!(delay, Duration::from_secs(1));
    }

    #[test]
    fn retry_strategy_delay_for_attempt_1_doubles() {
        let s = RetryStrategy::exponential(3, Duration::from_secs(1), Duration::from_secs(60));
        let delay = s.delay_for_attempt(1);
        assert_eq!(delay, Duration::from_secs(2));
    }

    #[test]
    fn retry_strategy_delay_capped_at_max() {
        let s = RetryStrategy::exponential(5, Duration::from_secs(10), Duration::from_secs(15));
        // 10 * 2^2 = 40, capped at 15
        let delay = s.delay_for_attempt(2);
        assert_eq!(delay, Duration::from_secs(15));
    }

    #[test]
    fn retry_strategy_delay_for_attempt_at_limit_is_zero() {
        let s = RetryStrategy::exponential(3, Duration::from_secs(1), Duration::from_secs(30));
        // attempt 3 >= max_attempts 3 → zero delay
        let delay = s.delay_for_attempt(3);
        assert_eq!(delay, Duration::ZERO);
    }

    #[test]
    fn retry_strategy_should_retry_within_limit() {
        let s = RetryStrategy::exponential(3, Duration::from_secs(1), Duration::from_secs(30));
        assert!(s.should_retry(0));
        assert!(s.should_retry(1));
        assert!(s.should_retry(2));
    }

    #[test]
    fn retry_strategy_should_not_retry_at_limit() {
        let s = RetryStrategy::exponential(3, Duration::from_secs(1), Duration::from_secs(30));
        assert!(!s.should_retry(3));
        assert!(!s.should_retry(10));
    }

    #[test]
    fn retry_strategy_none_never_retries() {
        let s = RetryStrategy::none();
        assert!(!s.should_retry(0));
    }

    #[test]
    fn display_retry_strategy_shows_key_fields() {
        let s = RetryStrategy::exponential(3, Duration::from_secs(1), Duration::from_secs(30));
        let display = format!("{}", s);
        assert!(display.contains("RetryStrategy"));
        assert!(display.contains("3"));
        assert!(display.contains("2"));
    }

    #[test]
    fn retry_strategy_linear_delay_is_constant() {
        let s = RetryStrategy::linear(3, Duration::from_secs(5));
        let delay0 = s.delay_for_attempt(0);
        let delay1 = s.delay_for_attempt(1);
        let delay2 = s.delay_for_attempt(2);
        assert_eq!(delay0, delay1);
        assert_eq!(delay1, delay2);
    }
}
