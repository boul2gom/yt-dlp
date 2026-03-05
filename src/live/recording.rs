//! Pure-Rust live stream recorder using reqwest for HLS segment fetching.
//!
//! This is the primary recording engine. It polls the HLS media playlist,
//! downloads new segments as they appear, and writes them sequentially to
//! the output file. The recording loop is cancellable via a `CancellationToken`
//! and optionally bounded by a maximum duration.

use std::collections::HashSet;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

use super::{RecordingConfig, StreamRecordingConfig, hls};
use crate::error::{Error, Result};
use crate::events::DownloadEvent;
use crate::events::types::RecordingMethod;

/// Progress throttle interval (50 ms) to avoid flooding the event bus.
const PROGRESS_THROTTLE_NANOS: u64 = 50_000_000;

/// Maximum number of retry attempts per segment fetch.
const SEGMENT_RETRY_ATTEMPTS: u32 = 3;

/// Delay between segment fetch retries.
const SEGMENT_RETRY_DELAY: Duration = Duration::from_millis(500);

/// Buffered writer capacity for recording output.
const OUTPUT_BUFFER_CAPACITY: usize = 64 * 1024;

/// Channel capacity for streaming fragments.
const FRAGMENT_CHANNEL_CAPACITY: usize = 32;

/// Divisor applied to target duration to derive poll interval.
const POLL_INTERVAL_DIVISOR: f64 = 2.0;

/// Bitrate conversion multiplier for bytes to bits.
const BITS_PER_BYTE: f64 = 8.0;

/// Result stream type for live fragment delivery.
pub type LiveFragmentStream = ReceiverStream<Result<LiveFragment>>;

/// A single live fragment downloaded from an HLS stream.
#[derive(Debug, Clone)]
pub struct LiveFragment {
    /// The segment sequence number.
    pub sequence: u64,
    /// The segment duration.
    pub duration: Duration,
    /// The absolute URL for the fragment.
    pub url: String,
    /// The fragment bytes.
    pub data: Vec<u8>,
}

/// Reqwest-based live stream recorder.
///
/// Downloads HLS segments in order and writes them to a single output file.
/// Designed for live streams where the media playlist is continuously updated.
#[derive(Debug)]
pub struct LiveRecorder {
    /// Shared recording state and logic.
    core: LiveRecordingCore,
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
            core: LiveRecordingCore::new(
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
        // Create output file with async buffered writer
        if let Some(parent) = self.output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let file = tokio::fs::File::create(&self.output_path)
            .await
            .map_err(|e| Error::io_with_path("creating recording output", &self.output_path, e))?;
        let mut writer = tokio::io::BufWriter::with_capacity(OUTPUT_BUFFER_CAPACITY, file);

        let stats = self.core.record_to_writer(&mut writer, &self.output_path).await?;

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
}

impl std::fmt::Display for LiveRecorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LiveRecorder(video_id={}, quality={}, output={})",
            self.core.video_id,
            self.core.quality,
            self.output_path.display()
        )
    }
}

/// Streamer for live HLS fragments.
#[derive(Debug)]
pub struct LiveFragmentStreamer {
    /// Shared recording state and logic.
    core: LiveRecordingCore,
}

impl LiveFragmentStreamer {
    /// Creates a new `LiveFragmentStreamer`.
    ///
    /// # Arguments
    ///
    /// * `config` - Common streaming configuration (URL, duration, events).
    /// * `client` - Shared HTTP client.
    pub fn new(config: StreamRecordingConfig, client: Arc<reqwest::Client>) -> Self {
        Self {
            core: LiveRecordingCore::new(
                config.stream_url,
                config.video_id,
                config.quality,
                config.max_duration,
                config.cancellation_token,
                client,
                config.event_bus,
                None,
            ),
        }
    }

    /// Starts streaming fragments from the live stream.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist or any segment cannot be fetched.
    ///
    /// # Returns
    ///
    /// A [`LiveFragmentStream`] that yields fragments as they arrive.
    pub async fn stream(&self) -> Result<LiveFragmentStream> {
        let (sender, receiver) = tokio::sync::mpsc::channel(FRAGMENT_CHANNEL_CAPACITY);
        let core = self.core.clone();

        tokio::spawn(async move {
            let result = core
                .run_loop(
                    |fragment| async {
                        if sender.send(Ok(fragment)).await.is_err() {
                            core.cancellation_token.cancel();
                            return Ok(());
                        }
                        Ok(())
                    },
                    || async { Ok(()) },
                )
                .await;

            if let Err(error) = result {
                core.event_bus.emit_if_subscribed(DownloadEvent::LiveStreamFailed {
                    video_id: core.video_id.clone(),
                    error: error.to_string(),
                });
                let _ = sender.send(Err(error)).await;
            }
        });

        Ok(ReceiverStream::new(receiver))
    }
}

