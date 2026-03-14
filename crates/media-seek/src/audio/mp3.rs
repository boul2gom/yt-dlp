//! MPEG Audio (Layer I / Layer II / Layer III) container index parsing.
//!
//! Supports all three MPEG audio layers:
//! - **Layer III (MP3 / `.mp3`)**: VBR via Xing/Info TOC or VBRI table; CBR fallback;
//!   free-format (bi==0) via consecutive sync-word detection.
//! - **Layer II (`.mp2`)**: CBR only — Xing/VBRI headers are Layer III-specific.
//! - **Layer I (`.mp1`)**: CBR only — same limitation.
//!
//! **ID3v1 trailing tag:** When `total_size` is provided and the file extends beyond
//! the probe, the last 128 bytes are fetched to detect and subtract an ID3v1 tag.
//! When the probe covers the entire file, the tag is detected locally without a fetch.
//!
//! **Free-format (bi==0) streams:** MPEG Layer III permits a non-standard constant
//! frame size negotiated out-of-band (`bitrate_index == 0b0000`). This parser detects
//! the frame size automatically by scanning for two additional sync words at consistent
//! intervals. If the frame size cannot be determined from the probe, `ParseFailed` is
//! returned with a descriptive message.

use crate::RangeFetcher;
use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

// ==================== Constants ====================

/// Size of each MP3 frame sync lookup window.
const SYNC_SEARCH_LIMIT: usize = 8192;

/// VBRI header always appears at this offset after the frame header start.
const VBRI_OFFSET: usize = 36;

/// ID3v2 header total size in bytes (before the tag body).
const ID3V2_HEADER_SIZE: usize = 10;

/// ID3v2 footer flag bitmask in the flags byte (byte 5).
const ID3V2_FOOTER_FLAG: u8 = 0x10;

/// Size of an ID3v1 trailing tag in bytes.
const ID3V1_TAG_SIZE: usize = 128;

/// Magic bytes at the start of an ID3v2 tag (always "ID3").
pub(crate) const ID3V2_MAGIC: &[u8; 3] = b"ID3";

/// Magic bytes identifying an ID3v1 trailing tag.
const ID3V1_MAGIC: &[u8; 3] = b"TAG";

/// Side information size: MPEG-1 mono (Layer III).
const SIDE_INFO_MPEG1_MONO: usize = 17;
/// Side information size: MPEG-1 stereo (Layer III).
const SIDE_INFO_MPEG1_STEREO: usize = 32;
/// Side information size: MPEG-2/2.5 mono (Layer III).
const SIDE_INFO_MPEG2_MONO: usize = 9;
/// Side information size: MPEG-2/2.5 stereo (Layer III).
const SIDE_INFO_MPEG2_STEREO: usize = 17;

/// Xing TOC divides the byte range into 256 units — each entry is `value / 256.0`.
const XING_TOC_SCALE: f64 = 256.0;

/// Xing header flag: total frames field is present.
const XING_FLAG_FRAMES: u32 = 0x01;
/// Xing header flag: total bytes field is present.
const XING_FLAG_BYTES: u32 = 0x02;
/// Xing header flag: TOC (100-entry seek table) is present.
const XING_FLAG_TOC: u32 = 0x04;

/// Fallback byte rate when no frame count or duration is available (128 kbps / 8).
pub(crate) const FALLBACK_BYTE_RATE: f64 = 128_000.0 / 8.0;

/// Number of entries in a Xing TOC.
const XING_TOC_ENTRIES: usize = 100;

/// Minimum plausible free-format (bi==0) MP3 frame size in bytes (practical lower bound).
const FREE_FORMAT_MIN_FRAME_SIZE: usize = 24;

/// Maximum plausible free-format (bi==0) MP3 frame size in bytes.
/// Also defines the scan window for the second sync word.
const FREE_FORMAT_MAX_FRAME_SIZE: usize = 4096;

// ==================== Layer constants ====================

/// Layer bits value for MPEG Layer I (`.mp1`) in the frame header byte.
const LAYER_I: u8 = 0x03;

/// Layer bits value for MPEG Layer II (`.mp2`) in the frame header byte.
const LAYER_II: u8 = 0x02;

/// Layer bits value for MPEG Layer III (`.mp3`) in the frame header byte.
const LAYER_III: u8 = 0x01;

/// Number of PCM samples per MPEG Layer I frame (all MPEG versions).
const SAMPLES_PER_FRAME_LAYER_I: u64 = 384;

