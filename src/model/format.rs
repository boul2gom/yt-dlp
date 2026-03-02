//! Formats-related models.

use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

use ordered_float::OrderedFloat;
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

use crate::model::DrmStatus;
use crate::model::utils::serde::json_none;

/// Represents an available format of a video.
/// It can be audio, video, both of them, a manifest, or a storyboard.
///
/// A manifest is a file that contains metadata about the video streams, and how to assemble them.
/// A storyboard is a file that contains grid of images from the video, allowing users to preview the video.
/// Usually, these formats are not meant to be downloaded, but to be used by the player.
/// So, in most cases, you can ignore them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Format {
    /// The display name of the format, e.g. '303 - 1920x1080 (1080p60)'.
    pub format: String,
    /// The format ID, e.g. '303'.
    pub format_id: String,
    /// The format note, e.g. '1080p60'.
    pub format_note: Option<String>,

    /// The type of the format.
    #[serde(default)]
    pub protocol: Protocol,
    /// The language of the format.
    pub language: Option<String>,

    /// If the format has DRM.
    pub has_drm: Option<DrmStatus>,
    /// The extension of the file containing the format.
    #[serde(default)]
    pub container: Option<Container>,

    /// The Unix timestamp when this format's stream URL became available (yt-dlp fetch time).
    /// Used to detect CDN URL expiry: YouTube URLs typically last ~6 hours after this timestamp.
    pub available_at: Option<i64>,
    /// yt-dlp internal language preference score for this format.
    pub language_preference: Option<i64>,
    /// yt-dlp internal source preference score for this format.
    pub source_preference: Option<i64>,

    /// All the codec-related information.
    #[serde(flatten)]
    pub codec_info: CodecInfo,
    /// All the video resolution-related information.
    #[serde(flatten)]
    pub video_resolution: VideoResolution,
    /// All the download-related information.
    #[serde(flatten)]
    pub download_info: DownloadInfo,
    /// All the quality-related information.
    #[serde(flatten)]
    pub quality_info: QualityInfo,
    /// All the file-related information.
    #[serde(flatten)]
    pub file_info: FileInfo,
    /// All the storyboard-related information.
    #[serde(flatten)]
    pub storyboard_info: StoryboardInfo,
    /// All the rates-related information.
    #[serde(flatten)]
    pub rates_info: RatesInfo,

    /// The ID of the video this format belongs to.
    /// This field is not part of the yt-dlp output, but is added by the library
    /// to associate formats with their videos for caching purposes.
    #[serde(skip)]
    pub video_id: Option<String>,
}

impl Format {
    /// Checks if the format is a video format.
    pub fn is_video(&self) -> bool {
        let format_type = self.format_type();

        format_type.is_video()
    }

    /// Checks if the format is an audio format.
    pub fn is_audio(&self) -> bool {
        let format_type = self.format_type();

        format_type.is_audio()
    }

    /// Gets the type of the format.
    /// It can be audio, video, both of them, a manifest, or a storyboard.
    ///
    /// # Returns
    ///
    /// The [`FormatType`] determined from the codec and manifest information.
    pub fn format_type(&self) -> FormatType {
        if self.download_info.manifest_url.is_some() {
            return FormatType::Manifest;
        }

        if self.storyboard_info.fragments.is_some() {
            return FormatType::Storyboard;
        }

        let audio = self.codec_info.audio_codec.is_some();
        let video = self.codec_info.video_codec.is_some();

        match (audio, video) {
            (true, true) => FormatType::AudioVideo,
            (true, false) => FormatType::Audio,
            (false, true) => FormatType::Video,
            _ => FormatType::Unknown,
        }
    }

