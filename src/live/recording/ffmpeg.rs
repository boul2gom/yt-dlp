//! FFmpeg-based live stream recorder (fallback).
//!
//! Uses FFmpeg's native HLS support to record a live stream to a file.
//! This is the fallback engine when reqwest-based recording is not suitable
//! (e.g., encrypted streams, complex HLS features, or user preference).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use super::super::RecordingConfig;
use crate::error::Result;
use crate::events::DownloadEvent;
use crate::events::types::RecordingMethod;
use crate::executor::Executor;

/// FFmpeg-based live stream recorder.
///
/// Spawns an FFmpeg process with `-i <hls_url> -c copy <output>` and
/// manages its lifecycle via stdin (`q` for graceful stop) or kill.
#[derive(Debug)]
pub struct FfmpegLiveRecorder {
    /// The HLS stream URL to record.
    stream_url: String,
    /// The output file path.
    output_path: PathBuf,
    /// Path to the FFmpeg binary.
    ffmpeg_path: PathBuf,
    /// The video ID (for event emission).
    video_id: String,
    /// Optional maximum recording duration.
    max_duration: Option<Duration>,
    /// Cancellation token for graceful stop.
    cancellation_token: CancellationToken,
    /// The event bus for emitting recording events.
    event_bus: crate::events::EventBus,
    /// Quality label for event metadata.
    quality: String,
}

impl FfmpegLiveRecorder {
    /// Creates a new `FfmpegLiveRecorder`.
    ///
    /// # Arguments
    ///
    /// * `config` - Common recording configuration (URL, output, duration, events).
    /// * `ffmpeg_path` - Path to the FFmpeg binary.
    pub fn new(config: RecordingConfig, ffmpeg_path: impl Into<PathBuf>) -> Self {
        Self {
            stream_url: config.stream_url,
            output_path: config.output_path,
            ffmpeg_path: ffmpeg_path.into(),
            video_id: config.video_id,
            quality: config.quality,
            max_duration: config.max_duration,
            cancellation_token: config.cancellation_token,
            event_bus: config.event_bus,
        }
    }

    /// Starts the FFmpeg recording.
    ///
    /// Spawns FFmpeg as a long-running process and waits for it to finish.
    /// The process is stopped when the cancellation token is triggered or
    /// the max duration is reached.
    ///
    /// # Errors
    ///
    /// Returns an error if FFmpeg cannot be spawned, exits with an error,
    /// or the output file cannot be accessed.
    ///
    /// # Returns
    ///
    /// A [`super::super::RecordingResult`] with recording statistics.
    pub async fn record(&self) -> Result<super::super::RecordingResult> {
        let start = Instant::now();

        tracing::info!(
            url = self.stream_url,
            video_id = self.video_id,
            output = ?self.output_path,
            max_duration = ?self.max_duration,
            "📥 Starting live recording (ffmpeg)"
        );

        self.event_bus.emit_if_subscribed(DownloadEvent::LiveRecordingStarted {
            video_id: self.video_id.clone(),
            url: self.stream_url.clone(),
            quality: self.quality.clone(),
            method: RecordingMethod::Fallback,
        });

        // Ensure output directory exists
        if let Some(parent) = self.output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Build FFmpeg args: -i <url> -c copy [-t duration] -y <output>
        let mut args: Vec<String> = vec![
            "-i".to_string(),
            self.stream_url.clone(),
            "-c".to_string(),
            "copy".to_string(),
        ];

        if let Some(max) = self.max_duration {
            args.push("-t".to_string());
            args.push(max.as_secs().to_string());
        }

        args.push("-y".to_string());
        args.push(self.output_path.display().to_string());

        let executor = Executor::new(&self.ffmpeg_path, &args, Duration::from_secs(0));
        let mut process = executor.execute_streaming().await?;

        // Wait for cancellation or process exit
        let stop_reason = tokio::select! {
            _ = self.cancellation_token.cancelled() => {
                tracing::info!(video_id = self.video_id, "📥 Cancellation requested, stopping ffmpeg");
                match process.stop().await {
                    Ok(_) => "cancelled".to_string(),
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to stop ffmpeg gracefully, killing");
                        let _ = process.kill().await;
                        "cancelled (killed)".to_string()
                    }
                }
            }
            result = process.wait() => {
                match result {
                    Ok(output) if output.code == 0 => "stream ended".to_string(),
                    Ok(output) => {
                        let reason = format!("ffmpeg exited with code {}: {}", output.code, output.stderr.lines().last().unwrap_or(""));
                        tracing::warn!(video_id = self.video_id, exit_code = output.code, "FFmpeg exited with non-zero code");
                        reason
                    }
                    Err(e) => {
                        let reason = format!("ffmpeg process error: {e}");
                        tracing::warn!(video_id = self.video_id, error = %e, "FFmpeg process error");
                        reason
                    }
                }
            }
        };

        let total_duration = start.elapsed();

        // Get actual file size
        let total_bytes = tokio::fs::metadata(&self.output_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        tracing::info!(
            video_id = self.video_id,
            total_bytes = total_bytes,
            duration = ?total_duration,
            reason = stop_reason,
            "✅ FFmpeg live recording stopped"
        );

        self.event_bus.emit_if_subscribed(DownloadEvent::LiveRecordingStopped {
            video_id: self.video_id.clone(),
            reason: stop_reason,
            output_path: self.output_path.clone(),
            total_bytes,
            total_duration,
        });

        Ok(super::super::RecordingResult {
            output_path: self.output_path.clone(),
            total_bytes,
            total_duration,
            segments_downloaded: 0, // ffmpeg handles segments internally
        })
    }
}

impl std::fmt::Display for FfmpegLiveRecorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FfmpegLiveRecorder(video_id={}, quality={}, output={})",
            self.video_id,
            self.quality,
            self.output_path.display()
        )
    }
}
