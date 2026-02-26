//! Live stream recording module.
//!
//! Provides two recording engines for HLS live streams:
//! - **Reqwest** (primary): Pure-Rust segment fetcher with zero-copy writes.
//! - **FFmpeg** (fallback): Delegates to an FFmpeg process with `-c copy`.
//!
//! Recording is controlled via a [`CancellationToken`](tokio_util::sync::CancellationToken)
//! and optionally bounded by a maximum duration. Events are emitted through
//! the crate's event bus for progress tracking.

pub mod ffmpeg_recording;
pub mod hls;
pub mod recording;

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub use ffmpeg_recording::FfmpegLiveRecorder;
pub use hls::{HlsPlaylist, HlsSegment, HlsVariant};
pub use recording::LiveRecorder;
use tokio_util::sync::CancellationToken;

use crate::Downloader;
use crate::error::{Error, Result};
use crate::events::types::RecordingMethod;
use crate::model::Video;
use crate::model::format::Format;

/// The result of a live recording session.
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
pub struct LiveRecordingBuilder<'a> {
    downloader: &'a Downloader,
    video: &'a Video,
    output_path: PathBuf,
    method: RecordingMethod,
    max_duration: Option<Duration>,
    format: Option<&'a Format>,
    cancellation_token: Option<CancellationToken>,
}

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

    /// Starts the live recording.
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
        // Validate the video is live
        if !self.video.is_currently_live() {
            return Err(Error::live_unavailable(
                self.video.webpage_url.as_deref().unwrap_or("unknown"),
                &self.video.live_status,
                "video is not currently live",
            ));
        }

        // Select the format to record
        let live_formats = self.video.live_formats();
        let format = match self.format {
            Some(f) => f,
            None => live_formats.last().ok_or_else(|| {
                Error::live_recording(
                    self.video.webpage_url.as_deref().unwrap_or("unknown"),
                    "no HLS formats available",
                )
            })?,
        };

        let stream_url = format.url()?.clone();
        let quality = format
            .video_resolution
            .height
            .map(|h| format!("{h}p"))
            .unwrap_or_else(|| "unknown".to_string());

        let cancellation_token = self
            .cancellation_token
            .unwrap_or_else(|| self.downloader.cancellation_token.child_token());

        tracing::info!(
            video_id = self.video.id,
            method = ?self.method,
            quality = quality,
            output = ?self.output_path,
            "📥 Starting live recording"
        );

        match self.method {
            RecordingMethod::Native => {
                let client = Arc::new(
                    reqwest::Client::builder()
                        .tcp_nodelay(true)
                        .build()
                        .map_err(|e| Error::http(&stream_url, "building HTTP client", e))?,
                );

                let recorder = LiveRecorder::new(
                    stream_url,
                    self.output_path,
                    &self.video.id,
                    &quality,
                    self.max_duration,
                    cancellation_token,
                    client,
                    self.downloader.event_bus.clone(),
                );

                recorder.record().await
            }
            RecordingMethod::Fallback => {
                let recorder = FfmpegLiveRecorder::new(
                    stream_url,
                    self.output_path,
                    &self.downloader.libraries.ffmpeg,
                    &self.video.id,
                    &quality,
                    self.max_duration,
                    cancellation_token,
                    self.downloader.event_bus.clone(),
                );

                recorder.record().await
            }
        }
    }
}

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
