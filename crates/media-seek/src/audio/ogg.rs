//! OGG container index parsing via page granule position binary search.
//!
//! An OGG stream is a sequence of 28-byte pages. The granule position in each
//! page header encodes the decoded sample count up to that page. This module
//! binary-searches for page boundaries corresponding to the requested timestamps
//! by fetching ranges from the stream.

use crate::RangeFetcher;
use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

/// OGG page capture pattern.
const OGG_CAPTURE: &[u8; 4] = b"OggS";
/// Minimum OGG page header size (bytes).
const PAGE_HEADER_MIN: usize = 27;

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

    // Build a coarse index: sample up to 64 equally spaced byte positions and
    // scan forward to find the next OGG page, reading its granule position.
    const SEEK_POINTS: u64 = 64;
    const WINDOW: u64 = 8192; // bytes to fetch per probe

    let mut points: Vec<(u64, u64)> = Vec::new(); // (granule, byte_offset)

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
        let window_end = (byte_pos + WINDOW).min(total);

        let chunk = fetcher.fetch(byte_pos, window_end).await.map_err(Error::fetch)?;

        // Scan the chunk for the first OGG page sync pattern
        if let Some(sync_off) = find_ogg_sync(&chunk)
            && let Some((granule, _)) = read_page_granule(&chunk, sync_off)
            && granule != u64::MAX
        {
            // granule 0xFFFF_FFFF_FFFF_FFFF means "no packets complete on this page"
            points.push((granule, byte_pos + sync_off as u64));
        }
    }

    if points.is_empty() {
        return Err(Error::parse("no OGG seek points found"));
    }

    // Deduplicate and sort by byte offset
    points.sort_unstable_by_key(|&(_, off)| off);
    points.dedup_by_key(|&mut (granule, _)| granule);

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
        let sr = u32::from_le_bytes(pkt[12..16].try_into().ok()?);
        return Some(sr);
    }
    // FLAC-in-OGG: `\x7fFLAC` — sample rate encoded differently; use 44100 as safe default
    if pkt.starts_with(b"\x7fFLAC") {
        return Some(44100);
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
