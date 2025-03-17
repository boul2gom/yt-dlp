//! The models used to represent the data fetched by 'yt-dlp'.
//!
//! The represented data is the video information, thumbnails, automatic captions, and formats.

use crate::model::caption::AutomaticCaption;
use crate::model::format::Format;
use crate::model::format_selector::{matches_audio_codec, matches_video_codec};
use crate::model::thumbnail::Thumbnail;
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

pub mod caption;
pub mod format;
pub mod format_selector;
pub mod thumbnail;
pub mod utils;

// Re-export traits for easier access
pub use utils::{AllTraits, CommonTraits};
// Re-export format selectors for easier access
pub use format_selector::{AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality};

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
    pub release_git_head: Option<String>,
    /// The repository of the 'yt-dlp' version used, e.g. 'yt-dlp/yt-dlp'.
    pub repository: String,
}

impl Video {
    /// Returns the best format available.
    /// Formats sorting : "quality", "video resolution", "fps", "video bitrate"
    /// If the video has no formats video formats, it returns None.
    pub fn best_video_format(&self) -> Option<&Format> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Selecting best video format for video: {}", self.id);

        self.formats
            .iter()
            .filter(|f| f.is_video())
            .max_by(|a, b| self.compare_video_formats(a, b))
    }

    /// Returns the best audio format available.
    /// Formats sorting : "quality", "audio bitrate", "sample rate", "audio channels"
    /// If the video has no formats audio formats, it returns None.
    pub fn best_audio_format(&self) -> Option<&Format> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Selecting best audio format for video: {}", self.id);

        self.formats
            .iter()
            .filter(|f| f.is_audio())
            .max_by(|a, b| self.compare_audio_formats(a, b))
    }

    /// Returns the worst video format available.
    /// Formats sorting : "quality", "video resolution", "fps", "video bitrate"
    /// If the video has no formats video formats, it returns None.
    pub fn worst_video_format(&self) -> Option<&Format> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Selecting worst video format for video: {}", self.id);

        self.formats
            .iter()
            .filter(|f| f.is_video())
            .min_by(|a, b| self.compare_video_formats(a, b))
    }

    /// Returns the worst audio format available.
    /// Formats sorting : "quality", "audio bitrate", "sample rate", "audio channels"
    /// If the video has no formats audio formats, it returns None.
    pub fn worst_audio_format(&self) -> Option<&Format> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Selecting worst audio format for video: {}", self.id);

        self.formats
            .iter()
            .filter(|f| f.is_audio())
            .min_by(|a, b| self.compare_audio_formats(a, b))
    }

    /// Compares two video formats.
    /// Formats sorting : "quality", "video resolution", "fps", "video bitrate"
    pub fn compare_video_formats(&self, a: &Format, b: &Format) -> std::cmp::Ordering {
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "Comparing video formats: {} and {}",
            a.format_id,
            b.format_id
        );

        let a_quality = a.quality_info.quality.unwrap_or(OrderedFloat(0.0));
        let b_quality = b.quality_info.quality.unwrap_or(OrderedFloat(0.0));

        let cmp_quality = a_quality.cmp(&b_quality);
        if cmp_quality != std::cmp::Ordering::Equal {
            return cmp_quality;
        }

        let a_height = a.video_resolution.height.unwrap_or(0);
        let b_height = b.video_resolution.height.unwrap_or(0);

        let cmp_height = a_height.cmp(&b_height);
        if cmp_height != std::cmp::Ordering::Equal {
            return cmp_height;
        }

        let a_fps = a.video_resolution.fps.map(|f| *f).unwrap_or(0.0);
        let b_fps = b.video_resolution.fps.map(|f| *f).unwrap_or(0.0);

        let cmp_fps = OrderedFloat(a_fps).cmp(&OrderedFloat(b_fps));
        if cmp_fps != std::cmp::Ordering::Equal {
            return cmp_fps;
        }

        let a_vbr = a.rates_info.video_rate.map(|vr| *vr).unwrap_or(0.0);
        let b_vbr = b.rates_info.video_rate.map(|vr| *vr).unwrap_or(0.0);

        OrderedFloat(a_vbr).cmp(&OrderedFloat(b_vbr))
    }

    /// Compares two audio formats.
    /// Formats sorting : "quality", "audio bitrate", "sample rate", "audio channels"
    pub fn compare_audio_formats(&self, a: &Format, b: &Format) -> std::cmp::Ordering {
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "Comparing audio formats: {} and {}",
            a.format_id,
            b.format_id
        );

        let a_quality = a.quality_info.quality.unwrap_or(OrderedFloat(0.0));
        let b_quality = b.quality_info.quality.unwrap_or(OrderedFloat(0.0));

        let cmp_quality = a_quality.cmp(&b_quality);
        if cmp_quality != std::cmp::Ordering::Equal {
            return cmp_quality;
        }

        let a_abr = a.rates_info.audio_rate.map(|ar| *ar).unwrap_or(0.0);
        let b_abr = b.rates_info.audio_rate.map(|ar| *ar).unwrap_or(0.0);

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

    /// Selects a video format based on quality preference and codec preference.
    ///
    /// # Arguments
    ///
    /// * `quality` - The desired video quality
    /// * `codec` - The preferred video codec
    ///
    /// # Returns
    ///
    /// The selected format, or None if no suitable format is found
    pub fn select_video_format(
        &self,
        quality: VideoQuality,
        codec: VideoCodecPreference,
    ) -> Option<&Format> {
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "Selecting video format with quality: {:?}, codec: {:?}",
            quality,
            codec
        );

        let video_formats: Vec<&Format> = self
            .formats
            .iter()
            .filter(|format| format.is_video())
            .collect();

        if video_formats.is_empty() {
            return None;
        }

        // Filter by codec if a specific one is requested
        let codec_filtered: Vec<&Format> = match codec {
            VideoCodecPreference::Any => video_formats,
            _ => {
                let filtered: Vec<&Format> = video_formats
                    .iter()
                    .filter(|format| {
                        if let Some(video_codec) = &format.codec_info.video_codec {
                            matches_video_codec(video_codec, &codec)
                        } else {
                            false
                        }
                    })
                    .copied()
                    .collect();

                if filtered.is_empty() {
                    video_formats
                } else {
                    filtered
                }
            }
        };

        // Select based on quality preference
        match quality {
            VideoQuality::Best => codec_filtered
                .into_iter()
                .max_by(|a, b| self.compare_video_formats(a, b)),

            VideoQuality::Worst => codec_filtered
                .into_iter()
                .min_by(|a, b| self.compare_video_formats(a, b)),

            VideoQuality::High => select_closest_video_height(codec_filtered, 1080, self),

            VideoQuality::Medium => select_closest_video_height(codec_filtered, 720, self),

            VideoQuality::Low => select_closest_video_height(codec_filtered, 480, self),

            VideoQuality::CustomHeight(height) => {
                select_closest_video_height(codec_filtered, height, self)
            }

            VideoQuality::CustomWidth(width) => {
                select_closest_video_width(codec_filtered, width, self)
            }
        }
    }

    /// Selects an audio format based on quality preference and codec preference.
    ///
    /// # Arguments
    ///
    /// * `quality` - The desired audio quality
    /// * `codec` - The preferred audio codec
    ///
    /// # Returns
    ///
    /// The selected format, or None if no suitable format is found
    pub fn select_audio_format(
        &self,
        quality: AudioQuality,
        codec: AudioCodecPreference,
    ) -> Option<&Format> {
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "Selecting audio format with quality: {:?}, codec: {:?}",
            quality,
            codec
        );

        let audio_formats: Vec<&Format> = self
            .formats
            .iter()
            .filter(|format| format.is_audio())
            .collect();

        if audio_formats.is_empty() {
            return None;
        }

        // Filter by codec if a specific one is requested
        let codec_filtered: Vec<&Format> = match codec {
            AudioCodecPreference::Any => audio_formats,
            _ => {
                let filtered: Vec<&Format> = audio_formats
                    .iter()
                    .filter(|format| {
                        if let Some(audio_codec) = &format.codec_info.audio_codec {
                            matches_audio_codec(audio_codec, &codec)
                        } else {
                            false
                        }
                    })
                    .copied()
                    .collect();

                if filtered.is_empty() {
                    audio_formats
                } else {
                    filtered
                }
            }
        };

        // Select based on quality preference
        match quality {
            AudioQuality::Best => codec_filtered
                .into_iter()
                .max_by(|a, b| self.compare_audio_formats(a, b)),

            AudioQuality::Worst => codec_filtered
                .into_iter()
                .min_by(|a, b| self.compare_audio_formats(a, b)),

            AudioQuality::High => select_closest_audio_bitrate(codec_filtered, 192, self),

            AudioQuality::Medium => select_closest_audio_bitrate(codec_filtered, 128, self),

            AudioQuality::Low => select_closest_audio_bitrate(codec_filtered, 96, self),

            AudioQuality::CustomBitrate(bitrate) => {
                select_closest_audio_bitrate(codec_filtered, bitrate, self)
            }
        }
    }
}

