use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use yt_dlp::events::{DownloadEvent, EventFilter, WebhookConfig, WebhookDelivery};

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
