//! Live stream recording and streaming module.
//!
//! Provides two recording engines for HLS live streams:
//! - **Reqwest** (primary): Pure-Rust segment fetcher with zero-copy writes.
//! - **FFmpeg** (fallback): Delegates to an FFmpeg process with `-c copy`.
//!
//! Recording is controlled via a [`CancellationToken`](tokio_util::sync::CancellationToken)
//! and optionally bounded by a maximum duration. Events are emitted through
//! the crate's event bus for progress tracking.

#[cfg(any(feature = "live-recording", feature = "live-streaming"))]
mod core;
#[cfg(any(feature = "live-recording", feature = "live-streaming"))]
pub mod hls;
#[cfg(feature = "live-recording")]
pub mod recording;
#[cfg(feature = "live-streaming")]
pub mod streaming;

#[cfg(feature = "live-streaming")]
pub use core::LiveFragment;
use std::fmt;
#[cfg(feature = "live-recording")]
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[cfg(any(feature = "live-recording", feature = "live-streaming"))]
pub use hls::{HlsPlaylist, HlsSegment, HlsVariant};
#[cfg(feature = "live-recording")]
pub use recording::FfmpegLiveRecorder;
#[cfg(feature = "live-recording")]
pub use recording::LiveRecorder;
#[cfg(feature = "live-streaming")]
pub use streaming::{LiveFragmentStream, LiveFragmentStreamer};
use tokio_util::sync::CancellationToken;

use crate::Downloader;
use crate::error::{Error, Result};
#[cfg(feature = "live-recording")]
use crate::events::types::RecordingMethod;
use crate::model::Video;
use crate::model::format::{Format, Protocol};

/// Common configuration shared across live recording engines.
#[cfg(feature = "live-recording")]
pub struct RecordingConfig {
    /// The HLS stream URL to record.
    pub stream_url: String,
    /// The output file path.
    pub output_path: PathBuf,
    /// The video ID (for event emission).
    pub video_id: String,
    /// Quality label (e.g. "1080p").
    pub quality: String,
    /// Optional maximum recording duration.
    pub max_duration: Option<Duration>,
    /// Cancellation token for graceful stop.
    pub cancellation_token: CancellationToken,
    /// The event bus for emitting recording events.
    pub event_bus: crate::events::EventBus,
}

/// Common configuration shared across live fragment streaming.
#[cfg(feature = "live-streaming")]
pub struct LiveStreamConfig {
    /// The HLS stream URL to stream live fragments from.
    pub stream_url: String,
    /// The video ID (for event emission).
    pub video_id: String,
    /// Quality label (e.g. "1080p").
    pub quality: String,
    /// Optional maximum streaming duration for this live fragment session.
    pub max_duration: Option<Duration>,
    /// Cancellation token for graceful stop.
    pub cancellation_token: CancellationToken,
    /// The event bus for emitting streaming events.
    pub event_bus: crate::events::EventBus,
}

/// The result of a live recording session.
#[cfg(feature = "live-recording")]
#[derive(Debug, Clone)]
pub struct RecordingResult {
    /// The path to the recorded file.
    pub output_path: PathBuf,
    /// Total bytes written.
    pub total_bytes: u64,
    /// Total recording duration.
    pub total_duration: Duration,
    /// Number of HLS segments downloaded (0 for FFmpeg engine).
    pub segments_downloaded: u64,
}

#[cfg(feature = "live-recording")]
impl fmt::Display for RecordingResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RecordingResult(output={}, bytes={}, duration={:.1}s, segments={})",
            self.output_path.display(),
            self.total_bytes,
            self.total_duration.as_secs_f64(),
            self.segments_downloaded
        )
    }
}

/// Fluent builder for configuring and starting a live recording.
///
/// Created via [`Downloader::record_live`]. Allows configuring the recording method,
/// format selection, maximum duration, and cancellation token before starting.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::Downloader;
/// # use yt_dlp::client::deps::Libraries;
/// # use std::path::PathBuf;
/// # use std::time::Duration;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
/// # let downloader = Downloader::builder(libraries, "output").build().await?;
/// let video = downloader.fetch_video_infos("https://youtube.com/watch?v=LIVE_ID").await?;
///
/// let result = downloader.record_live(&video, "live-recording.ts")
///     .with_max_duration(Duration::from_secs(3600))
///     .execute()
///     .await?;
///
/// println!("Recorded {} bytes", result.total_bytes);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "live-recording")]
pub struct LiveRecordingBuilder<'a> {
    downloader: &'a Downloader,
    video: &'a Video,
    output_path: PathBuf,
    method: RecordingMethod,
    max_duration: Option<Duration>,
    format: Option<&'a Format>,
    cancellation_token: Option<CancellationToken>,
}

