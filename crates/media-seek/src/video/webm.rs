//! WebM / Matroska container index parsing via the EBML Cues element.
//!
//! Reads the SeekHead to locate the Cues element, fetches it if it lies outside
//! the probe window, then parses each CuePoint into a `SegmentEntry`.

use crate::RangeFetcher;
use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

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

/// Locates the Segment element and parses its SeekHead and Info.
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
    let (_, seg_sz_len) = read_vint(data, pos)?;
    pos += seg_sz_len;

    let segment_data_start = pos as u64;

    let mut cues_offset: Option<u64> = None;
    let mut timestamp_scale_ns: u64 = 1_000_000; // default 1 ms
    let mut duration_scaled: Option<f64> = None;

    // Walk top-level elements inside Segment until we've found SeekHead and Info
    while pos + 1 < data.len() {
        let (elem_id, id_len) = read_elem_id(data, pos)?;
        pos += id_len;
        let (elem_size, sz_len) = read_vint(data, pos)?;
        pos += sz_len;
        let end = (pos + elem_size as usize).min(data.len());

        match elem_id {
            ID_SEEK_HEAD => {
                cues_offset = parse_seek_head(&data[pos..end]);
            }
            ID_INFO => {
                parse_info(&data[pos..end], &mut timestamp_scale_ns, &mut duration_scaled);
            }
            _ => {}
        }

        pos = end;
        if cues_offset.is_some() && duration_scaled.is_some() {
            break;
        }
    }

    Some(Locations {
        segment_data_start,
        cues_offset,
        timestamp_scale_ns,
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
    let scale_secs = timestamp_scale_ns as f64 / 1_000_000_000.0;
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

        if id == ID_CUE_POINT {
            let mut cue_time: Option<u64> = None;
            let mut cluster_pos: Option<u64> = None;
            let mut inner = pos;
            while inner < end {
                let Some((fid, fl)) = read_elem_id(data, inner) else {
                    break;
                };
                inner += fl;
                let Some((fsz, fsl)) = read_vint(data, inner) else {
                    break;
                };
                inner += fsl;
                let fend = (inner + fsz as usize).min(data.len());
                match fid {
                    ID_CUE_TIME => {
                        cue_time = read_uint(data, inner, fsz as usize);
                    }
                    ID_CUE_TRACK_POSITIONS => {
                        // Parse nested element for CueClusterPosition
                        let mut ni = inner;
                        while ni < fend {
                            let Some((nid, nl)) = read_elem_id(data, ni) else { break };
                            ni += nl;
                            let Some((nsz, nsl)) = read_vint(data, ni) else { break };
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
            if let (Some(t), Some(cp)) = (cue_time, cluster_pos) {
                let abs_offset = segment_data_start + cp;
                let t_secs = t as f64 * scale_secs;
                // Fix the previous entry now that we know where the next cluster starts
                if let Some(prev) = segments.last_mut() {
                    prev.byte_size = abs_offset.saturating_sub(prev.byte_offset);
                    prev.end_secs = t_secs;
                }
                segments.push(SegmentEntry {
                    start_secs: t_secs,
                    end_secs: 0.0, // fixed by next iteration or below
                    byte_offset: abs_offset,
                    byte_size: 0, // fixed by next iteration or below
                });
            }
        }

        pos = end;
    }

    // Fix the final entry
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
        last.end_secs = last.start_secs;
    }

    segments
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
        .ok_or_else(|| Error::parse("no Cues element found in SeekHead"))?;

    // Absolute byte position of the Cues element
    let cues_abs = loc.segment_data_start + cues_offset;

    // Decide whether we need to fetch the Cues data
    let cues_data: Vec<u8>;
    let cues_slice: &[u8];

    if cues_abs as usize + 16 < probe.len() {
        // Cues starts within the probe — try to read the size and see if it's fully contained
        let (_, id_len) = read_elem_id(probe, cues_abs as usize)
            .ok_or_else(|| Error::parse("could not read Cues element ID from probe"))?;
        let (cues_body_size, sz_len) = read_vint(probe, cues_abs as usize + id_len)
            .ok_or_else(|| Error::parse("could not read Cues size from probe"))?;
        let cues_end = cues_abs as usize + id_len + sz_len + cues_body_size as usize;

        if cues_end <= probe.len() {
            // Fully contained — use the probe slice
            cues_slice = &probe[cues_abs as usize + id_len + sz_len..cues_end];
        } else {
            // Partially in probe — fetch the missing tail
            cues_data = fetcher
                .fetch(cues_abs, (cues_end as u64).saturating_sub(1))
                .await
                .map_err(Error::fetch)?;
            let (_, id_len2) =
                read_elem_id(&cues_data, 0).ok_or_else(|| Error::parse("fetched Cues data malformed"))?;
            let (_, sz_len2) =
                read_vint(&cues_data, id_len2).ok_or_else(|| Error::parse("fetched Cues size malformed"))?;
            let body_start = id_len2 + sz_len2;
            cues_slice = if body_start < cues_data.len() {
                &cues_data[body_start..]
            } else {
                &[]
            };
        }
    } else {
        // Cues is beyond the probe — fetch a window starting at the Cues offset.
        // We don't know the Cues size yet, so fetch 256 KB — enough for most long-form
        // videos' Cues table, eliminating a second RTT in the common case.
        const INITIAL_FETCH: u64 = 262_144;
        let header_data = fetcher
            .fetch(cues_abs, cues_abs + INITIAL_FETCH - 1)
            .await
            .map_err(Error::fetch)?;
        let (_, id_len) = read_elem_id(&header_data, 0).ok_or_else(|| Error::parse("fetched Cues header malformed"))?;
        let (cues_body_size, sz_len) =
            read_vint(&header_data, id_len).ok_or_else(|| Error::parse("fetched Cues size malformed"))?;
        let body_start = id_len + sz_len;
        let total_needed = body_start as u64 + cues_body_size;

        if total_needed <= INITIAL_FETCH {
            cues_data = header_data;
            cues_slice = &cues_data[body_start..body_start + cues_body_size as usize];
        } else {
            cues_data = fetcher
                .fetch(cues_abs, cues_abs + total_needed - 1)
                .await
                .map_err(Error::fetch)?;
            cues_slice = &cues_data[body_start..body_start + cues_body_size as usize];
        }
    }

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
