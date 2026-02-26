//! Pure-Rust live stream recorder using reqwest for HLS segment fetching.
//!
//! This is the primary recording engine. It polls the HLS media playlist,
//! downloads new segments as they appear, and writes them sequentially to
//! the output file. The recording loop is cancellable via a `CancellationToken`
//! and optionally bounded by a maximum duration.

use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use super::hls;
use crate::error::{Error, Result};
use crate::events::DownloadEvent;
use crate::events::types::RecordingMethod;

/// Progress throttle interval (50 ms) to avoid flooding the event bus.
const PROGRESS_THROTTLE_NANOS: u64 = 50_000_000;

/// Maximum number of segments to prefetch concurrently.
#[allow(dead_code)]
const PREFETCH_CONCURRENCY: usize = 3;

/// Reqwest-based live stream recorder.
///
/// Downloads HLS segments in order and writes them to a single output file.
/// Designed for live streams where the media playlist is continuously updated.
#[derive(Debug)]
pub struct LiveRecorder {
    /// The URL of the HLS media playlist to poll.
    playlist_url: String,
    /// The output file path.
    output_path: PathBuf,
    /// The video ID (for event emission).
    video_id: String,
    /// Optional maximum recording duration.
    max_duration: Option<Duration>,
    /// Cancellation token for graceful stop.
    cancellation_token: CancellationToken,
    /// Shared HTTP client.
    client: Arc<reqwest::Client>,
    /// The event bus for emitting recording events.
    event_bus: crate::events::EventBus,
    /// Quality label for event metadata.
    quality: String,
}