#[derive(Debug, Clone)]
struct LiveRecordingCore {
    /// The URL of the HLS media playlist to poll.
    playlist_url: String,
    /// The video ID (for event emission).
    video_id: String,
    /// Quality label for event metadata.
    quality: String,
    /// Optional maximum recording duration.
    max_duration: Option<Duration>,
    /// Cancellation token for graceful stop.
    cancellation_token: CancellationToken,
    /// Shared HTTP client.
    client: Arc<reqwest::Client>,
    /// The event bus for emitting recording events.
    event_bus: crate::events::EventBus,
    /// Optional output path for recording mode.
    output_path: Option<PathBuf>,
}

impl LiveRecordingCore {
    #[allow(clippy::too_many_arguments)]
    fn new(
        playlist_url: String,
        video_id: String,
        quality: String,
        max_duration: Option<Duration>,
        cancellation_token: CancellationToken,
        client: Arc<reqwest::Client>,
        event_bus: crate::events::EventBus,
        output_path: Option<PathBuf>,
    ) -> Self {
        Self {
            playlist_url,
            video_id,
            quality,
            max_duration,
            cancellation_token,
            client,
            event_bus,
            output_path,
        }
    }

    async fn record_to_writer(
        &self,
        writer: &mut tokio::io::BufWriter<tokio::fs::File>,
        output_path: &PathBuf,
    ) -> Result<RecordingStats> {
        self.record_loop(writer, output_path).await
    }

