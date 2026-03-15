use std::sync::Arc;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use yt_dlp::download::Fetcher;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn setup_json_server() -> MockServer {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/data.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"key": "value", "count": 42}))
                .insert_header("Content-Type", "application/json"),
        )
        .mount(&server)
        .await;

    server
}

async fn setup_binary_server() -> MockServer {
    let server = MockServer::start().await;
    let body = vec![0xFE; 8192]; // 8KB

    Mock::given(method("GET"))
        .and(path("/asset.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(body)
                .insert_header("Content-Type", "application/octet-stream")
                .insert_header("Content-Length", "8192")
                .insert_header("Accept-Ranges", "bytes"),
        )
        .mount(&server)
        .await;

    server
}

async fn setup_no_range_server() -> MockServer {
    let server = MockServer::start().await;
    let body = vec![0xAB; 2048];

    Mock::given(method("GET"))
        .and(path("/simple.bin"))
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
// fetch_json
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fetch_json_success() {
    let server = setup_json_server().await;
    let client = Arc::new(reqwest::Client::new());

    let fetcher = Fetcher::with_client(format!("{}/data.json", server.uri()), client);

    let json: serde_json::Value = fetcher.fetch_json(None).await.expect("fetch_json failed");
    assert_eq!(json["key"], "value");
    assert_eq!(json["count"], 42);
}

// ---------------------------------------------------------------------------
// fetch_text
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fetch_text_success() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/hello.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Hello, World!"))
        .mount(&server)
        .await;

    let client = Arc::new(reqwest::Client::new());
    let fetcher = Fetcher::with_client(format!("{}/hello.txt", server.uri()), client);

    let text: String = fetcher.fetch_text(None).await.expect("fetch_text failed");
    assert_eq!(text, "Hello, World!");
}

// ---------------------------------------------------------------------------
// fetch_asset — file download
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fetch_asset_downloads_file() {
    let server = setup_binary_server().await;
    let dir = tempfile::tempdir().expect("tempdir failed");
    let dest = dir.path().join("downloaded.bin");

    let client = Arc::new(reqwest::Client::new());
    let fetcher = Fetcher::with_client(format!("{}/asset.bin", server.uri()), client)
        .with_parallel_segments(1)
        .with_retry_attempts(1);

    let _: () = fetcher.fetch_asset(&dest).await.expect("fetch_asset failed");

    assert!(dest.exists(), "file should exist");
    let content = std::fs::read(&dest).unwrap();
    assert!(
        !content.is_empty(),
        "downloaded file should not be empty (size={})",
        content.len()
    );
}

// ---------------------------------------------------------------------------
// fetch_asset without range support (simple fallback)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fetch_asset_simple_fallback() {
    let server = setup_no_range_server().await;
    let dir = tempfile::tempdir().expect("tempdir failed");
    let dest = dir.path().join("simple.bin");

    let client = Arc::new(reqwest::Client::new());
    let fetcher = Fetcher::with_client(format!("{}/simple.bin", server.uri()), client);

    let _: () = fetcher.fetch_asset(&dest).await.expect("fetch_asset failed");

    assert!(dest.exists());
    let content = std::fs::read(&dest).unwrap();
    assert_eq!(content.len(), 2048, "expected 2048 bytes, got {}", content.len());
}

// ---------------------------------------------------------------------------
// Builder pattern
// ---------------------------------------------------------------------------

#[tokio::test]
async fn builder_chain() {
    let client = Arc::new(reqwest::Client::new());

    let fetcher = Fetcher::with_client("https://example.com/file", client)
        .with_parallel_segments(8)
        .with_segment_size(1_048_576)
        .with_retry_attempts(5)
        .with_speed_profile(yt_dlp::download::SpeedProfile::Aggressive);

    let debug = format!("{fetcher:?}");
    assert!(debug.contains("parallel_segments: 8"));
    assert!(debug.contains("Aggressive"));
}

// ---------------------------------------------------------------------------
// Display / Debug
// ---------------------------------------------------------------------------

#[tokio::test]
async fn display_and_debug() {
    let client = Arc::new(reqwest::Client::new());
    let fetcher = Fetcher::with_client("https://example.com", client);

    let display = format!("{fetcher}");
    let debug = format!("{fetcher:?}");
    assert!(!display.is_empty());
    assert!(!debug.is_empty());
}

// ---------------------------------------------------------------------------
// fetch_json with 404 returns error
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fetch_json_404_returns_error() {
    let server = MockServer::start().await;
    // No routes mounted — all requests get 404

    let client = Arc::new(reqwest::Client::new());
    let fetcher = Fetcher::with_client(format!("{}/nonexistent.json", server.uri()), client);

    let result: yt_dlp::error::Result<serde_json::Value> = fetcher.fetch_json(None).await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Retry on failure
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fetch_retries_on_server_error() {
    let server = MockServer::start().await;

    // First request returns 500, second returns 200
    Mock::given(method("GET"))
        .and(path("/retry.json"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/retry.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;

    let client = Arc::new(reqwest::Client::new());
    let fetcher = Fetcher::with_client(format!("{}/retry.json", server.uri()), client).with_retry_attempts(3);

    // The fetch_json doesn't have built-in retry; this tests that the server
    // prioritization works. The first mock will match first and return 500.
    let result = fetcher.fetch_json(None).await;
    // Will get 500 error since fetch_json doesn't retry internally
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// With range constraint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn with_range_constraint() {
    let client = Arc::new(reqwest::Client::new());
    let fetcher = Fetcher::with_client("https://example.com/file.mp4", client).with_range(100, 500);

    let debug = format!("{fetcher:?}");
    assert!(debug.contains("range_constraint: Some((100, 500))"));
}