impl LiveRecorder {
    /// Creates a new `LiveRecorder`.
    ///
    /// # Arguments
    ///
    /// * `playlist_url` - The HLS media playlist URL to poll.
    /// * `output_path` - Where to write the recorded stream.
    /// * `video_id` - The video ID (for events).
    /// * `quality` - Quality label (e.g. "1080p").
    /// * `max_duration` - Optional maximum recording duration.
    /// * `cancellation_token` - Token to cancel recording.
    /// * `client` - Shared HTTP client.
    /// * `event_bus` - Event bus for broadcasting progress.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        playlist_url: impl Into<String>,
        output_path: impl Into<PathBuf>,
        video_id: impl Into<String>,
        quality: impl Into<String>,
        max_duration: Option<Duration>,
        cancellation_token: CancellationToken,
        client: Arc<reqwest::Client>,
        event_bus: crate::events::EventBus,
    ) -> Self {
        Self {
            playlist_url: playlist_url.into(),
            output_path: output_path.into(),
            video_id: video_id.into(),
            quality: quality.into(),
            max_duration,
            cancellation_token,
            client,
            event_bus,
        }
    }

    /// Starts the recording loop.
    ///
    /// Polls the HLS media playlist at intervals of `target_duration / 2`,
    /// downloads new segments, and appends them to the output file.
    /// Stops when the cancellation token is triggered, the stream ends
    /// (`#EXT-X-ENDLIST`), or the max duration is reached.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist cannot be fetched, segments fail to download,
    /// or the output file cannot be written.
    ///
    /// # Returns
    ///
    /// A [`super::RecordingResult`] with recording statistics.
    pub async fn record(&self) -> Result<super::RecordingResult> {
        let start = Instant::now();
        let bytes_written = Arc::new(AtomicU64::new(0));
        let mut segments_downloaded: u64 = 0;
        let mut seen_sequences: HashSet<u64> = HashSet::new();
        let mut last_progress_nanos: u64 = 0;

        tracing::info!(
            url = self.playlist_url,
            video_id = self.video_id,
            output = ?self.output_path,
            max_duration = ?self.max_duration,
            "📥 Starting live recording (reqwest)"
        );

        self.event_bus.emit_if_subscribed(DownloadEvent::LiveRecordingStarted {
            video_id: self.video_id.clone(),
            url: self.playlist_url.clone(),
            quality: self.quality.clone(),
            method: RecordingMethod::Reqwest,
        });

        // Create output file with buffered writer
        if let Some(parent) = self.output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let file = std::fs::File::create(&self.output_path)
            .map_err(|e| Error::io_with_path("creating recording output", &self.output_path, e))?;
        let mut writer = std::io::BufWriter::with_capacity(64 * 1024, file);

        // Initial playlist fetch to determine poll interval
        let initial = hls::parse_media(&self.client, &self.playlist_url).await?;
        let poll_interval = Duration::from_secs_f64(initial.target_duration / 2.0);

        // Seed seen set with initial segments (don't re-download them)
        for seg in &initial.segments {
            seen_sequences.insert(seg.sequence);
        }

        // Download initial segments to start the file
        for seg in &initial.segments {
            if self.cancellation_token.is_cancelled() {
                break;
            }
            let data = self.fetch_segment(&seg.url).await?;
            writer
                .write_all(&data)
                .map_err(|e| Error::io_with_path("writing segment", &self.output_path, e))?;
            bytes_written.fetch_add(data.len() as u64, Ordering::Relaxed);
            segments_downloaded += 1;
        }
        writer
            .flush()
            .map_err(|e| Error::io_with_path("flushing output", &self.output_path, e))?;

        // Poll loop
        let stop_reason = loop {
            // Check max duration
            if let Some(max) = self.max_duration
                && start.elapsed() >= max
            {
                break "max duration reached".to_string();
            }

            // Wait for next poll or cancellation
            tokio::select! {
                _ = self.cancellation_token.cancelled() => {
                    break "cancelled".to_string();
                }
                _ = tokio::time::sleep(poll_interval) => {}
            }

            // Fetch updated playlist
            let playlist = match hls::parse_media(&self.client, &self.playlist_url).await {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "HLS playlist fetch failed, retrying next cycle");
                    continue;
                }
            };

            // Stream ended
            if playlist.is_endlist && playlist.segments.iter().all(|s| seen_sequences.contains(&s.sequence)) {
                break "stream ended".to_string();
            }

            // Download new segments
            let new_segments: Vec<_> = playlist
                .segments
                .iter()
                .filter(|s| !seen_sequences.contains(&s.sequence))
                .collect();

            for seg in &new_segments {
                if self.cancellation_token.is_cancelled() {
                    break;
                }

                let data = self.fetch_segment(&seg.url).await?;
                writer
                    .write_all(&data)
                    .map_err(|e| Error::io_with_path("writing segment", &self.output_path, e))?;
                bytes_written.fetch_add(data.len() as u64, Ordering::Relaxed);
                segments_downloaded += 1;
                seen_sequences.insert(seg.sequence);
            }
            writer
                .flush()
                .map_err(|e| Error::io_with_path("flushing output", &self.output_path, e))?;

            // Emit progress (throttled)
            let now_nanos = start.elapsed().as_nanos() as u64;
            if now_nanos - last_progress_nanos >= PROGRESS_THROTTLE_NANOS {
                last_progress_nanos = now_nanos;
                let total_bytes = bytes_written.load(Ordering::Relaxed);
                let elapsed = start.elapsed();
                let bitrate_bps = if elapsed.as_secs() > 0 {
                    ((total_bytes * 8) / elapsed.as_secs()) as f64
                } else {
                    0.0
                };

                self.event_bus.emit_if_subscribed(DownloadEvent::LiveRecordingProgress {
                    video_id: self.video_id.clone(),
                    elapsed,
                    bytes_written: total_bytes,
                    segments: segments_downloaded,
                    bitrate_bps,
                });
            }

            // If endlist was seen, stop after downloading remaining
            if playlist.is_endlist {
                break "stream ended".to_string();
            }
        };

        let total_bytes = bytes_written.load(Ordering::Relaxed);
        let total_duration = start.elapsed();

        tracing::info!(
            video_id = self.video_id,
            total_bytes = total_bytes,
            segments = segments_downloaded,
            duration = ?total_duration,
            reason = stop_reason,
            "✅ Live recording stopped"
        );

        self.event_bus.emit_if_subscribed(DownloadEvent::LiveRecordingStopped {
            video_id: self.video_id.clone(),
            reason: stop_reason,
            output_path: self.output_path.clone(),
            total_bytes,
            total_duration,
        });

        Ok(super::RecordingResult {
            output_path: self.output_path.clone(),
            total_bytes,
            total_duration,
            segments_downloaded,
        })
    }

    /// Fetches a single segment's bytes.
    async fn fetch_segment(&self, url: &str) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::http(url, "fetching HLS segment", e))?;

        if !response.status().is_success() {
            return Err(Error::live_recording(
                url,
                format!("segment fetch returned HTTP {}", response.status()),
            ));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| Error::http(url, "reading segment body", e))?;

        Ok(bytes.to_vec())
    }
}

impl std::fmt::Display for LiveRecorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LiveRecorder(video_id={}, quality={}, output={})",
            self.video_id,
            self.quality,
            self.output_path.display()
        )
    }
}
