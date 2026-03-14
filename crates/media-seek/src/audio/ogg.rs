//! OGG container index parsing via page granule position binary search.
//!
//! An OGG stream is a sequence of 28-byte pages. The granule position in each
//! page header encodes the decoded sample count up to that page. This module
//! binary-searches for page boundaries corresponding to the requested timestamps
//! by fetching ranges from the stream.
//!
//! Supported codecs in the identification page: Vorbis, Opus, FLAC-in-OGG.
//! Unknown codecs (Theora video, Speex, CELT, …) produce a `ParseFailed` error
//! with a descriptive message naming the unrecognised codec magic.

use futures_util::future::try_join_all;

use crate::RangeFetcher;
use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

// ==================== Constants ====================

/// OGG page capture pattern.
pub(crate) const OGG_CAPTURE: &[u8; 4] = b"OggS";

/// Minimum OGG page header size (bytes).
const PAGE_HEADER_MIN: usize = 27;

/// Opus granule positions are always in 48 kHz units regardless of input sample rate.
const OPUS_GRANULE_RATE: u32 = 48000;

/// Number of evenly-spaced seek point probes across the stream.
const SEEK_POINTS: u64 = 64;

/// Bytes fetched per seek-point probe window.
const PROBE_WINDOW: u64 = 8192;

// ==================== Public entry point ====================

/// Parses an OGG stream and returns a `ContainerIndex`.
///
/// Reads the identification page from `probe` to extract the codec sample rate,
/// then binary-searches the stream via `fetcher` to build a page-boundary index
/// covering at most 64 evenly-spaced seek points.
///
/// Seek points whose byte positions fall entirely within `probe` are processed
/// directly without issuing an extra `fetch` call.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the OGG stream (should include at least the first two pages).
/// * `total_size` - Total stream size in bytes (required for binary search).
/// * `fetcher` - Used to fetch pages outside `probe`.
///
/// # Errors
///
/// Returns `Error::ParseFailed` when identification fails, `Error::FetchFailed`
/// on Range request errors.
pub(crate) async fn parse<F>(probe: &[u8], total_size: Option<u64>, fetcher: &F) -> Result<ContainerIndex>
where
    F: RangeFetcher,
{
    tracing::debug!(probe_len = probe.len(), total_size = ?total_size, "⚙️ Parsing OGG stream");
    let sample_rate = read_sample_rate(probe).ok_or_else(|| Error::parse(identify_codec_error(probe)))?;

    let total = total_size.ok_or_else(|| Error::parse("OGG binary search requires total_size"))?;

    let result = collect_probe_points(probe, total);
    let mut points = result.points;
    let fetch_positions = result.fetch_positions;

    // Fetch all out-of-probe seek-point windows in parallel.
    if !fetch_positions.is_empty() {
        tracing::debug!(fetches = fetch_positions.len(), "⚙️ OGG parallel seek-point fetches");
        let fetches = fetch_positions.iter().map(|&(start, end)| async move {
            let chunk = fetcher.fetch(start, end).await.map_err(Error::fetch)?;
            Ok::<_, Error>((start, chunk))
        });
        let results = try_join_all(fetches).await?;
        for (byte_pos, chunk) in results {
            if let Some(sync_off) = find_ogg_sync(&chunk)
                && let Some((granule, _)) = read_page_granule(&chunk, sync_off)
                && granule != u64::MAX
            {
                points.push((granule, byte_pos + sync_off as u64));
            }
        }
    }

    if points.is_empty() {
        return Err(Error::parse("no OGG seek points found"));
    }

    // Deduplicate by granule, then sort by byte offset for the final index.
    points.sort_unstable_by_key(|&(granule, off)| (granule, off));
    points.dedup_by_key(|&mut (granule, _)| granule);
    points.sort_unstable_by_key(|&(_, off)| off);

    let segments = build_ogg_segments(&points, total, sample_rate);
    tracing::debug!(points = segments.len(), "✅ OGG index built");

    let init_end = if points.len() >= 2 {
        points[1].1.saturating_sub(1)
    } else {
        points.first().map(|&(_, o)| o.saturating_sub(1)).unwrap_or(0)
    };

    Ok(ContainerIndex {
        init_end_byte: init_end,
        inner: Inner::Segments(segments),
    })
}

