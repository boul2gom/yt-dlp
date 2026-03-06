//! Video model and related operations.
//!
//! This module contains the Video struct and all its implementations,
//! including format selection and comparison logic.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_with::{DefaultOnNull, serde_as};

use crate::model::caption::{AutomaticCaption, Subtitle};
use crate::model::chapter::Chapter;
#[cfg(any(feature = "live-recording", feature = "live-streaming"))]
use crate::model::format::Protocol;
use crate::model::format::{Format, FormatType};
use crate::model::heatmap::Heatmap;
use crate::model::thumbnail::Thumbnail;

/// CDN lifetime for YouTube format stream URLs after their `available_at` timestamp.
/// YouTube URLs typically expire approximately 6 hours after being fetched.
pub const FORMAT_URL_LIFETIME: i64 = 6 * 3600;

// Import DrmStatus from parent module
use super::DrmStatus;

/// Represents a YouTube video, the output of 'yt-dlp'.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Video {
    /// The ID of the video.
    pub id: String,
    /// The title of the video.
    pub title: String,
    /// The thumbnail URL of the video, usually the highest quality.
    pub thumbnail: Option<String>,
    /// The description of the video.
    pub description: Option<String>,
    /// If the video is public, unlisted, or private.
    pub availability: Option<String>,
    /// The upload date of the video.
    #[serde(rename = "timestamp")]
    pub upload_date: Option<i64>,
    /// The duration of the video in seconds.
    pub duration: Option<i64>,
    /// The duration of the video as a human-readable string, e.g. '41:21'.
    pub duration_string: Option<String>,
    /// The canonical webpage URL of the video.
    pub webpage_url: Option<String>,
    /// The primary language of the video, e.g. 'fr' or 'en'.
    pub language: Option<String>,
    /// The type of media: 'video', 'short', 'podcast', etc.
    pub media_type: Option<String>,
    /// Whether the video is currently a live stream.
    pub is_live: Option<bool>,
    /// Whether the video was originally a live stream.
    pub was_live: Option<bool>,
    /// Unix timestamp of a scheduled premiere or live start time.
    pub release_timestamp: Option<i64>,
    /// Release year, if different from the upload year.
    pub release_year: Option<i64>,
    #[cfg(any(feature = "live-recording", feature = "live-streaming"))]
    /// The number of concurrent viewers (live streams only).
    pub concurrent_view_count: Option<i64>,

    /// The number of views the video has.
    pub view_count: Option<i64>,
    /// The number of likes the video has. None, when the author has hidden it.
    pub like_count: Option<i64>,
    /// The number of comments the video has. None, when the author has disabled comments.
    pub comment_count: Option<i64>,

    /// The channel display name.
    pub channel: Option<String>,
    /// The channel ID, not the @username.
    pub channel_id: Option<String>,
    /// The URL of the channel.
    pub channel_url: Option<String>,
    /// The number of subscribers the channel has.
    pub channel_follower_count: Option<i64>,

    /// The uploader name (often legacy or same as channel).
    pub uploader: Option<String>,
    /// The uploader ID.
    pub uploader_id: Option<String>,
    /// The URL of the uploader's profile page.
    pub uploader_url: Option<String>,
    /// Whether the channel has a verified badge.
    pub channel_is_verified: Option<bool>,

    /// The available formats of the video.
    #[serde(default)]
    pub formats: Vec<Format>,
    /// The thumbnails of the video.
    #[serde(default)]
    pub thumbnails: Vec<Thumbnail>,
    /// The automatic captions of the video.
    #[serde(default)]
    pub automatic_captions: HashMap<String, Vec<AutomaticCaption>>,
    /// The subtitles of the video (user-uploaded and automatic).
    #[serde(default)]
    pub subtitles: HashMap<String, Vec<Subtitle>>,
    /// The chapters of the video.
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub chapters: Vec<Chapter>,
    /// The heatmap data for the video (most replayed segments).
    #[serde(default)]
    pub heatmap: Option<Heatmap>,

    /// The tags of the video.
    #[serde(default)]
    pub tags: Vec<String>,
    /// The categories of the video.
    #[serde(default)]
    pub categories: Vec<String>,

    /// If the video is age restricted, the age limit is different from 0.
    pub age_limit: i64,
    /// If the video is available in the country.
    #[serde(rename = "_has_drm")]
    pub has_drm: Option<DrmStatus>,
    /// If the video was a live stream.
    pub live_status: String,
    /// If the video is playable in an embed.
    pub playable_in_embed: bool,

    /// The extractor information.
    #[serde(flatten)]
    pub extractor_info: ExtractorInfo,
    /// The version of 'yt-dlp' used to fetch the video.
    #[serde(rename = "_version")]
    pub version: Version,
}

