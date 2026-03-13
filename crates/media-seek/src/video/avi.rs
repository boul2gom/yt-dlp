//! AVI (RIFF AVI) container index parsing via the `idx1` chunk or OpenDML `indx`.
//!
//! The legacy AVI index (`idx1`) is stored at the end of the file and contains
//! one 16-byte entry per chunk (video frame or audio block). This module fetches
//! the last 64 KB of the stream via a Range request to locate and parse `idx1`.
//!
//! For audio-only AVI files (no video frames), the parser falls back to collecting
//! audio keyframe entries from `idx1`. OpenDML / AVI 2.0 streams that lack `idx1`
//! produce `Error::IndexNotFound` with a descriptive message.

use crate::RangeFetcher;
use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

/// Window fetched from the tail of the file to find the `idx1` chunk.
const TAIL_WINDOW: u64 = 65536;

/// AVI index entry flags.
const AVIIF_KEYFRAME: u32 = 0x0000_0010;

/// Size of a single `idx1` entry in bytes.
const IDX1_ENTRY_SIZE: usize = 16;

/// Default fallback frame rate if `avih` is missing or invalid.
const DEFAULT_FPS: f64 = 25.0;

/// Fallback byte rate (128 kbps / 8) when audio timing cannot be determined from headers.
const FALLBACK_AUDIO_BYTE_RATE: f64 = 128_000.0 / 8.0;

/// Parses an AVI stream and returns a `ContainerIndex`.
///
/// Fetches the last `64 KB` of the stream to locate the `idx1` chunk, then
/// builds a coarse keyframe-only index suitable for seeking. Falls back to
/// audio keyframes for audio-only AVI files.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the AVI stream (used to read the `avih` header).
/// * `total_size` - Total file size in bytes (required for the tail Range fetch).
/// * `fetcher` - Provides the tail bytes containing `idx1`.
///
/// # Errors
///
/// Returns `Error::ParseFailed` when the AVI header is malformed.
/// Returns `Error::IndexNotFound` when `idx1` is not found (e.g., OpenDML/AVI 2.0)
/// or when no usable keyframes are present.
/// Returns `Error::FetchFailed` on Range request failure.
pub(crate) async fn parse<F>(probe: &[u8], total_size: Option<u64>, fetcher: &F) -> Result<ContainerIndex>
where
    F: RangeFetcher,
{
    tracing::debug!(probe_len = probe.len(), total_size = ?total_size, "⚙️ Parsing AVI stream");
    let total = total_size.ok_or_else(|| Error::parse("AVI idx1 parsing requires total_size"))?;

    let fps = read_fps_from_avih(probe);

    // Check for OpenDML indx chunk in the probe first.
    if let Some(index) = try_parse_odml_index(probe, fps) {
        tracing::debug!(segments = ?index.inner, "✅ AVI OpenDML indx index parsed");
        return Ok(index);
    }

    let tail_start = total.saturating_sub(TAIL_WINDOW);
    let tail = fetcher
        .fetch(tail_start, total.saturating_sub(1))
        .await
        .map_err(Error::fetch)?;

    let idx1 = find_idx1(&tail).ok_or_else(|| {
        Error::index_not_found(
            "idx1 chunk not found in AVI tail (OpenDML/AVI 2.0 or truncated file)",
        )
    })?;

    let result = parse_idx1(idx1, fps, tail_start, probe);
    if let Ok(ref idx) = result
        && let Inner::Segments(ref segs) = idx.inner
    {
        tracing::debug!(keyframes = segs.len(), "✅ AVI index parsed");
    }
    result
}