// ==================== Internal helpers ====================

/// Output of [`collect_probe_points`].
struct OggProbeResult {
    /// `(granule, byte_offset)` pairs found within the probe buffer.
    points: Vec<(u64, u64)>,
    /// `(start, end)` byte-range windows that must be fetched remotely.
    fetch_positions: Vec<(u64, u64)>,
}

/// Collects seek points that are already resident in `probe` and returns the byte positions
/// that need to be fetched for the remaining seek points.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the OGG stream.
/// * `total` - Total stream size in bytes.
///
/// # Returns
///
/// An [`OggProbeResult`] containing in-probe granule/offset pairs and the byte-range
/// windows that must be fetched remotely.
fn collect_probe_points(probe: &[u8], total: u64) -> OggProbeResult {
    let mut points: Vec<(u64, u64)> = Vec::new();
    let mut fetch_positions: Vec<(u64, u64)> = Vec::new();

    // Always include the first page granule (filter u64::MAX which is the OGG convention for -1).
    if let Some((granule, _)) = read_page_granule(probe, 0)
        && granule != u64::MAX
    {
        points.push((granule, 0));
    }

    for i in 1..SEEK_POINTS {
        let byte_pos = i * total / SEEK_POINTS;
        if byte_pos < probe.len() as u64 {
            let slice = &probe[byte_pos as usize..];
            if let Some(sync_off) = find_ogg_sync(slice)
                && let Some((granule, _)) = read_page_granule(slice, sync_off)
                && granule != u64::MAX
            {
                points.push((granule, byte_pos + sync_off as u64));
            }
        } else {
            let window_end = (byte_pos + PROBE_WINDOW).min(total).saturating_sub(1);
            fetch_positions.push((byte_pos, window_end));
        }
    }

    OggProbeResult {
        points,
        fetch_positions,
    }
}

/// Builds a list of `SegmentEntry` values from sorted, deduplicated `(granule, byte_offset)` pairs.
///
/// The last segment's end granule is estimated from the previous inter-point gap when `total_samples`
/// is unknown, so that `end_secs` is never equal to `start_secs`.
fn build_ogg_segments(points: &[(u64, u64)], total: u64, sample_rate: u32) -> Vec<SegmentEntry> {
    let mut segments = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        let (granule, byte_offset) = points[i];
        let start_secs = granule as f64 / sample_rate as f64;
        let (next_granule, next_byte) = if i + 1 < points.len() {
            points[i + 1]
        } else {
            let step = if i > 0 {
                granule.saturating_sub(points[i - 1].0)
            } else {
                0
            };
            (granule.saturating_add(step), total)
        };
        let end_secs = next_granule as f64 / sample_rate as f64;
        segments.push(SegmentEntry {
            start_secs,
            end_secs,
            byte_offset,
            byte_size: next_byte.saturating_sub(byte_offset),
        });
    }
    segments
}

