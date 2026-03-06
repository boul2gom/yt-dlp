use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::{fs, time};

use super::core::{
    BITS_PER_BYTE, LiveCore, PROGRESS_THROTTLE_NANOS, RecordingStats, SegmentErrorMode, ZERO_F64, ZERO_U64,
};
use super::{RecordingConfig, hls};
use crate::error::{Error, Result};
use crate::events::DownloadEvent;
use crate::events::types::RecordingMethod;

/// Buffered writer capacity for recording output.
const OUTPUT_BUFFER_CAPACITY: usize = 64 * 1024;

/// Reqwest-based live stream recorder.
///
/// Downloads HLS segments in order and writes them to a single output file.
/// Designed for live streams where the media playlist is continuously updated.
#[derive(Debug)]
pub struct LiveRecorder {
    /// Shared recording state and logic.
    core: LiveCore,
    /// The output file path.
    output_path: PathBuf,
}

impl LiveRecorder {
    /// Creates a new `LiveRecorder`.
    ///
    /// # Arguments
    ///
    /// * `config` - Common recording configuration (URL, output, duration, events).
    /// * `client` - Shared HTTP client.
    pub fn new(config: RecordingConfig, client: Arc<reqwest::Client>) -> Self {
        Self {
            core: LiveCore::new(
                config.stream_url,
                config.video_id,
                config.quality,
                config.max_duration,
                config.cancellation_token,
                client,
                config.event_bus,
                Some(config.output_path.clone()),
            ),
            output_path: config.output_path,
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
        if let Some(parent) = self.output_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let file = fs::File::create(&self.output_path)
            .await
            .map_err(|e| Error::io_with_path("creating recording output", &self.output_path, e))?;
        let mut writer = BufWriter::with_capacity(OUTPUT_BUFFER_CAPACITY, file);

        let stats = self.record_to_writer(&mut writer, &self.output_path).await?;

        tracing::info!(
            video_id = self.core.video_id,
            total_bytes = stats.total_bytes,
            segments = stats.segments_downloaded,
            duration = ?stats.total_duration,
            reason = stats.stop_reason,
            "✅ Live recording stopped"
        );

        Ok(super::RecordingResult {
            output_path: self.output_path.clone(),
            total_bytes: stats.total_bytes,
            total_duration: stats.total_duration,
            segments_downloaded: stats.segments_downloaded,
        })
    }

    async fn record_to_writer(
        &self,
        writer: &mut BufWriter<fs::File>,
        output_path: &PathBuf,
    ) -> Result<RecordingStats> {
        self.record_loop(writer, output_path).await
    }

    async fn record_loop(&self, writer: &mut BufWriter<fs::File>, output_path: &PathBuf) -> Result<RecordingStats> {
        let start = Instant::now();
        let bytes_written = Arc::new(AtomicU64::new(ZERO_U64));
        let mut segments_downloaded: u64 = ZERO_U64;
        let mut seen_sequences: HashSet<u64> = HashSet::new();
        let mut last_progress_nanos: u64 = ZERO_U64;

        tracing::info!(
            url = self.core.playlist_url,
            video_id = self.core.video_id,
            max_duration = ?self.core.max_duration,
            "📥 Starting live recording (reqwest)"
        );

        self.core
            .event_bus
            .emit_if_subscribed(DownloadEvent::LiveRecordingStarted {
                video_id: self.core.video_id.clone(),
                url: self.core.playlist_url.clone(),
                quality: self.core.quality.clone(),
                method: RecordingMethod::Native,
            });

        let initial = hls::parse_media(&self.core.client, &self.core.playlist_url).await?;
        let poll_interval = Duration::from_secs_f64(initial.target_duration / super::core::POLL_INTERVAL_DIVISOR);

        for seg in &initial.segments {
            seen_sequences.insert(seg.sequence);
        }

        for seg in &initial.segments {
            if self.core.cancellation_token.is_cancelled() {
                break;
            }

            let fragment = self.core.fetch_fragment(seg, SegmentErrorMode::Recording).await?;
            writer
                .write_all(&fragment.data)
                .await
                .map_err(|e| Error::io_with_path("writing segment", output_path, e))?;
            bytes_written.fetch_add(fragment.data.len() as u64, Ordering::Relaxed);
            segments_downloaded += 1;
            seen_sequences.insert(seg.sequence);
        }

        writer
            .flush()
            .await
            .map_err(|e| Error::io_with_path("flushing output", output_path, e))?;

        let stop_reason = loop {
            if let Some(max) = self.core.max_duration
                && start.elapsed() >= max
            {
                break "max duration reached".to_string();
            }

            tokio::select! {
                _ = self.core.cancellation_token.cancelled() => {
                    break "cancelled".to_string();
                }
                _ = time::sleep(poll_interval) => {}
            }

            let playlist = match hls::parse_media(&self.core.client, &self.core.playlist_url).await {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "HLS playlist fetch failed, retrying next cycle");
                    continue;
                }
            };

            if playlist.is_endlist && playlist.segments.iter().all(|s| seen_sequences.contains(&s.sequence)) {
                break "stream ended".to_string();
            }

            let new_segments: Vec<_> = playlist
                .segments
                .iter()
                .filter(|s| !seen_sequences.contains(&s.sequence))
                .collect();

            for seg in &new_segments {
                if self.core.cancellation_token.is_cancelled() {
                    break;
                }

                let fragment = self.core.fetch_fragment(seg, SegmentErrorMode::Recording).await?;
                writer
                    .write_all(&fragment.data)
                    .await
                    .map_err(|e| Error::io_with_path("writing segment", output_path, e))?;
                bytes_written.fetch_add(fragment.data.len() as u64, Ordering::Relaxed);
                segments_downloaded += 1;
                seen_sequences.insert(seg.sequence);
            }

            writer
                .flush()
                .await
                .map_err(|e| Error::io_with_path("flushing output", output_path, e))?;

            let now_nanos = start.elapsed().as_nanos() as u64;
            if now_nanos - last_progress_nanos >= PROGRESS_THROTTLE_NANOS {
                last_progress_nanos = now_nanos;
                let total_bytes = bytes_written.load(Ordering::Relaxed);
                let elapsed = start.elapsed();
                let bitrate_bps = if elapsed.as_secs_f64() > ZERO_F64 {
                    (total_bytes as f64 * BITS_PER_BYTE) / elapsed.as_secs_f64()
                } else {
                    ZERO_F64
                };

                self.core
                    .event_bus
                    .emit_if_subscribed(DownloadEvent::LiveRecordingProgress {
                        video_id: self.core.video_id.clone(),
                        elapsed,
                        bytes_written: total_bytes,
                        segments: segments_downloaded,
                        bitrate_bps,
                    });
            }

            if playlist.is_endlist {
                break "stream ended".to_string();
            }
        };

        let total_duration = start.elapsed();
        let total_bytes = bytes_written.load(Ordering::Relaxed);

        let output_path = self
            .core
            .output_path
            .clone()
            .ok_or_else(|| Error::live_recording(&self.core.playlist_url, "missing output path for recording"))?;

        self.core
            .event_bus
            .emit_if_subscribed(DownloadEvent::LiveRecordingStopped {
                video_id: self.core.video_id.clone(),
                reason: stop_reason.clone(),
                output_path,
                total_bytes,
                total_duration,
            });

        Ok(RecordingStats {
            total_bytes,
            total_duration,
            segments_downloaded,
            stop_reason,
        })
    }
}

impl fmt::Display for LiveRecorder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LiveRecorder(video_id={}, quality={}, output={})",
            self.core.video_id,
            self.core.quality,
            self.output_path.display()
        )
    }
}
