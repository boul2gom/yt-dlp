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

/// Helper function to check if a video codec matches the preference
pub fn matches_video_codec(codec: &str, preference: &VideoCodecPreference) -> bool {
    let codec_lower = codec.to_lowercase();
    match preference {
        VideoCodecPreference::VP9 => codec_lower.contains("vp9"),
        VideoCodecPreference::AVC1 => {
            codec_lower.contains("avc1")
                || codec_lower.contains("h264")
                || codec_lower.contains("h.264")
        }
        VideoCodecPreference::AV1 => codec_lower.contains("av1") || codec_lower.contains("av01"),
        VideoCodecPreference::Custom(custom) => codec_lower.contains(&custom.to_lowercase()),
        VideoCodecPreference::Any => true,
    }
}

/// Helper function to check if an audio codec matches the preference
pub fn matches_audio_codec(codec: &str, preference: &AudioCodecPreference) -> bool {
    let codec_lower = codec.to_lowercase();
    match preference {
        AudioCodecPreference::Opus => codec_lower.contains("opus"),
        AudioCodecPreference::AAC => codec_lower.contains("aac") || codec_lower.contains("mp4a"),
        AudioCodecPreference::MP3 => codec_lower.contains("mp3"),
        AudioCodecPreference::Custom(custom) => codec_lower.contains(&custom.to_lowercase()),
        AudioCodecPreference::Any => true,
    }
}
