//! YouTube extractor with platform-specific optimizations.
//!
//! This extractor provides highly optimized YouTube downloading with:
//! - Player client selection (Android, iOS, Web, TV Embedded)
//! - Format presets for common use cases
//! - YouTube-specific shortcuts (channel, user, search)
//! - Performance optimizations

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::Result;
use crate::extractor::VideoExtractor;
use crate::model::Video;
use crate::model::playlist::Playlist;

/// YouTube player client types.
///
/// Different player clients have different capabilities and performance characteristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerClient {
    /// Android client (bypasses some throttling, works on restricted videos)
    Android,
    /// iOS client (good quality, reliable)
    IOS,
    /// Web client (all formats available, well-tested)
    Web,
    /// TV Embedded client (bypasses age restrictions)
    TvEmbedded,
}

impl PlayerClient {
    fn as_arg(&self) -> &str {
        match self {
            PlayerClient::Android => "android",
            PlayerClient::IOS => "ios",
            PlayerClient::Web => "web",
            PlayerClient::TvEmbedded => "tv_embedded",
        }
    }
}

/// Format preset for YouTube downloads.
///
/// These presets provide common format selection patterns optimized for different use cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatPreset {
    /// Best available quality (highest resolution + best audio)
    Best,
    /// Premium quality (1080p+ with high bitrate audio)
    Premium,
    /// High quality (1080p with good audio)
    High,
    /// Medium quality (720p with standard audio)
    Medium,
    /// Low quality (480p or lower, smaller file size)
    Low,
    /// Audio only (best audio quality)
    AudioOnly,
    /// Modern codecs (VP9/AV1 + Opus for smaller files)
    ModernCodecs,
    /// Legacy compatibility (H.264 + AAC for older devices)
    LegacyCompatible,
    /// Custom format selector string
    Custom(String),
}

impl FormatPreset {
    fn to_format_selector(&self) -> String {
        match self {
            Self::Best => "bestvideo+bestaudio/best".to_string(),
            Self::Premium => "bestvideo[height>=1080]+bestaudio[abr>=192]/best".to_string(),
            Self::High => "bestvideo[height>=1080]+bestaudio/best".to_string(),
            Self::Medium => "bestvideo[height<=720]+bestaudio/best".to_string(),
            Self::Low => "bestvideo[height<=480]+bestaudio/best".to_string(),
            Self::AudioOnly => "bestaudio/best".to_string(),
            Self::ModernCodecs => "bestvideo[vcodec^=vp9]+bestaudio[acodec=opus]/best".to_string(),
            Self::LegacyCompatible => "best[ext=mp4]/best".to_string(),
            Self::Custom(selector) => selector.clone(),
        }
    }
}

/// YouTube extractor with optimizations.
///
/// This struct provides access to YouTube-specific features and optimizations
/// that go beyond generic video downloading.
#[derive(Debug)]
pub struct Youtube {
    executable_path: PathBuf,
    player_client: Option<PlayerClient>,
    skip_dash: bool,
    format_preset: Option<FormatPreset>,
    args: Vec<String>,
    timeout: Duration,
}

impl Youtube {
    /// Create a new YouTube extractor.
    ///
    /// # Arguments
    ///
    /// * `executable_path` - Path to the yt-dlp executable
    ///
    /// # Returns
    ///
    /// A new Youtube extractor instance
    pub fn new(executable_path: PathBuf) -> Self {
        tracing::debug!(
            executable = ?executable_path,
            "Creating new Youtube extractor"
        );

        Self {
            executable_path,
            player_client: None,
            skip_dash: false,
            format_preset: None,
            args: Vec::new(),
            timeout: crate::client::DEFAULT_TIMEOUT,
        }
    }

    /// Set YouTube player client for optimal performance.
    ///
    /// # Arguments
    ///
    /// * `client` - The player client to use
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use yt_dlp::extractor::Youtube;
    /// # use yt_dlp::extractor::youtube::PlayerClient;
    /// # use std::path::PathBuf;
    /// let mut extractor = Youtube::new(PathBuf::from("yt-dlp"));
    /// extractor.with_player_client(PlayerClient::Android);
    /// ```
    pub fn with_player_client(&mut self, client: PlayerClient) -> &mut Self {
        tracing::debug!(
            player_client = ?client,
            "Setting YouTube player client"
        );

        self.player_client = Some(client);
        self
    }

    /// Skip DASH manifest for faster extraction.
    ///
    /// This speeds up video information fetching but may miss some formats.
    ///
    /// # Arguments
    ///
    /// * `skip` - Whether to skip DASH manifest parsing
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn skip_dash_manifest(&mut self, skip: bool) -> &mut Self {
        tracing::debug!(skip_dash = skip, "Setting DASH manifest skip option");

        self.skip_dash = skip;
        self
    }

