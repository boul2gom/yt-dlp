//! Video model and related operations.
//!
//! This module contains the Video struct and all its implementations,
//! including format selection and comparison logic.

use crate::model::caption::{AutomaticCaption, Subtitle};
use crate::model::chapter::Chapter;
use crate::model::format::Format;
use crate::model::heatmap::Heatmap;

use crate::model::thumbnail::Thumbnail;
use serde::{Deserialize, Serialize};
use serde_with::{DefaultOnNull, serde_as};

use std::collections::HashMap;
use std::fmt;

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

    /// The available formats of the video.
    pub formats: Vec<Format>,
    /// The thumbnails of the video.
    pub thumbnails: Vec<Thumbnail>,
    /// The automatic captions of the video.
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
    pub tags: Vec<String>,
    /// The categories of the video.
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
}

// Implementation of the Display trait for Video
impl fmt::Display for Video {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Video(id = {}, title = \"{}\", channel = \"{}\", formats = {})",
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
            "ExtractorInfo(extractor = {}, key = {})",
            self.extractor, self.extractor_key
        )
    }
}

// Implementation of the Display trait for Version
impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Version(version = {}, repository = {})",
            self.version, self.repository
        )
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
