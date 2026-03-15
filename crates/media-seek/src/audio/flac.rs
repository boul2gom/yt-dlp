//! FLAC container index parsing via the SEEKTABLE metadata block.
//!
//! FLAC streams begin with `fLaC` followed by a sequence of metadata blocks.
//! The SEEKTABLE block (type 3) contains seek points mapping sample numbers to
//! byte offsets and audio frame sample counts.

use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

pub(crate) const FLAC_MARKER: &[u8; 4] = b"fLaC";
const BLOCK_TYPE_STREAMINFO: u8 = 0;
const BLOCK_TYPE_SEEKTABLE: u8 = 3;
const SEEKTABLE_PLACEHOLDER: u64 = u64::MAX;
/// Minimum STREAMINFO block size in bytes.
const STREAMINFO_MIN_SIZE: usize = 18;
/// Size of a single SEEKTABLE entry in bytes.
const SEEK_POINT_SIZE: usize = 18;
/// Fallback byte rate when total samples are unknown (128 kbps / 8).
const FALLBACK_BYTE_RATE: f64 = 128_000.0 / 8.0;

/// Parsed data collected from a FLAC metadata block scan.
struct FlacMetadata {
    sample_rate: u32,
    total_samples: u64,
    seek_points: Option<Vec<(u64, u64, u16)>>,
    audio_start: u64,
}

/// Scans the FLAC metadata block sequence starting at byte 4 (after the `fLaC` marker).
///
/// Returns `None` when STREAMINFO is missing or reports a zero sample rate.
fn scan_metadata_blocks(probe: &[u8]) -> Option<FlacMetadata> {
    let mut pos = 4usize;
    let mut sample_rate: u32 = 0;
    let mut total_samples: u64 = 0;
    let mut seek_points: Option<Vec<(u64, u64, u16)>> = None;
    let mut audio_start: u64 = 4;

    loop {
        if pos + 4 > probe.len() {
            break;
        }
        let header = u32::from_be_bytes(probe[pos..pos + 4].try_into().unwrap());
        let is_last = (header >> 31) & 1 == 1;
        let block_type = ((header >> 24) & 0x7F) as u8;
        let block_len = (header & 0x00FF_FFFF) as usize;
        pos += 4;

        let block_end = pos + block_len;
        if block_end > probe.len() {
            if is_last {
                audio_start = block_end as u64;
            }
            pos = block_end;
            continue;
        }

        match block_type {
            BLOCK_TYPE_STREAMINFO => {
                let (sr, ts) = parse_streaminfo_block(&probe[pos..block_end]);
                sample_rate = sr;
                total_samples = ts;
            }
            BLOCK_TYPE_SEEKTABLE => {
                seek_points = Some(parse_seektable_block(&probe[pos..block_end]));
            }
            _ => {}
        }

        if is_last {
            audio_start = block_end as u64;
            break;
        }
        pos = block_end;
    }

    if sample_rate == 0 {
        return None;
    }
    Some(FlacMetadata {
        sample_rate,
        total_samples,
        seek_points,
        audio_start,
    })
}

/// Parses a FLAC stream and returns a `ContainerIndex`.
///
/// Reads the STREAMINFO block for the sample rate and total sample count, then
/// the SEEKTABLE block (if present) to build a segmented index. Falls back to a
/// `Linear` index if no SEEKTABLE is present.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the FLAC stream.
/// * `total_size` - Total stream size in bytes, used to compute the linear byte rate
///   when no SEEKTABLE is present and the file is larger than the probe.
///
/// # Errors
///
/// Returns `Error::ParseFailed` when the stream does not start with the FLAC marker
/// or the STREAMINFO block is malformed.
pub(crate) fn parse(probe: &[u8], total_size: Option<u64>) -> Result<ContainerIndex> {
    tracing::debug!(probe_len = probe.len(), "⚙️ Parsing FLAC stream");
    if !probe.starts_with(FLAC_MARKER) {
        return Err(Error::parse("missing fLaC marker"));
    }

    let meta =
        scan_metadata_blocks(probe).ok_or_else(|| Error::parse("FLAC STREAMINFO missing or sample_rate is zero"))?;

    if let Some(points) = meta.seek_points.filter(|p| !p.is_empty()) {
        return build_seektable_segments(
            &points,
            meta.audio_start,
            meta.sample_rate,
            meta.total_samples,
            total_size,
        );
    }

    // No SEEKTABLE — fall back to linear.
    let total_secs = if meta.total_samples > 0 {
        meta.total_samples as f64 / meta.sample_rate as f64
    } else {
        0.0
    };
    let audio_bytes = total_size
        .unwrap_or(probe.len() as u64)
        .saturating_sub(meta.audio_start) as f64;
    let byte_rate = if total_secs > 0.0 {
        audio_bytes / total_secs
    } else {
        FALLBACK_BYTE_RATE
    };

    tracing::debug!("✅ FLAC index parsed (mode=linear)");
    Ok(ContainerIndex {
        init_end_byte: meta.audio_start.saturating_sub(1),
        inner: Inner::Linear {
            byte_rate,
            block_align: 1,
        },
    })
}

