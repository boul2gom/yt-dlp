//! WebM / Matroska container index parsing via the EBML Cues element.
//!
//! Reads the SeekHead to locate the Cues element, fetches it if it lies outside
//! the probe window, then parses each CuePoint into a `SegmentEntry`.

use crate::RangeFetcher;
use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

/// EBML header magic bytes — the first four bytes of every WebM / Matroska file.
pub(crate) const EBML_MAGIC: &[u8; 4] = &[0x1A, 0x45, 0xDF, 0xA3];
/// Default EBML TimestampScale: 1 ms in nanoseconds.
const DEFAULT_TIMESTAMP_SCALE_NS: u64 = 1_000_000;
/// Nanoseconds per second.
const NS_PER_SEC: f64 = 1_000_000_000.0;
/// Initial fetch size for Cues element when beyond the probe (256 KB).
const INITIAL_CUES_FETCH: u64 = 262_144;

/// EBML unknown-size sentinel for each VINT width (width 1..=8).
///
/// A VINT whose all data bits are 1 signals "unknown size" per the EBML spec.
/// For width W the sentinel value (after stripping the leading marker bit) is
/// `(1 << (7 * W)) - 1`. We only need width 8 in practice (Segment element).
const VINT_UNKNOWN: [u64; 8] = [
    0x7F,
    0x3FFF,
    0x1F_FFFF,
    0x0FFF_FFFF,
    0x07_FFFF_FFFF,
    0x03FF_FFFF_FFFF,
    0x01_FFFF_FFFF_FFFF,
    0x00FF_FFFF_FFFF_FFFF,
];

/// Returns `true` when `(value, width)` represents an EBML unknown-size sentinel.
fn is_vint_unknown(value: u64, width: usize) -> bool {
    (1..=8).contains(&width) && value == VINT_UNKNOWN[width - 1]
}

// EBML element IDs of interest
const ID_EBML: u32 = 0x1A45_DFA3;
const ID_SEGMENT: u32 = 0x1853_8067;
const ID_SEEK_HEAD: u32 = 0x114D_9B74;
const ID_SEEK: u32 = 0x4DBB;
const ID_SEEK_ID: u32 = 0x53AB;
const ID_SEEK_POSITION: u32 = 0x53AC;
const ID_INFO: u32 = 0x1549_A966;
const ID_DURATION: u32 = 0x4489;
const ID_TIMESTAMP_SCALE: u32 = 0x002A_D7B1;
const ID_CUES: u32 = 0x1C53_BB6B;
const ID_CUE_POINT: u32 = 0xBB;
const ID_CUE_TIME: u32 = 0xB3;
const ID_CUE_TRACK_POSITIONS: u32 = 0xB7;
const ID_CUE_CLUSTER_POSITION: u32 = 0xF1;

/// Reads a variable-length EBML integer (VINT) from `data[pos..]`.
///
/// Returns `(value, bytes_consumed)` or `None` if the data is too short.
/// The caller can use [`is_vint_unknown`] with `(value, bytes_consumed)` to detect
/// the "unknown size" sentinel.
fn read_vint(data: &[u8], pos: usize) -> Option<(u64, usize)> {
    if pos >= data.len() {
        return None;
    }
    let first = data[pos];
    let width = first.leading_zeros() as usize + 1;
    if width > 8 || pos + width > data.len() {
        return None;
    }
    let mask = (1u64 << (8 - width)) - 1;
    let mut value = (first as u64) & mask;
    for &b in &data[pos + 1..pos + width] {
        value = (value << 8) | b as u64;
    }
    Some((value, width))
}

/// Reads an EBML element ID (up to 4 bytes) from `data[pos..]`.
///
/// Returns `(id, bytes_consumed)` or `None` if the data is too short.
fn read_elem_id(data: &[u8], pos: usize) -> Option<(u32, usize)> {
    if pos >= data.len() {
        return None;
    }
    let first = data[pos];
    let width = first.leading_zeros() as usize + 1;
    if width > 4 || pos + width > data.len() {
        return None;
    }
    let mut id = first as u32;
    for &b in &data[pos + 1..pos + width] {
        id = (id << 8) | b as u32;
    }
    Some((id, width))
}

/// Reads a big-endian unsigned integer of `size` bytes from `data[pos..]`.
fn read_uint(data: &[u8], pos: usize, size: usize) -> Option<u64> {
    if size == 0 || size > 8 || pos + size > data.len() {
        return None;
    }
    let mut v = 0u64;
    for &b in &data[pos..pos + size] {
        v = (v << 8) | b as u64;
    }
    Some(v)
}

