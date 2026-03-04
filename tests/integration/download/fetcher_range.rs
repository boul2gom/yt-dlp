use std::sync::Arc;

use media_seek::RangeFetcher;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use yt_dlp::download::HttpRangeFetcher;

// ============================== Helpers ==============================

async fn setup_range_server() -> (MockServer, Vec<u8>) {
    let server = MockServer::start().await;
    let body: Vec<u8> = (0..=255).cycle().take(1024).collect();

    Mock::given(method("GET"))
        .and(path("/data.bin"))
        .and(header("Range", "bytes=100-199"))
        .respond_with(
            ResponseTemplate::new(206)
                .set_body_bytes(body[100..200].to_vec())
                .insert_header("Content-Range", "bytes 100-199/1024")
                .insert_header("Content-Length", "100"),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/data.bin"))
        .and(header("Range", "bytes=0-49"))
        .respond_with(
            ResponseTemplate::new(206)
                .set_body_bytes(body[0..50].to_vec())
                .insert_header("Content-Range", "bytes 0-49/1024")
                .insert_header("Content-Length", "50"),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/data.bin"))
        .and(header("Range", "bytes=1000-1023"))
        .respond_with(
            ResponseTemplate::new(206)
                .set_body_bytes(body[1000..1024].to_vec())
                .insert_header("Content-Range", "bytes 1000-1023/1024")
                .insert_header("Content-Length", "24"),
        )
        .mount(&server)
        .await;

    (server, body)
}

// ============================== Tests ==============================

#[tokio::test]
async fn fetch_returns_correct_bytes() {
    let (server, body) = setup_range_server().await;
    let client = Arc::new(reqwest::Client::new());
    let url = format!("{}/data.bin", server.uri());
    let fetcher = HttpRangeFetcher::new(client, url, Default::default());

    let bytes = fetcher.fetch(100, 199).await.unwrap();
    assert_eq!(bytes.len(), 100);
    assert_eq!(bytes, &body[100..200]);
}

#[tokio::test]
async fn fetch_first_50_bytes() {
    let (server, body) = setup_range_server().await;
    let client = Arc::new(reqwest::Client::new());
    let url = format!("{}/data.bin", server.uri());
    let fetcher = HttpRangeFetcher::new(client, url, Default::default());

    let bytes = fetcher.fetch(0, 49).await.unwrap();
    assert_eq!(bytes.len(), 50);
    assert_eq!(bytes, &body[0..50]);
}

#[tokio::test]
async fn fetch_tail_bytes() {
    let (server, body) = setup_range_server().await;
    let client = Arc::new(reqwest::Client::new());
    let url = format!("{}/data.bin", server.uri());
    let fetcher = HttpRangeFetcher::new(client, url, Default::default());

    let bytes = fetcher.fetch(1000, 1023).await.unwrap();
    assert_eq!(bytes.len(), 24);
    assert_eq!(bytes, &body[1000..1024]);
}

#[tokio::test]
async fn fetch_with_custom_headers() {
    let server = MockServer::start().await;
    let payload = vec![0xAB; 64];

    Mock::given(method("GET"))
        .and(path("/auth.bin"))
        .and(header("Range", "bytes=0-63"))
        .and(header("X-Custom-Token", "secret123"))
        .respond_with(
            ResponseTemplate::new(206)
                .set_body_bytes(payload.clone())
                .insert_header("Content-Range", "bytes 0-63/64"),
        )
        .mount(&server)
        .await;

    let client = Arc::new(reqwest::Client::new());
    let url = format!("{}/auth.bin", server.uri());
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("X-Custom-Token", "secret123".parse().unwrap());

    let fetcher = HttpRangeFetcher::new(client, url, headers);
    let bytes = fetcher.fetch(0, 63).await.unwrap();
    assert_eq!(bytes, payload);
}

#[tokio::test]
async fn fetch_404_returns_error() {
    let server = MockServer::start().await;

    // No mock mounted — wiremock returns 404
    let client = Arc::new(reqwest::Client::new());
    let url = format!("{}/missing.bin", server.uri());
    let fetcher = HttpRangeFetcher::new(client, url, Default::default());

    let result = fetcher.fetch(0, 100).await;
    assert!(result.is_err(), "404 should produce an error");
}

#[tokio::test]
async fn fetch_server_error_returns_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/error.bin"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = Arc::new(reqwest::Client::new());
    let url = format!("{}/error.bin", server.uri());
    let fetcher = HttpRangeFetcher::new(client, url, Default::default());

    let result = fetcher.fetch(0, 100).await;
    assert!(result.is_err(), "500 should produce an error");
}
