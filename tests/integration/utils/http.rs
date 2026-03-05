use std::time::Duration;

use yt_dlp::utils::http::{HttpClientConfig, build_http_client, create_http_client};

// ---------------------------------------------------------------------------
// build_http_client — default config
// ---------------------------------------------------------------------------

#[test]
fn build_http_client_default_config_succeeds() {
    let client = build_http_client(HttpClientConfig::default());
    assert!(client.is_ok(), "default HTTP client build should succeed");
}

#[test]
fn build_http_client_with_custom_timeout() {
    let config = HttpClientConfig {
        timeout: Some(Duration::from_secs(30)),
        ..Default::default()
    };
    let client = build_http_client(config);
    assert!(client.is_ok());
}

#[test]
fn build_http_client_with_user_agent() {
    let config = HttpClientConfig {
        user_agent: Some("test-agent/1.0".to_string()),
        ..Default::default()
    };
    let client = build_http_client(config);
    assert!(client.is_ok());
}

#[test]
fn build_http_client_with_http2_adaptive_window() {
    let config = HttpClientConfig {
        http2_adaptive_window: true,
        ..Default::default()
    };
    let client = build_http_client(config);
    assert!(client.is_ok());
}

// ---------------------------------------------------------------------------
// create_http_client — simple API
// ---------------------------------------------------------------------------

#[test]
fn create_http_client_no_proxy_succeeds() {
    let client = create_http_client(None);
    assert!(client.is_ok());
}

// ---------------------------------------------------------------------------
// HttpClientConfig Display
// ---------------------------------------------------------------------------

#[test]
fn display_http_client_config_default_shows_false() {
    let config = HttpClientConfig::default();
    let display = format!("{}", config);
    assert!(display.contains("HttpClientConfig"));
    assert!(display.contains("false")); // proxy and http2 both false
}

#[test]
fn display_http_client_config_with_timeout_shows_seconds() {
    let config = HttpClientConfig {
        timeout: Some(Duration::from_secs(45)),
        ..Default::default()
    };
    let display = format!("{}", config);
    assert!(display.contains("45s"));
}