/// Attempts to parse an OpenDML `indx` super-index from the probe.
///
/// Returns `Some(ContainerIndex)` if a usable `indx` chunk is found, `None` otherwise.
fn try_parse_odml_index(probe: &[u8], fps: Option<f64>) -> Option<ContainerIndex> {
    // Search for "indx" chunk inside the probe (typically inside the hdrl LIST).
    let tag = b"indx";
    let pos = probe.windows(4).position(|w| w == tag)?;
    if pos + 8 > probe.len() {
        return None;
    }
    let chunk_size = u32::from_le_bytes(probe[pos + 4..pos + 8].try_into().ok()?) as usize;
    let data_start = pos + 8;
    let data_end = (data_start + chunk_size).min(probe.len());
    let data = &probe[data_start..data_end];

    // OpenDML index header (24 bytes minimum):
    // 2 bytes longs_per_entry, 1 byte index_sub_type, 1 byte index_type
    // 4 bytes entries_in_use, 4 bytes chunk_id, 3×8 bytes base_offset + reserved
    if data.len() < 24 {
        return None;
    }
    let entries_in_use = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    // base_offset: absolute file offset to the start of the data referenced by this index.
    let base_offset = u64::from_le_bytes(data[12..20].try_into().ok()?);
    let entry_size = 8usize; // each OpenDML index entry is 8 bytes: offset(4) + size+flags(4)
    let entries_start = 24usize;

    if data.len() < entries_start + entries_in_use * entry_size {
        return None;
    }

    let fps_val = fps.unwrap_or(DEFAULT_FPS);
    let mut keyframes: Vec<(u64, u64)> = Vec::new(); // (frame_index, byte_offset)

    for (frame_index, i) in (0..entries_in_use).enumerate() {
        let off = entries_start + i * entry_size;
        let offset32 = u32::from_le_bytes(data[off..off + 4].try_into().ok()?) as u64;
        let size_flags = u32::from_le_bytes(data[off + 4..off + 8].try_into().ok()?);
        let is_delta = (size_flags >> 31) & 1 == 1; // bit 31 = delta frame
        if !is_delta {
            keyframes.push((frame_index as u64, base_offset + offset32));
        }
    }

    if keyframes.len() < 2 {
        return None;
    }

    let segments = keyframes_to_segments(&keyframes, fps_val, base_offset + (entries_in_use as u64 * 8));
    let init_end_byte = segments.first().map(|s| s.byte_offset.saturating_sub(1)).unwrap_or(0);
    Some(ContainerIndex {
        init_end_byte,
        inner: Inner::Segments(segments),
    })
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
    let tag = b"idx1";
    let pos = data.windows(4).position(|w| w == tag)?;
    if pos + 8 > data.len() {
        return None;
    }
    let size = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().ok()?) as usize;
    let end = (pos + 8 + size).min(data.len());
    Some(&data[pos + 8..end])
}

/// Parses the `idx1` payload and returns a `ContainerIndex` built from keyframe entries.
///
/// `idx1` entries are 16 bytes each:
/// - 4 bytes chunk_id (e.g. "00dc" for video frame 0)
/// - 4 bytes flags (AVIIF_KEYFRAME = 0x10)
/// - 4 bytes chunk_offset (relative to movi list start)
/// - 4 bytes chunk_size
///
/// When no video keyframes are found (audio-only AVI), falls back to audio chunk
/// entries with `AVIIF_KEYFRAME` set.
fn parse_idx1(idx1: &[u8], fps: Option<f64>, tail_start: u64, probe: &[u8]) -> Result<ContainerIndex> {
    // Locate the movi list start byte in the file to convert idx1 offsets to absolute positions.
    let movi_start = find_movi_start(probe).unwrap_or(0);

    let n_entries = idx1.len() / IDX1_ENTRY_SIZE;
    let mut video_keyframes: Vec<(u64, u64)> = Vec::new(); // (frame_index, byte_offset)
    let mut audio_keyframes: Vec<(u64, u64)> = Vec::new(); // (block_index, byte_offset)
    let mut video_frame_index = 0u64;
    let mut audio_block_index = 0u64;

    // Heuristic: check whether idx1 offsets are absolute (>= movi_start)
    // or relative to the movi list. Some muxers write absolute file offsets.
    let first_offset = if idx1.len() >= IDX1_ENTRY_SIZE {
        u32::from_le_bytes(idx1[8..12].try_into().unwrap()) as u64
    } else {
        0
    };
    let offsets_are_absolute = movi_start > 0 && first_offset >= movi_start;

    for i in 0..n_entries {
        let off = i * IDX1_ENTRY_SIZE;
        if off + IDX1_ENTRY_SIZE > idx1.len() {
            break;
        }
        let chunk_id = &idx1[off..off + 4];
        let flags = u32::from_le_bytes(idx1[off + 4..off + 8].try_into().unwrap());
        let chunk_offset = u32::from_le_bytes(idx1[off + 8..off + 12].try_into().unwrap()) as u64;

        let abs_offset = if offsets_are_absolute {
            chunk_offset
        } else {
            movi_start + chunk_offset
        };

        // Video frames: chunk_id ends in 'dc' (compressed) or 'db' (uncompressed).
        let is_video = chunk_id.len() == 4 && chunk_id[2] == b'd' && (chunk_id[3] == b'b' || chunk_id[3] == b'c');
        // Audio blocks: chunk_id ends in 'wb'.
        let is_audio = chunk_id.len() == 4 && chunk_id[2] == b'w' && chunk_id[3] == b'b';

        if is_video {
            if flags & AVIIF_KEYFRAME != 0 {
                video_keyframes.push((video_frame_index, abs_offset));
            }
            video_frame_index += 1;
        } else if is_audio {
            if flags & AVIIF_KEYFRAME != 0 {
                audio_keyframes.push((audio_block_index, abs_offset));
            }
            audio_block_index += 1;
        }
    }

    // Prefer video keyframes; fall back to audio keyframes for audio-only AVI.
    let (keyframes, is_audio_only) = if !video_keyframes.is_empty() {
        (video_keyframes, false)
    } else if !audio_keyframes.is_empty() {
        tracing::debug!(
            audio_blocks = audio_block_index,
            "⚙️ AVI audio-only: building index from audio keyframes"
        );
        (audio_keyframes, true)
    } else {
        return Err(Error::index_not_found(
            "idx1 contains no video or audio keyframes",
        ));
    };

    let fps_val = if is_audio_only {
        // For audio-only AVI without a frame rate, use byte-rate timing later.
        None
    } else {
        Some(fps.unwrap_or(DEFAULT_FPS))
    };

    let last_byte = tail_start; // approximate end of last segment
    let segments = if let Some(fps_v) = fps_val {
        keyframes_to_segments(&keyframes, fps_v, last_byte)
    } else {
        audio_keyframes_to_segments(&keyframes, last_byte)
    };

    if segments.is_empty() {
        return Err(Error::index_not_found("idx1 produced no segments"));
    }

    let init_end_byte = segments.first().map(|s| s.byte_offset.saturating_sub(1)).unwrap_or(0);

    Ok(ContainerIndex {
        init_end_byte,
        inner: Inner::Segments(segments),
    })
}