    /// Set format preset for video quality.
    ///
    /// # Arguments
    ///
    /// * `preset` - The format preset to use
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_format_preset(&mut self, preset: FormatPreset) -> &mut Self {
        tracing::debug!(
            preset = ?preset,
            "Setting format preset"
        );

        self.format_preset = Some(preset);
        self
    }

    /// Add custom yt-dlp argument.
    ///
    /// # Arguments
    ///
    /// * `arg` - The argument to add
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_arg(&mut self, arg: String) -> &mut Self {
        tracing::debug!(
            arg = %arg,
            "Adding custom argument"
        );

        self.args.push(arg);
        self
    }

    /// Set timeout for yt-dlp operations.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The timeout duration
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_timeout(&mut self, timeout: Duration) -> &mut Self {
        tracing::debug!(
            timeout_secs = timeout.as_secs(),
            "Setting timeout for extractor"
        );

        self.timeout = timeout;
        self
    }

    /// Use a Netscape cookie file for authentication.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the Netscape cookie file
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_cookies(&mut self, path: impl AsRef<Path>) -> &mut Self {
        let cookie_path = path.as_ref().display().to_string();
        tracing::debug!(
            cookie_file = cookie_path,
            "Adding cookie file for authentication"
        );
        self.args.push(format!("--cookies={}", cookie_path));
        self
    }

    /// Extract cookies from a browser for authentication.
    ///
    /// # Arguments
    ///
    /// * `browser` - Browser name (e.g. `"chrome"`, `"firefox"`)
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_cookies_from_browser(&mut self, browser: &str) -> &mut Self {
        tracing::debug!(browser = browser, "Adding browser cookie extraction");
        self.args
            .push(format!("--cookies-from-browser={}", browser));
        self
    }

    /// Use .netrc for authentication.
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_netrc(&mut self) -> &mut Self {
        tracing::debug!("Enabling .netrc authentication");
        self.args.push("--netrc".to_string());
        self
    }

    // ========== YouTube-Specific Methods ==========

    /// Fetch channel by ID (fast, direct API).
    ///
    /// # Arguments
    ///
    /// * `channel_id` - The YouTube channel ID
    ///
    /// # Returns
    ///
    /// Playlist containing all channel videos
    ///
    /// # Errors
    ///
    /// Returns error if channel is not found or inaccessible
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use yt_dlp::extractor::Youtube;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let extractor = Youtube::new(PathBuf::from("yt-dlp"));
    /// let channel = extractor.fetch_channel("Underscore_").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_channel(&self, channel_id: &str) -> Result<Playlist> {
        tracing::debug!(channel_id = channel_id, "Fetching YouTube channel by ID");

