//! Post-processing configuration for video and audio processing.
//!
//! This module provides comprehensive post-processing options using FFmpeg,
//! including codec conversion, bitrate adjustment, video filters, and more.

use std::fmt;

/// Video codec options for encoding
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub enum VideoCodec {
    /// H.264/AVC codec (libx264)
    H264,
    /// H.265/HEVC codec (libx265)
    H265,
    /// VP9 codec (libvpx-vp9)
    VP9,
    /// AV1 codec (libaom-av1)
    AV1,
    /// Copy video stream without re-encoding
    #[default]
    Copy,
}

impl VideoCodec {
    /// Converts to FFmpeg codec name
    ///
    /// # Returns
    ///
    /// The FFmpeg codec name string
    pub fn to_ffmpeg_name(&self) -> &str {
        match self {
            Self::H264 => "libx264",
            Self::H265 => "libx265",
            Self::VP9 => "libvpx-vp9",
            Self::AV1 => "libaom-av1",
            Self::Copy => "copy",
        }
    }
}

impl fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::H264 => f.write_str("H264"),
            Self::H265 => f.write_str("H265"),
            Self::VP9 => f.write_str("VP9"),
            Self::AV1 => f.write_str("AV1"),
            Self::Copy => f.write_str("Copy"),
        }
    }
}

/// Audio codec options for encoding
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub enum AudioCodec {
    /// AAC codec    
    AAC,
    /// MP3 codec (libmp3lame)
    MP3,
    /// Opus codec
    Opus,
    /// Vorbis codec
    Vorbis,
    /// Copy audio stream without re-encoding
    #[default]
    Copy,
}

impl AudioCodec {
    /// Converts to FFmpeg codec name
    ///
    /// # Returns
    ///
    /// The FFmpeg codec name string
    pub fn to_ffmpeg_name(&self) -> &str {
        match self {
            Self::AAC => "aac",
            Self::MP3 => "libmp3lame",
            Self::Opus => "libopus",
            Self::Vorbis => "libvorbis",
            Self::Copy => "copy",
        }
    }
}

impl fmt::Display for AudioCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AAC => f.write_str("AAC"),
            Self::MP3 => f.write_str("MP3"),
            Self::Opus => f.write_str("Opus"),
            Self::Vorbis => f.write_str("Vorbis"),
            Self::Copy => f.write_str("Copy"),
        }
    }
}

/// Video resolution preset
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Resolution {
    /// 7680x4320 (8K)
    UHD8K,
    /// 3840x2160 (4K)
    UHD4K,
    /// 2560x1440 (2K/QHD)
    QHD,
    /// 1920x1080 (Full HD)
    FullHD,
    /// 1280x720 (HD)
    HD,
    /// 854x480 (SD)
    SD,
    /// 640x360
    Low,
    /// Custom resolution
    Custom { width: u32, height: u32 },
}

impl Resolution {
    /// Returns the width and height for this resolution
    ///
    /// # Returns
    ///
    /// A tuple (width, height) in pixels
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::UHD8K => (7680, 4320),
            Self::UHD4K => (3840, 2160),
            Self::QHD => (2560, 1440),
            Self::FullHD => (1920, 1080),
            Self::HD => (1280, 720),
            Self::SD => (854, 480),
            Self::Low => (640, 360),
            Self::Custom { width, height } => (*width, *height),
        }
    }

    /// Converts to FFmpeg scale filter format
    ///
    /// # Returns
    ///
    /// FFmpeg scale filter string (e.g., "1920:1080")
    pub fn to_ffmpeg_scale(&self) -> String {
        let (width, height) = self.dimensions();
        format!("{}:{}", width, height)
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UHD8K => f.write_str("UHD8K"),
            Self::UHD4K => f.write_str("UHD4K"),
            Self::QHD => f.write_str("QHD"),
            Self::FullHD => f.write_str("FullHD"),
            Self::HD => f.write_str("HD"),
            Self::SD => f.write_str("SD"),
            Self::Low => f.write_str("Low"),
            Self::Custom { width, height } => {
                write!(f, "Custom(width={}, height={})", width, height)
            }
        }
    }
}

/// Encoding preset for quality/speed trade-off
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub enum EncodingPreset {
    /// Ultra fast encoding (lowest quality)
    UltraFast,
    /// Super fast encoding
    SuperFast,
    /// Very fast encoding
    VeryFast,
    /// Fast encoding
    Fast,
    /// Medium encoding (balanced)
    #[default]
    Medium,
    /// Slow encoding (better quality)
    Slow,
    /// Slower encoding
    Slower,
    /// Very slow encoding (best quality)
    VerySlow,
}