/// Number of PCM samples per MPEG Layer II frame (all MPEG versions).
const SAMPLES_PER_FRAME_LAYER_II: u64 = 1152;

// ==================== Bitrate tables ====================

/// MPEG-1 Layer III bitrate table (kbps, indexed by 4-bit field).
const BITRATES_MPEG1_L3: [u32; 16] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0];
/// MPEG-2/2.5 Layer III bitrate table (kbps, indexed by 4-bit field).
const BITRATES_MPEG2_L3: [u32; 16] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0];

/// MPEG-1 Layer II bitrate table (kbps, indexed by 4-bit field).
const BITRATES_MPEG1_L2: [u32; 16] = [0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 0];
/// MPEG-2/2.5 Layer II bitrate table (kbps, indexed by 4-bit field).
const BITRATES_MPEG2_L2: [u32; 16] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0];

/// MPEG-1 Layer I bitrate table (kbps, indexed by 4-bit field).
const BITRATES_MPEG1_L1: [u32; 16] = [0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448, 0];
/// MPEG-2/2.5 Layer I bitrate table (kbps, indexed by 4-bit field).
const BITRATES_MPEG2_L1: [u32; 16] = [0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256, 0];

/// MPEG-1 sample rate table (Hz, indexed by 2-bit field).
const SAMPLE_RATES_MPEG1: [u32; 4] = [44100, 48000, 32000, 0];
/// MPEG-2 sample rate table (Hz, indexed by 2-bit field).
const SAMPLE_RATES_MPEG2: [u32; 4] = [22050, 24000, 16000, 0];
/// MPEG-2.5 sample rate table (Hz, indexed by 2-bit field).
const SAMPLE_RATES_MPEG25: [u32; 4] = [11025, 12000, 8000, 0];

// ==================== Parsed frame header ====================

/// Decoded fields from an MPEG audio frame header.
struct FrameHeader {
    sample_rate: u32,
    channels: u8,
    bitrate_bps: u32,
    /// 3 = MPEG-1, 2 = MPEG-2, 0 = MPEG-2.5.
    mpeg_version: u8,
    /// `LAYER_I` (0x03), `LAYER_II` (0x02), or `LAYER_III` (0x01).
    layer: u8,
}

// ==================== Public entry point ====================

/// Parses an MPEG audio stream (Layer I, II, or III) and returns a [`ContainerIndex`].
///
/// Skips any ID3v2 header(s), locates the first MPEG sync frame, then checks for
/// a Xing/Info or VBRI VBR header (Layer III only). Falls back to CBR for all layers
/// when no VBR header is found.
///
/// For free-format (bi==0) Layer III streams, the frame size is detected by scanning
/// for two additional consecutive sync words at consistent intervals.
///
/// When `total_size` exceeds `probe.len()`, the last 128 bytes are fetched via
/// `fetcher` to detect and subtract an ID3v1 trailing tag.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the MPEG audio stream.
/// * `total_size` - Total stream size in bytes, for accurate byte-rate calculation.
/// * `fetcher` - Used to fetch the ID3v1 tail when `total_size > probe.len()`.
///
/// # Errors
///
/// Returns [`Error::ParseFailed`] when no sync frame is found within the probe,
/// or when a free-format frame size cannot be determined.
pub(crate) async fn parse<F: RangeFetcher>(
    probe: &[u8],
    total_size: Option<u64>,
    fetcher: &F,
) -> Result<ContainerIndex> {
    tracing::debug!(probe_len = probe.len(), "⚙️ Parsing MPEG audio stream");
    let audio_start = skip_id3(probe);

    // Standard pass: look for a non-free-format (bi != 0) sync frame.
    let frame_start = if let Some(pos) = find_sync_frame(probe, audio_start) {
        pos
    } else {
        // No standard sync frame found — try free-format detection (Layer III only).
        return parse_free_format(probe, audio_start);
    };

    let frame = &probe[frame_start..];
    if frame.len() < 4 {
        return Err(Error::parse("MPEG audio frame header truncated"));
    }

    let header = parse_frame_header(frame).ok_or_else(|| Error::parse("invalid MPEG audio frame header"))?;

    // Fetch ID3v1 only when file extends beyond probe — best-effort, failure is silent.
    let id3v1_fetched = if total_size.is_some_and(|s| s > probe.len() as u64) {
        fetch_id3v1_present(total_size.unwrap(), fetcher).await
    } else {
        false
    };
    let audio_size = compute_audio_size(probe, total_size, frame_start, id3v1_fetched);

    // Xing/VBRI headers are exclusive to Layer III.
    if header.layer == LAYER_III
        && let Some(idx) = try_parse_vbr_header(frame, &header, frame_start as u64, audio_size)?
    {
        return Ok(idx);
    }

    // CBR: use constant bitrate for a Linear index (all layers).
    if header.bitrate_bps == 0 {
        return Err(Error::parse("MPEG audio CBR bitrate is zero"));
    }
    let byte_rate = header.bitrate_bps as f64 / 8.0;
    tracing::debug!(
        layer = header.layer,
        bitrate_bps = header.bitrate_bps,
        "✅ MPEG audio index parsed (mode=cbr)"
    );
    Ok(ContainerIndex {
        init_end_byte: frame_start as u64,
        inner: Inner::Linear {
            byte_rate,
            block_align: 1,
        },
    })
}