/// Reads a big-endian IEEE 754 float (`size` = 4 or 8) from `data[pos..]`.
fn read_float(data: &[u8], pos: usize, size: usize) -> Option<f64> {
    match size {
        4 => {
            let bits = u32::from_be_bytes(data[pos..pos + 4].try_into().ok()?);
            Some(f32::from_bits(bits) as f64)
        }
        8 => {
            let bits = u64::from_be_bytes(data[pos..pos + 8].try_into().ok()?);
            Some(f64::from_bits(bits))
        }
        _ => None,
    }
}

/// Segment-relative byte offset of the Cues element found in the SeekHead, and
/// the segment data offset (first byte of the segment body relative to the stream start).
struct Locations {
    segment_data_start: u64,
    cues_offset: Option<u64>,
    timestamp_scale_ns: u64,
}

/// Result of walking top-level Segment elements.
struct SegmentWalkResult {
    cues_offset: Option<u64>,
    timestamp_scale_ns: u64,
}

/// Walks top-level elements inside a Segment body until SeekHead+Info are found.
///
/// `duration_scaled` is tracked internally for the early-exit heuristic but is not returned.
fn walk_segment_elements(data: &[u8], start: usize, segment_end: usize) -> SegmentWalkResult {
    let mut pos = start;
    let mut cues_offset: Option<u64> = None;
    let mut timestamp_scale_ns: u64 = DEFAULT_TIMESTAMP_SCALE_NS;
    let mut duration_scaled: Option<f64> = None;

    while pos + 1 < segment_end {
        let Some((elem_id, id_len)) = read_elem_id(data, pos) else {
            break;
        };
        pos += id_len;
        let Some((elem_size, sz_len)) = read_vint(data, pos) else {
            break;
        };
        pos += sz_len;

        // Guard against unknown-size child elements: treat as extending to segment end.
        let elem_body_end = if is_vint_unknown(elem_size, sz_len) {
            segment_end
        } else {
            (pos + elem_size as usize).min(segment_end)
        };
        match elem_id {
            ID_SEEK_HEAD => {
                cues_offset = parse_seek_head(&data[pos..elem_body_end]);
            }
            ID_INFO => {
                parse_info(&data[pos..elem_body_end], &mut timestamp_scale_ns, &mut duration_scaled);
            }
            _ => {}
        }
        pos = elem_body_end;
        if cues_offset.is_some() && duration_scaled.is_some() {
            break;
        }
    }
    SegmentWalkResult {
        cues_offset,
        timestamp_scale_ns,
    }
}

/// Linearly scans the probe for the Cues element ID when no SeekHead was found.
fn probe_scan_for_cues(data: &[u8], segment_data_start: u64) -> Option<u64> {
    let cues_id_bytes: [u8; 4] = [
        ((ID_CUES >> 24) & 0xFF) as u8,
        ((ID_CUES >> 16) & 0xFF) as u8,
        ((ID_CUES >> 8) & 0xFF) as u8,
        (ID_CUES & 0xFF) as u8,
    ];
    let search_start = segment_data_start as usize;
    if search_start < data.len()
        && let Some(rel) = data[search_start..].windows(4).position(|w| w == cues_id_bytes)
    {
        let abs_pos = search_start + rel;
        tracing::debug!(cues_abs = abs_pos, "⚙️ WebM Cues found by probe scan (no SeekHead)");
        Some((abs_pos as u64).saturating_sub(segment_data_start))
    } else {
        None
    }
}

/// Locates the Segment element and parses its SeekHead and Info.
///
/// Handles the EBML "unknown size" sentinel for the Segment element (common in
/// streaming WebM) and falls back to a linear probe scan for the Cues element
/// when no SeekHead is present or SeekHead does not reference Cues.
fn locate_segment(data: &[u8]) -> Option<Locations> {
    let mut pos = 0usize;

    // Skip the EBML header
    let (id, id_len) = read_elem_id(data, pos)?;
    if id != ID_EBML {
        return None;
    }
    pos += id_len;
    let (ebml_size, sz_len) = read_vint(data, pos)?;
    pos += sz_len + ebml_size as usize;

    // Now at the Segment element
    if pos >= data.len() {
        return None;
    }
    let (seg_id, seg_id_len) = read_elem_id(data, pos)?;
    if seg_id != ID_SEGMENT {
        return None;
    }
    pos += seg_id_len;
    let (seg_size, seg_sz_len) = read_vint(data, pos)?;
    pos += seg_sz_len;

    // Determine how far the segment body extends in the probe.
    let segment_end = if is_vint_unknown(seg_size, seg_sz_len) {
        data.len()
    } else {
        (pos + seg_size as usize).min(data.len())
    };

    let segment_data_start = pos as u64;
    let walked = walk_segment_elements(data, pos, segment_end);
    let cues_offset = walked
        .cues_offset
        .or_else(|| probe_scan_for_cues(data, segment_data_start));

    Some(Locations {
        segment_data_start,
        cues_offset,
        timestamp_scale_ns: walked.timestamp_scale_ns,
    })
}

