//! YouTube extractor with platform-specific optimizations.
//!
//! This extractor provides highly optimized YouTube downloading with:
//! - Player client selection (Android, iOS, Web, TV Embedded)
//! - Format presets for common use cases
//! - YouTube-specific shortcuts (channel, user, search)
//! - Performance optimizations

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;

use crate::error::Result;
use crate::extractor::{ExtractorBase, VideoExtractor, execute_and_parse_playlist, execute_and_parse_video};
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
#[derive(Debug, Clone)]
pub struct Youtube {
    executable_path: PathBuf,
    player_client: Option<PlayerClient>,
    skip_dash: bool,
    format_preset: Option<FormatPreset>,
    args: Vec<String>,
    timeout: Duration,
}

crate::extractor::impl_extractor_config!(Youtube);

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
            "⚙️ Creating new Youtube extractor"
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
        self.format_preset = Some(preset);
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
        tracing::debug!(channel_id = channel_id, "📡 Fetching YouTube channel by ID");

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
        tracing::debug!(handle = handle, "📡 Fetching YouTube channel by handle");

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
        tracing::debug!(username = username, "📡 Fetching YouTube user uploads");

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
    pub async fn fetch_playlist_paginated(&self, playlist_id: &str, start: usize, count: usize) -> Result<Playlist> {
        let end = start.saturating_add(count).saturating_sub(1);
        tracing::debug!(
            playlist_id = playlist_id,
            start = start,
            count = count,
            end = end,
            "📡 Fetching paginated YouTube playlist"
        );

        let mut args = self.build_base_args();
        args.push("--flat-playlist".to_string());
        args.push(format!("--playlist-start={}", start));
        args.push(format!("--playlist-end={}", end));

        let url = format!("https://www.youtube.com/playlist?list={}", playlist_id);
        args.push(url);

        execute_and_parse_playlist(self.executable_path(), &args, self.timeout()).await
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
        tracing::debug!(query = query, max_results = max_results, "📡 Searching YouTube videos");

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
        tracing::debug!(query = query, "📡 Searching for first YouTube video result");

        let url = format!("ytsearch1:{}", query);
        let mut args = self.build_base_args();
        args.push(url);

        execute_and_parse_video(self.executable_path(), &args, self.timeout()).await
    }

    /// Check if URL is supported by YouTube extractor.
    pub fn supports_url(url: &str) -> bool {
        let url_lower = url.to_lowercase();

        // Check search/playlist prefixes first
        let has_valid_prefix = ["ytsearch", "ytplaylist"]
            .iter()
            .any(|prefix| url_lower.starts_with(prefix));

        if has_valid_prefix {
            return true;
        }

        // Extract host from URL to avoid substring false positives (e.g. "notyoutube.com")
        let host = url_lower
            .split("://")
            .nth(1)
            .unwrap_or(&url_lower)
            .split('/')
            .next()
            .unwrap_or("")
            .split(':')
            .next()
            .unwrap_or("");

        ["youtube.com", "youtu.be", "youtube-nocookie.com"]
            .iter()
            .any(|domain| host == *domain || host.ends_with(&format!(".{}", domain)))
    }
}

#[async_trait]
impl ExtractorBase for Youtube {
    fn executable_path(&self) -> PathBuf {
        self.executable_path.clone()
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }

    fn build_base_args(&self) -> Vec<String> {
        let mut args = vec!["--no-progress".to_string(), "--dump-single-json".to_string()];

        // Build extractor args (must be merged into a single --extractor-args flag)
        let mut extractor_parts = Vec::new();
        if let Some(client) = self.player_client {
            extractor_parts.push(format!("player_client={}", client.as_arg()));
        }
        if self.skip_dash {
            extractor_parts.push("skip=dash".to_string());
        }
        if !extractor_parts.is_empty() {
            args.push("--extractor-args".to_string());
            args.push(format!("youtube:{}", extractor_parts.join(";")));
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
}

#[async_trait]
impl VideoExtractor for Youtube {
    async fn fetch_video(&self, url: &str) -> Result<Video> {
        tracing::debug!(
            url = url,
            player_client = ?self.player_client,
            skip_dash = self.skip_dash,
            format_preset = ?self.format_preset,
            "📡 Fetching video with Youtube extractor"
        );
        self.log_and_fetch_video(url, "Youtube").await
    }

    async fn fetch_playlist(&self, url: &str) -> Result<Playlist> {
        tracing::debug!(
            url = url,
            player_client = ?self.player_client,
            "📡 Fetching playlist with Youtube extractor"
        );
        self.log_and_fetch_playlist(url, "Youtube").await
    }

    fn name(&self) -> crate::extractor::ExtractorName {
        crate::extractor::ExtractorName::Youtube
    }

    fn supports_url(&self, url: &str) -> bool {
        Self::supports_url(url)
    }
}
