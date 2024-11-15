//! The models used to represent the data fetched by 'yt-dlp'.
//!
//! The represented data is the video information, thumbnails, automatic captions, and formats.

use crate::model::caption::AutomaticCaption;
use crate::model::format::Format;
use crate::model::thumbnail::Thumbnail;
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod caption;
pub mod format;
pub mod thumbnail;
pub mod utils;

/// Represents a YouTube video, the output of 'yt-dlp'.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Video {
    /// The ID of the video.
    pub id: String,
    /// The title of the video.
    pub title: String,
    /// The thumbnail URL of the video, usually the highest quality.
    pub thumbnail: String,
    /// The description of the video.
    pub description: String,
    /// If the video is public, unlisted, or private.
    pub availability: String,
    /// The upload date of the video.
    #[serde(rename = "timestamp")]
    pub upload_date: i64,

    /// The number of views the video has.
    pub view_count: i64,
    /// The number of likes the video has. None, when the author has hidden it.
    pub like_count: Option<i64>,
    /// The number of comments the video has. None, when the author has disabled comments.
    pub comment_count: Option<i64>,

    /// The channel display name.
    pub channel: String,
    /// The channel ID, not the @username.
    pub channel_id: String,
    /// The URL of the channel.
    pub channel_url: String,
    /// The number of subscribers the channel has.
    pub channel_follower_count: Option<i64>,

    /// The available formats of the video.
    pub formats: Vec<Format>,
    /// The thumbnails of the video.
    pub thumbnails: Vec<Thumbnail>,
    /// The automatic captions of the video.
    pub automatic_captions: HashMap<String, Vec<AutomaticCaption>>,

    /// The tags of the video.
    pub tags: Vec<String>,
    /// The categories of the video.
    pub categories: Vec<String>,

    /// If the video is age restricted, the age limit is different from 0.
    pub age_limit: i64,
    /// If the video is available in the country.
    #[serde(rename = "_has_drm")]
    pub has_drm: Option<bool>,
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
    pub release_git_head: String,
    /// The repository of the 'yt-dlp' version used, e.g. 'yt-dlp/yt-dlp'.
    pub repository: String,
}

impl Video {
    /// Returns the best format available.
    /// Formats sorting : "quality", "video resolution", "fps", "video bitrate"
    /// If the video has no formats video formats, it returns None.
    pub fn best_video_format(&self) -> Option<&Format> {
        let video_formats = self.formats.iter().filter(|format| format.is_video());

        video_formats.max_by(|a, b| self.compare_video_formats(a, b))
    }

    /// Returns the best audio format available.
    /// Formats sorting : "quality", "audio bitrate", "sample rate", "audio channels"
    /// If the video has no formats audio formats, it returns None.
    pub fn best_audio_format(&self) -> Option<&Format> {
        let audio_formats = self.formats.iter().filter(|format| format.is_audio());

        audio_formats.max_by(|a, b| self.compare_audio_formats(a, b))
    }

    /// Returns the worst video format available.
    /// Formats sorting : "quality", "video resolution", "fps", "video bitrate"
    /// If the video has no formats video formats, it returns None.
    pub fn worst_video_format(&self) -> Option<&Format> {
        let video_formats = self.formats.iter().filter(|format| format.is_video());

        video_formats.min_by(|a, b| self.compare_video_formats(a, b))
    }

    /// Returns the worst audio format available.
    /// Formats sorting : "quality", "audio bitrate", "sample rate", "audio channels"
    /// If the video has no formats audio formats, it returns None.
    pub fn worst_audio_format(&self) -> Option<&Format> {
        let audio_formats = self.formats.iter().filter(|format| format.is_audio());

        audio_formats.min_by(|a, b| self.compare_audio_formats(a, b))
    }

    /// Compares two video formats.
    /// Formats sorting : "quality", "video resolution", "fps", "video bitrate"
    pub fn compare_video_formats(&self, a: &Format, b: &Format) -> std::cmp::Ordering {
        let a_quality = a.quality_info.quality.unwrap_or(0.0);
        let b_quality = b.quality_info.quality.unwrap_or(0.0);

        let cmp_quality = OrderedFloat(a_quality).cmp(&OrderedFloat(b_quality));
        if cmp_quality != std::cmp::Ordering::Equal {
            return cmp_quality;
        }

        let a_height = a.video_resolution.height.unwrap_or(0);
        let b_height = b.video_resolution.height.unwrap_or(0);

        let cmp_height = a_height.cmp(&b_height);
        if cmp_height != std::cmp::Ordering::Equal {
            return cmp_height;
        }

        let a_fps = a.video_resolution.fps.unwrap_or(0.0);
        let b_fps = b.video_resolution.fps.unwrap_or(0.0);

        let cmp_fps = OrderedFloat(a_fps).cmp(&OrderedFloat(b_fps));
        if cmp_fps != std::cmp::Ordering::Equal {
            return cmp_fps;
        }

        let a_vbr = a.rates_info.video_rate.unwrap_or(0.0);
        let b_vbr = b.rates_info.video_rate.unwrap_or(0.0);

        OrderedFloat(a_vbr).cmp(&OrderedFloat(b_vbr))
    }

    /// Compares two audio formats.
    /// Formats sorting : "quality", "audio bitrate", "sample rate", "audio channels"
    pub fn compare_audio_formats(&self, a: &Format, b: &Format) -> std::cmp::Ordering {
        let a_quality = a.quality_info.quality.unwrap_or(0.0);
        let b_quality = b.quality_info.quality.unwrap_or(0.0);

        let cmp_quality = OrderedFloat(a_quality).cmp(&OrderedFloat(b_quality));
        if cmp_quality != std::cmp::Ordering::Equal {
            return cmp_quality;
        }

        let a_abr = a.rates_info.audio_rate.unwrap_or(0.0);
        let b_abr = b.rates_info.audio_rate.unwrap_or(0.0);

        let cmp_abr = OrderedFloat(a_abr).cmp(&OrderedFloat(b_abr));
        if cmp_abr != std::cmp::Ordering::Equal {
            return cmp_abr;
        }

        let a_asr = a.codec_info.asr.unwrap_or(0);
        let b_asr = b.codec_info.asr.unwrap_or(0);

        let cmp_asr = a_asr.cmp(&b_asr);
        if cmp_asr != std::cmp::Ordering::Equal {
            return cmp_asr;
        }

        let a_channels = a.codec_info.audio_channels.unwrap_or(0);
        let b_channels = b.codec_info.audio_channels.unwrap_or(0);

        a_channels.cmp(&b_channels)
    }
}
