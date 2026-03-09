use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use crate::download::DownloadPriority;
use crate::model::Video;
use crate::model::chapter::Chapter;
use crate::model::format::Format;
use crate::model::playlist::Playlist;

/// The method used for live recording.
#[cfg(feature = "live-recording")]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum RecordingMethod {
    /// Pure Rust recording via reqwest HLS segment fetching
    Native,
    /// FFmpeg-based recording (fallback)
    Fallback,
}

/// Represents all possible events that can occur during download operations
#[derive(Debug, Clone, serde::Serialize)]
#[allow(clippy::large_enum_variant)]
pub enum DownloadEvent {
    /// Video metadata has been fetched from the URL
    VideoFetched {
        url: String,
        video: Box<Video>,
        duration: Duration,
    },

    /// Video metadata fetch failed
    VideoFetchFailed {
        url: String,
        error: String,
        duration: Duration,
    },

    /// Download has been queued in the download manager
    DownloadQueued {
        download_id: u64,
        url: String,
        priority: DownloadPriority,
        output_path: PathBuf,
    },

    /// Download has started processing
    DownloadStarted {
        download_id: u64,
        url: String,
        total_bytes: u64,
        format_id: Option<String>,
    },

    /// Download progress update
    DownloadProgress {
        download_id: u64,
        downloaded_bytes: u64,
        total_bytes: u64,
        speed_bytes_per_sec: f64,
        eta_seconds: Option<u64>,
    },

    /// Download has been paused
    DownloadPaused { download_id: u64, reason: String },

    /// Download has been resumed
    DownloadResumed { download_id: u64 },

    /// Download completed successfully
    DownloadCompleted {
        download_id: u64,
        url: String,
        output_path: PathBuf,
        duration: Duration,
        total_bytes: u64,
    },

    /// Download failed with error
    DownloadFailed {
        download_id: u64,
        url: String,
        error: String,
        retry_count: u32,
    },

    /// Download was canceled
    DownloadCanceled { download_id: u64, reason: String },

    /// Format has been selected for download
    FormatSelected {
        video_id: String,
        format: Format,
        quality: String,
    },

    /// Metadata has been applied to a file
    MetadataApplied { path: PathBuf, metadata_type: MetadataType },

    /// Chapters have been embedded into a file
    ChaptersEmbedded { path: PathBuf, chapters: Vec<Chapter> },

    /// Post-processing has started
    PostProcessStarted {
        input_path: PathBuf,
        operation: PostProcessOperation,
    },

    /// Post-processing completed successfully
    PostProcessCompleted {
        input_path: PathBuf,
        output_path: PathBuf,
        operation: PostProcessOperation,
        duration: Duration,
    },

    /// Post-processing failed
    PostProcessFailed {
        input_path: PathBuf,
        operation: PostProcessOperation,
        error: String,
    },

    /// Playlist metadata has been fetched
    PlaylistFetched {
        url: String,
        playlist: Playlist,
        duration: Duration,
    },

    /// Playlist metadata fetch failed
    PlaylistFetchFailed {
        url: String,
        error: String,
        duration: Duration,
    },

    /// Playlist item download has started
    PlaylistItemStarted {
        playlist_id: String,
        index: usize,
        total: usize,
        video_id: String,
    },

    /// Playlist item download completed
    PlaylistItemCompleted {
        playlist_id: String,
        index: usize,
        total: usize,
        video_id: String,
        output_path: PathBuf,
    },

    /// Playlist item download failed
    PlaylistItemFailed {
        playlist_id: String,
        index: usize,
        total: usize,
        video_id: String,
        error: String,
    },

    /// Entire playlist download completed
    PlaylistCompleted {
        playlist_id: String,
        total_items: usize,
        successful: usize,
        failed: usize,
        duration: Duration,
    },

    /// Segment download started (for parallel downloads)
    SegmentStarted {
        download_id: u64,
        segment_index: usize,
        total_segments: usize,
    },

