use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::time;
use tokio_util::sync::CancellationToken;

use super::hls;
use crate::error::{Error, Result};
use crate::events::{DownloadEvent, EventBus};

/// Progress throttle interval (50 ms) to avoid flooding the event bus.
pub(super) const PROGRESS_THROTTLE_NANOS: u64 = 50_000_000;

/// Maximum number of retry attempts per segment fetch.
pub(super) const SEGMENT_RETRY_ATTEMPTS: u32 = 3;

/// Delay between segment fetch retries.
pub(super) const SEGMENT_RETRY_DELAY: Duration = Duration::from_millis(500);

/// Divisor applied to target duration to derive poll interval.
pub(super) const POLL_INTERVAL_DIVISOR: f64 = 2.0;

/// Bitrate conversion multiplier for bytes to bits.
pub(super) const BITS_PER_BYTE: f64 = 8.0;

/// Zero value for u64 counters.
pub(super) const ZERO_U64: u64 = 0;

/// Zero value for f64 calculations.
pub(super) const ZERO_F64: f64 = 0.0;

/// Determines which error variant to use for segment fetch failures.
#[derive(Debug, Clone, Copy)]
pub(super) enum SegmentErrorMode {
    #[cfg(feature = "live-recording")]
    Recording,
    #[cfg(feature = "live-streaming")]
    Streaming,
}

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

/// Configuration required to construct a [`LiveCore`] instance.
pub(super) struct LiveCoreConfig {
    /// The URL of the HLS media playlist to poll.
    pub(super) playlist_url: String,
    /// The video ID (for event emission).
    pub(super) video_id: String,
    /// Quality label for event metadata.
    pub(super) quality: String,
    /// Optional maximum recording duration.
    pub(super) max_duration: Option<Duration>,
    /// Cancellation token for graceful stop.
    pub(super) cancellation_token: CancellationToken,
    /// Shared HTTP client.
    pub(super) client: Arc<reqwest::Client>,
    /// The event bus for emitting recording events.
    pub(super) event_bus: EventBus,
}

/// Shared state and utilities for live recording/streaming.
#[derive(Debug, Clone)]
pub(super) struct LiveCore {
    /// The URL of the HLS media playlist to poll.
    pub(super) playlist_url: String,
    /// The video ID (for event emission).
    pub(super) video_id: String,
    /// Quality label for event metadata.
    pub(super) quality: String,
    /// Optional maximum recording duration.
    pub(super) max_duration: Option<Duration>,
    /// Cancellation token for graceful stop.
    pub(super) cancellation_token: CancellationToken,
    /// Shared HTTP client.
    pub(super) client: Arc<reqwest::Client>,
    /// The event bus for emitting recording events.
    pub(super) event_bus: EventBus,
}

impl LiveCore {
    /// Creates a new [`LiveCore`] from the provided configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - All shared state needed by the live recording/streaming engines.
    pub(super) fn new(config: LiveCoreConfig) -> Self {
        Self {
            playlist_url: config.playlist_url,
            video_id: config.video_id,
            quality: config.quality,
            max_duration: config.max_duration,
            cancellation_token: config.cancellation_token,
            client: config.client,
            event_bus: config.event_bus,
        }
    }

    /// Fetches a single segment's bytes with retries.
    pub(super) async fn fetch_segment(&self, url: &str, mode: SegmentErrorMode) -> Result<Vec<u8>> {
        let mut last_error = None;

        for attempt in 1..=SEGMENT_RETRY_ATTEMPTS {
            match self.fetch_segment_once(url, mode).await {
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
                        time::sleep(SEGMENT_RETRY_DELAY).await;
                    }
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap())
    }

    /// Single attempt to fetch a segment.
    pub(super) async fn fetch_segment_once(&self, url: &str, mode: SegmentErrorMode) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::http(url, "fetching HLS segment", e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.segment_fetch_failed(url, status, mode));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| Error::http(url, "reading segment body", e))?;

        Ok(bytes.to_vec())
    }

    /// Fetches a fragment and returns a structured fragment payload.
    pub(super) async fn fetch_fragment(
        &self,
        segment: &hls::HlsSegment,
        mode: SegmentErrorMode,
    ) -> Result<LiveFragment> {
        let data = self.fetch_segment(&segment.url, mode).await?;
        Ok(LiveFragment {
            sequence: segment.sequence,
            duration: Duration::from_secs_f64(segment.duration),
            url: segment.url.clone(),
            data,
        })
    }

    fn segment_fetch_failed(&self, url: &str, status: reqwest::StatusCode, mode: SegmentErrorMode) -> Error {
        match mode {
            #[cfg(feature = "live-recording")]
            SegmentErrorMode::Recording => {
                Error::live_recording(url, format!("segment fetch returned HTTP {}", status))
            }
            #[cfg(feature = "live-streaming")]
            SegmentErrorMode::Streaming => Error::live_segment_fetch_failed(url, status),
        }
    }
}

/// Result metrics produced by the recording loop.
#[cfg(feature = "live-recording")]
#[derive(Debug, Clone)]
pub(super) struct RecordingStats {
    pub(super) total_bytes: u64,
    pub(super) total_duration: Duration,
    pub(super) segments_downloaded: u64,
    pub(super) stop_reason: String,
}

/// Maximum number of HLS sequence numbers tracked for duplicate suppression.
///
/// When this window is full, the oldest entry is evicted from both the [`HashSet`]
/// and the [`VecDeque`] to prevent unbounded memory growth on long-running streams.
pub(super) const SEQUENCE_TRACK_WINDOW: usize = 512;

/// Registers a sequence number as seen, evicting the oldest entry when the window is full.
///
/// Inserts `sequence` into `seen` and appends it to the bounded `window` deque.
/// Once `window` exceeds [`SEQUENCE_TRACK_WINDOW`], the front element is popped and
/// removed from `seen`, keeping memory use constant over time.
pub(super) fn track_sequence(
    sequence: u64,
    seen: &mut std::collections::HashSet<u64>,
    window: &mut std::collections::VecDeque<u64>,
) {
    if seen.insert(sequence) {
        window.push_back(sequence);
    }
    while window.len() > SEQUENCE_TRACK_WINDOW {
        if let Some(evicted) = window.pop_front() {
            seen.remove(&evicted);
        }
    }
}

/// Emits a live progress event if the throttle interval has elapsed.
///
/// Computes elapsed time, byte count, and bitrate, then calls `make_event` to
/// construct the concrete `DownloadEvent` variant (streaming vs recording).
/// The caller supplies a closure so each module can emit its own event type
/// without duplicating the throttle / bitrate logic.
pub(super) fn emit_live_progress(
    core: &LiveCore,
    start: Instant,
    bytes_written: &AtomicU64,
    segments_downloaded: u64,
    last_progress_nanos: &mut u64,
    make_event: impl FnOnce(Duration, u64, u64, f64) -> DownloadEvent,
) {
    let now_nanos = start.elapsed().as_nanos() as u64;
    if now_nanos - *last_progress_nanos >= PROGRESS_THROTTLE_NANOS {
        *last_progress_nanos = now_nanos;
        let total_bytes = bytes_written.load(Ordering::Relaxed);
        let elapsed = start.elapsed();
        let bitrate_bps = if elapsed.as_secs_f64() > ZERO_F64 {
            (total_bytes as f64 * BITS_PER_BYTE) / elapsed.as_secs_f64()
        } else {
            ZERO_F64
        };
        core.event_bus
            .emit_if_subscribed(make_event(elapsed, total_bytes, segments_downloaded, bitrate_bps));
    }
}
