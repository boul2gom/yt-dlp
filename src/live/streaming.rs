use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tokio::time;
use tokio_stream::wrappers::ReceiverStream;

use super::core::{
    BITS_PER_BYTE, LiveCore, LiveFragment, POLL_INTERVAL_DIVISOR, PROGRESS_THROTTLE_NANOS, RecordingStats,
    SegmentErrorMode, ZERO_F64, ZERO_U64,
};
use super::{StreamRecordingConfig, hls};
use crate::error::Result;
use crate::events::DownloadEvent;

/// Channel capacity for streaming fragments.
const FRAGMENT_CHANNEL_CAPACITY: usize = 32;

/// Result stream type for live fragment delivery.
pub type LiveFragmentStream = ReceiverStream<Result<LiveFragment>>;

/// Streamer for live HLS fragments.
#[derive(Debug)]
pub struct LiveFragmentStreamer {
    /// Shared recording state and logic.
    core: LiveCore,
}

impl LiveFragmentStreamer {
    /// Creates a new `LiveFragmentStreamer`.
    ///
    /// # Arguments
    ///
    /// * `config` - Common streaming configuration (URL, duration, events).
    /// * `client` - Shared HTTP client.
    ///
    /// # Returns
    ///
    /// A new [`LiveFragmentStreamer`] instance.
    pub fn new(config: StreamRecordingConfig, client: Arc<reqwest::Client>) -> Self {
        Self {
            core: LiveCore::new(
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
        let (sender, receiver) = mpsc::channel(FRAGMENT_CHANNEL_CAPACITY);
        let core = self.core.clone();

        tokio::spawn(async move {
            let result = run_loop(
                core.clone(),
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

async fn run_loop<F, Fut, B, BFut>(core: LiveCore, mut on_fragment: F, mut on_batch: B) -> Result<RecordingStats>
where
    F: FnMut(LiveFragment) -> Fut,
    Fut: Future<Output = Result<()>>,
    B: FnMut() -> BFut,
    BFut: Future<Output = Result<()>>,
{
    let start = Instant::now();
    let bytes_written = Arc::new(AtomicU64::new(ZERO_U64));
    let mut segments_downloaded: u64 = ZERO_U64;
    let mut seen_sequences: HashSet<u64> = HashSet::new();
    let mut last_progress_nanos: u64 = ZERO_U64;

    tracing::info!(
        url = core.playlist_url,
        video_id = core.video_id,
        max_duration = ?core.max_duration,
        "📥 Starting live streaming (reqwest)"
    );

    core.event_bus.emit_if_subscribed(DownloadEvent::LiveStreamStarted {
        video_id: core.video_id.clone(),
        url: core.playlist_url.clone(),
        quality: core.quality.clone(),
    });

    let initial = hls::parse_media(&core.client, &core.playlist_url).await?;
    let poll_interval = Duration::from_secs_f64(initial.target_duration / POLL_INTERVAL_DIVISOR);

    for seg in &initial.segments {
        seen_sequences.insert(seg.sequence);
    }

    for seg in &initial.segments {
        if core.cancellation_token.is_cancelled() {
            break;
        }
        let fragment = core.fetch_fragment(seg, SegmentErrorMode::Streaming).await?;
        bytes_written.fetch_add(fragment.data.len() as u64, Ordering::Relaxed);
        segments_downloaded += 1;
        on_fragment(fragment).await?;
    }
    on_batch().await?;

    let stop_reason = loop {
        if let Some(max) = core.max_duration
            && start.elapsed() >= max
        {
            break "max duration reached".to_string();
        }

        tokio::select! {
            _ = core.cancellation_token.cancelled() => {
                break "cancelled".to_string();
            }
            _ = time::sleep(poll_interval) => {}
        }

        let playlist = match hls::parse_media(&core.client, &core.playlist_url).await {
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
            if core.cancellation_token.is_cancelled() {
                break;
            }

            let fragment = core.fetch_fragment(seg, SegmentErrorMode::Streaming).await?;
            bytes_written.fetch_add(fragment.data.len() as u64, Ordering::Relaxed);
            segments_downloaded += 1;
            seen_sequences.insert(seg.sequence);
            on_fragment(fragment).await?;
        }
        on_batch().await?;

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

            core.event_bus.emit_if_subscribed(DownloadEvent::LiveStreamProgress {
                video_id: core.video_id.clone(),
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

    core.event_bus.emit_if_subscribed(DownloadEvent::LiveStreamStopped {
        video_id: core.video_id.clone(),
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