// ==================== Internal helpers ====================

/// Returns the byte offset of the first ID3v2-free audio data.
///
/// Loops to handle stacked (chained) ID3v2 tags: some taggers write multiple
/// consecutive ID3v2 headers, each valid on its own.
fn skip_id3(data: &[u8]) -> usize {
    let mut pos = 0usize;
    loop {
        if data.len() < pos + ID3V2_HEADER_SIZE || &data[pos..pos + 3] != b"ID3" {
            return pos;
        }
        // ID3v2 size is encoded as four 7-bit syncsafe bytes.
        let size = ((data[pos + 6] as u32) << 21)
            | ((data[pos + 7] as u32) << 14)
            | ((data[pos + 8] as u32) << 7)
            | (data[pos + 9] as u32);
        let footer = data[pos + 5] & ID3V2_FOOTER_FLAG != 0;
        let tag_end = pos + ID3V2_HEADER_SIZE + size as usize + if footer { ID3V2_HEADER_SIZE } else { 0 };
        if tag_end >= data.len() {
            return data.len();
        }
        pos = tag_end;
    }
}

/// Checks whether the file ends with an ID3v1 tag by fetching the last 128 bytes.
///
/// Returns `false` silently on fetch failure — ID3v1 detection is best-effort.
async fn fetch_id3v1_present<F: RangeFetcher>(total_size: u64, fetcher: &F) -> bool {
    if total_size < ID3V1_TAG_SIZE as u64 {
        return false;
    }
    let start = total_size - ID3V1_TAG_SIZE as u64;
    match fetcher.fetch(start, total_size - 1).await {
        Ok(bytes) if bytes.len() >= 3 => &bytes[..3] == ID3V1_MAGIC,
        _ => false,
    }
}

/// Returns the usable audio byte count starting after `frame_start`.
///
/// Uses `total_size` as the authoritative file size (falling back to `probe.len()`).
/// Subtracts 128 bytes when an ID3v1 trailer is detected — either directly from the
/// probe tail (when the probe covers the entire file) or via the `id3v1_fetched` flag.
fn compute_audio_size(probe: &[u8], total_size: Option<u64>, frame_start: usize, id3v1_fetched: bool) -> u64 {
    let raw_size = total_size.unwrap_or(probe.len() as u64);

    // Detect ID3v1 from probe tail when the probe contains the complete file.
    let full_file_in_probe = total_size.is_some_and(|s| s == probe.len() as u64);
    let probe_has_id3v1 = full_file_in_probe
        && probe.len() >= ID3V1_TAG_SIZE
        && &probe[probe.len() - ID3V1_TAG_SIZE..probe.len() - ID3V1_TAG_SIZE + 3] == ID3V1_MAGIC;

    let file_size = if probe_has_id3v1 || id3v1_fetched {
        tracing::debug!("⚙️ MP3 ID3v1 trailer detected, subtracting {} bytes", ID3V1_TAG_SIZE);
        raw_size.saturating_sub(ID3V1_TAG_SIZE as u64)
    } else {
        raw_size
    };

    file_size.saturating_sub(frame_start as u64)
}