    /// Returns the decrypted URL for this format.
    ///
    /// # Errors
    ///
    /// Returns [`Error::FormatNoUrl`](crate::error::Error::FormatNoUrl) if the format has no URL.
    ///
    /// # Returns
    ///
    /// A reference to the format URL string.
    pub fn url(&self) -> Result<&String, crate::error::Error> {
        self.download_info
            .url
            .as_ref()
            .ok_or_else(|| crate::error::Error::FormatNoUrl {
                video_id: self.video_id.clone().unwrap_or_else(|| "unknown".to_string()),
                format_id: self.format_id.clone(),
            })
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Format(id={}, format={:?})", self.format_id, self.format)
    }
}

/// Represents the codec information of a format.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CodecInfo {
    /// The name of the audio codec, e.g. 'opus' or 'mp4a.xx' (where 'xx' is the codec version).
    #[serde(default)]
    #[serde(rename = "acodec")]
    #[serde(deserialize_with = "json_none")]
    pub audio_codec: Option<String>,
    /// The name of the video codec, e.g. 'vp9' or 'avc1.xx' (where 'xx' is the codec version).
    #[serde(default)]
    #[serde(rename = "vcodec")]
    #[serde(deserialize_with = "json_none")]
    pub video_codec: Option<String>,
    /// The extension of the audio file.
    #[serde(default)]
    pub audio_ext: Extension,
    /// The extension of the video file.
    #[serde(default)]
    pub video_ext: Extension,
    /// The number of audio channels.
    pub audio_channels: Option<i64>,
    /// The audio sample rate.
    pub asr: Option<i64>,
}

impl fmt::Display for CodecInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CodecInfo(audio={}, video={})",
            self.audio_codec.as_deref().unwrap_or("none"),
            self.video_codec.as_deref().unwrap_or("none")
        )
    }
}

/// Represents the video resolution information of a format.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoResolution {
    /// The width of the video.
    pub width: Option<u32>,
    /// The height of the video.
    pub height: Option<u32>,
    /// The combined resolution of the video, e.g. '1920x1080' or 'audio only'.
    pub resolution: Option<String>,
    /// The frames per second of the video, e.g. '24' or '25'.
    pub fps: Option<OrderedFloat<f64>>,
    /// The aspect ratio of the video, e.g. '1.77' or '1.78' (corresponding to 16:9).
    pub aspect_ratio: Option<OrderedFloat<f64>>,
}

impl fmt::Display for VideoResolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.width, self.height) {
            (Some(w), Some(h)) => write!(f, "VideoResolution(width={}, height={})", w, h),
            _ => write!(f, "VideoResolution(unknown)"),
        }
    }
}

/// Represents the download information of a format.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DownloadInfo {
    /// The decrypted URL of the format.
    pub url: Option<String>,
    /// The extension of the format.
    #[serde(default)]
    pub ext: Extension,
    /// The HTTP headers used by the downloader.
    pub http_headers: HttpHeaders,
    /// The manifest URL, if the format is a manifest.
    pub manifest_url: Option<String>,
    /// The options used by the downloader.
    pub downloader_options: Option<DownloaderOptions>,
}

impl fmt::Display for DownloadInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DownloadInfo(url={})", self.url.as_deref().unwrap_or("none"))
    }
}

/// Represents the quality information of a format.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QualityInfo {
    /// A relative quality score, e.g. '-1' (for example, if the format is a manifest) or '9.5'.
    pub quality: Option<OrderedFloat<f64>>,
    /// If the format is using a large dynamic range.
    #[serde(default)]
    pub dynamic_range: Option<DynamicRange>,
}

impl fmt::Display for QualityInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "QualityInfo(quality={})",
            self.quality
                .map(|q| q.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        )
    }
}

/// Represents the file information of a format.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileInfo {
    /// The approximate file size of the format.
    pub filesize_approx: Option<i64>,
    /// The exact file size of the format.
    pub filesize: Option<i64>,
}

impl fmt::Display for FileInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(size) = self.filesize {
            write!(f, "FileInfo(size={})", size)
        } else if let Some(approx) = self.filesize_approx {
            write!(f, "FileInfo(approx_size={})", approx)
        } else {
            write!(f, "FileInfo(size=unknown)")
        }
    }
}

