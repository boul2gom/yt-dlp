//! fMP4 / ISO BMFF container index parsing via the SIDX box.
//!
//! Parses the `sidx` (Segment Index Box) from the leading bytes of an fMP4 stream.
//! Each SIDX entry maps directly to one fragment; entries are accumulated into
//! absolute byte offsets and presentation timestamps for `ContainerIndex`.

use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

/// Parses the SIDX box from `probe` and returns a `ContainerIndex`.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the MP4/M4A stream.
///
/// # Errors
///
/// Returns `Error::ParseFailed` when no SIDX box is found or the box is truncated.
pub(crate) fn parse(probe: &[u8]) -> Result<ContainerIndex> {
    tracing::debug!(probe_len = probe.len(), "⚙️ Parsing fMP4/SIDX stream");
    let (sidx, sidx_end) = find_sidx_with_end(probe).ok_or_else(|| Error::parse("no SIDX box found in probe"))?;
    let result = parse_sidx(sidx, sidx_end);
    if let Ok(ref idx) = result
        && let Inner::Segments(ref segs) = idx.inner
    {
        tracing::debug!(segments = segs.len(), "✅ fMP4 SIDX index parsed");
    }
    result
}

/// Locates the `sidx` box within `data` in a single traversal.
///
/// Returns `(body, sidx_end)` where `body` is the box payload (after the 8-byte header)
/// and `sidx_end` is the byte position immediately after the box in `data`.
fn find_sidx_with_end(data: &[u8]) -> Option<(&[u8], usize)> {
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size32 = u32::from_be_bytes(data[pos..pos + 4].try_into().ok()?) as usize;

        // Determine actual box size per ISO 14496-12:
        // size == 1 → 64-bit extended size at offset 8
        // size == 0 → box extends to end of data
        // size < 8  → invalid (unless 0)
        let (header_len, box_size) = if size32 == 1 {
            if pos + 16 > data.len() {
                return None;
            }
            let extended = u64::from_be_bytes(data[pos + 8..pos + 16].try_into().ok()?) as usize;
            (16, extended)
        } else if size32 == 0 {
            (8, data.len() - pos)
        } else if size32 < 8 {
            return None;
        } else {
            (8, size32)
        };

        if &data[pos + 4..pos + 8] == b"sidx" {
            let end = (pos + box_size).min(data.len());
            return Some((&data[pos + header_len..end], pos + box_size));
        }
        pos += box_size;
    }
    None
}

/// Parses a SIDX box payload and returns a `ContainerIndex`.
///
/// The `sidx` slice starts immediately after the 4+4-byte size+type header.
/// SIDX layout (ISO 14496-12 §8.16.3):
///   - 1 byte  version
///   - 3 bytes flags
///   - 4 bytes reference_ID
///   - 4 bytes timescale
///   - version 0: 4+4 bytes earliest_presentation_time + first_offset
///   - version 1: 8+8 bytes earliest_presentation_time + first_offset
///   - 2 bytes reserved
///   - 2 bytes reference_count
///   - reference_count × 12 bytes (each entry)
fn parse_sidx(sidx: &[u8], sidx_end_in_probe: usize) -> Result<ContainerIndex> {
    if sidx.len() < 12 {
        return Err(Error::parse("SIDX box too short for header"));
    }

    let version = sidx[0];
    // Skip flags (3 bytes) + reference_ID (4 bytes) = 7 bytes after version
    let timescale = u32::from_be_bytes(
        sidx[8..12]
            .try_into()
            .map_err(|_| Error::parse("SIDX timescale truncated"))?,
    ) as f64;
    if timescale == 0.0 {
        return Err(Error::parse("SIDX timescale is zero"));
    }

    let (earliest_pts, first_offset, header_end) = if version == 0 {
        if sidx.len() < 20 {
            return Err(Error::parse("SIDX v0 too short"));
        }
        let ept = u32::from_be_bytes(sidx[12..16].try_into().unwrap()) as u64;
        let first_offset = u32::from_be_bytes(sidx[16..20].try_into().unwrap()) as u64;
        (ept, first_offset, 20usize)
    } else {
        if sidx.len() < 28 {
            return Err(Error::parse("SIDX v1 too short"));
        }
        let ept = u64::from_be_bytes(sidx[12..20].try_into().unwrap());
        let first_offset = u64::from_be_bytes(sidx[20..28].try_into().unwrap());
        (ept, first_offset, 28usize)
    };

    if sidx.len() < header_end + 4 {
        return Err(Error::parse("SIDX reference_count truncated"));
    }
    // 2 reserved bytes then 2 bytes reference_count
    let ref_count = u16::from_be_bytes(sidx[header_end + 2..header_end + 4].try_into().unwrap()) as usize;
    let entries_start = header_end + 4;

    if sidx.len() < entries_start + ref_count * 12 {
        return Err(Error::parse("SIDX entries truncated"));
    }

    // sidx_end_in_probe is passed in — content begins immediately after the SIDX box.

    let mut segments = Vec::with_capacity(ref_count);
    let mut current_pts = earliest_pts as f64;
    // first_offset is relative to the byte after the SIDX box
    let mut current_byte = sidx_end_in_probe as u64 + first_offset;

    for i in 0..ref_count {
        let off = entries_start + i * 12;
        let word0 = u32::from_be_bytes(sidx[off..off + 4].try_into().unwrap());
        let subseg_duration = u32::from_be_bytes(sidx[off + 4..off + 8].try_into().unwrap()) as f64;
        let ref_size = (word0 & 0x7FFF_FFFF) as u64; // low 31 bits

        let start_secs = current_pts / timescale;
        let end_secs = (current_pts + subseg_duration) / timescale;

        segments.push(SegmentEntry {
            start_secs,
            end_secs,
            byte_offset: current_byte,
            byte_size: ref_size,
        });

        current_pts += subseg_duration;
        current_byte += ref_size;
    }

    // init segment: everything up to (but not including) the first fragment
    let init_end_byte = segments.first().map(|s| s.byte_offset.saturating_sub(1)).unwrap_or(0);

    Ok(ContainerIndex {
        init_end_byte,
        inner: Inner::Segments(segments),
    })
}
