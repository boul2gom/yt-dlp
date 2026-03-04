//! HTTP [`media_seek::RangeFetcher`] implementation.
//!
//! [`HttpRangeFetcher`] wraps a shared `reqwest::Client` and a target URL, providing
//! byte-range fetches needed by `media_seek::parse()` to resolve container seek indices
//! that lie beyond the initial probe window.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::HeaderMap;

/// Maximum response size for a range fetch (16 MB).
const MAX_RANGE_RESPONSE: usize = 16 * 1024 * 1024;
/// Timeout for a single range fetch request.
const RANGE_FETCH_TIMEOUT: Duration = Duration::from_secs(30);

/// HTTP `RangeFetcher` backed by a shared `reqwest::Client`.
///
/// Forwards `Range: bytes=start-end` requests to the target URL, passing any
/// format-specific HTTP headers (e.g. signed cookies required by YouTube CDNs).
pub struct HttpRangeFetcher {
    client: Arc<reqwest::Client>,
    url: String,
    headers: HeaderMap,
}

impl HttpRangeFetcher {
    /// Creates a new fetcher targeting `url` with the given extra `headers`.
    ///
    /// # Arguments
    ///
    /// * `client` - Shared reqwest HTTP client
    /// * `url` - Target URL to fetch byte ranges from
    /// * `headers` - Extra HTTP headers (e.g. cookies for authenticated CDNs)
    ///
    /// # Returns
    ///
    /// A new `HttpRangeFetcher` ready to serve range requests.
    pub fn new(client: Arc<reqwest::Client>, url: impl Into<String>, headers: HeaderMap) -> Self {
        Self {
            client,
            url: url.into(),
            headers,
        }
    }
}

impl media_seek::RangeFetcher for HttpRangeFetcher {
    type Error = reqwest::Error;

    fn fetch(&self, start: u64, end: u64) -> impl Future<Output = std::result::Result<Vec<u8>, reqwest::Error>> + Send {
        let client = Arc::clone(&self.client);
        let url = self.url.clone();
        let headers = self.headers.clone();
        async move {
            let response = client
                .get(&url)
                .headers(headers)
                .header("Range", format!("bytes={}-{}", start, end))
                .timeout(RANGE_FETCH_TIMEOUT)
                .send()
                .await?
                .error_for_status()?;

            // Limit response body to prevent OOM on malicious servers
            let content_length = response.content_length().unwrap_or(0);
            if content_length > MAX_RANGE_RESPONSE as u64 {
                return Ok(Vec::new());
            }

            response.bytes().await.map(|b| b.to_vec())
        }
    }
}
