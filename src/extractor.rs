//! Extractor detection and management for different video platforms.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported video platforms/extractors.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Extractor {
    /// YouTube videos, playlists, and channels
    YouTube,
    /// Vimeo videos and channels
    Vimeo,
    /// Twitch videos, clips, and streams
    Twitch,
    /// TikTok videos and users
    TikTok,
    /// Instagram posts, stories, and reels
    Instagram,
    /// Twitter/X videos and spaces
    Twitter,
    /// Facebook videos and posts
    Facebook,
    /// Generic extractor for other supported sites
    Generic,
}

impl fmt::Display for Extractor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Extractor::YouTube => write!(f, "YouTube"),
            Extractor::Vimeo => write!(f, "Vimeo"),
            Extractor::Twitch => write!(f, "Twitch"),
            Extractor::TikTok => write!(f, "TikTok"),
            Extractor::Instagram => write!(f, "Instagram"),
            Extractor::Twitter => write!(f, "Twitter/X"),
            Extractor::Facebook => write!(f, "Facebook"),
            Extractor::Generic => write!(f, "Generic"),
        }
    }
}

/// URL pattern matcher for detecting extractors from URLs.
#[derive(Debug)]
pub struct ExtractorDetector {
    patterns: Vec<(Extractor, Regex)>,
}

impl Clone for ExtractorDetector {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl ExtractorDetector {
    /// Create a new extractor detector with predefined URL patterns.
    pub fn new() -> Self {
        let patterns = vec![
            // YouTube patterns
            (
                Extractor::YouTube,
                Regex::new(
                    r"(?i)(?:https?://)?(?:www\.)?(?:youtube\.com|youtu\.be|m\.youtube\.com)",
                )
                .unwrap(),
            ),
            // Vimeo patterns
            (
                Extractor::Vimeo,
                Regex::new(r"(?i)(?:https?://)?(?:www\.)?vimeo\.com").unwrap(),
            ),
            // Twitch patterns
            (
                Extractor::Twitch,
                Regex::new(r"(?i)(?:https?://)?(?:www\.)?(?:twitch\.tv|clips\.twitch\.tv)")
                    .unwrap(),
            ),
            // TikTok patterns
            (
                Extractor::TikTok,
                Regex::new(r"(?i)(?:https?://)?(?:www\.)?(?:tiktok\.com|vm\.tiktok\.com)").unwrap(),
            ),
            // Instagram patterns
            (
                Extractor::Instagram,
                Regex::new(r"(?i)(?:https?://)?(?:www\.)?instagram\.com").unwrap(),
            ),
            // Twitter/X patterns
            (
                Extractor::Twitter,
                Regex::new(r"(?i)(?:https?://)?(?:www\.)?(?:twitter\.com|x\.com)").unwrap(),
            ),
            // Facebook patterns
            (
                Extractor::Facebook,
                Regex::new(r"(?i)(?:https?://)?(?:www\.)?(?:facebook\.com|fb\.watch)").unwrap(),
            ),
        ];

        Self { patterns }
    }

    /// Detect the extractor for a given URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to analyze
    ///
    /// # Returns
    ///
    /// The detected extractor, or `Generic` if no specific pattern matches.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use yt_dlp::extractor::{ExtractorDetector, Extractor};
    ///
    /// let detector = ExtractorDetector::new();
    ///
    /// assert_eq!(detector.detect("https://www.youtube.com/watch?v=dQw4w9WgXcQ"), Extractor::YouTube);
    /// assert_eq!(detector.detect("https://vimeo.com/1084537"), Extractor::Vimeo);
    /// assert_eq!(detector.detect("https://www.twitch.tv/videos/123456"), Extractor::Twitch);
    /// ```
    pub fn detect(&self, url: &str) -> Extractor {
        for (extractor, pattern) in &self.patterns {
            if pattern.is_match(url) {
                return extractor.clone();
            }
        }
        Extractor::Generic
    }

    /// Check if a URL is supported by any extractor.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to check
    ///
    /// # Returns
    ///
    /// `true` if the URL matches any known pattern, `false` otherwise.
    pub fn is_supported(&self, url: &str) -> bool {
        self.detect(url) != Extractor::Generic
    }