/// Represents the rates information of a format.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RatesInfo {
    /// The video bitrate of the format.
    #[serde(rename = "vbr")]
    pub video_rate: Option<OrderedFloat<f64>>,
    /// The audio bitrate of the format.
    #[serde(rename = "abr")]
    pub audio_rate: Option<OrderedFloat<f64>>,
    /// The total bitrate (video + audio) of the format.
    #[serde(rename = "tbr")]
    pub total_rate: Option<OrderedFloat<f64>>,
}

impl fmt::Display for RatesInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RatesInfo(video={}, audio={}, total={})",
            self.video_rate
                .map(|r| r.to_string())
                .unwrap_or_else(|| "none".to_string()),
            self.audio_rate
                .map(|r| r.to_string())
                .unwrap_or_else(|| "none".to_string()),
            self.total_rate
                .map(|r| r.to_string())
                .unwrap_or_else(|| "none".to_string())
        )
    }
}

/// Represents the storyboard information of a format.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StoryboardInfo {
    /// The number of rows in the storyboard.
    pub rows: Option<i64>,
    /// The number of columns in the storyboard.
    pub columns: Option<i64>,
    /// The fragments of the storyboard.
    pub fragments: Option<Vec<Fragment>>,
}

impl fmt::Display for StoryboardInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.rows, self.columns) {
            (Some(r), Some(c)) => write!(f, "StoryboardInfo(rows={}, columns={})", r, c),
            _ => write!(f, "StoryboardInfo(unknown)"),
        }
    }
}

/// Represents a fragment of a storyboard.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fragment {
    /// The URL of the fragment.
    pub url: String,
    /// The duration of the fragment, in seconds.
    pub duration: OrderedFloat<f64>,
}

impl fmt::Display for Fragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fragment(url={}, duration={})", self.url, self.duration)
    }
}

/// Represents the options used by the downloader.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DownloaderOptions {
    /// The size of the HTTP chunk.
    pub http_chunk_size: i64,
}

impl fmt::Display for DownloaderOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DownloaderOptions(chunk_size={})", self.http_chunk_size)
    }
}

/// Represents the HTTP headers used by the downloader.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct HttpHeaders {
    /// The user agent used by the downloader.
    #[serde(rename = "User-Agent", default)]
    pub user_agent: String,
    /// The accept header used by the downloader.
    #[serde(default)]
    pub accept: String,
    /// The accept language used by the downloader.
    #[serde(rename = "Accept-Language", default)]
    pub accept_language: String,
    /// The accept encoding used by the downloader.
    #[serde(rename = "Sec-Fetch-Mode", default)]
    pub sec_fetch_mode: String,
}

impl HttpHeaders {
    /// Creates default browser-like headers for the given user agent.
    ///
    /// # Arguments
    ///
    /// * `user_agent` - The user agent string to use.
    ///
    /// # Returns
    ///
    /// An `HttpHeaders` with sensible browser defaults.
    pub fn browser_defaults(user_agent: String) -> Self {
        Self {
            user_agent,
            accept: "*/*".to_string(),
            accept_language: "en-US,en".to_string(),
            sec_fetch_mode: "navigate".to_string(),
        }
    }

    /// Converts these headers into a `reqwest::header::HeaderMap`.
    ///
    /// # Returns
    ///
    /// A `HeaderMap` with User-Agent, Accept, Accept-Language, and Sec-Fetch-Mode set.
    pub fn to_header_map(&self) -> reqwest::header::HeaderMap {
        let mut map = HeaderMap::new();
        if let Ok(hv) = HeaderValue::from_str(&self.user_agent) {
            map.insert(header::USER_AGENT, hv);
        }
        if let Ok(hv) = HeaderValue::from_str(&self.accept) {
            map.insert(header::ACCEPT, hv);
        }
        if let Ok(hv) = HeaderValue::from_str(&self.accept_language) {
            map.insert(header::ACCEPT_LANGUAGE, hv);
        }
        if let Ok(hv) = HeaderValue::from_bytes(self.sec_fetch_mode.as_bytes()) {
            map.insert("Sec-Fetch-Mode", hv);
        }
        map
    }
}

