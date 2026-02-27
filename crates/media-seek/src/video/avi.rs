//! AVI (RIFF AVI) container index parsing via the `idx1` chunk.
//!
//! The legacy AVI index (`idx1`) is stored at the end of the file and contains
//! one 16-byte entry per chunk (video frame or audio block). This module fetches
//! the last 64 KB of the stream via a Range request to locate and parse `idx1`.

use crate::RangeFetcher;
use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

/// Window fetched from the tail of the file to find the `idx1` chunk.
const TAIL_WINDOW: u64 = 65536;

/// AVI index entry flags.
const AVIIF_KEYFRAME: u32 = 0x0000_0010;

/// Parses an AVI stream and returns a `ContainerIndex`.
///
/// Fetches the last `64 KB` of the stream to locate the `idx1` chunk, then
/// builds a coarse keyframe-only index suitable for seeking.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the AVI stream (used to read the `avih` header).
/// * `total_size` - Total file size in bytes (required for the tail Range fetch).
/// * `fetcher` - Provides the tail bytes containing `idx1`.
///
/// # Errors
///
/// Returns `Error::ParseFailed` when the AVI header is malformed or `idx1` is
/// not found, and `Error::FetchFailed` on Range request failure.
pub(crate) async fn parse<F>(probe: &[u8], total_size: Option<u64>, fetcher: &F) -> Result<ContainerIndex>
where
    F: RangeFetcher,
{
    tracing::debug!(probe_len = probe.len(), total_size = ?total_size, "⚙️ Parsing AVI stream");
    let total = total_size.ok_or_else(|| Error::parse("AVI idx1 parsing requires total_size"))?;

    let fps = read_fps_from_avih(probe);

    let tail_start = total.saturating_sub(TAIL_WINDOW);
    let tail = fetcher
        .fetch(tail_start, total.saturating_sub(1))
        .await
        .map_err(Error::fetch)?;

    let idx1 = find_idx1(&tail).ok_or_else(|| Error::parse("idx1 chunk not found in AVI tail"))?;

    let result = parse_idx1(idx1, fps, tail_start, probe);
    if let Ok(ref idx) = result
        && let Inner::Segments(ref segs) = idx.inner
    {
        tracing::debug!(keyframes = segs.len(), "✅ AVI index parsed");
    }
    result
}