/// Finds the byte position of the first valid MPEG audio sync frame in `data[start..]`.
///
/// Accepts Layer I, II, and III. Excludes free-bitrate (bi == 0) and reserved bi (15).
fn find_sync_frame(data: &[u8], start: usize) -> Option<usize> {
    let limit = (start + SYNC_SEARCH_LIMIT).min(data.len().saturating_sub(3));
    for i in start..limit {
        if data[i] == 0xFF {
            let b1 = data[i + 1];
            let layer = (b1 >> 1) & 0x03;
            // Sync: upper 3 bits = 111; layer must not be reserved (0x00)
            if (b1 & 0xE0) == 0xE0 && layer >= 0x01 && i + 3 < data.len() {
                let bi = (data[i + 2] >> 4) & 0x0F;
                if bi != 0 && bi != 15 {
                    return Some(i);
                }
            }
        }
    }
    None
}

/// Finds the first MPEG audio sync frame including free-bitrate (bi == 0).
///
/// Excludes only the strictly reserved bitrate index (bi == 15).
fn find_sync_frame_any(data: &[u8], start: usize) -> Option<usize> {
    let limit = (start + SYNC_SEARCH_LIMIT).min(data.len().saturating_sub(3));
    for i in start..limit {
        if data[i] == 0xFF {
            let b1 = data[i + 1];
            let layer = (b1 >> 1) & 0x03;
            if (b1 & 0xE0) == 0xE0 && layer >= 0x01 && i + 3 < data.len() {
                let bi = (data[i + 2] >> 4) & 0x0F;
                if bi != 15 {
                    return Some(i);
                }
            }
        }
    }
    None
}

/// Detects the fixed frame size of a free-format (bi==0) MPEG Layer III stream by
/// scanning for consecutive sync words at consistent intervals.
fn detect_free_format_frame_size(data: &[u8], frame_start: usize) -> Option<usize> {
    if data.len() < frame_start + 4 {
        return None;
    }

    let ref_b1 = data[frame_start + 1] & 0xFE;
    let ref_sr_bits = data[frame_start + 2] & 0x0C;

    let search_end = (frame_start + FREE_FORMAT_MAX_FRAME_SIZE).min(data.len().saturating_sub(4));
    let search_start = frame_start + FREE_FORMAT_MIN_FRAME_SIZE;

    if search_start >= search_end {
        return None;
    }

    for candidate in search_start..search_end {
        if data[candidate] != 0xFF {
            continue;
        }
        if data[candidate + 1] & 0xFE != ref_b1 {
            continue;
        }
        if data[candidate + 2] & 0x0C != ref_sr_bits {
            continue;
        }
        let frame_size = candidate - frame_start;

        let third = frame_start + 2 * frame_size;
        if third + 3 >= data.len() {
            return Some(frame_size); // Insufficient probe to verify — accept tentatively.
        }
        if data[third] == 0xFF && data[third + 1] & 0xFE == ref_b1 && data[third + 2] & 0x0C == ref_sr_bits {
            return Some(frame_size);
        }
    }
    None
}

/// Handles free-format (bi==0) Layer III streams via consecutive sync word scanning.
fn parse_free_format(probe: &[u8], audio_start: usize) -> Result<ContainerIndex> {
    let free_pos = find_sync_frame_any(probe, audio_start)
        .ok_or_else(|| Error::parse("no MPEG Layer III sync frame found in probe"))?;

    let bi = (probe[free_pos + 2] >> 4) & 0x0F;
    if bi != 0 {
        return Err(Error::parse("MPEG audio sync frame has reserved bitrate index"));
    }

    let b1 = probe[free_pos + 1];
    let b2 = probe[free_pos + 2];
    let mpeg_version = (b1 >> 3) & 0x03;
    let sr_idx = ((b2 >> 2) & 0x03) as usize;
    let sample_rate = match mpeg_version {
        3 => SAMPLE_RATES_MPEG1[sr_idx],
        2 => SAMPLE_RATES_MPEG2[sr_idx],
        0 => SAMPLE_RATES_MPEG25[sr_idx],
        _ => 0,
    };
    if sample_rate == 0 {
        return Err(Error::parse("free-format MP3: reserved or invalid sample rate"));
    }

    let frame_size = detect_free_format_frame_size(probe, free_pos).ok_or_else(|| {
        Error::parse(
            "free-format MP3: frame size could not be determined — probe may be too short or stream is malformed",
        )
    })?;

    let spf = samples_per_frame(mpeg_version, LAYER_III);
    let byte_rate = frame_size as f64 * sample_rate as f64 / spf as f64;

    tracing::debug!(
        free_pos,
        frame_size,
        sample_rate,
        byte_rate,
        "✅ MPEG audio index parsed (mode=free-format)"
    );
    Ok(ContainerIndex {
        init_end_byte: free_pos as u64,
        inner: Inner::Linear {
            byte_rate,
            block_align: frame_size as u64,
        },
    })
}