impl fmt::Display for HttpHeaders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HttpHeaders(user_agent={})", self.user_agent)
    }
}

/// The available extensions of a format.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Extension {
    /// The M4A extension.
    #[serde(rename = "m4a")]
    // Override: rename_all would produce "m4_a" (broken, digit-uppercase boundary)
    M4A,
    /// The MP3 extension.
    Mp3,
    /// The MP4 extension.
    Mp4,
    /// The Webm extension.
    Webm,
    /// The FLAC extension.
    Flac,
    /// The OGG extension (Vorbis/Opus).
    Ogg,
    /// The WAV extension.
    Wav,
    /// The AAC extension.
    Aac,
    /// The AIFF extension.
    Aiff,
    /// The AVI extension.
    Avi,
    /// The MPEG-TS extension.
    Ts,
    /// The FLV extension.
    Flv,

    /// The MHTML extension.
    Mhtml,

    /// If there is no extension.
    None,
    /// An unknown extension.
    #[default]
    #[serde(other)]
    Unknown,
}

impl Extension {
    /// Returns the lowercase file extension string for this variant.
    /// Unknown/None variants return `"bin"` as a safe fallback.
    ///
    /// # Returns
    ///
    /// A static string slice with the file extension (e.g. `"mp4"`, `"webm"`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Extension::M4A => "m4a",
            Extension::Mp3 => "mp3",
            Extension::Mp4 => "mp4",
            Extension::Webm => "webm",
            Extension::Flac => "flac",
            Extension::Ogg => "ogg",
            Extension::Wav => "wav",
            Extension::Aac => "aac",
            Extension::Aiff => "aiff",
            Extension::Avi => "avi",
            Extension::Ts => "ts",
            Extension::Flv => "flv",
            Extension::Mhtml => "mhtml",
            Extension::None | Extension::Unknown => "bin",
        }
    }
}

impl fmt::Display for Extension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Extension::M4A => f.write_str("M4A"),
            Extension::Mp3 => f.write_str("Mp3"),
            Extension::Mp4 => f.write_str("Mp4"),
            Extension::Webm => f.write_str("Webm"),
            Extension::Flac => f.write_str("Flac"),
            Extension::Ogg => f.write_str("Ogg"),
            Extension::Wav => f.write_str("Wav"),
            Extension::Aac => f.write_str("Aac"),
            Extension::Aiff => f.write_str("Aiff"),
            Extension::Avi => f.write_str("Avi"),
            Extension::Ts => f.write_str("Ts"),
            Extension::Flv => f.write_str("Flv"),
            Extension::Mhtml => f.write_str("Mhtml"),
            Extension::None => f.write_str("None"),
            Extension::Unknown => f.write_str("Unknown"),
        }
    }
}

impl FromStr for Extension {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "m4a" => Ok(Extension::M4A),
            "mp3" => Ok(Extension::Mp3),
            "mp4" => Ok(Extension::Mp4),
            "webm" => Ok(Extension::Webm),
            "flac" => Ok(Extension::Flac),
            "ogg" | "oga" | "opus" => Ok(Extension::Ogg),
            "wav" => Ok(Extension::Wav),
            "aac" => Ok(Extension::Aac),
            "aiff" | "aif" => Ok(Extension::Aiff),
            "avi" => Ok(Extension::Avi),
            "ts" | "m2ts" | "mts" => Ok(Extension::Ts),
            "flv" => Ok(Extension::Flv),
            "mhtml" => Ok(Extension::Mhtml),
            "" | "none" => Ok(Extension::None),
            _ => Ok(Extension::Unknown),
        }
    }
}