#[cfg(feature = "live-recording")]
impl<'a> LiveRecordingBuilder<'a> {
    /// Creates a new live recording builder.
    ///
    /// # Arguments
    ///
    /// * `downloader` - Reference to the downloader.
    /// * `video` - The video metadata (must be a live stream).
    /// * `output_path` - Where to write the recorded stream.
    pub(crate) fn new(downloader: &'a Downloader, video: &'a Video, output_path: impl Into<PathBuf>) -> Self {
        Self {
            downloader,
            video,
            output_path: output_path.into(),
            method: RecordingMethod::Native,
            max_duration: None,
            format: None,
            cancellation_token: None,
        }
    }

    /// Sets the recording method.
    ///
    /// # Arguments
    ///
    /// * `method` - [`RecordingMethod::Native`] (default) or [`RecordingMethod::Fallback`].
    pub fn with_method(mut self, method: RecordingMethod) -> Self {
        self.method = method;
        self
    }

    /// Sets the maximum recording duration.
    ///
    /// # Arguments
    ///
    /// * `duration` - Maximum time to record before automatically stopping.
    pub fn with_max_duration(mut self, duration: Duration) -> Self {
        self.max_duration = Some(duration);
        self
    }

    /// Selects a specific HLS format for recording.
    ///
    /// If not set, the best quality live format is automatically selected.
    ///
    /// # Arguments
    ///
    /// * `format` - The HLS format to record.
    pub fn with_format(mut self, format: &'a Format) -> Self {
        self.format = Some(format);
        self
    }

    /// Sets a custom cancellation token.
    ///
    /// If not set, the downloader's cancellation token is used.
    ///
    /// # Arguments
    ///
    /// * `token` - The cancellation token to control recording lifecycle.
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = Some(token);
        self
    }

    /// Starts a live recording and writes it to a file.
    ///
    /// # Errors
    ///
    /// Returns an error if the video is not a live stream, no HLS format is available,
    /// or the recording engine encounters an error.
    ///
    /// # Returns
    ///
    /// A [`RecordingResult`] containing recording statistics.
    pub async fn execute(self) -> Result<RecordingResult> {
        let resolved = resolve_live_format(self.video, self.format, LiveMode::Recording)?;
        let cancellation_token = self
            .cancellation_token
            .unwrap_or_else(|| self.downloader.cancellation_token.child_token());

        tracing::info!(
            video_id = self.video.id,
            method = ?self.method,
            quality = resolved.quality,
            output = ?self.output_path,
            "📥 Starting live recording"
        );

        match self.method {
            RecordingMethod::Native => {
                let client = Arc::new(
                    reqwest::Client::builder()
                        .tcp_nodelay(true)
                        .build()
                        .map_err(|e| Error::http(&resolved.stream_url, "building HTTP client", e))?,
                );

                let config = RecordingConfig {
                    stream_url: resolved.stream_url,
                    output_path: self.output_path,
                    video_id: self.video.id.clone(),
                    quality: resolved.quality,
                    max_duration: self.max_duration,
                    cancellation_token,
                    event_bus: self.downloader.event_bus.clone(),
                };

                let recorder = LiveRecorder::new(config, client);
                recorder.record().await
            }
            RecordingMethod::Fallback => {
                let config = RecordingConfig {
                    stream_url: resolved.stream_url,
                    output_path: self.output_path,
                    video_id: self.video.id.clone(),
                    quality: resolved.quality,
                    max_duration: self.max_duration,
                    cancellation_token,
                    event_bus: self.downloader.event_bus.clone(),
                };

                let recorder = FfmpegLiveRecorder::new(config, &self.downloader.libraries.ffmpeg);
                recorder.record().await
            }
        }
    }
}

/// Fluent builder for configuring and starting a live fragment stream.
///
/// Created via [`Downloader::stream_live`]. Allows configuring format selection,
/// maximum duration, and cancellation token before starting.
#[cfg(feature = "live-streaming")]
pub struct LiveStreamBuilder<'a> {
    downloader: &'a Downloader,
    video: &'a Video,
    max_duration: Option<Duration>,
    format: Option<&'a Format>,
    cancellation_token: Option<CancellationToken>,
}

#[cfg(feature = "live-streaming")]
impl fmt::Debug for LiveStreamBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LiveStreamBuilder")
            .field("video_id", &self.video.id)
            .field("max_duration", &self.max_duration)
            .field("has_format", &self.format.is_some())
            .field("has_token", &self.cancellation_token.is_some())
            .finish()
    }
}

#[cfg(feature = "live-streaming")]
impl<'a> LiveStreamBuilder<'a> {
    /// Creates a new live stream builder.
    ///
    /// # Arguments
    ///
    /// * `downloader` - Reference to the downloader.
    /// * `video` - The video metadata (must be a live stream).
    pub(crate) fn new(downloader: &'a Downloader, video: &'a Video) -> Self {
        Self {
            downloader,
            video,
            max_duration: None,
            format: None,
            cancellation_token: None,
        }
    }