    async fn record_loop(
        &self,
        writer: &mut tokio::io::BufWriter<tokio::fs::File>,
        output_path: &PathBuf,
    ) -> Result<RecordingStats> {
        let start = Instant::now();
        let bytes_written = Arc::new(AtomicU64::new(0));
        let mut segments_downloaded: u64 = 0;
        let mut seen_sequences: HashSet<u64> = HashSet::new();
        let mut last_progress_nanos: u64 = 0;

        tracing::info!(
            url = self.playlist_url,
            video_id = self.video_id,
            max_duration = ?self.max_duration,
            "📥 Starting live recording (reqwest)"
        );

        self.event_bus.emit_if_subscribed(DownloadEvent::LiveRecordingStarted {
            video_id: self.video_id.clone(),
            url: self.playlist_url.clone(),
            quality: self.quality.clone(),
            method: RecordingMethod::Native,
        });

        let initial = hls::parse_media(&self.client, &self.playlist_url).await?;
        let poll_interval = Duration::from_secs_f64(initial.target_duration / POLL_INTERVAL_DIVISOR);

        for seg in &initial.segments {
            seen_sequences.insert(seg.sequence);
        }

        for seg in &initial.segments {
            if self.cancellation_token.is_cancelled() {
                break;
            }

            let fragment = self.fetch_fragment(seg).await?;
            writer
                .write_all(&fragment.data)
                .await
                .map_err(|e| Error::io_with_path("writing segment", output_path, e))?;
            bytes_written.fetch_add(fragment.data.len() as u64, Ordering::Relaxed);
            segments_downloaded += 1;
        }

        writer
            .flush()
            .await
            .map_err(|e| Error::io_with_path("flushing output", output_path, e))?;

        let stop_reason = loop {
            if let Some(max) = self.max_duration
                && start.elapsed() >= max
            {
                break "max duration reached".to_string();
            }

            tokio::select! {
                _ = self.cancellation_token.cancelled() => {
                    break "cancelled".to_string();
                }
                _ = tokio::time::sleep(poll_interval) => {}
            }

            let playlist = match hls::parse_media(&self.client, &self.playlist_url).await {
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
                if self.cancellation_token.is_cancelled() {
                    break;
                }

                let fragment = self.fetch_fragment(seg).await?;
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
                let bitrate_bps = if elapsed.as_secs_f64() > 0.0 {
                    (total_bytes as f64 * BITS_PER_BYTE) / elapsed.as_secs_f64()
                } else {
                    0.0
                };

                self.event_bus.emit_if_subscribed(DownloadEvent::LiveStreamProgress {
                    video_id: self.video_id.clone(),
                    elapsed,
                    bytes_received: total_bytes,
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
            .output_path
            .clone()
            .ok_or_else(|| Error::live_recording(&self.playlist_url, "missing output path for recording"))?;

        self.event_bus.emit_if_subscribed(DownloadEvent::LiveRecordingStopped {
            video_id: self.video_id.clone(),
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

    async fn run_loop<F, Fut, B, BFut>(&self, mut on_fragment: F, mut on_batch: B) -> Result<RecordingStats>
    where
        F: FnMut(LiveFragment) -> Fut,
        Fut: Future<Output = Result<()>>,
        B: FnMut() -> BFut,
        BFut: Future<Output = Result<()>>,
    {
        let start = Instant::now();
        let bytes_written = Arc::new(AtomicU64::new(0));
        let mut segments_downloaded: u64 = 0;
        let mut seen_sequences: HashSet<u64> = HashSet::new();
        let mut last_progress_nanos: u64 = 0;

        tracing::info!(
            url = self.playlist_url,
            video_id = self.video_id,
            max_duration = ?self.max_duration,
            "📥 Starting live streaming (reqwest)"
        );

        self.event_bus.emit_if_subscribed(DownloadEvent::LiveStreamStarted {
            video_id: self.video_id.clone(),
            url: self.playlist_url.clone(),
            quality: self.quality.clone(),
        });

        // Initial playlist fetch to determine poll interval
        let initial = hls::parse_media(&self.client, &self.playlist_url).await?;
        let poll_interval = Duration::from_secs_f64(initial.target_duration / POLL_INTERVAL_DIVISOR);

        // Seed seen set with initial segments (don't re-download them)
        for seg in &initial.segments {
            seen_sequences.insert(seg.sequence);
        }

        // Download initial segments to start the output
        for seg in &initial.segments {
            if self.cancellation_token.is_cancelled() {
                break;
            }
            let fragment = self.fetch_fragment(seg).await?;
            bytes_written.fetch_add(fragment.data.len() as u64, Ordering::Relaxed);
            segments_downloaded += 1;
            on_fragment(fragment).await?;
        }
        on_batch().await?;

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

                let fragment = self.fetch_fragment(seg).await?;
                bytes_written.fetch_add(fragment.data.len() as u64, Ordering::Relaxed);
                segments_downloaded += 1;
                seen_sequences.insert(seg.sequence);
                on_fragment(fragment).await?;
            }
            on_batch().await?;

            // Emit progress (throttled)
            let now_nanos = start.elapsed().as_nanos() as u64;
            if now_nanos - last_progress_nanos >= PROGRESS_THROTTLE_NANOS {
                last_progress_nanos = now_nanos;
                let total_bytes = bytes_written.load(Ordering::Relaxed);
                let elapsed = start.elapsed();
                let bitrate_bps = if elapsed.as_secs_f64() > 0.0 {
                    (total_bytes as f64 * BITS_PER_BYTE) / elapsed.as_secs_f64()
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

        let total_duration = start.elapsed();
        let total_bytes = bytes_written.load(Ordering::Relaxed);

        self.event_bus.emit_if_subscribed(DownloadEvent::LiveStreamStopped {
            video_id: self.video_id.clone(),
            reason: stop_reason.clone(),
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

    /// Fetches a single segment's bytes with retries.
    async fn fetch_segment(&self, url: &str) -> Result<Vec<u8>> {
        let mut last_error = None;

        for attempt in 1..=SEGMENT_RETRY_ATTEMPTS {
            match self.fetch_segment_once(url).await {
                Ok(data) => return Ok(data),
                Err(e) => {
                    if attempt < SEGMENT_RETRY_ATTEMPTS {
                        tracing::warn!(
                            url = url,
                            attempt = attempt,
                            max_attempts = SEGMENT_RETRY_ATTEMPTS,
                            error = %e,
                            "Segment fetch failed, retrying"
                        );
                        tokio::time::sleep(SEGMENT_RETRY_DELAY).await;
                    }
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap())
    }

    /// Single attempt to fetch a segment.
    async fn fetch_segment_once(&self, url: &str) -> Result<Vec<u8>> {
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

    async fn fetch_fragment(&self, segment: &hls::HlsSegment) -> Result<LiveFragment> {
        let data = self.fetch_segment(&segment.url).await?;
        Ok(LiveFragment {
            sequence: segment.sequence,
            duration: Duration::from_secs_f64(segment.duration),
            url: segment.url.clone(),
            data,
        })
    }
}

#[derive(Debug, Clone)]
struct RecordingStats {
    total_bytes: u64,
    total_duration: Duration,
    segments_downloaded: u64,
    stop_reason: String,
}