    /// Get all supported extractors.
    pub fn supported_extractors(&self) -> Vec<Extractor> {
        vec![
            Extractor::YouTube,
            Extractor::Vimeo,
            Extractor::Twitch,
            Extractor::TikTok,
            Extractor::Instagram,
            Extractor::Twitter,
            Extractor::Facebook,
        ]
    }
}

impl Default for ExtractorDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration options specific to different extractors.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractorConfig {
    /// YouTube-specific options
    pub youtube: YouTubeConfig,
    /// Vimeo-specific options
    pub vimeo: VimeoConfig,
    /// Twitch-specific options
    pub twitch: TwitchConfig,
    /// TikTok-specific options
    pub tiktok: TikTokConfig,
    /// Instagram-specific options
    pub instagram: InstagramConfig,
    /// Twitter-specific options
    pub twitter: TwitterConfig,
    /// Facebook-specific options
    pub facebook: FacebookConfig,
}

/// YouTube-specific configuration options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct YouTubeConfig {
    /// Skip unavailable videos in playlists
    pub skip_unavailable: bool,
    /// Include live streams
    pub include_live: bool,
    /// Include premieres
    pub include_premieres: bool,
}

/// Vimeo-specific configuration options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VimeoConfig {
    /// Include password-protected videos (requires password)
    pub include_password_protected: bool,
    /// Vimeo password for protected videos
    pub password: Option<String>,
}

/// Twitch-specific configuration options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TwitchConfig {
    /// Include subscriber-only content
    pub include_subscriber_only: bool,
    /// Include chat replay
    pub include_chat: bool,
}

/// TikTok-specific configuration options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TikTokConfig {
    /// Include watermark in downloaded videos
    pub include_watermark: bool,
    /// Download music separately
    pub download_music: bool,
}

/// Instagram-specific configuration options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstagramConfig {
    /// Include stories
    pub include_stories: bool,
    /// Include highlights
    pub include_highlights: bool,
}

/// Twitter-specific configuration options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TwitterConfig {
    /// Include retweets
    pub include_retweets: bool,
    /// Include replies
    pub include_replies: bool,
}

/// Facebook-specific configuration options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FacebookConfig {
    /// Include live streams
    pub include_live: bool,
    /// Include stories
    pub include_stories: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extractor_detection() {
        let detector = ExtractorDetector::new();

        // YouTube URLs
        assert_eq!(
            detector.detect("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            Extractor::YouTube
        );
        assert_eq!(
            detector.detect("https://youtu.be/dQw4w9WgXcQ"),
            Extractor::YouTube
        );
        assert_eq!(
            detector.detect("https://m.youtube.com/watch?v=dQw4w9WgXcQ"),
            Extractor::YouTube
        );

        // Vimeo URLs
        assert_eq!(
            detector.detect("https://vimeo.com/1084537"),
            Extractor::Vimeo
        );
        assert_eq!(
            detector.detect("https://www.vimeo.com/1084537"),
            Extractor::Vimeo
        );

        // Twitch URLs
        assert_eq!(
            detector.detect("https://www.twitch.tv/videos/123456"),
            Extractor::Twitch
        );
        assert_eq!(
            detector.detect("https://clips.twitch.tv/clip-id"),
            Extractor::Twitch
        );

        // TikTok URLs
        assert_eq!(
            detector.detect("https://www.tiktok.com/@user/video/123456"),
            Extractor::TikTok
        );
        assert_eq!(
            detector.detect("https://vm.tiktok.com/shortlink"),
            Extractor::TikTok
        );

        // Instagram URLs
        assert_eq!(
            detector.detect("https://www.instagram.com/p/post-id/"),
            Extractor::Instagram
        );

        // Twitter URLs
        assert_eq!(
            detector.detect("https://twitter.com/user/status/123456"),
            Extractor::Twitter
        );
        assert_eq!(
            detector.detect("https://x.com/user/status/123456"),
            Extractor::Twitter
        );

        // Facebook URLs
        assert_eq!(
            detector.detect("https://www.facebook.com/video/123456"),
            Extractor::Facebook
        );
        assert_eq!(
            detector.detect("https://fb.watch/shortlink"),
            Extractor::Facebook
        );

        // Generic/unsupported URLs
        assert_eq!(
            detector.detect("https://example.com/video"),
            Extractor::Generic
        );
    }

    #[test]
    fn test_is_supported() {
        let detector = ExtractorDetector::new();

        assert!(detector.is_supported("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
        assert!(detector.is_supported("https://vimeo.com/1084537"));
        assert!(!detector.is_supported("https://example.com/video"));
    }
}
