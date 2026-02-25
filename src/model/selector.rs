//! Format selector enumerations for audio and video formats.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Represents video quality preferences for format selection.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VideoQuality {
    /// Best available video quality (highest resolution, fps, and bitrate)
    #[default]
    Best,
    /// High quality video (1080p or better if available)
    High,
    /// Medium quality video (720p if available)
    Medium,
    /// Low quality video (480p or lower)
    Low,
    /// Worst available video quality (lowest resolution, fps, and bitrate)
    Worst,
    /// Custom resolution with preference for specified height
    CustomHeight(u32),
    /// Custom resolution with preference for specified width
    CustomWidth(u32),
}

impl fmt::Display for VideoQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VideoQuality::Best => f.write_str("Best"),
            VideoQuality::High => f.write_str("High"),
            VideoQuality::Medium => f.write_str("Medium"),
            VideoQuality::Low => f.write_str("Low"),
            VideoQuality::Worst => f.write_str("Worst"),
            VideoQuality::CustomHeight(h) => write!(f, "CustomHeight(height={h})"),
            VideoQuality::CustomWidth(w) => write!(f, "CustomWidth(width={w})"),
        }
    }
}

/// Represents audio quality preferences for format selection.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioQuality {
    /// Best available audio quality (highest bitrate and sample rate)
    #[default]
    Best,
    /// High quality audio (192kbps or better if available)
    High,
    /// Medium quality audio (128kbps if available)
    Medium,
    /// Low quality audio (96kbps or lower)
    Low,
    /// Worst available audio quality (lowest bitrate and sample rate)
    Worst,
    /// Custom audio with preference for specified bitrate in kbps
    CustomBitrate(u32),
}

impl fmt::Display for AudioQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioQuality::Best => f.write_str("Best"),
            AudioQuality::High => f.write_str("High"),
            AudioQuality::Medium => f.write_str("Medium"),
            AudioQuality::Low => f.write_str("Low"),
            AudioQuality::Worst => f.write_str("Worst"),
            AudioQuality::CustomBitrate(b) => write!(f, "CustomBitrate(bitrate={b})"),
        }
    }
}

/// Represents codec preferences for video format selection.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VideoCodecPreference {
    /// Prefer VP9 codec
    VP9,
    /// Prefer AVC1/H.264 codec
    AVC1,
    /// Prefer AV01/AV1 codec
    AV1,
    /// Custom codec preference
    Custom(String),
    /// No specific codec preference
    #[default]
    Any,
}

impl fmt::Display for VideoCodecPreference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VideoCodecPreference::VP9 => f.write_str("VP9"),
            VideoCodecPreference::AVC1 => f.write_str("AVC1"),
            VideoCodecPreference::AV1 => f.write_str("AV1"),
            VideoCodecPreference::Custom(c) => write!(f, "Custom(codec={c})"),
            VideoCodecPreference::Any => f.write_str("Any"),
        }
    }
}

/// Represents codec preferences for audio format selection.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioCodecPreference {
    /// Prefer Opus codec
    Opus,
    /// Prefer AAC codec
    AAC,
    /// Prefer MP3 codec
    MP3,
    /// Custom codec preference
    Custom(String),
    /// No specific codec preference
    #[default]
    Any,
}

impl fmt::Display for AudioCodecPreference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioCodecPreference::Opus => f.write_str("Opus"),
            AudioCodecPreference::AAC => f.write_str("AAC"),
            AudioCodecPreference::MP3 => f.write_str("MP3"),
            AudioCodecPreference::Custom(c) => write!(f, "Custom(codec={c})"),
            AudioCodecPreference::Any => f.write_str("Any"),
        }
    }
}

/// Represents quality preferences for storyboard format selection.
///
/// A storyboard is a grid of video preview images embedded in MHTML fragments.
/// Higher quality storyboards have more fragments and larger per-frame resolution.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StoryboardQuality {
    /// Best available storyboard (highest resolution, most fragments).
    #[default]
    Best,
    /// Worst available storyboard (lowest resolution, fewest fragments).
    Worst,
}

impl fmt::Display for StoryboardQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoryboardQuality::Best => f.write_str("Best"),
            StoryboardQuality::Worst => f.write_str("Worst"),
        }
    }
}

