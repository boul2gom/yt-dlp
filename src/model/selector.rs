//! Format selector enumerations for audio and video formats.

use serde::{Deserialize, Serialize};

/// Represents video quality preferences for format selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoQuality {
    /// Best available video quality (highest resolution, fps, and bitrate)
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

/// Represents audio quality preferences for format selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioQuality {
    /// Best available audio quality (highest bitrate and sample rate)
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

/// Represents codec preferences for video format selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    Any,
}

/// Represents codec preferences for audio format selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    Any,
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

/// Helper function to check if a video codec matches the preference
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

/// Helper function to check if an audio codec matches the preference
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
