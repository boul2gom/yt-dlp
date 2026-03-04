use std::fmt;

use async_trait::async_trait;
use yt_dlp::error::Result;
use yt_dlp::extractor::{ExtractorName, VideoExtractor};
use yt_dlp::model::Video;
use yt_dlp::model::playlist::Playlist;

use super::fixtures;

/// A test-only implementation of [`VideoExtractor`] that returns fixture data
/// instead of invoking the real yt-dlp binary.
///
/// This covers the Rust dispatch code paths (trait resolution, URL matching,
/// post-processing) without requiring any external binary.
#[derive(Clone)]
pub struct TestExtractor {
    name: ExtractorName,
    mock_server_url: Option<String>,
}

impl fmt::Debug for TestExtractor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TestExtractor(name={})", self.name)
    }
}

impl TestExtractor {
    /// Creates a TestExtractor that behaves like the YouTube extractor.
    pub fn youtube() -> Self {
        Self {
            name: ExtractorName::Youtube,
            mock_server_url: None,
        }
    }

    /// Creates a TestExtractor that behaves like a generic extractor.
    pub fn generic(site: &str) -> Self {
        Self {
            name: ExtractorName::Generic(Some(site.to_string())),
            mock_server_url: None,
        }
    }

    /// Sets the mock server URL used to rewrite format URLs in fixtures.
    pub fn with_mock_server(mut self, url: &str) -> Self {
        self.mock_server_url = Some(url.to_string());
        self
    }
}

#[async_trait]
impl VideoExtractor for TestExtractor {
    async fn fetch_video(&self, _url: &str) -> Result<Video> {
        match &self.mock_server_url {
            Some(url) => Ok(fixtures::load_video_with_mock_urls(url)),
            None => Ok(fixtures::load_video_fixture()),
        }
    }

    async fn fetch_playlist(&self, _url: &str) -> Result<Playlist> {
        Ok(fixtures::load_playlist_fixture())
    }

    fn name(&self) -> ExtractorName {
        self.name.clone()
    }

    fn supports_url(&self, url: &str) -> bool {
        match &self.name {
            ExtractorName::Youtube => url.contains("youtube.com") || url.contains("youtu.be"),
            ExtractorName::Generic(_) => true,
        }
    }
}