/// Parses a SeekHead element and returns the segment-relative position of the Cues element.
fn parse_seek_head(data: &[u8]) -> Option<u64> {
    let mut pos = 0usize;
    while pos < data.len() {
        let (id, id_len) = read_elem_id(data, pos)?;
        pos += id_len;
        let (size, sz_len) = read_vint(data, pos)?;
        pos += sz_len;
        let end = (pos + size as usize).min(data.len());

        if id == ID_SEEK {
            let mut seek_id: Option<u32> = None;
            let mut seek_pos: Option<u64> = None;
            let mut inner = pos;
            while inner < end {
                let (fid, fl) = read_elem_id(data, inner)?;
                inner += fl;
                let (fsz, fsl) = read_vint(data, inner)?;
                inner += fsl;
                let fend = (inner + fsz as usize).min(data.len());
                match fid {
                    ID_SEEK_ID => {
                        // Stored as binary — read as big-endian integer
                        seek_id = read_uint(data, inner, fsz as usize).map(|v| v as u32);
                    }
                    ID_SEEK_POSITION => {
                        seek_pos = read_uint(data, inner, fsz as usize);
                    }
                    _ => {}
                }
                inner = fend;
            }
            if seek_id == Some(ID_CUES)
                && let Some(p) = seek_pos
            {
                return Some(p);
            }
        }

        pos = end;
    }
    None
}

/// Parses an Info element to extract TimestampScale and Duration.
fn parse_info(data: &[u8], scale: &mut u64, duration: &mut Option<f64>) {
    let mut pos = 0usize;
    while pos < data.len() {
        let Some((id, id_len)) = read_elem_id(data, pos) else {
            break;
        };
        pos += id_len;
        let Some((size, sz_len)) = read_vint(data, pos) else {
            break;
        };
        pos += sz_len;
        let end = (pos + size as usize).min(data.len());
        match id {
            ID_TIMESTAMP_SCALE => {
                if let Some(v) = read_uint(data, pos, size as usize) {
                    *scale = v;
                }
            }
            ID_DURATION => {
                *duration = read_float(data, pos, size as usize);
            }
            _ => {}
        }
        pos = end;
    }
}

/// A parsed CuePoint: timestamp (in raw EBML ticks) and segment-relative cluster offset.
#[derive(Debug, Clone, Copy)]
struct CuePoint {
    time: u64,
    cluster_pos: u64,
}

/// Parses one CuePoint element body `data[pos..end]` into a `CuePoint`.
///
/// Handles nested `ID_CUE_TRACK_POSITIONS` to extract `ID_CUE_CLUSTER_POSITION`.
/// Returns `None` if either mandatory field is absent or data is truncated.
fn parse_cue_point(data: &[u8], pos: usize, end: usize) -> Option<CuePoint> {
    let mut cue_time: Option<u64> = None;
    let mut cluster_pos: Option<u64> = None;
    let mut inner = pos;
    while inner < end {
        let (fid, fl) = read_elem_id(data, inner)?;
        inner += fl;
        let (fsz, fsl) = read_vint(data, inner)?;
        inner += fsl;
        let fend = (inner + fsz as usize).min(data.len());
        match fid {
            ID_CUE_TIME => {
                cue_time = read_uint(data, inner, fsz as usize);
            }
            ID_CUE_TRACK_POSITIONS => {
                let mut ni = inner;
                while ni < fend {
                    let (nid, nl) = read_elem_id(data, ni)?;
                    ni += nl;
                    let (nsz, nsl) = read_vint(data, ni)?;
                    ni += nsl;
                    let nend = (ni + nsz as usize).min(data.len());
                    if nid == ID_CUE_CLUSTER_POSITION {
                        cluster_pos = read_uint(data, ni, nsz as usize);
                    }
                    ni = nend;
                }
            }
            _ => {}
        }
        inner = fend;
    }
    match (cue_time, cluster_pos) {
        (Some(time), Some(cluster_pos)) => Some(CuePoint { time, cluster_pos }),
        _ => None,
    }
}