    /// Segment download completed
    SegmentCompleted {
        download_id: u64,
        segment_index: usize,
        total_segments: usize,
        bytes: u64,
    },

    /// Live recording has started
    #[cfg(feature = "live-recording")]
    LiveRecordingStarted {
        video_id: String,
        url: String,
        quality: String,
        method: RecordingMethod,
    },

    /// Live recording progress update
    #[cfg(feature = "live-recording")]
    LiveRecordingProgress {
        video_id: String,
        elapsed: Duration,
        bytes_written: u64,
        segments: u64,
        bitrate_bps: f64,
    },

    /// Live recording stopped (graceful)
    #[cfg(feature = "live-recording")]
    LiveRecordingStopped {
        video_id: String,
        reason: String,
        output_path: PathBuf,
        total_bytes: u64,
        total_duration: Duration,
    },

    /// Live recording failed
    #[cfg(feature = "live-recording")]
    LiveRecordingFailed { video_id: String, error: String },

    /// Live fragment streaming started
    #[cfg(feature = "live-streaming")]
    LiveStreamStarted {
        video_id: String,
        url: String,
        quality: String,
    },

    /// Live fragment streaming progress update
    #[cfg(feature = "live-streaming")]
    LiveStreamProgress {
        video_id: String,
        elapsed: Duration,
        bytes_received: u64,
        segments: u64,
        bitrate_bps: f64,
    },

    /// Live fragment streaming stopped
    #[cfg(feature = "live-streaming")]
    LiveStreamStopped {
        video_id: String,
        reason: String,
        total_bytes: u64,
        total_duration: Duration,
    },

    /// Live fragment streaming failed
    #[cfg(feature = "live-streaming")]
    LiveStreamFailed { video_id: String, error: String },
}

/// Types of metadata that can be applied
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, serde::Serialize)]
pub enum MetadataType {
    /// MP3 ID3 tags
    Mp3,
    /// MP4/M4A metadata
    Mp4,
    /// FFmpeg metadata
    Ffmpeg,
}

impl fmt::Display for MetadataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mp3 => f.write_str("Mp3"),
            Self::Mp4 => f.write_str("Mp4"),
            Self::Ffmpeg => f.write_str("Ffmpeg"),
        }
    }
}

/// Post-processing operations
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum PostProcessOperation {
    /// Combining audio and video streams
    CombineStreams { audio_path: PathBuf, video_path: PathBuf },
    /// Converting audio format
    ConvertAudio { target_format: String },
    /// Embedding subtitles
    EmbedSubtitles { subtitle_path: PathBuf },
    /// Embedding thumbnail
    EmbedThumbnail { thumbnail_path: PathBuf },
    /// Custom FFmpeg operation
    Custom { description: String },
    /// Splitting a video into individual chapter files
    SplitChapters { source_path: PathBuf, chapter_count: usize },
}

impl fmt::Display for PostProcessOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CombineStreams { .. } => f.write_str("CombineStreams"),
            Self::ConvertAudio { target_format } => {
                write!(f, "ConvertAudio(format={})", target_format)
            }
            Self::EmbedSubtitles { .. } => f.write_str("EmbedSubtitles"),
            Self::EmbedThumbnail { .. } => f.write_str("EmbedThumbnail"),
            Self::Custom { description } => write!(f, "Custom(description={})", description),
            Self::SplitChapters { chapter_count, .. } => write!(f, "SplitChapters(chapters={})", chapter_count),
        }
    }
}

#[cfg(feature = "live-recording")]
impl fmt::Display for RecordingMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native => f.write_str("Native"),
            Self::Fallback => f.write_str("Fallback"),
        }
    }
}

impl DownloadEvent {
    /// Returns the download ID if this event is associated with a specific download
    pub fn download_id(&self) -> Option<u64> {
        match self {
            Self::DownloadQueued { download_id, .. }
            | Self::DownloadStarted { download_id, .. }
            | Self::DownloadProgress { download_id, .. }
            | Self::DownloadPaused { download_id, .. }
            | Self::DownloadResumed { download_id, .. }
            | Self::DownloadCompleted { download_id, .. }
            | Self::DownloadFailed { download_id, .. }
            | Self::DownloadCanceled { download_id, .. }
            | Self::SegmentStarted { download_id, .. }
            | Self::SegmentCompleted { download_id, .. } => Some(*download_id),
            _ => None,
        }
    }