/// Parses an MPEG audio frame header and returns a [`FrameHeader`].
///
/// Accepts Layer I, II, and III. Returns `None` for reserved layer (0) or
/// reserved MPEG version (1).
fn parse_frame_header(frame: &[u8]) -> Option<FrameHeader> {
    if frame.len() < 4 {
        return None;
    }
    let b1 = frame[1];
    let b2 = frame[2];
    let b3 = frame[3];

    let mpeg_version = (b1 >> 3) & 0x03;
    let layer = (b1 >> 1) & 0x03;

    // layer 0x00 = reserved, mpeg_version 0x01 = reserved
    if layer == 0x00 || mpeg_version == 0x01 {
        return None;
    }

    let bitrate_idx = (b2 >> 4) as usize;
    let sr_idx = ((b2 >> 2) & 0x03) as usize;
    let channel_mode = (b3 >> 6) & 0x03;
    let channels = if channel_mode == 3 { 1u8 } else { 2u8 };

    let bitrate_kbps = match (mpeg_version, layer) {
        (3, LAYER_III) => BITRATES_MPEG1_L3[bitrate_idx],
        (3, LAYER_II) => BITRATES_MPEG1_L2[bitrate_idx],
        (3, LAYER_I) => BITRATES_MPEG1_L1[bitrate_idx],
        (_, LAYER_III) => BITRATES_MPEG2_L3[bitrate_idx],
        (_, LAYER_II) => BITRATES_MPEG2_L2[bitrate_idx],
        (_, LAYER_I) => BITRATES_MPEG2_L1[bitrate_idx],
        _ => return None,
    };

    let sample_rate = match mpeg_version {
        3 => SAMPLE_RATES_MPEG1[sr_idx],
        2 => SAMPLE_RATES_MPEG2[sr_idx],
        0 => SAMPLE_RATES_MPEG25[sr_idx],
        _ => return None,
    };

    if bitrate_kbps == 0 || sample_rate == 0 {
        return None;
    }

    Some(FrameHeader {
        sample_rate,
        channels,
        bitrate_bps: bitrate_kbps * 1000,
        mpeg_version,
        layer,
    })
}

/// Returns the offset of the Xing header within a Layer III frame (after side information).
fn xing_header_offset(mpeg_version: u8, channels: u8) -> usize {
    let side_info = match (mpeg_version, channels) {
        (3, 1) => SIDE_INFO_MPEG1_MONO,
        (3, _) => SIDE_INFO_MPEG1_STEREO,
        (_, 1) => SIDE_INFO_MPEG2_MONO,
        _ => SIDE_INFO_MPEG2_STEREO,
    };
    4 + side_info
}

/// Returns the number of PCM samples per frame for the given MPEG version and layer.
fn samples_per_frame(mpeg_version: u8, layer: u8) -> u64 {
    match layer {
        LAYER_I => SAMPLES_PER_FRAME_LAYER_I,
        LAYER_II => SAMPLES_PER_FRAME_LAYER_II,
        LAYER_III => {
            if mpeg_version == 3 {
                1152
            } else {
                576
            }
        }
        _ => 576,
    }
}

/// Reads the optional `total_frames` field from a Xing header.
///
/// Returns `(frames, bytes_consumed)`. When the flag is absent, frames is 0 and consumed is 0.
fn read_xing_frames(frame: &[u8], off: usize, flags: u32) -> Result<(u32, usize)> {
    if flags & XING_FLAG_FRAMES == 0 {
        return Ok((0, 0));
    }
    if frame.len() < off + 4 {
        return Err(Error::parse("Xing total_frames truncated"));
    }
    let frames = u32::from_be_bytes(frame[off..off + 4].try_into().unwrap());
    Ok((frames, 4))
}

/// Reads the optional `total_bytes` field from a Xing header.
///
/// Returns `(bytes, bytes_consumed)`. When the flag is absent, uses `fallback` and consumed is 0.
fn read_xing_bytes(frame: &[u8], off: usize, flags: u32, fallback: u64) -> Result<(u64, usize)> {
    if flags & XING_FLAG_BYTES == 0 {
        return Ok((fallback, 0));
    }
    if frame.len() < off + 4 {
        return Err(Error::parse("Xing total_bytes truncated"));
    }
    let bytes = u32::from_be_bytes(frame[off..off + 4].try_into().unwrap()) as u64;
    Ok((bytes, 4))
}

