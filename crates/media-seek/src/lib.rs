#![doc = include_str!("../README.md")]

pub(crate) mod audio;
mod detect;
pub(crate) mod video;

pub mod error;
pub mod index;

use std::future::Future;

pub use error::{Error, Result};
pub use index::{ByteRange, ContainerIndex};

/// A source capable of fetching an arbitrary byte range from a remote stream.
///
/// Implementors provide the HTTP (or other) transport. `media-seek` calls this
/// only for container formats where the seek index lies outside the initial probe
/// (e.g. WebM Cues deep in the file, OGG page bisection, AVI idx1 at EOF).
///
/// # Examples
///
/// ```rust,no_run
/// use media_seek::RangeFetcher;
///
/// struct HttpFetcher {
///     url: String,
/// }
///
/// impl RangeFetcher for HttpFetcher {
///     type Error = std::io::Error;
///
///     async fn fetch(&self, start: u64, end: u64) -> std::result::Result<Vec<u8>, Self::Error> {
///         // Issue a Range: bytes={start}-{end} HTTP request and return the body.
///         todo!()
///     }
/// }
/// ```
pub trait RangeFetcher {
    /// The error type produced when a fetch fails.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Fetches bytes `[start, end]` (inclusive) from the remote stream.
    ///
    /// # Arguments
    ///
    /// * `start` - First byte to fetch (inclusive).
    /// * `end` - Last byte to fetch (inclusive).
    ///
    /// # Errors
    ///
    /// Returns `Self::Error` when the underlying transport fails.
    ///
    /// # Returns
    ///
    /// The fetched bytes as an owned `Vec<u8>`.
    fn fetch(&self, start: u64, end: u64) -> impl Future<Output = std::result::Result<Vec<u8>, Self::Error>> + Send;
}

/// Parses the container index for a stream and returns a [`ContainerIndex`].
///
/// Detects the container format from the magic bytes in `probe`, then dispatches
/// to the appropriate parser. Formats that require additional byte ranges beyond
/// the probe (WebM Cues outside the initial window, OGG page bisection, AVI idx1,
/// MPEG-TS PCR binary search) call `fetcher.fetch()` as needed.
///
/// # Arguments
///
/// * `probe` - The leading bytes of the stream. 512 KB is recommended for reliable
///   format detection and, where possible, index parsing without extra fetches.
/// * `total_size` - Total stream size in bytes. Required for binary-search formats
///   (OGG, AVI, MPEG-TS) and for certain PCM-derived calculations.
/// * `fetcher` - Provides additional byte ranges when the probe is insufficient.
///
/// # Errors
///
/// - [`Error::UnsupportedFormat`] — container is MHTML, `None`, or not recognised.
/// - [`Error::ParseFailed`] — the container index could not be parsed.
/// - [`Error::FetchFailed`] — a required extra Range request failed.
///
/// # Returns
///
/// A [`ContainerIndex`] from which [`ContainerIndex::find_byte_range`] returns
/// the `(content_start_byte, content_end_byte)` byte range covering the
/// requested time window, plus [`ContainerIndex::init_end_byte`] for the
/// init-segment prefix.
pub async fn parse<F: RangeFetcher>(probe: &[u8], total_size: Option<u64>, fetcher: &F) -> Result<ContainerIndex> {
    tracing::debug!(
        probe_len = probe.len(),
        total_size = ?total_size,
        "⚙️ Parsing container index"
    );

    let format = detect::detect(probe).ok_or(Error::UnsupportedFormat)?;

    tracing::debug!(format = ?format, "⚙️ Detected container format");

    let result = match format {
        detect::Format::Mp4 => video::mp4::parse(probe, total_size, fetcher).await,
        detect::Format::Webm => video::webm::parse(probe, total_size, fetcher).await,
        detect::Format::Mp3 => audio::mp3::parse(probe, total_size, fetcher).await,
        detect::Format::Ogg => audio::ogg::parse(probe, total_size, fetcher).await,
        detect::Format::Flac => audio::flac::parse(probe, total_size),
        detect::Format::Wav => audio::pcm::parse_wav(probe),
        detect::Format::Aiff => audio::pcm::parse_aiff(probe),
        detect::Format::Adts => audio::adts::parse(probe),
        detect::Format::Flv => video::flv::parse(probe),
        detect::Format::Avi => video::avi::parse(probe, total_size, fetcher).await,
        detect::Format::Ts => video::ts::parse(probe, total_size, fetcher).await,
    };

    if result.is_ok() {
        tracing::debug!("✅ Container index parsed");
    }
    result
}
