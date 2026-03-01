//! HTTP [`media_seek::RangeFetcher`] implementation.
//!
//! [`HttpRangeFetcher`] wraps a shared `reqwest::Client` and a target URL, providing
//! byte-range fetches needed by `media_seek::parse()` to resolve container seek indices
//! that lie beyond the initial probe window.

use std::future::Future;
use std::sync::Arc;

use reqwest::header::HeaderMap;

/// HTTP `RangeFetcher` backed by a shared `reqwest::Client`.
///
/// Forwards `Range: bytes=start-end` requests to the target URL, passing any
/// format-specific HTTP headers (e.g. signed cookies required by YouTube CDNs).
pub(crate) struct HttpRangeFetcher {
    client: Arc<reqwest::Client>,
    url: String,
    headers: HeaderMap,
}

impl HttpRangeFetcher {
    /// Creates a new fetcher targeting `url` with the given extra `headers`.
    pub(crate) fn new(client: Arc<reqwest::Client>, url: impl Into<String>, headers: HeaderMap) -> Self {
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
            client
                .get(&url)
                .headers(headers)
                .header("Range", format!("bytes={}-{}", start, end))
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await
                .map(|b| b.to_vec())
        }
    }
}