/// Converts `(frame_index, byte_offset)` keyframe pairs to `SegmentEntry` list using `fps`.
fn keyframes_to_segments(keyframes: &[(u64, u64)], fps: f64, last_byte: u64) -> Vec<SegmentEntry> {
    let mut segments = Vec::with_capacity(keyframes.len());
    for i in 0..keyframes.len() {
        let (fidx, byte_offset) = keyframes[i];
        let start_secs = fidx as f64 / fps;
        let (next_fidx, next_byte) = if i + 1 < keyframes.len() {
            keyframes[i + 1]
        } else {
            (fidx, last_byte)
        };
        let end_secs = next_fidx as f64 / fps;
        segments.push(SegmentEntry {
            start_secs,
            end_secs,
            byte_offset,
            byte_size: next_byte.saturating_sub(byte_offset),
        });
    }
    segments
}

/// Converts audio-only `(block_index, byte_offset)` pairs to `SegmentEntry` using byte-rate timing.
///
/// When no FPS is available, timestamps are estimated from byte offsets and the fallback byte rate.
fn audio_keyframes_to_segments(keyframes: &[(u64, u64)], last_byte: u64) -> Vec<SegmentEntry> {
    // Compute total audio bytes to derive approximate duration.
    let first_byte = keyframes.first().map(|k| k.1).unwrap_or(0);
    let total_bytes = last_byte.saturating_sub(first_byte) as f64;
    let total_secs = total_bytes / FALLBACK_AUDIO_BYTE_RATE;

    let mut segments = Vec::with_capacity(keyframes.len());
    for i in 0..keyframes.len() {
        let (_, byte_offset) = keyframes[i];
        let byte_frac = byte_offset.saturating_sub(first_byte) as f64 / total_bytes.max(1.0);
        let start_secs = byte_frac * total_secs;
        let (next_frac, next_byte) = if i + 1 < keyframes.len() {
            let (_, nb) = keyframes[i + 1];
            let nf = nb.saturating_sub(first_byte) as f64 / total_bytes.max(1.0);
            (nf, nb)
        } else {
            (1.0, last_byte)
        };
        let end_secs = next_frac * total_secs;
        segments.push(SegmentEntry {
            start_secs,
            end_secs,
            byte_offset,
            byte_size: next_byte.saturating_sub(byte_offset),
        });
    }
    segments
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
