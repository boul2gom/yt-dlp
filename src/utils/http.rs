//! HTTP utilities and connection pooling.
//!
//! This module provides HTTP client utilities with connection pooling
//! and optimal configuration for the library.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use reqwest::header::HeaderMap;

use crate::client::proxy::ProxyConfig;

// HTTP connection pool configuration
const HTTP_POOL_IDLE_TIMEOUT_SECS: u64 = 90;
const HTTP_POOL_MAX_IDLE_PER_HOST: usize = 32;
const HTTP_TCP_KEEPALIVE_SECS: u64 = 60;
const REQUEST_TIMEOUT_SECS: u64 = 60;
const CONNECT_TIMEOUT_SECS: u64 = 10;

const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

/// Configuration for building an HTTP client.
#[derive(Debug, Clone, Default)]
pub struct HttpClientConfig<'a> {
    pub proxy: Option<&'a ProxyConfig>,
    pub timeout: Option<Duration>,
    pub user_agent: Option<String>,
    pub default_headers: Option<HeaderMap>,
    pub http2_adaptive_window: bool,
}

impl fmt::Display for HttpClientConfig<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HttpClientConfig(proxy={}, timeout={}, http2={})",
            self.proxy.is_some(),
            self.timeout
                .map_or("default".to_string(), |d| format!("{}s", d.as_secs())),
            self.http2_adaptive_window
        )
    }
}

/// Creates a new HTTP client with optimal pooling configuration.
///
/// # Arguments
///
/// * `config` - Client configuration (proxy, timeout, headers, etc.)
///
/// # Returns
///
/// An Arc-wrapped HTTP client configured with connection pooling
///
/// # Errors
///
/// Returns an error if the HTTP client cannot be built
pub fn build_http_client(config: HttpClientConfig) -> crate::error::Result<Arc<Client>> {
    let timeout = config.timeout.unwrap_or(Duration::from_secs(REQUEST_TIMEOUT_SECS));

    tracing::debug!(
        has_proxy = config.proxy.is_some(),
        timeout_secs = timeout.as_secs(),
        pool_idle_timeout_secs = HTTP_POOL_IDLE_TIMEOUT_SECS,
        max_idle_per_host = HTTP_POOL_MAX_IDLE_PER_HOST,
        http2 = config.http2_adaptive_window,
        "⚙️ Creating HTTP client with connection pooling"
    );

    let mut builder = Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .pool_idle_timeout(Duration::from_secs(HTTP_POOL_IDLE_TIMEOUT_SECS))
        .pool_max_idle_per_host(HTTP_POOL_MAX_IDLE_PER_HOST)
        .tcp_keepalive(Duration::from_secs(HTTP_TCP_KEEPALIVE_SECS))
        .tcp_nodelay(true)
        .user_agent(config.user_agent.as_deref().unwrap_or(DEFAULT_USER_AGENT));

    if config.http2_adaptive_window {
        builder = builder.http2_adaptive_window(true);
    }

    if let Some(headers) = config.default_headers {
        builder = builder.default_headers(headers);
    }

    if let Some(proxy_config) = config.proxy {
        match proxy_config.to_reqwest_proxy() {
            Ok(proxy) => {
                tracing::debug!("⚙️ Adding proxy configuration to HTTP client");
                builder = builder.proxy(proxy);
            }
            Err(e) => {
                tracing::warn!(error = %e, "Proxy configuration failed — client will connect directly without proxy");
            }
        }
    }

    let client = builder.build()?;

    tracing::debug!("✅ HTTP client created successfully");

    Ok(Arc::new(client))
}

/// Creates a new HTTP client with optimal pooling configuration (simple API).
///
/// # Arguments
///
/// * `proxy` - Optional proxy configuration
///
/// # Returns
///
/// An Arc-wrapped HTTP client configured with connection pooling
///
/// # Errors
///
/// Returns an error if the HTTP client cannot be built
pub fn create_http_client(proxy: Option<&ProxyConfig>) -> crate::error::Result<Arc<Client>> {
    build_http_client(HttpClientConfig {
        proxy,
        ..Default::default()
    })
}