/// Applies a decoded cue point to `segments`: fixes the previous entry's `byte_size`
/// and `end_secs`, then pushes a new open entry.
fn apply_cue_point(segments: &mut Vec<SegmentEntry>, abs_offset: u64, t_secs: f64) {
    if let Some(prev) = segments.last_mut() {
        prev.byte_size = abs_offset.saturating_sub(prev.byte_offset);
        prev.end_secs = t_secs;
    }
    segments.push(SegmentEntry {
        start_secs: t_secs,
        end_secs: 0.0, // fixed by next iteration or fixup_last_cue_segment
        byte_offset: abs_offset,
        byte_size: 0, // fixed by next iteration or fixup_last_cue_segment
    });
}

/// Fixes the final `SegmentEntry` once all cue points have been processed.
///
/// Uses average cluster duration to estimate the last segment's end time.
fn fixup_last_cue_segment(segments: &mut [SegmentEntry], total_size: Option<u64>) {
    let seg_count = segments.len();
    if seg_count >= 2 {
        let first_start = segments[0].start_secs;
        if let Some(last) = segments.last_mut() {
            let total = total_size.unwrap_or(last.byte_offset);
            last.byte_size = total.saturating_sub(last.byte_offset);
            let avg_dur = (last.start_secs - first_start) / (seg_count - 1) as f64;
            last.end_secs = last.start_secs + avg_dur;
        }
    } else if let Some(last) = segments.last_mut() {
        let total = total_size.unwrap_or(last.byte_offset);
        last.byte_size = total.saturating_sub(last.byte_offset);
        // Estimate duration from byte_size to avoid zero-duration single-segment files.
        let estimated_secs = if last.byte_size > 0 && total > 0 {
            last.start_secs + (last.byte_size as f64 / total as f64) * last.start_secs.max(1.0)
        } else {
            last.start_secs + 1.0
        };
        last.end_secs = estimated_secs;
    }
}

/// Parses the Cues element and returns `Vec<SegmentEntry>`.
///
/// Builds the segment list in a single pass: each new CuePoint fixes the previous
/// entry's `byte_size` and `end_secs` in-place before appending the new entry.
fn parse_cues(
    data: &[u8],
    segment_data_start: u64,
    timestamp_scale_ns: u64,
    total_size: Option<u64>,
) -> Vec<SegmentEntry> {
    let scale_secs = timestamp_scale_ns as f64 / NS_PER_SEC;
    let mut segments: Vec<SegmentEntry> = Vec::new();

    let mut pos = 0usize;
    while pos < data.len() {
        let Some((id, id_len)) = read_elem_id(data, pos) else {
            break;
        };
        pos += id_len;
        let Some((size, sz_len)) = read_vint(data, pos) else {
            break;
        };
        pos += sz_len;
        let end = (pos + size as usize).min(data.len());

        if id == ID_CUE_POINT
            && let Some(cp) = parse_cue_point(data, pos, end)
        {
            let abs_offset = segment_data_start + cp.cluster_pos;
            let t_secs = cp.time as f64 * scale_secs;
            apply_cue_point(&mut segments, abs_offset, t_secs);
        }

        pos = end;
    }

    fixup_last_cue_segment(&mut segments, total_size);
    segments
}