        let url = format!("https://www.youtube.com/channel/{}", channel_id);
        self.fetch_playlist(&url).await
    }

    /// Fetch channel by handle (@username).
    ///
    /// # Arguments
    ///
    /// * `handle` - The YouTube channel handle (without @)
    ///
    /// # Returns
    ///
    /// Playlist containing all channel videos
    ///
    /// # Errors
    ///
    /// Returns error if channel is not found or inaccessible
    pub async fn fetch_channel_by_handle(&self, handle: &str) -> Result<Playlist> {
        tracing::debug!(handle = handle, "Fetching YouTube channel by handle");

        let url = format!("https://www.youtube.com/@{}", handle);
        self.fetch_playlist(&url).await
    }

    /// Fetch user's uploads (legacy URL format).
    ///
    /// # Arguments
    ///
    /// * `username` - The YouTube username
    ///
    /// # Returns
    ///
    /// Playlist containing all user videos
    ///
    /// # Errors
    ///
    /// Returns error if user is not found or inaccessible
    pub async fn fetch_user(&self, username: &str) -> Result<Playlist> {
        tracing::debug!(username = username, "Fetching YouTube user uploads");

        let url = format!("https://www.youtube.com/user/{}", username);
        self.fetch_playlist(&url).await
    }

    /// Fetch playlist with pagination control.
    ///
    /// # Arguments
    ///
    /// * `playlist_id` - The YouTube playlist ID
    /// * `start` - Starting video index (1-based)
    /// * `count` - Number of videos to fetch
    ///
    /// # Returns
    ///
    /// Playlist containing specified range of videos
    ///
    /// # Errors
    ///
    /// Returns error if playlist is not found or inaccessible
    pub async fn fetch_playlist_paginated(
        &self,
        playlist_id: &str,
        start: usize,
        count: usize,
    ) -> Result<Playlist> {
        tracing::debug!(
            playlist_id = playlist_id,
            start = start,
            count = count,
            end = start + count - 1,
            "Fetching paginated YouTube playlist"
        );

        let mut args = self.build_base_args();
        args.push("--flat-playlist".to_string());
        args.push(format!("--playlist-start={}", start));
        args.push(format!("--playlist-end={}", start + count - 1));

        let url = format!("https://www.youtube.com/playlist?list={}", playlist_id);
        args.push(url);

        self.execute_for_playlist(&args).await
    }

    /// Search YouTube videos.
    ///
    /// # Arguments
    ///
    /// * `query` - The search query
    /// * `max_results` - Maximum number of results to return
    ///
    /// # Returns
    ///
    /// Playlist containing search results
    ///
    /// # Errors
    ///
    /// Returns error if search fails
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use yt_dlp::extractor::Youtube;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let extractor = Youtube::new(PathBuf::from("yt-dlp"));
    /// let results = extractor.search("rust programming", 10).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn search(&self, query: &str, max_results: usize) -> Result<Playlist> {
        tracing::debug!(
            query = query,
            max_results = max_results,
            "Searching YouTube videos"
        );

        let url = format!("ytsearch{}:{}", max_results, query);
        self.fetch_playlist(&url).await
    }

    /// Search and return first result.
    ///
    /// # Arguments
    ///
    /// * `query` - The search query
    ///
    /// # Returns
    ///
    /// First video matching the search
    ///
    /// # Errors
    ///
    /// Returns error if no results found
    pub async fn search_first(&self, query: &str) -> Result<Video> {
        tracing::debug!(query = query, "Searching for first YouTube video result");

        let url = format!("ytsearch1:{}", query);
        let mut args = self.build_base_args();
        args.push(url);

        self.execute_for_video(&args).await
    }

    // ========== Internal Helper Methods ==========

    fn build_base_args(&self) -> Vec<String> {
        let mut args = vec!["--no-progress".to_string(), "--dump-json".to_string()];

        // Player client
        if let Some(client) = self.player_client {
            args.push("--extractor-args".to_string());
            args.push(format!("youtube:player_client={}", client.as_arg()));
        }

        // Skip DASH
        if self.skip_dash {
            args.push("--extractor-args".to_string());
            args.push("youtube:skip=dash".to_string());
        }

        // Format preset
        if let Some(preset) = &self.format_preset {
            args.push("-f".to_string());
            args.push(preset.to_format_selector());
        }

        // Custom args
        args.extend(self.args.clone());

        args
    }

    async fn execute_for_video(&self, args: &[String]) -> Result<Video> {
        super::execute_and_parse_video(self.executable_path.clone(), args, self.timeout).await
    }

    async fn execute_for_playlist(&self, args: &[String]) -> Result<Playlist> {
        super::execute_and_parse_playlist(self.executable_path.clone(), args, self.timeout).await
    }

    /// Check if URL is supported by YouTube extractor.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to check
    ///
    /// # Returns
    ///
    /// true if the URL is a YouTube URL, false otherwise
    pub fn supports_url(url: &str) -> bool {
        let url_lower = url.to_lowercase();
        url_lower.contains("youtube.com")
            || url_lower.contains("youtu.be")
            || url_lower.contains("youtube-nocookie.com")
            || url_lower.starts_with("ytsearch")
            || url_lower.starts_with("ytplaylist")
    }
}

#[async_trait]
impl VideoExtractor for Youtube {
    async fn fetch_video(&self, url: &str) -> Result<Video> {
        tracing::debug!(
            url = %url,
            player_client = ?self.player_client,
            skip_dash = self.skip_dash,
            format_preset = ?self.format_preset,
            "Fetching video with Youtube extractor"
        );

        let mut args = self.build_base_args();
        args.push(url.to_string());

        let result = self.execute_for_video(&args).await;

        match &result {
            Ok(video) => tracing::debug!(
                url = %url,
                video_id = %video.id,
                title = %video.title,
                format_count = video.formats.len(),
                "Video fetched successfully with Youtube extractor"
            ),
            Err(e) => tracing::warn!(
                url = %url,
                error = %e,
                "Failed to fetch video with Youtube extractor"
            ),
        }

        result
    }

    async fn fetch_playlist(&self, url: &str) -> Result<Playlist> {
        tracing::debug!(
            url = %url,
            player_client = ?self.player_client,
            "Fetching playlist with Youtube extractor"
        );

        let mut args = self.build_base_args();
        args.push("--flat-playlist".to_string());
        args.push(url.to_string());

        let result = self.execute_for_playlist(&args).await;

        match &result {
            Ok(playlist) => tracing::debug!(
                url = %url,
                playlist_id = %playlist.id,
                title = %playlist.title,
                entry_count = playlist.entries.len(),
                "Playlist fetched successfully with Youtube extractor"
            ),
            Err(e) => tracing::warn!(
                url = %url,
                error = %e,
                "Failed to fetch playlist with Youtube extractor"
            ),
        }

        result
    }

    fn name(&self) -> crate::extractor::ExtractorName {
        crate::extractor::ExtractorName::Youtube
    }

    fn supports_url(&self, url: &str) -> bool {
        Self::supports_url(url)
    }
}