impl EncodingPreset {
    /// Converts to FFmpeg preset name
    ///
    /// # Returns
    ///
    /// The FFmpeg preset name string
    pub fn to_ffmpeg_name(&self) -> &str {
        match self {
            Self::UltraFast => "ultrafast",
            Self::SuperFast => "superfast",
            Self::VeryFast => "veryfast",
            Self::Fast => "fast",
            Self::Medium => "medium",
            Self::Slow => "slow",
            Self::Slower => "slower",
            Self::VerySlow => "veryslow",
        }
    }
}

impl fmt::Display for EncodingPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UltraFast => f.write_str("UltraFast"),
            Self::SuperFast => f.write_str("SuperFast"),
            Self::VeryFast => f.write_str("VeryFast"),
            Self::Fast => f.write_str("Fast"),
            Self::Medium => f.write_str("Medium"),
            Self::Slow => f.write_str("Slow"),
            Self::Slower => f.write_str("Slower"),
            Self::VerySlow => f.write_str("VerySlow"),
        }
    }
}

/// Watermark position on the video
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum WatermarkPosition {
    /// Top left corner
    TopLeft,
    /// Top right corner
    TopRight,
    /// Bottom left corner
    BottomLeft,
    /// Bottom right corner
    BottomRight,
    /// Center
    Center,
    /// Custom position (x, y coordinates)
    Custom { x: u32, y: u32 },
}

impl WatermarkPosition {
    /// Converts to FFmpeg overlay position
    pub fn to_ffmpeg_position(&self) -> String {
        match self {
            Self::TopLeft => "x=10:y=10".to_string(),
            Self::TopRight => "x=W-w-10:y=10".to_string(),
            Self::BottomLeft => "x=10:y=H-h-10".to_string(),
            Self::BottomRight => "x=W-w-10:y=H-h-10".to_string(),
            Self::Center => "x=(W-w)/2:y=(H-h)/2".to_string(),
            Self::Custom { x, y } => format!("x={}:y={}", x, y),
        }
    }
}

impl fmt::Display for WatermarkPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TopLeft => f.write_str("TopLeft"),
            Self::TopRight => f.write_str("TopRight"),
            Self::BottomLeft => f.write_str("BottomLeft"),
            Self::BottomRight => f.write_str("BottomRight"),
            Self::Center => f.write_str("Center"),
            Self::Custom { x, y } => write!(f, "Custom(x={}, y={})", x, y),
        }
    }
}

/// Video filter options
#[derive(Clone, Debug, PartialEq)]
pub enum FfmpegFilter {
    /// Crop video to specific dimensions
    Crop { width: u32, height: u32, x: u32, y: u32 },
    /// Rotate video by degrees
    Rotate { angle: i32 },
    /// Add watermark image
    Watermark { path: String, position: WatermarkPosition },
    /// Adjust brightness (-1.0 to 1.0)
    Brightness { value: f32 },
    /// Adjust contrast (0.0 to 4.0)
    Contrast { value: f32 },
    /// Adjust saturation (0.0 to 3.0)
    Saturation { value: f32 },
    /// Apply blur effect
    Blur { radius: u32 },
    /// Flip horizontally
    FlipHorizontal,
    /// Flip vertically
    FlipVertical,
    /// Denoise video
    Denoise,
    /// Sharpen video
    Sharpen,
    /// Custom FFmpeg filter string
    Custom { filter: String },
}

impl FfmpegFilter {
    /// Converts filter to FFmpeg filter string
    ///
    /// # Returns
    ///
    /// The FFmpeg filter string
    pub fn to_ffmpeg_string(&self) -> String {
        match self {
            Self::Crop { width, height, x, y } => format!("crop={}:{}:{}:{}", width, height, x, y),
            Self::Rotate { angle } => {
                let radians = (*angle as f64) * std::f64::consts::PI / 180.0;
                format!("rotate={}:ow=rotw({}):oh=roth({})", radians, radians, radians)
            }
            Self::Watermark { path, position } => {
                format!("movie={}[wm];[in][wm]overlay={}", path, position.to_ffmpeg_position())
            }
            Self::Brightness { value } => format!("eq=brightness={}", value),
            Self::Contrast { value } => format!("eq=contrast={}", value),
            Self::Saturation { value } => format!("eq=saturation={}", value),
            Self::Blur { radius } => format!("boxblur={}:{}", radius, radius),
            Self::FlipHorizontal => "hflip".to_string(),
            Self::FlipVertical => "vflip".to_string(),
            Self::Denoise => "hqdn3d".to_string(),
            Self::Sharpen => "unsharp=5:5:1.0:5:5:0.0".to_string(),
            Self::Custom { filter } => filter.clone(),
        }
    }
}