/// Reads the optional 100-entry TOC from a Xing header.
///
/// Returns `Ok(Some(toc))` when present, `Ok(None)` when flag is absent.
fn read_xing_toc(frame: &[u8], off: usize, flags: u32) -> Result<Option<[u8; XING_TOC_ENTRIES]>> {
    if flags & XING_FLAG_TOC == 0 {
        return Ok(None);
    }
    if frame.len() < off + XING_TOC_ENTRIES {
        return Err(Error::parse("Xing TOC truncated"));
    }
    let mut t = [0u8; XING_TOC_ENTRIES];
    t.copy_from_slice(&frame[off..off + XING_TOC_ENTRIES]);
    Ok(Some(t))
}

/// Builds a segmented `ContainerIndex` from a Xing TOC.
fn build_toc_segments(
    toc: [u8; XING_TOC_ENTRIES],
    total_bytes: u64,
    total_duration: f64,
    frame_start_byte: u64,
) -> Vec<SegmentEntry> {
    let mut segments = Vec::with_capacity(XING_TOC_ENTRIES);
    for i in 0..XING_TOC_ENTRIES {
        let pct = toc[i] as f64 / XING_TOC_SCALE;
        let byte_offset = frame_start_byte + (pct * total_bytes as f64) as u64;
        let start_secs = i as f64 * total_duration / XING_TOC_ENTRIES as f64;
        let end_secs = (i + 1) as f64 * total_duration / XING_TOC_ENTRIES as f64;
        let next_pct = if i + 1 < XING_TOC_ENTRIES {
            toc[i + 1] as f64 / XING_TOC_SCALE
        } else {
            1.0
        };
        let next_byte = frame_start_byte + (next_pct * total_bytes as f64) as u64;
        segments.push(SegmentEntry {
            start_secs,
            end_secs,
            byte_offset,
            byte_size: next_byte.saturating_sub(byte_offset),
        });
    }
    segments
}

/// Parses a Xing/Info VBR header and constructs a segmented [`ContainerIndex`].
///
/// `audio_size` is the usable audio byte count (actual file size minus ID3v1 and
/// bytes before `frame_start_byte`), computed by [`compute_audio_size`].
fn parse_xing(
    frame: &[u8],
    xing_off: usize,
    sample_rate: u32,
    mpeg_version: u8,
    frame_start_byte: u64,
    audio_size: u64,
) -> Result<ContainerIndex> {
    let x = xing_off;
    if frame.len() < x + 8 {
        return Err(Error::parse("Xing header truncated"));
    }
    let flags = u32::from_be_bytes(frame[x + 4..x + 8].try_into().unwrap());

    let mut off = x + 8;
    let (total_frames, consumed) = read_xing_frames(frame, off, flags)?;
    off += consumed;
    let (total_bytes, consumed) = read_xing_bytes(frame, off, flags, audio_size)?;
    off += consumed;
    let toc = read_xing_toc(frame, off, flags)?;

    // When total_frames == 0 we cannot compute duration from frames.
    if total_frames == 0 {
        return Ok(ContainerIndex {
            init_end_byte: frame_start_byte,
            inner: Inner::Linear {
                byte_rate: FALLBACK_BYTE_RATE,
                block_align: 1,
            },
        });
    }

    let spf = samples_per_frame(mpeg_version, LAYER_III);
    let total_duration = total_frames as f64 * spf as f64 / sample_rate as f64;

    if let Some(toc) = toc {
        return Ok(ContainerIndex {
            init_end_byte: frame_start_byte,
            inner: Inner::Segments(build_toc_segments(toc, total_bytes, total_duration, frame_start_byte)),
        });
    }

    let byte_rate = total_bytes as f64 / total_duration;
    Ok(ContainerIndex {
        init_end_byte: frame_start_byte,
        inner: Inner::Linear {
            byte_rate,
            block_align: 1,
        },
    })
}