// ==================== Internal helpers ====================

/// Extracts `(sample_rate, total_samples)` from a STREAMINFO block body.
fn parse_streaminfo_block(block: &[u8]) -> (u32, u64) {
    if block.len() < STREAMINFO_MIN_SIZE {
        return (0, 0);
    }
    let sr_word = u32::from_be_bytes(block[10..14].try_into().unwrap());
    let sample_rate = sr_word >> 12;
    // bits [35:0] of bytes 13-17 encode total_samples (36 bits).
    // byte 13 has low 4 bits of sr, then bits 35-32 of total_samples in bits 3-0.
    let ts_hi = (block[13] & 0x0F) as u64;
    let ts_lo = u32::from_be_bytes(block[14..18].try_into().unwrap()) as u64;
    let total_samples = (ts_hi << 32) | ts_lo;
    (sample_rate, total_samples)
}

/// Parses a SEEKTABLE block body into a list of `(sample_number, stream_offset, frame_samples)`.
///
/// Placeholder entries (`sample_number == u64::MAX`) are skipped.
fn parse_seektable_block(block: &[u8]) -> Vec<(u64, u64, u16)> {
    let n = block.len() / SEEK_POINT_SIZE;
    let mut points = Vec::with_capacity(n);
    for i in 0..n {
        let off = i * SEEK_POINT_SIZE;
        if off + SEEK_POINT_SIZE > block.len() {
            break;
        }
        let sample_num = u64::from_be_bytes(block[off..off + 8].try_into().unwrap());
        let stream_off = u64::from_be_bytes(block[off + 8..off + 16].try_into().unwrap());
        let frame_samples = u16::from_be_bytes(block[off + 16..off + 18].try_into().unwrap());
        if sample_num != SEEKTABLE_PLACEHOLDER {
            points.push((sample_num, stream_off, frame_samples));
        }
    }
    points
}

/// The next-segment boundary resolved for a single SEEKTABLE entry.
#[derive(Debug)]
struct NextPointBounds {
    next_sample: u64,
    next_byte_offset: u64,
}

/// Determines the next seek-point boundary for entry `i` in `points`.
///
/// Returns `None` when there is exactly one seek point and `total_samples` is
/// unknown — the caller should fall back to a linear index in that case.
fn next_point_bounds(
    points: &[(u64, u64, u16)],
    i: usize,
    total_samples: u64,
    audio_start: u64,
    total_size: Option<u64>,
    byte_offset: u64,
) -> Option<NextPointBounds> {
    if i + 1 < points.len() {
        return Some(NextPointBounds {
            next_sample: points[i + 1].0,
            next_byte_offset: audio_start + points[i + 1].1,
        });
    }
    let last_sample = if total_samples > 0 {
        total_samples
    } else if i > 0 {
        let prev_sample = points[i - 1].0;
        let gap = points[i].0.saturating_sub(prev_sample);
        points[i].0.saturating_add(gap)
    } else {
        return None; // Single seek point, unknown total — caller falls back to linear.
    };
    Some(NextPointBounds {
        next_sample: last_sample,
        next_byte_offset: total_size.unwrap_or(byte_offset),
    })
}

/// Builds a segmented `ContainerIndex` from SEEKTABLE seek points.
///
/// Falls back to a linear index when only a single seek point exists and
/// `total_samples` is unknown.
fn build_seektable_segments(
    points: &[(u64, u64, u16)],
    audio_start: u64,
    sample_rate: u32,
    total_samples: u64,
    total_size: Option<u64>,
) -> Result<ContainerIndex> {
    let mut segments = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        let (sample_num, stream_off, _) = points[i];
        let byte_offset = audio_start + stream_off;
        let start_secs = sample_num as f64 / sample_rate as f64;

        let Some(bounds) = next_point_bounds(points, i, total_samples, audio_start, total_size, byte_offset) else {
            // Single seek point, unknown total — cannot estimate duration.
            tracing::debug!("✅ FLAC index parsed (mode=linear-fallback, single-seekpoint)");
            return Ok(ContainerIndex {
                init_end_byte: audio_start.saturating_sub(1),
                inner: Inner::Linear {
                    byte_rate: FALLBACK_BYTE_RATE,
                    block_align: 1,
                },
            });
        };

        let end_secs = bounds.next_sample as f64 / sample_rate as f64;
        let byte_size = bounds.next_byte_offset.saturating_sub(byte_offset);
        segments.push(SegmentEntry {
            start_secs,
            end_secs,
            byte_offset,
            byte_size,
        });
    }

    tracing::debug!("✅ FLAC index parsed (mode=seektable)");
    Ok(ContainerIndex {
        init_end_byte: audio_start.saturating_sub(1),
        inner: Inner::Segments(segments),
    })
}