/// Represents the extractor information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractorInfo {
    /// The id of the extractor.
    pub extractor: String,
    /// The name of the extractor.
    pub extractor_key: String,
}

/// Represents the version of 'yt-dlp' used to fetch the video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Version {
    /// The version of 'yt-dlp', e.g. '2024.10.22'.
    pub version: String,
    /// The commit hash of the current 'yt-dlp' version, if not a release.
    pub current_git_head: Option<String>,
    /// The commit hash of the release 'yt-dlp' version.
    pub release_git_head: Option<String>,
    /// The repository of the 'yt-dlp' version used, e.g. 'yt-dlp/yt-dlp'.
    pub repository: String,
}

impl Video {
    /// Returns the chapters of the video.
    ///
    /// # Returns
    ///
    /// A slice containing all chapters in the video
    pub fn get_chapters(&self) -> &[Chapter] {
        &self.chapters
    }

    /// Finds the chapter at a specific timestamp.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The timestamp in seconds
    ///
    /// # Returns
    ///
    /// The chapter containing the timestamp, or None if no chapter matches
    pub fn get_chapter_at_time(&self, timestamp: f64) -> Option<&Chapter> {
        self.get_chapters()
            .iter()
            .find(|chapter| chapter.contains_timestamp(timestamp))
    }

    /// Checks if the video has chapters.
    ///
    /// # Returns
    ///
    /// true if the video has at least one chapter, false otherwise
    pub fn has_chapters(&self) -> bool {
        !self.get_chapters().is_empty()
    }

    /// Returns the heatmap data for the video if available.
    ///
    /// # Returns
    ///
    /// A reference to the heatmap, or None if no heatmap data is available
    pub fn get_heatmap(&self) -> Option<&Heatmap> {
        self.heatmap.as_ref()
    }

    /// Checks if the video has heatmap data.
    ///
    /// # Returns
    ///
    /// true if the video has heatmap data, false otherwise
    pub fn has_heatmap(&self) -> bool {
        self.heatmap.is_some()
    }

    /// Returns the earliest `available_at` timestamp across all downloadable formats.
    ///
    /// Excludes storyboard and manifest formats since their URLs do not have CDN expiry.
    ///
    /// # Returns
    ///
    /// The minimum `available_at` Unix timestamp, or `None` if no format carries this field.
    pub fn formats_available_at(&self) -> Option<i64> {
        self.formats
            .iter()
            .filter(|f| !matches!(f.format_type(), FormatType::Storyboard | FormatType::Manifest))
            .filter_map(|f| f.available_at)
            .min()
    }

    /// Returns true if the format stream URLs are still within their CDN lifetime.
    ///
    /// YouTube CDN URLs expire approximately [`FORMAT_URL_LIFETIME`] seconds after the
    /// `available_at` timestamp. Returns `true` when no `available_at` data is present
    /// (falls back to the fixed TTL configured on the cache).
    ///
    /// # Returns
    ///
    /// `true` if format URLs are fresh or if expiry data is unavailable.
    pub fn are_format_urls_fresh(&self) -> bool {
        let Some(available_at) = self.formats_available_at() else {
            return true;
        };
        let now = crate::utils::current_timestamp();
        now < available_at + FORMAT_URL_LIFETIME
    }