/// Fetches the Cues element body, returning `(buffer, body_start_offset)`.
///
/// Three strategies are tried in order:
/// 1. Cues is fully within `probe` — copies the relevant slice.
/// 2. Cues starts within `probe` but extends past it — fetches the full extent.
/// 3. Cues is entirely beyond `probe` — fetches an initial window (`INITIAL_CUES_FETCH`),
///    then a second fetch only if the Cues body does not fit in the first window.
///
/// # Arguments
///
/// * `probe` - The initial probe buffer.
/// * `cues_abs` - Absolute stream byte offset of the Cues element start.
/// * `fetcher` - Used to fetch ranges beyond or partly beyond the probe.
///
/// # Errors
///
/// Returns `Error::ParseFailed` when the Cues header is malformed.
/// Returns `Error::FetchFailed` on a failed Range request.
async fn fetch_cues_body<F: RangeFetcher>(probe: &[u8], cues_abs: u64, fetcher: &F) -> Result<(Vec<u8>, usize)> {
    if cues_abs as usize + 16 < probe.len() {
        // Cues starts within the probe — read its header and check if body is fully contained.
        let (_, id_len) = read_elem_id(probe, cues_abs as usize)
            .ok_or_else(|| Error::parse("could not read Cues element ID from probe"))?;
        let (cues_body_size, sz_len) = read_vint(probe, cues_abs as usize + id_len)
            .ok_or_else(|| Error::parse("could not read Cues size from probe"))?;
        let body_start = id_len + sz_len;
        let cues_end = cues_abs as usize + body_start + cues_body_size as usize;

        if cues_end <= probe.len() {
            // Fully contained — copy the probe slice into a Vec.
            return Ok((probe[cues_abs as usize..cues_end].to_vec(), body_start));
        }

        // Partially in probe — fetch the full extent.
        let buf = fetcher
            .fetch(cues_abs, (cues_end as u64).saturating_sub(1))
            .await
            .map_err(Error::fetch)?;
        let (_, id_len2) = read_elem_id(&buf, 0).ok_or_else(|| Error::parse("fetched Cues data malformed"))?;
        let (_, sz_len2) = read_vint(&buf, id_len2).ok_or_else(|| Error::parse("fetched Cues size malformed"))?;
        return Ok((buf, id_len2 + sz_len2));
    }

    // Cues is entirely beyond the probe — fetch a 256 KB window first.
    let header_data = fetcher
        .fetch(cues_abs, cues_abs + INITIAL_CUES_FETCH - 1)
        .await
        .map_err(Error::fetch)?;
    let (_, id_len) = read_elem_id(&header_data, 0).ok_or_else(|| Error::parse("fetched Cues header malformed"))?;
    let (cues_body_size, sz_len) =
        read_vint(&header_data, id_len).ok_or_else(|| Error::parse("fetched Cues size malformed"))?;
    let body_start = id_len + sz_len;
    let total_needed = body_start as u64 + cues_body_size;

    if total_needed <= INITIAL_CUES_FETCH {
        return Ok((header_data, body_start));
    }

    // Initial fetch was too small — fetch the complete Cues element.
    let buf = fetcher
        .fetch(cues_abs, cues_abs + total_needed - 1)
        .await
        .map_err(Error::fetch)?;
    Ok((buf, body_start))
}

/// Parses a WebM/Matroska stream and returns a `ContainerIndex`.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the WebM stream.
/// * `total_size` - Total stream size in bytes, used to estimate the last cluster's extent.
/// * `fetcher` - Provides additional byte ranges when the Cues element lies outside `probe`.
///
/// # Errors
///
/// Returns `Error::ParseFailed` when no Cues element is found, or `Error::FetchFailed`
/// when an extra Range request fails.
pub(crate) async fn parse<F>(probe: &[u8], total_size: Option<u64>, fetcher: &F) -> Result<ContainerIndex>
where
    F: RangeFetcher,
{
    tracing::debug!(probe_len = probe.len(), total_size = ?total_size, "⚙️ Parsing WebM/Matroska stream");

    let loc = locate_segment(probe).ok_or_else(|| Error::parse("could not locate Segment element"))?;

    let cues_offset = loc
        .cues_offset
        .ok_or_else(|| Error::parse("Cues element not found in SeekHead or probe scan"))?;

    // Absolute byte position of the Cues element
    let cues_abs = loc.segment_data_start + cues_offset;

    let (cues_buf, cues_body_start) = fetch_cues_body(probe, cues_abs, fetcher).await?;
    let cues_slice = if cues_body_start <= cues_buf.len() {
        &cues_buf[cues_body_start..]
    } else {
        &[]
    };

    let segments = parse_cues(cues_slice, loc.segment_data_start, loc.timestamp_scale_ns, total_size);
    if segments.is_empty() {
        return Err(Error::parse("Cues element contained no CuePoints"));
    }

    // init_end_byte: the last byte before the first cluster referenced in Cues
    let init_end_byte = segments.first().map(|s| s.byte_offset.saturating_sub(1)).unwrap_or(0);

    tracing::debug!(segments = segments.len(), "✅ WebM index parsed");
    Ok(ContainerIndex {
        init_end_byte,
        inner: Inner::Segments(segments),
    })
}