/// Selects the video format with the closest height to the target
fn select_closest_video_height<'a>(
    formats: Vec<&'a Format>,
    target_height: u32,
    video: &Video,
) -> Option<&'a Format> {
    #[cfg(feature = "tracing")]
    tracing::trace!(
        "Selecting video format closest to height: {}",
        target_height
    );

    if formats.is_empty() {
        return None;
    }

    // First try to find formats with height >= target
    let formats_above_target: Vec<&Format> = formats
        .iter()
        .filter(|format| {
            format
                .video_resolution
                .height
                .is_some_and(|h| h >= target_height)
        })
        .copied()
        .collect();

    if !formats_above_target.is_empty() {
        // Find the one with the closest height to target
        return formats_above_target.into_iter().min_by(|a, b| {
            let a_diff = a
                .video_resolution
                .height
                .unwrap_or(0)
                .saturating_sub(target_height);
            let b_diff = b
                .video_resolution
                .height
                .unwrap_or(0)
                .saturating_sub(target_height);

            // Compare difference then quality
            a_diff
                .cmp(&b_diff)
                .then_with(|| video.compare_video_formats(a, b))
        });
    }

    // If no format with height >= target, get the highest available
    formats.into_iter().max_by(|a, b| {
        let a_height = a.video_resolution.height.unwrap_or(0);
        let b_height = b.video_resolution.height.unwrap_or(0);

        // Compare height then quality
        a_height
            .cmp(&b_height)
            .then_with(|| video.compare_video_formats(a, b))
    })
}

