use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::time;
use tokio_util::sync::CancellationToken;

use super::hls;
use crate::error::{Error, Result};
use crate::events::EventBus;

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
    #[allow(dead_code)]
    pub sequence: u64,
    /// The segment duration.
    #[allow(dead_code)]
    pub duration: Duration,
    /// The absolute URL for the fragment.
    #[allow(dead_code)]
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
    /// Optional output path for recording mode.
    pub(super) output_path: Option<PathBuf>,
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
    /// Optional output path for recording mode.
    #[allow(dead_code)]
    pub(super) output_path: Option<PathBuf>,
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
            output_path: config.output_path,
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

/// Result metrics produced by the recording/streaming loops.
#[derive(Debug, Clone)]
pub(super) struct RecordingStats {
    #[allow(dead_code)]
    pub(super) total_bytes: u64,
    #[allow(dead_code)]
    pub(super) total_duration: Duration,
    #[allow(dead_code)]
    pub(super) segments_downloaded: u64,
    #[allow(dead_code)]
    pub(super) stop_reason: String,
}