/// Reads the frame rate from the `avih` (AVI main header) chunk in `probe`.
///
/// Returns `None` if the header is not found or the microseconds-per-frame field is zero.
fn read_fps_from_avih(probe: &[u8]) -> Option<f64> {
    // RIFF header: "RIFF" (4) + size (4) + "AVI " (4) = 12
    // Then a "LIST" "hdrl" chunk contains "avih"
    let mut pos = 12usize;
    while pos + 8 <= probe.len() {
        let chunk_id = &probe[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(probe[pos + 4..pos + 8].try_into().ok()?) as usize;
        pos += 8;
        if chunk_id == b"LIST" {
            // The next 4 bytes are the list type
            let list_type = &probe[pos..pos + 4];
            if list_type == b"hdrl" {
                // Search for avih inside hdrl
                let mut inner = pos + 4;
                let hdrl_end = pos + chunk_size;
                while inner + 8 <= probe.len() && inner < hdrl_end {
                    let id = &probe[inner..inner + 4];
                    let sz = u32::from_le_bytes(probe[inner + 4..inner + 8].try_into().ok()?) as usize;
                    inner += 8;
                    if id == b"avih" && sz >= 8 {
                        // dwMicroSecPerFrame is the first 4 bytes of avih data
                        let us_per_frame = u32::from_le_bytes(probe[inner..inner + 4].try_into().ok()?) as f64;
                        if us_per_frame > 0.0 {
                            return Some(1_000_000.0 / us_per_frame);
                        }
                    }
                    inner += sz;
                }
            }
        }
        pos += chunk_size;
    }
    None
}

/// Locates the `idx1` chunk within `data` and returns its payload slice.
fn find_idx1(data: &[u8]) -> Option<&[u8]> {
    // Scan for "idx1" tag
    let tag = b"idx1";
    let limit = data.len().saturating_sub(8);
    for i in 0..limit {
        if &data[i..i + 4] == tag {
            let size = u32::from_le_bytes(data[i + 4..i + 8].try_into().ok()?) as usize;
            let end = (i + 8 + size).min(data.len());
            return Some(&data[i + 8..end]);
        }
    }
    None
}

/// Parses the `idx1` payload and returns a `ContainerIndex` built from keyframe entries.
///
/// `idx1` entries are 16 bytes each:
/// - 4 bytes chunk_id (e.g. "00dc" for video frame 0)
/// - 4 bytes flags (AVIIF_KEYFRAME = 0x10)
/// - 4 bytes chunk_offset (relative to movi list start)
/// - 4 bytes chunk_size
fn parse_idx1(idx1: &[u8], fps: Option<f64>, tail_start: u64, probe: &[u8]) -> Result<ContainerIndex> {
    // Locate the movi list start byte in the file to convert idx1 offsets to absolute positions.
    let movi_start = find_movi_start(probe).unwrap_or(0);

    let n_entries = idx1.len() / 16;
    let mut keyframes: Vec<(u64, u64)> = Vec::new(); // (frame_index, byte_offset_in_movi)
    let mut frame_index = 0u64;

    for i in 0..n_entries {
        let off = i * 16;
        if off + 16 > idx1.len() {
            break;
        }
        let chunk_id = &idx1[off..off + 4];
        let flags = u32::from_le_bytes(idx1[off + 4..off + 8].try_into().unwrap());
        let chunk_offset = u32::from_le_bytes(idx1[off + 8..off + 12].try_into().unwrap()) as u64;

        // Only count video frames (chunk_id typically "00dc" or "00db")
        let is_video = chunk_id.len() == 4 && (chunk_id[2] == b'd') && (chunk_id[3] == b'b' || chunk_id[3] == b'c');
        if is_video {
            if flags & AVIIF_KEYFRAME != 0 {
                keyframes.push((frame_index, movi_start + chunk_offset));
            }
            frame_index += 1;
        }
    }

    if keyframes.is_empty() {
        return Err(Error::parse("idx1 contains no video keyframes"));
    }

    let fps_val = fps.unwrap_or(25.0);
    let mut segments = Vec::with_capacity(keyframes.len());
    for i in 0..keyframes.len() {
        let (fidx, byte_offset) = keyframes[i];
        let start_secs = fidx as f64 / fps_val;
        let (next_fidx, next_byte) = if i + 1 < keyframes.len() {
            keyframes[i + 1]
        } else {
            (fidx, tail_start) // approximate
        };
        let end_secs = next_fidx as f64 / fps_val;
        segments.push(SegmentEntry {
            start_secs,
            end_secs,
            byte_offset,
            byte_size: next_byte.saturating_sub(byte_offset),
        });
    }

    let init_end_byte = segments.first().map(|s| s.byte_offset.saturating_sub(1)).unwrap_or(0);

    Ok(ContainerIndex {
        init_end_byte,
        inner: Inner::Segments(segments),
    })
}

/// Finds the absolute byte offset of the `movi` list data start in the file.
fn find_movi_start(probe: &[u8]) -> Option<u64> {
    let mut pos = 12usize;
    while pos + 8 <= probe.len() {
        let id = &probe[pos..pos + 4];
        let size = u32::from_le_bytes(probe[pos + 4..pos + 8].try_into().ok()?) as usize;
        pos += 8;
        if id == b"LIST" && pos + 4 <= probe.len() && &probe[pos..pos + 4] == b"movi" {
            return Some(pos as u64 + 4); // first byte after "movi" four-CC
        }
        pos += size;
    }
    None
}
