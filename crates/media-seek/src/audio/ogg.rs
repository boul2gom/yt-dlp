//! OGG container index parsing via page granule position binary search.
//!
//! An OGG stream is a sequence of 28-byte pages. The granule position in each
//! page header encodes the decoded sample count up to that page. This module
//! binary-searches for page boundaries corresponding to the requested timestamps
//! by fetching ranges from the stream.

use futures_util::future::try_join_all;

use crate::RangeFetcher;
use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

/// OGG page capture pattern.
const OGG_CAPTURE: &[u8; 4] = b"OggS";
/// Minimum OGG page header size (bytes).
const PAGE_HEADER_MIN: usize = 27;
/// Opus granule positions are always in 48 kHz units regardless of input sample rate.
const OPUS_GRANULE_RATE: u32 = 48000;
/// Fallback sample rate for FLAC-in-OGG when extraction fails.
const FLAC_FALLBACK_RATE: u32 = 44100;
/// Number of evenly-spaced seek point probes across the stream.
const SEEK_POINTS: u64 = 64;
/// Bytes fetched per seek-point probe window.
const PROBE_WINDOW: u64 = 8192;

/// Parses an OGG stream and returns a `ContainerIndex`.
///
/// Reads the identification page from `probe` to extract the codec sample rate,
/// then binary-searches the stream via `fetcher` to build a page-boundary index
/// covering at most 64 evenly-spaced seek points.
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
    let sample_rate =
        read_sample_rate(probe).ok_or_else(|| Error::parse("could not read OGG identification page sample rate"))?;

    let total = total_size.ok_or_else(|| Error::parse("OGG binary search requires total_size"))?;

    // Build a coarse index: sample up to SEEK_POINTS equally spaced byte positions and
    // scan forward to find the next OGG page, reading its granule position.

    let mut points: Vec<(u64, u64)> = Vec::new(); // (granule, byte_offset)
    let mut fetch_positions: Vec<(u64, u64)> = Vec::new(); // (byte_pos, window_end)

    // Always include the first page granule
    if let Some((granule, page_end)) = read_page_granule(probe, 0) {
        points.push((granule, 0));
        // Second page (comment header then first audio page)
        if let Some((g2, _)) = read_page_granule(probe, page_end) {
            let _ = g2; // comment header — skip
        }
    }

    for i in 1..SEEK_POINTS {
        let byte_pos = i * total / SEEK_POINTS;
        let window_end = (byte_pos + PROBE_WINDOW).min(total).saturating_sub(1);

        fetch_positions.push((byte_pos, window_end));
    }

    // Fetch all seek-point windows in parallel
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

    if points.is_empty() {
        return Err(Error::parse("no OGG seek points found"));
    }

    // Deduplicate by granule (same time position seen from different probes),
    // then sort by byte offset for the final index.
    points.sort_unstable_by_key(|&(granule, _)| granule);
    points.dedup_by_key(|&mut (granule, _)| granule);
    points.sort_unstable_by_key(|&(_, off)| off);

    let mut segments = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        let (granule, byte_offset) = points[i];
        let start_secs = granule as f64 / sample_rate as f64;
        let (next_granule, next_byte) = if i + 1 < points.len() {
            points[i + 1]
        } else {
            (granule, total)
        };
        let end_secs = next_granule as f64 / sample_rate as f64;
        segments.push(SegmentEntry {
            start_secs,
            end_secs,
            byte_offset,
            byte_size: next_byte.saturating_sub(byte_offset),
        });
    }

    tracing::debug!(points = segments.len(), "✅ OGG index built");
    Ok(ContainerIndex {
        init_end_byte: points.first().map(|&(_, o)| o.saturating_sub(1)).unwrap_or(0),
        inner: Inner::Segments(segments),
    })
}

/// Reads the sample rate from the OGG identification page.
///
/// The first page in a Vorbis or Opus stream is the identification page.
/// - Vorbis: magic `\x01vorbis`, sample_rate at bytes 12-15 (LE u32)
/// - Opus: magic `OpusHead`, input_sample_rate at bytes 12-15 (LE u32, nominally 48000)
fn read_sample_rate(data: &[u8]) -> Option<u32> {
    // Skip the OGG page header to reach the packet data
    let page_payload_off = page_payload_offset(data, 0)?;
    let pkt = &data[page_payload_off..];

    if pkt.len() >= 16 && pkt.starts_with(b"\x01vorbis") {
        let sr = u32::from_le_bytes(pkt[12..16].try_into().ok()?);
        return Some(sr);
    }
    if pkt.len() >= 16 && pkt.starts_with(b"OpusHead") {
        return Some(OPUS_GRANULE_RATE);
    }
    // FLAC-in-OGG: `\x7fFLAC` header followed by STREAMINFO
    // OGG FLAC mapping: \x7fFLAC + version(2) + num_headers(2) + fLaC(4) + block_header(4) + STREAMINFO
    // STREAMINFO has sample rate at bytes 10-12 (20 bits starting at bit 80)
    if pkt.starts_with(b"\x7fFLAC") && pkt.len() >= 21 {
        // Skip: \x7fFLAC(5) + major(1) + minor(1) + num_headers(2) + fLaC(4) + block_header(4) = 17
        // STREAMINFO starts at offset 17; sample rate is at bytes 10-12 of STREAMINFO (offset 27)
        // But first 4 bytes of STREAMINFO are min/max block size, next 3+3 are min/max frame size
        // Then bytes 8-11 contain: sample_rate(20 bits) + channels(3 bits) + bps(5 bits) + ...
        let streaminfo_start = 17; // after \x7fFLAC(5) + version(2) + num_headers(2) + fLaC(4) + block_header(4)
        if pkt.len() >= streaminfo_start + 12 {
            let si = &pkt[streaminfo_start..];
            // Bytes 8-10 of STREAMINFO: sample_rate is top 20 bits of bytes[8..11]
            let sr = ((si[8] as u32) << 12) | ((si[9] as u32) << 4) | ((si[10] as u32) >> 4);
            if sr > 0 {
                return Some(sr);
            }
        }
        return Some(FLAC_FALLBACK_RATE);
    }
    None
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
