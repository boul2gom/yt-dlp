//! HTTP utilities and connection pooling.
//!
//! This module provides HTTP client utilities with connection pooling
//! and optimal configuration for the library.

use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;

// HTTP connection pool configuration
const HTTP_POOL_IDLE_TIMEOUT_SECS: u64 = 90;
const HTTP_POOL_MAX_IDLE_PER_HOST: usize = 32;
const HTTP_TCP_KEEPALIVE_SECS: u64 = 60;
const REQUEST_TIMEOUT_SECS: u64 = 60;

/// Creates a new HTTP client with optimal pooling configuration
pub fn create_http_client() -> Arc<Client> {
    let client = Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .pool_idle_timeout(Duration::from_secs(HTTP_POOL_IDLE_TIMEOUT_SECS))
        .pool_max_idle_per_host(HTTP_POOL_MAX_IDLE_PER_HOST)
        .tcp_keepalive(Duration::from_secs(HTTP_TCP_KEEPALIVE_SECS))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .expect("Failed to build HTTP client");

    Arc::new(client)
}