/// Reads the sample rate from the OGG identification page.
///
/// Supported codecs:
/// - Vorbis: magic `\x01vorbis`, sample_rate at bytes 12–15 (LE u32)
/// - Opus: magic `OpusHead`, uses a fixed granule rate of 48000 Hz
/// - FLAC-in-OGG: magic `\x7fFLAC`, sample_rate extracted from STREAMINFO
fn read_sample_rate(data: &[u8]) -> Option<u32> {
    // Skip the OGG page header to reach the packet data.
    let page_payload_off = page_payload_offset(data, 0)?;
    let pkt = &data[page_payload_off..];

    if pkt.len() >= 16 && pkt.starts_with(b"\x01vorbis") {
        let sr = u32::from_le_bytes(pkt[12..16].try_into().ok()?);
        if sr == 0 {
            return None;
        }
        return Some(sr);
    }
    if pkt.len() >= 16 && pkt.starts_with(b"OpusHead") {
        return Some(OPUS_GRANULE_RATE);
    }
    // FLAC-in-OGG: `\x7fFLAC` header followed by STREAMINFO.
    // OGG FLAC mapping: \x7fFLAC(5) + major(1) + minor(1) + num_headers(2) + fLaC(4) + block_header(4)
    // STREAMINFO starts at offset 17; bytes 8–10 of STREAMINFO hold the sample_rate (top 20 bits).
    if pkt.starts_with(b"\x7fFLAC") && pkt.len() >= 21 {
        let streaminfo_start = 17;
        if pkt.len() >= streaminfo_start + 12 {
            let si = &pkt[streaminfo_start..];
            // sample_rate is the top 20 bits of bytes[8..11]
            let sr = ((si[8] as u32) << 12) | ((si[9] as u32) << 4) | ((si[10] as u32) >> 4);
            if sr > 0 {
                return Some(sr);
            }
        }
        // STREAMINFO extraction failed — return None so the caller emits a clear error.
        // A silent 44100 Hz fallback would produce wrong seek positions for other rates.
        return None;
    }

    None
}

/// Returns a descriptive error message for an unrecognised OGG codec.
///
/// Inspects the identification packet magic to name the codec where possible.
fn identify_codec_error(data: &[u8]) -> &'static str {
    let Some(off) = page_payload_offset(data, 0) else {
        return "OGG identification page truncated or missing";
    };
    let pkt = &data[off..];

    if pkt.starts_with(b"\x80theora") {
        return "OGG Theora video streams cannot be seeked by sample position";
    }
    if pkt.starts_with(b"Speex   ") {
        return "OGG Speex codec not supported";
    }
    if pkt.starts_with(b"CELT    ") {
        return "OGG CELT codec not supported";
    }
    if pkt.starts_with(b"\x7fFLAC") {
        return "OGG FLAC-in-OGG: could not extract sample rate from STREAMINFO";
    }

    "OGG identification page: unrecognised codec"
}

/// Returns the byte offset of the first page's data payload.
fn page_payload_offset(data: &[u8], page_start: usize) -> Option<usize> {
    if !data[page_start..].starts_with(OGG_CAPTURE) {
        return None;
    }
    if data.len() < page_start + PAGE_HEADER_MIN {
        return None;
    }
    let n_segs = data[page_start + 26] as usize;
    let header_end = page_start + PAGE_HEADER_MIN + n_segs;
    if data.len() < header_end {
        return None;
    }
    Some(header_end)
}

/// Finds the byte offset of the first OGG capture pattern in `data`.
fn find_ogg_sync(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|w| w == OGG_CAPTURE)
}

/// Reads the granule position and the byte offset past the page from an OGG page at `offset`.
///
/// Returns `(granule_position, next_page_start_offset)`.
fn read_page_granule(data: &[u8], offset: usize) -> Option<(u64, usize)> {
    if data.len() < offset + PAGE_HEADER_MIN {
        return None;
    }
    let page = &data[offset..];
    if !page.starts_with(OGG_CAPTURE) {
        return None;
    }
    // granule_position: bytes 6-13 (little-endian u64)
    let granule = u64::from_le_bytes(page[6..14].try_into().ok()?);
    let n_segs = page[26] as usize;
    let header_len = PAGE_HEADER_MIN + n_segs;
    if data.len() < offset + header_len {
        return None;
    }
    let payload_len: usize = page[27..27 + n_segs].iter().map(|&b| b as usize).sum();
    Some((granule, offset + header_len + payload_len))
}