/// The available containers extensions of a format.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Container {
    /// The Webm container.
    #[serde(rename = "webm_dash")]
    Webm,
    /// The M4A container.
    #[serde(rename = "m4a_dash")]
    M4A,
    /// The MP4 container.
    #[serde(rename = "mp4_dash")]
    Mp4,

    /// An unknown container.
    #[default]
    #[serde(other)]
    Unknown,
}

impl fmt::Display for Container {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Container::Mp4 => f.write_str("Mp4"),
            Container::Webm => f.write_str("Webm"),
            Container::M4A => f.write_str("M4A"),
            Container::Unknown => f.write_str("Unknown"),
        }
    }
}

/// The available protocols of a format.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    /// The HTTP protocol, used for audio and video formats.
    Https,
    /// The M3U8 protocol, used for manifest formats.
    #[serde(rename = "m3u8_native")]
    M3U8Native,
    /// The MHTML protocol, used for storyboard formats.
    Mhtml,

    /// An unknown protocol.
    #[default]
    #[serde(other)]
    Unknown,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::Https => f.write_str("Https"),
            Protocol::M3U8Native => f.write_str("HLS"),
            Protocol::Mhtml => f.write_str("Mhtml"),
            Protocol::Unknown => f.write_str("Unknown"),
        }
    }
}

/// The available dynamic ranges of a format.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DynamicRange {
    /// The SDR dynamic range.
    SDR,
    /// The HDR dynamic range.
    HDR,

    /// An unknown dynamic range.
    #[default]
    #[serde(other)]
    Unknown,
}

impl fmt::Display for DynamicRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DynamicRange::SDR => f.write_str("SDR"),
            DynamicRange::HDR => f.write_str("HDR"),
            DynamicRange::Unknown => f.write_str("Unknown"),
        }
    }
}

/// The available format types.
/// It can be audio, video, both of them, a manifest, or a storyboard.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FormatType {
    /// The format contains only audio.
    Audio,
    /// The format contains only video.
    Video,
    /// The format contains both audio and video.
    AudioVideo,
    /// The format is a manifest.
    Manifest,
    /// The format is a storyboard.
    Storyboard,

    /// An unknown format type.
    #[default]
    #[serde(other)]
    Unknown,
}

impl FormatType {
    /// Checks if the format is an audio and video format.
    ///
    /// # Returns
    ///
    /// `true` if the format type is [`FormatType::AudioVideo`].
    pub fn is_audio_and_video(&self) -> bool {
        matches!(self, FormatType::AudioVideo)
    }

    /// Checks if the format is a video format.
    ///
    /// # Returns
    ///
    /// `true` if the format type is [`FormatType::Video`].
    pub fn is_video(&self) -> bool {
        matches!(self, FormatType::Video)
    }

    /// Checks if the format is an audio format.
    ///
    /// # Returns
    ///
    /// `true` if the format type is [`FormatType::Audio`].
    pub fn is_audio(&self) -> bool {
        matches!(self, FormatType::Audio)
    }

    /// Checks if the format is a storyboard format.
    ///
    /// # Returns
    ///
    /// `true` if the format type is [`FormatType::Storyboard`].
    pub fn is_storyboard(&self) -> bool {
        matches!(self, FormatType::Storyboard)
    }

    /// Checks if the format is a manifest format.
    pub fn is_manifest(&self) -> bool {
        matches!(self, FormatType::Manifest)
    }
}

impl fmt::Display for FormatType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatType::Audio => f.write_str("Audio"),
            FormatType::Video => f.write_str("Video"),
            FormatType::AudioVideo => f.write_str("AudioVideo"),
            FormatType::Manifest => f.write_str("Manifest"),
            FormatType::Storyboard => f.write_str("Storyboard"),
            FormatType::Unknown => f.write_str("Unknown"),
        }
    }
}