impl fmt::Display for FfmpegFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Crop { width, height, x, y } => {
                write!(f, "Crop(width={}, height={}, x={}, y={})", width, height, x, y)
            }
            Self::Rotate { angle } => write!(f, "Rotate(angle={})", angle),
            Self::Watermark { position, .. } => write!(f, "Watermark(position={})", position),
            Self::Brightness { value } => write!(f, "Brightness(value={})", value),
            Self::Contrast { value } => write!(f, "Contrast(value={})", value),
            Self::Saturation { value } => write!(f, "Saturation(value={})", value),
            Self::Blur { radius } => write!(f, "Blur(radius={})", radius),
            Self::FlipHorizontal => f.write_str("FlipHorizontal"),
            Self::FlipVertical => f.write_str("FlipVertical"),
            Self::Denoise => f.write_str("Denoise"),
            Self::Sharpen => f.write_str("Sharpen"),
            Self::Custom { filter } => write!(f, "Custom(filter={})", filter),
        }
    }
}

/// Comprehensive post-processing configuration
#[derive(Clone, Debug, PartialEq)]
pub struct PostProcessConfig {
    /// Video codec to use for encoding
    pub video_codec: Option<VideoCodec>,
    /// Audio codec to use for encoding
    pub audio_codec: Option<AudioCodec>,
    /// Video bitrate (e.g., "2M", "5M")
    pub video_bitrate: Option<String>,
    /// Audio bitrate (e.g., "128k", "192k", "320k")
    pub audio_bitrate: Option<String>,
    /// Target resolution for scaling
    pub resolution: Option<Resolution>,
    /// Target framerate
    pub framerate: Option<u32>,
    /// Encoding preset (quality/speed trade-off)
    pub preset: Option<EncodingPreset>,
    /// Video filters to apply
    pub filters: Vec<FfmpegFilter>,
}

impl PostProcessConfig {
    /// Creates a new post-processing configuration
    ///
    /// # Returns
    ///
    /// An empty PostProcessConfig with all options set to None
    pub fn new() -> Self {
        tracing::debug!("✂️ Created new post-processing configuration");

        Self {
            video_codec: None,
            audio_codec: None,
            video_bitrate: None,
            audio_bitrate: None,
            resolution: None,
            framerate: None,
            preset: None,
            filters: Vec::new(),
        }
    }

    /// Sets the video codec
    ///
    /// # Arguments
    ///
    /// * `codec` - Video codec to use
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_video_codec(mut self, codec: VideoCodec) -> Self {
        self.video_codec = Some(codec);
        self
    }

    /// Sets the audio codec
    ///
    /// # Arguments
    ///
    /// * `codec` - Audio codec to use
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_audio_codec(mut self, codec: AudioCodec) -> Self {
        self.audio_codec = Some(codec);
        self
    }

    /// Sets the video bitrate
    pub fn with_video_bitrate(mut self, bitrate: impl Into<String>) -> Self {
        self.video_bitrate = Some(bitrate.into());
        self
    }

    /// Sets the audio bitrate
    pub fn with_audio_bitrate(mut self, bitrate: impl Into<String>) -> Self {
        self.audio_bitrate = Some(bitrate.into());
        self
    }

    /// Sets the target resolution
    pub fn with_resolution(mut self, resolution: Resolution) -> Self {
        self.resolution = Some(resolution);
        self
    }

    /// Sets the target framerate
    pub fn with_framerate(mut self, fps: u32) -> Self {
        self.framerate = Some(fps);
        self
    }

    /// Sets the encoding preset
    pub fn with_preset(mut self, preset: EncodingPreset) -> Self {
        self.preset = Some(preset);
        self
    }

    /// Adds a filter to the processing pipeline
    ///
    /// # Arguments
    ///
    /// * `filter` - FFmpeg filter to add
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn add_filter(mut self, filter: FfmpegFilter) -> Self {
        tracing::debug!(filter = ?filter, "✂️ Adding FFmpeg filter to post-processing config");

        self.filters.push(filter);
        self
    }

    /// Checks if any post-processing is configured
    ///
    /// # Returns
    ///
    /// true if no post-processing options are set, false otherwise
    pub fn is_empty(&self) -> bool {
        self.video_codec.is_none()
            && self.audio_codec.is_none()
            && self.video_bitrate.is_none()
            && self.audio_bitrate.is_none()
            && self.resolution.is_none()
            && self.framerate.is_none()
            && self.preset.is_none()
            && self.filters.is_empty()
    }
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PostProcessConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let video = self.video_codec.as_ref().map_or("None".to_string(), |c| c.to_string());
        let audio = self.audio_codec.as_ref().map_or("None".to_string(), |c| c.to_string());
        write!(
            f,
            "PostProcessConfig(video_codec={}, audio_codec={}, filters={})",
            video,
            audio,
            self.filters.len()
        )
    }
}