    /// Returns the best thumbnail by resolution (width × height), breaking ties by preference.
    ///
    /// Falls back to the highest-preference thumbnail if none have resolution metadata.
    ///
    /// # Returns
    ///
    /// A reference to the best `Thumbnail`, or `None` if the list is empty.
    pub fn best_thumbnail(&self) -> Option<&Thumbnail> {
        self.thumbnails
            .iter()
            .filter(|t| t.width.is_some() && t.height.is_some())
            .max_by_key(|t| (t.width.unwrap_or(0) * t.height.unwrap_or(0), t.preference))
            .or_else(|| self.thumbnails.iter().max_by_key(|t| t.preference))
    }

    /// Returns the worst thumbnail by resolution (width × height), breaking ties by preference.
    ///
    /// Falls back to the lowest-preference thumbnail if none have resolution metadata.
    ///
    /// # Returns
    ///
    /// A reference to the worst `Thumbnail`, or `None` if the list is empty.
    pub fn worst_thumbnail(&self) -> Option<&Thumbnail> {
        self.thumbnails
            .iter()
            .filter(|t| t.width.is_some() && t.height.is_some())
            .min_by_key(|t| (t.width.unwrap_or(0) * t.height.unwrap_or(0), t.preference))
            .or_else(|| self.thumbnails.iter().min_by_key(|t| t.preference))
    }

    /// Returns the smallest thumbnail that meets the given minimum dimensions.
    ///
    /// Useful when you need at least a certain resolution without over-fetching.
    ///
    /// # Arguments
    ///
    /// * `min_width` - Minimum width in pixels.
    /// * `min_height` - Minimum height in pixels.
    ///
    /// # Returns
    ///
    /// The smallest `Thumbnail` satisfying the constraints, or `None` if none qualify.
    pub fn thumbnail_for_size(&self, min_width: u32, min_height: u32) -> Option<&Thumbnail> {
        self.thumbnails
            .iter()
            .filter(|t| {
                t.width.is_some_and(|w| w >= min_width as i64) && t.height.is_some_and(|h| h >= min_height as i64)
            })
            .min_by_key(|t| t.width.unwrap_or(0) * t.height.unwrap_or(0))
    }

    /// Returns the best storyboard format (most fragments, then highest resolution).
    ///
    /// Storyboard formats are grids of video preview images embedded in MHTML fragments.
    /// The best storyboard has the most fragments (temporal coverage) and the largest
    /// per-frame resolution as a tiebreaker.
    ///
    /// # Returns
    ///
    /// A reference to the best storyboard `Format`, or `None` if no storyboard is available.
    pub fn best_storyboard_format(&self) -> Option<&Format> {
        self.formats
            .iter()
            .filter(|f| f.format_type() == FormatType::Storyboard)
            .max_by(|a, b| {
                let a_frags = a.storyboard_info.fragments.as_ref().map_or(0, Vec::len);
                let b_frags = b.storyboard_info.fragments.as_ref().map_or(0, Vec::len);
                let a_area =
                    a.video_resolution.width.unwrap_or(0) as u64 * a.video_resolution.height.unwrap_or(0) as u64;
                let b_area =
                    b.video_resolution.width.unwrap_or(0) as u64 * b.video_resolution.height.unwrap_or(0) as u64;
                a_frags.cmp(&b_frags).then_with(|| a_area.cmp(&b_area))
            })
    }