    /// Sets the maximum streaming duration.
    ///
    /// # Arguments
    ///
    /// * `duration` - Maximum time to stream before automatically stopping.
    pub fn with_max_duration(mut self, duration: Duration) -> Self {
        self.max_duration = Some(duration);
        self
    }

    /// Selects a specific HLS format for streaming.
    ///
    /// If not set, the best quality live format is automatically selected.
    ///
    /// # Arguments
    ///
    /// * `format` - The HLS format to stream.
    pub fn with_format(mut self, format: &'a Format) -> Self {
        self.format = Some(format);
        self
    }

    /// Sets a custom cancellation token.
    ///
    /// If not set, the downloader's cancellation token is used.
    ///
    /// # Arguments
    ///
    /// * `token` - The cancellation token to control streaming lifecycle.
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = Some(token);
        self
    }

    /// Starts streaming live fragments.
    ///
    /// # Errors
    ///
    /// Returns an error if the video is not a live stream, no HLS format is available,
    /// or the streaming engine encounters an error.
    ///
    /// # Returns
    ///
    /// A [`LiveFragmentStream`] that yields HLS fragments as they are downloaded.
    pub async fn execute(self) -> Result<LiveFragmentStream> {
        let resolved = resolve_live_format(self.video, self.format, LiveMode::Streaming)?;
        let cancellation_token = self
            .cancellation_token
            .unwrap_or_else(|| self.downloader.cancellation_token.child_token());

        tracing::info!(
            video_id = self.video.id,
            quality = resolved.quality,
            "📥 Starting live fragment stream"
        );

        let client = Arc::new(
            reqwest::Client::builder()
                .tcp_nodelay(true)
                .build()
                .map_err(|e| Error::http(&resolved.stream_url, "building HTTP client", e))?,
        );

        let config = LiveStreamConfig {
            stream_url: resolved.stream_url,
            video_id: self.video.id.clone(),
            quality: resolved.quality,
            max_duration: self.max_duration,
            cancellation_token,
            event_bus: self.downloader.event_bus.clone(),
        };

        let streamer = LiveFragmentStreamer::new(config, client);
        streamer.stream().await
    }
}

#[cfg(any(feature = "live-recording", feature = "live-streaming"))]
#[derive(Debug, Clone)]
struct ResolvedLiveFormat {
    stream_url: String,
    quality: String,
}

#[cfg(any(feature = "live-recording", feature = "live-streaming"))]
#[derive(Debug, Clone, Copy)]
enum LiveMode {
    #[cfg(feature = "live-recording")]
    Recording,
    #[cfg(feature = "live-streaming")]
    Streaming,
}

#[cfg(any(feature = "live-recording", feature = "live-streaming"))]
fn resolve_live_format(video: &Video, format: Option<&Format>, mode: LiveMode) -> Result<ResolvedLiveFormat> {
    if !video.is_currently_live() {
        return Err(Error::live_unavailable(
            video.webpage_url.as_deref().unwrap_or("unknown"),
            &video.live_status,
            "video is not currently live",
        ));
    }

    let live_formats = video.live_formats();
    let format = match format {
        Some(f) => {
            if f.protocol != Protocol::M3U8Native {
                return Err(live_format_error(video, mode, "format is not an HLS manifest"));
            }
            f
        }
        None => live_formats
            .last()
            .ok_or_else(|| live_format_error(video, mode, "no HLS formats available"))?,
    };

    let stream_url = format.url()?.clone();
    let quality = format
        .video_resolution
        .height
        .map(|h| format!("{h}p"))
        .unwrap_or_else(|| "unknown".to_string());

    Ok(ResolvedLiveFormat { stream_url, quality })
}

#[cfg(any(feature = "live-recording", feature = "live-streaming"))]
fn live_format_error(video: &Video, mode: LiveMode, reason: &str) -> Error {
    let url = video.webpage_url.as_deref().unwrap_or("unknown");

    match mode {
        #[cfg(feature = "live-recording")]
        LiveMode::Recording => Error::live_recording(url, reason),
        #[cfg(feature = "live-streaming")]
        LiveMode::Streaming => Error::live_streaming(url, reason),
    }
}

#[cfg(feature = "live-recording")]
impl fmt::Debug for LiveRecordingBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LiveRecordingBuilder")
            .field("video_id", &self.video.id)
            .field("output_path", &self.output_path)
            .field("method", &self.method)
            .field("max_duration", &self.max_duration)
            .field("has_format", &self.format.is_some())
            .field("has_token", &self.cancellation_token.is_some())
            .finish()
    }
}