    /// Returns true if this is a terminal event (download completed, failed, or canceled)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::DownloadCompleted { .. } | Self::DownloadFailed { .. } | Self::DownloadCanceled { .. }
        )
    }

    /// Returns true if this is a progress event
    pub fn is_progress(&self) -> bool {
        match self {
            Self::DownloadProgress { .. } => true,
            #[cfg(feature = "live-recording")]
            Self::LiveRecordingProgress { .. } => true,
            #[cfg(feature = "live-streaming")]
            Self::LiveStreamProgress { .. } => true,
            _ => false,
        }
    }

    /// Returns a human-readable event type name
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::VideoFetched { .. } => "video_fetched",
            Self::VideoFetchFailed { .. } => "video_fetch_failed",
            Self::DownloadQueued { .. } => "download_queued",
            Self::DownloadStarted { .. } => "download_started",
            Self::DownloadProgress { .. } => "download_progress",
            Self::DownloadPaused { .. } => "download_paused",
            Self::DownloadResumed { .. } => "download_resumed",
            Self::DownloadCompleted { .. } => "download_completed",
            Self::DownloadFailed { .. } => "download_failed",
            Self::DownloadCanceled { .. } => "download_canceled",
            Self::FormatSelected { .. } => "format_selected",
            Self::MetadataApplied { .. } => "metadata_applied",
            Self::ChaptersEmbedded { .. } => "chapters_embedded",
            Self::PostProcessStarted { .. } => "post_process_started",
            Self::PostProcessCompleted { .. } => "post_process_completed",
            Self::PostProcessFailed { .. } => "post_process_failed",
            Self::PlaylistFetched { .. } => "playlist_fetched",
            Self::PlaylistFetchFailed { .. } => "playlist_fetch_failed",
            Self::PlaylistItemStarted { .. } => "playlist_item_started",
            Self::PlaylistItemCompleted { .. } => "playlist_item_completed",
            Self::PlaylistItemFailed { .. } => "playlist_item_failed",
            Self::PlaylistCompleted { .. } => "playlist_completed",
            Self::SegmentStarted { .. } => "segment_started",
            Self::SegmentCompleted { .. } => "segment_completed",
            #[cfg(feature = "live-recording")]
            Self::LiveRecordingStarted { .. } => "live_recording_started",
            #[cfg(feature = "live-recording")]
            Self::LiveRecordingProgress { .. } => "live_recording_progress",
            #[cfg(feature = "live-recording")]
            Self::LiveRecordingStopped { .. } => "live_recording_stopped",
            #[cfg(feature = "live-recording")]
            Self::LiveRecordingFailed { .. } => "live_recording_failed",
            #[cfg(feature = "live-streaming")]
            Self::LiveStreamStarted { .. } => "live_stream_started",
            #[cfg(feature = "live-streaming")]
            Self::LiveStreamProgress { .. } => "live_stream_progress",
            #[cfg(feature = "live-streaming")]
            Self::LiveStreamStopped { .. } => "live_stream_stopped",
            #[cfg(feature = "live-streaming")]
            Self::LiveStreamFailed { .. } => "live_stream_failed",
        }
    }
}

impl fmt::Display for DownloadEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DownloadProgress {
                download_id,
                downloaded_bytes,
                total_bytes,
                ..
            } => {
                write!(
                    f,
                    "DownloadProgress(id={}, downloaded={}, total={})",
                    download_id, downloaded_bytes, total_bytes
                )
            }
            _ => {
                let event_type = self.event_type();
                if let Some(id) = self.download_id() {
                    write!(f, "{}(id={})", event_type, id)
                } else {
                    f.write_str(event_type)
                }
            }
        }
    }
}
