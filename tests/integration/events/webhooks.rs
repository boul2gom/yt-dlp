use std::collections::HashMap;
use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use yt_dlp::events::{DownloadEvent, EventFilter, RetryStrategy, WebhookConfig, WebhookDelivery, WebhookMethod};

// ---------------------------------------------------------------------------
// Basic webhook delivery
// ---------------------------------------------------------------------------

#[tokio::test]
async fn webhook_delivers_event_to_server() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/webhook"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let delivery = WebhookDelivery::new();
    let config = WebhookConfig::new(format!("{}/webhook", server.uri()));
    delivery.register(config).await;

    assert_eq!(delivery.count().await, 1);

    let event = DownloadEvent::DownloadCompleted {
        download_id: 1,
        url: "https://example.com/video.mp4".into(),
        output_path: "/tmp/output.mp4".into(),
        duration: Duration::from_secs(5),
        total_bytes: 1024,
    };

    delivery.process_event(&event).await;

    // Give the async delivery task time to complete
    tokio::time::sleep(Duration::from_millis(500)).await;
}

// ---------------------------------------------------------------------------
// WebhookConfig builder
// ---------------------------------------------------------------------------

#[tokio::test]
async fn webhook_config_builder_methods() {
    let config = WebhookConfig::new("https://example.com/hook")
        .with_method(yt_dlp::events::WebhookMethod::Put)
        .with_header("X-Custom", "value")
        .with_timeout(Duration::from_secs(10))
        .with_full_data(true);

    assert_eq!(config.url(), "https://example.com/hook");
}

// ---------------------------------------------------------------------------
// Filtered webhook — only completed events
// ---------------------------------------------------------------------------

#[tokio::test]
async fn webhook_with_filter_skips_non_matching() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/webhook"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1) // only the completed event should arrive
        .mount(&server)
        .await;

    let delivery = WebhookDelivery::new();
    let config = WebhookConfig::new(format!("{}/webhook", server.uri())).with_filter(EventFilter::only_completed());
    delivery.register(config).await;

    // Non-matching: progress event
    delivery
        .process_event(&DownloadEvent::DownloadProgress {
            download_id: 1,
            downloaded_bytes: 500,
            total_bytes: 1000,
            speed_bytes_per_sec: 100.0,
            eta_seconds: Some(5),
        })
        .await;

    // Matching: completed event
    delivery
        .process_event(&DownloadEvent::DownloadCompleted {
            download_id: 1,
            url: "https://example.com".into(),
            output_path: "/tmp/out.mp4".into(),
            duration: Duration::from_secs(3),
            total_bytes: 1000,
        })
        .await;

    tokio::time::sleep(Duration::from_millis(500)).await;
}

// ---------------------------------------------------------------------------
// Clear webhooks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn clear_removes_all_webhooks() {
    let delivery = WebhookDelivery::new();
    delivery.register(WebhookConfig::new("https://example.com/a")).await;
    delivery.register(WebhookConfig::new("https://example.com/b")).await;

    assert_eq!(delivery.count().await, 2);
    delivery.clear().await;
    assert_eq!(delivery.count().await, 0);
}

// ---------------------------------------------------------------------------
// Display / Debug
// ---------------------------------------------------------------------------

#[tokio::test]
async fn display_and_debug() {
    let delivery = WebhookDelivery::new();
    let display = format!("{delivery}");
    let debug = format!("{delivery:?}");
    assert!(!display.is_empty());
    assert!(!debug.is_empty());
}

// ---------------------------------------------------------------------------
// WebhookMethod and WebhookConfig Display
// ---------------------------------------------------------------------------

#[test]
fn webhook_method_display_variants() {
    assert_eq!(format!("{}", WebhookMethod::Post), "Post");
    assert_eq!(format!("{}", WebhookMethod::Put), "Put");
    assert_eq!(format!("{}", WebhookMethod::Patch), "Patch");
}

#[test]
fn webhook_config_display_contains_url() {
    let config = WebhookConfig::new("https://example.com/hook");
    let display = format!("{}", config);
    assert!(display.contains("WebhookConfig"));
    assert!(display.contains("https://example.com/hook"));
    assert!(display.contains("10s")); // default timeout
}

// ---------------------------------------------------------------------------
// from_env — no URL set
// ---------------------------------------------------------------------------

#[test]
fn webhook_from_env_without_url_returns_none() {
    // Ensure the env var is not set in this test
    // SAFETY: test-only, single-threaded context
    unsafe { std::env::remove_var("YTDLP_WEBHOOK_URL") };
    let config = WebhookConfig::from_env();
    assert!(config.is_none());
}

// ---------------------------------------------------------------------------
// with_headers (multiple)
// ---------------------------------------------------------------------------

#[test]
fn webhook_config_with_multiple_headers_builds_correctly() {
    let mut headers = HashMap::new();
    headers.insert("X-Auth".to_string(), "token123".to_string());
    headers.insert("X-Service".to_string(), "test-service".to_string());

    let config = WebhookConfig::new("https://example.com").with_headers(headers);

    assert_eq!(config.url(), "https://example.com");
}

// ---------------------------------------------------------------------------
// with_retry_strategy
// ---------------------------------------------------------------------------

#[test]
fn webhook_config_with_retry_strategy_builds_correctly() {
    let strategy = RetryStrategy::exponential(3, Duration::from_millis(100), Duration::from_secs(5));
    let config = WebhookConfig::new("https://example.com").with_retry_strategy(strategy);

    assert_eq!(config.url(), "https://example.com");
}

// ---------------------------------------------------------------------------
// Retry on 5xx — server always returns 503
// ---------------------------------------------------------------------------

#[tokio::test]
async fn webhook_retries_on_5xx_responses() {
    let server = MockServer::start().await;

    // Expect up to 3 delivery attempts (initial + 2 retries)
    Mock::given(method("POST"))
        .and(path("/retry-webhook"))
        .respond_with(ResponseTemplate::new(503))
        .expect(3)
        .mount(&server)
        .await;

    let delivery = WebhookDelivery::new();
    let strategy = RetryStrategy::exponential(3, Duration::from_millis(20), Duration::from_millis(100));
    let config = WebhookConfig::new(format!("{}/retry-webhook", server.uri())).with_retry_strategy(strategy);
    delivery.register(config).await;

    let event = DownloadEvent::DownloadCompleted {
        download_id: 99,
        url: "https://example.com/video.mp4".into(),
        output_path: "/tmp/output.mp4".into(),
        duration: Duration::from_secs(5),
        total_bytes: 512,
    };
    delivery.process_event(&event).await;

    // Allow enough time for 3 retries with 20ms delays
    tokio::time::sleep(Duration::from_millis(800)).await;
}

// ---------------------------------------------------------------------------
// Non-retryable 4xx — should only attempt once
// ---------------------------------------------------------------------------

#[tokio::test]
async fn webhook_no_retry_on_4xx_response() {
    let server = MockServer::start().await;

    // Only 1 attempt expected for 404 (non-retryable)
    Mock::given(method("POST"))
        .and(path("/not-found-webhook"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;

    let delivery = WebhookDelivery::new();
    let config = WebhookConfig::new(format!("{}/not-found-webhook", server.uri()));
    delivery.register(config).await;

    let event = DownloadEvent::DownloadCompleted {
        download_id: 100,
        url: "https://example.com/video.mp4".into(),
        output_path: "/tmp/output.mp4".into(),
        duration: Duration::from_secs(5),
        total_bytes: 512,
    };
    delivery.process_event(&event).await;

    tokio::time::sleep(Duration::from_millis(500)).await;
}