/// Checks for Xing/Info or VBRI VBR headers in a Layer III frame and parses them if found.
///
/// Returns `Ok(Some(index))` when a VBR header is found and parsed, `Ok(None)` when absent.
fn try_parse_vbr_header(
    frame: &[u8],
    header: &FrameHeader,
    frame_start_byte: u64,
    audio_size: u64,
) -> Result<Option<ContainerIndex>> {
    let xing_offset = xing_header_offset(header.mpeg_version, header.channels);
    if frame.len() >= xing_offset + 4 {
        let tag = &frame[xing_offset..xing_offset + 4];
        if tag == b"Xing" || tag == b"Info" {
            tracing::debug!("✅ MPEG audio index parsed (mode=xing)");
            return parse_xing(
                frame,
                xing_offset,
                header.sample_rate,
                header.mpeg_version,
                frame_start_byte,
                audio_size,
            )
            .map(Some);
        }
    }
    if frame.len() >= VBRI_OFFSET + 4 && &frame[VBRI_OFFSET..VBRI_OFFSET + 4] == b"VBRI" {
        tracing::debug!("✅ MPEG audio index parsed (mode=vbri)");
        return parse_vbri(
            frame,
            VBRI_OFFSET,
            header.sample_rate,
            header.mpeg_version,
            frame_start_byte,
            header.bitrate_bps,
        )
        .map(Some);
    }
    Ok(None)
}

/// Assembles a big-endian multi-byte integer value from `entry_bytes` bytes at `data[off..]`.
fn assemble_entry_value(data: &[u8], off: usize, entry_bytes: usize) -> u64 {
    let mut val = 0u64;
    for j in 0..entry_bytes {
        val = (val << 8) | data[off + j] as u64;
    }
    val
}

/// Parses a VBRI VBR header and constructs a segmented [`ContainerIndex`].
///
/// `fallback_bps` is the CBR bitrate from the frame header, used when the VBRI table
/// is unusable (invalid `entry_bytes` or truncated table).
fn parse_vbri(
    frame: &[u8],
    vbri_off: usize,
    sample_rate: u32,
    mpeg_version: u8,
    frame_start_byte: u64,
    fallback_bps: u32,
) -> Result<ContainerIndex> {
    let v = vbri_off;
    if frame.len() < v + 26 {
        return Err(Error::parse("VBRI header truncated"));
    }
    let total_frames = u32::from_be_bytes(frame[v + 14..v + 18].try_into().unwrap());
    let table_size = u16::from_be_bytes(frame[v + 18..v + 20].try_into().unwrap()) as usize;
    let table_scale = u16::from_be_bytes(frame[v + 20..v + 22].try_into().unwrap()) as u64;
    let entry_bytes = u16::from_be_bytes(frame[v + 22..v + 24].try_into().unwrap()) as usize;
    let frames_per_entry = u16::from_be_bytes(frame[v + 24..v + 26].try_into().unwrap()) as u64;

    let table_start = v + 26;

    let table_ok = (1..=8).contains(&entry_bytes) && frame.len() >= table_start + table_size * entry_bytes;
    if !table_ok {
        tracing::warn!(entry_bytes, table_size, "⚙️ VBRI table unusable, falling back to CBR");
        let byte_rate = if fallback_bps > 0 {
            fallback_bps as f64 / 8.0
        } else {
            FALLBACK_BYTE_RATE
        };
        return Ok(ContainerIndex {
            init_end_byte: frame_start_byte,
            inner: Inner::Linear {
                byte_rate,
                block_align: 1,
            },
        });
    }

    let spf = samples_per_frame(mpeg_version, LAYER_III);
    let total_duration = total_frames as f64 * spf as f64 / sample_rate as f64;

    let mut segments = Vec::with_capacity(table_size);
    let mut byte_cursor = frame_start_byte;
    for i in 0..table_size {
        let off = table_start + i * entry_bytes;
        let chunk_bytes = assemble_entry_value(frame, off, entry_bytes) * table_scale;
        let start_secs = i as f64 * frames_per_entry as f64 * spf as f64 / sample_rate as f64;
        let end_secs = (i + 1) as f64 * frames_per_entry as f64 * spf as f64 / sample_rate as f64;
        segments.push(SegmentEntry {
            start_secs,
            end_secs: end_secs.min(total_duration),
            byte_offset: byte_cursor,
            byte_size: chunk_bytes,
        });
        byte_cursor += chunk_bytes;
    }

    Ok(ContainerIndex {
        init_end_byte: frame_start_byte,
        inner: Inner::Segments(segments),
    })
}