/// Represents quality preferences for thumbnail format selection.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThumbnailQuality {
    /// Best available thumbnail (highest resolution)
    #[default]
    Best,
    /// Minimum resolution preference (minimum width, minimum height)
    MinimumResolution(u32, u32),
    /// Worst available thumbnail (lowest resolution)
    Worst,
}

impl fmt::Display for ThumbnailQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThumbnailQuality::Best => f.write_str("Best"),
            ThumbnailQuality::MinimumResolution(w, h) => {
                write!(f, "MinimumResolution(width={w}, height={h})")
            }
            ThumbnailQuality::Worst => f.write_str("Worst"),
        }
    }
}

/// Case-insensitive substring check without allocation.
fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
}

/// Checks whether a video codec string matches the given codec preference.
///
/// The comparison is case-insensitive and checks for substring containment,
/// so `"vp9.0"` will match [`VideoCodecPreference::VP9`]. The `Any` preference
/// always returns `true`.
///
/// # Arguments
///
/// * `codec` - The codec identifier string to check (e.g. `"vp9"`, `"avc1.64001f"`).
/// * `preference` - The desired codec preference to match against.
///
/// # Returns
///
/// `true` if the codec matches the preference, or if the preference is `Any`.
pub fn matches_video_codec(codec: &str, preference: &VideoCodecPreference) -> bool {
    match preference {
        VideoCodecPreference::VP9 => contains_ignore_ascii_case(codec, "vp9"),
        VideoCodecPreference::AVC1 => {
            contains_ignore_ascii_case(codec, "avc1")
                || contains_ignore_ascii_case(codec, "h264")
                || contains_ignore_ascii_case(codec, "h.264")
        }
        VideoCodecPreference::AV1 => {
            contains_ignore_ascii_case(codec, "av1") || contains_ignore_ascii_case(codec, "av01")
        }
        VideoCodecPreference::Custom(custom) => contains_ignore_ascii_case(codec, custom),
        VideoCodecPreference::Any => true,
    }
}

/// Checks whether an audio codec string matches the given codec preference.
///
/// The comparison is case-insensitive and checks for substring containment,
/// so `"mp4a.40.2"` will match [`AudioCodecPreference::AAC`]. The `Any` preference
/// always returns `true`.
///
/// # Arguments
///
/// * `codec` - The codec identifier string to check (e.g. `"opus"`, `"mp4a.40.2"`).
/// * `preference` - The desired codec preference to match against.
///
/// # Returns
///
/// `true` if the codec matches the preference, or if the preference is `Any`.
pub fn matches_audio_codec(codec: &str, preference: &AudioCodecPreference) -> bool {
    match preference {
        AudioCodecPreference::Opus => contains_ignore_ascii_case(codec, "opus"),
        AudioCodecPreference::AAC => {
            contains_ignore_ascii_case(codec, "aac") || contains_ignore_ascii_case(codec, "mp4a")
        }
        AudioCodecPreference::MP3 => contains_ignore_ascii_case(codec, "mp3"),
        AudioCodecPreference::Custom(custom) => contains_ignore_ascii_case(codec, custom),
        AudioCodecPreference::Any => true,
    }
}

/// Bundles video/audio quality and codec preferences for format cache lookups.
///
/// Used to pass download preferences through cache layers without repeating
/// four separate `Option` parameters everywhere.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FormatPreferences {
    /// Preferred video quality.
    pub video_quality: Option<VideoQuality>,
    /// Preferred audio quality.
    pub audio_quality: Option<AudioQuality>,
    /// Preferred video codec.
    pub video_codec: Option<VideoCodecPreference>,
    /// Preferred audio codec.
    pub audio_codec: Option<AudioCodecPreference>,
}

impl FormatPreferences {
    /// Returns `true` if at least one preference is set.
    pub fn has_any(&self) -> bool {
        self.video_quality.is_some()
            || self.audio_quality.is_some()
            || self.video_codec.is_some()
            || self.audio_codec.is_some()
    }
}

impl fmt::Display for FormatPreferences {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FormatPreferences(video_quality={}, audio_quality={}, video_codec={}, audio_codec={})",
            self.video_quality
                .as_ref()
                .map_or("none".to_string(), |q| q.to_string()),
            self.audio_quality
                .as_ref()
                .map_or("none".to_string(), |q| q.to_string()),
            self.video_codec.as_ref().map_or("none".to_string(), |c| c.to_string()),
            self.audio_codec.as_ref().map_or("none".to_string(), |c| c.to_string()),
        )
    }
}