/// Selects the video format with the closest width to the target
fn select_closest_video_width<'a>(
    formats: Vec<&'a Format>,
    target_width: u32,
    video: &Video,
) -> Option<&'a Format> {
    #[cfg(feature = "tracing")]
    tracing::trace!("Selecting video format closest to width: {}", target_width);

    if formats.is_empty() {
        return None;
    }

    // First try to find formats with width >= target
    let formats_above_target: Vec<&Format> = formats
        .iter()
        .filter(|format| {
            format
                .video_resolution
                .width
                .is_some_and(|w| w >= target_width)
        })
        .copied()
        .collect();

    if !formats_above_target.is_empty() {
        // Find the one with the closest width to target
        return formats_above_target.into_iter().min_by(|a, b| {
            let a_diff = a
                .video_resolution
                .width
                .unwrap_or(0)
                .saturating_sub(target_width);
            let b_diff = b
                .video_resolution
                .width
                .unwrap_or(0)
                .saturating_sub(target_width);

            // Compare difference then quality
            a_diff
                .cmp(&b_diff)
                .then_with(|| video.compare_video_formats(a, b))
        });
    }

    // If no format with width >= target, get the highest available
    formats.into_iter().max_by(|a, b| {
        let a_width = a.video_resolution.width.unwrap_or(0);
        let b_width = b.video_resolution.width.unwrap_or(0);

        // Compare width then quality
        a_width
            .cmp(&b_width)
            .then_with(|| video.compare_video_formats(a, b))
    })
}

/// Selects the audio format with the closest bitrate to the target
fn select_closest_audio_bitrate<'a>(
    formats: Vec<&'a Format>,
    target_bitrate: u32,
    video: &Video,
) -> Option<&'a Format> {
    #[cfg(feature = "tracing")]
    tracing::trace!(
        "Selecting audio format closest to bitrate: {}",
        target_bitrate
    );

    if formats.is_empty() {
        return None;
    }

    let target_float = OrderedFloat(target_bitrate as f64);

    // First try to find formats with bitrate >= target
    let formats_above_target: Vec<&Format> = formats
        .iter()
        .filter(|format| {
            format
                .rates_info
                .audio_rate
                .is_some_and(|r| r >= target_float)
        })
        .copied()
        .collect();

    if !formats_above_target.is_empty() {
        // Find the one with the closest bitrate to target
        return formats_above_target.into_iter().min_by(|a, b| {
            let a_rate = a.rates_info.audio_rate.unwrap_or(OrderedFloat(0.0));
            let b_rate = b.rates_info.audio_rate.unwrap_or(OrderedFloat(0.0));

            let a_diff = (a_rate.0 - target_bitrate as f64).abs();
            let b_diff = (b_rate.0 - target_bitrate as f64).abs();

            // Compare bitrate difference then quality
            OrderedFloat(a_diff)
                .partial_cmp(&OrderedFloat(b_diff))
                .unwrap_or(Ordering::Equal)
                .then_with(|| video.compare_audio_formats(a, b))
        });
    }

    // If no format with bitrate >= target, get the highest available
    formats.into_iter().max_by(|a, b| {
        let a_rate = a.rates_info.audio_rate.unwrap_or(OrderedFloat(0.0));
        let b_rate = b.rates_info.audio_rate.unwrap_or(OrderedFloat(0.0));

        // Compare bitrate then quality
        a_rate
            .partial_cmp(&b_rate)
            .unwrap_or(Ordering::Equal)
            .then_with(|| video.compare_audio_formats(a, b))
    })
}

// Implementation of the Display trait for Video
impl fmt::Display for Video {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Video(id = {}, title = \"{}\", channel = \"{}\", formats = {})",
            self.id,
            self.title,
            self.channel,
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