    /// Returns the worst storyboard format (fewest fragments, then lowest resolution).
    ///
    /// # Returns
    ///
    /// A reference to the worst storyboard `Format`, or `None` if no storyboard is available.
    pub fn worst_storyboard_format(&self) -> Option<&Format> {
        self.formats
            .iter()
            .filter(|f| f.format_type() == FormatType::Storyboard)
            .min_by(|a, b| {
                let a_frags = a.storyboard_info.fragments.as_ref().map_or(0, Vec::len);
                let b_frags = b.storyboard_info.fragments.as_ref().map_or(0, Vec::len);
                let a_area =
                    a.video_resolution.width.unwrap_or(0) as u64 * a.video_resolution.height.unwrap_or(0) as u64;
                let b_area =
                    b.video_resolution.width.unwrap_or(0) as u64 * b.video_resolution.height.unwrap_or(0) as u64;
                a_frags.cmp(&b_frags).then_with(|| a_area.cmp(&b_area))
            })
    }

    /// Returns the best format that contains both audio and video.
    ///
    /// # Returns
    ///
    /// The best combined `Format`, or an error if none are available.
    pub fn best_audio_video_format(&self) -> Result<&Format, crate::error::Error> {
        self.formats
            .iter()
            .find(|f| f.format_type().is_audio_and_video())
            .ok_or_else(|| crate::error::Error::FormatNotAvailable {
                video_id: self.id.clone(),
                format_type: FormatType::AudioVideo,
                available_formats: self.formats.iter().map(|f| f.format_id.clone()).collect(),
            })
    }

    /// Returns whether the video is currently a live stream.
    ///
    /// # Returns
    ///
    /// `true` if the video is currently being broadcast live.
    #[cfg(any(feature = "live-recording", feature = "live-streaming"))]
    pub fn is_currently_live(&self) -> bool {
        const STATUS: &str = "is_live";

        self.is_live == Some(true) || self.live_status == STATUS
    }

    /// Returns whether the video is an upcoming/scheduled stream.
    ///
    /// # Returns
    ///
    /// `true` if the video is scheduled but has not started yet.
    #[cfg(any(feature = "live-recording", feature = "live-streaming"))]
    pub fn is_upcoming(&self) -> bool {
        const STATUS: &str = "is_upcoming";

        self.live_status == STATUS
    }

    /// Returns all formats using the HLS (m3u8) protocol.
    ///
    /// Live streams exclusively use HLS formats. Each format is a pre-muxed
    /// audio+video stream at a specific quality level.
    ///
    /// # Returns
    ///
    /// A vector of references to HLS formats, sorted by total bitrate (ascending).
    #[cfg(any(feature = "live-recording", feature = "live-streaming"))]
    pub fn live_formats(&self) -> Vec<&Format> {
        let mut formats: Vec<&Format> = self
            .formats
            .iter()
            .filter(|f| f.protocol == Protocol::M3U8Native)
            .collect();
        formats.sort_by(|a, b| {
            a.rates_info
                .total_rate
                .partial_cmp(&b.rates_info.total_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        formats
    }
}

// Implementation of the Display trait for Video
impl fmt::Display for Video {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Video(id={}, title={:?}, channel={:?}, formats={})",
            self.id,
            self.title,
            self.channel.as_deref().unwrap_or("Unknown"),
            self.formats.len()
        )
    }
}

// Implementation of the Display trait for ExtractorInfo
impl fmt::Display for ExtractorInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtractorInfo(extractor={}, key={})",
            self.extractor, self.extractor_key
        )
    }
}

// Implementation of the Display trait for Version
impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Version(version={}, repository={})", self.version, self.repository)
    }
}

// Implementation of Eq for structures that support it
impl Eq for Video {}
impl Eq for Version {}
impl Eq for ExtractorInfo {}

// Implementation of Hash for structures that support it
impl std::hash::Hash for Video {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.title.hash(state);
        self.channel.hash(state);
        self.channel_id.hash(state);
    }
}

impl std::hash::Hash for Version {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.version.hash(state);
        self.repository.hash(state);
    }
}

impl std::hash::Hash for ExtractorInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.extractor.hash(state);
        self.extractor_key.hash(state);
    }
}
