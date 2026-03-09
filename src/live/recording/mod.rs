//! Live recording engines.
//!
//! Groups the two recording implementations:
//! - [`native`]: Reqwest-based HLS segment recorder (primary).
//! - [`ffmpeg`]: FFmpeg-based recorder (fallback).

#[cfg(feature = "live-recording")]
pub mod ffmpeg;
#[cfg(feature = "live-recording")]
pub mod native;

#[cfg(feature = "live-recording")]
pub use ffmpeg::FfmpegLiveRecorder;
#[cfg(feature = "live-recording")]
pub use native::LiveRecorder;
