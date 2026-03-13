//! FLAC container index parsing via the SEEKTABLE metadata block.
//!
//! FLAC streams begin with `fLaC` followed by a sequence of metadata blocks.
//! The SEEKTABLE block (type 3) contains seek points mapping sample numbers to
//! byte offsets and audio frame sample counts.

use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

const FLAC_MARKER: &[u8; 4] = b"fLaC";
const BLOCK_TYPE_STREAMINFO: u8 = 0;
const BLOCK_TYPE_SEEKTABLE: u8 = 3;
const SEEKTABLE_PLACEHOLDER: u64 = u64::MAX;
/// Minimum STREAMINFO block size in bytes.
const STREAMINFO_MIN_SIZE: usize = 18;
/// Size of a single SEEKTABLE entry in bytes.
const SEEK_POINT_SIZE: usize = 18;
/// Fallback byte rate when total samples are unknown (128 kbps / 8).
const FALLBACK_BYTE_RATE: f64 = 128_000.0 / 8.0;

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

    let mut pos = 4usize;
    let mut sample_rate: u32 = 0;
    let mut total_samples: u64 = 0;
    let mut seek_points: Option<Vec<(u64, u64, u16)>> = None; // (sample_number, stream_offset, frame_samples)
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
            // Truncated block body — record audio_start if this is the last block,
            // then advance pos so the loop exits on the next header check.
            if is_last {
                audio_start = block_end as u64;
            }
            pos = block_end; // exceeds probe.len(); loop exits on next iteration
            continue;
        }

        match block_type {
            BLOCK_TYPE_STREAMINFO => {
                if block_len >= STREAMINFO_MIN_SIZE {
                    let sr_word = u32::from_be_bytes(probe[pos + 10..pos + 14].try_into().unwrap());
                    sample_rate = sr_word >> 12;
                    // bits [35:0] of bytes 13-17 encode total_samples (36 bits)
                    // byte 13 has low 4 bits of sr, then bits 35-32 of total_samples in bits 3-0
                    let ts_hi = (probe[pos + 13] & 0x0F) as u64;
                    let ts_lo = u32::from_be_bytes(probe[pos + 14..pos + 18].try_into().unwrap()) as u64;
                    total_samples = (ts_hi << 32) | ts_lo;
                }
            }
            BLOCK_TYPE_SEEKTABLE => {
                let n = block_len / SEEK_POINT_SIZE;
                let mut points = Vec::with_capacity(n);
                for i in 0..n {
                    let off = pos + i * SEEK_POINT_SIZE;
                    if off + SEEK_POINT_SIZE > probe.len() {
                        break;
                    }
                    let sample_num = u64::from_be_bytes(probe[off..off + 8].try_into().unwrap());
                    let stream_off = u64::from_be_bytes(probe[off + 8..off + 16].try_into().unwrap());
                    let frame_samples = u16::from_be_bytes(probe[off + 16..off + 18].try_into().unwrap());
                    if sample_num != SEEKTABLE_PLACEHOLDER {
                        points.push((sample_num, stream_off, frame_samples));
                    }
                }
                seek_points = Some(points);
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
        return Err(Error::parse("FLAC STREAMINFO missing or sample_rate is zero"));
    }

    if let Some(points) = seek_points.filter(|p| !p.is_empty()) {
        let mut segments = Vec::with_capacity(points.len());
        for i in 0..points.len() {
            let (sample_num, stream_off, _) = points[i];
            let byte_offset = audio_start + stream_off;
            let start_secs = sample_num as f64 / sample_rate as f64;
            let (next_sample, next_off) = if i + 1 < points.len() {
                (points[i + 1].0, audio_start + points[i + 1].1)
            } else {
                // Last segment: use total_samples if known, otherwise estimate from
                // the previous segment's duration to avoid end_secs=0 inversion.
                let last_sample = if total_samples > 0 {
                    total_samples
                } else if i > 0 {
                    // Estimate: extend by the same interval as the previous seek point gap.
                    // Use saturating arithmetic to guard against non-monotonic SEEKTABLE entries.
                    let prev_sample = points[i - 1].0;
                    let gap = sample_num.saturating_sub(prev_sample);
                    sample_num.saturating_add(gap)
                } else {
                    // Single seek point with unknown total: cannot estimate duration reliably.
                    // Fall back to a linear index using the fallback byte rate.
                    tracing::debug!("✅ FLAC index parsed (mode=linear-fallback, single-seekpoint)");
                    return Ok(ContainerIndex {
                        init_end_byte: audio_start.saturating_sub(1),
                        inner: Inner::Linear {
                            byte_rate: FALLBACK_BYTE_RATE,
                            block_align: 1,
                        },
                    });
                };
                (last_sample, byte_offset)
            };
            let end_secs = next_sample as f64 / sample_rate as f64;
            let byte_size = next_off.saturating_sub(byte_offset);
            segments.push(SegmentEntry {
                start_secs,
                end_secs,
                byte_offset,
                byte_size,
            });
        }
        tracing::debug!("✅ FLAC index parsed (mode=seektable)");
        return Ok(ContainerIndex {
            init_end_byte: audio_start.saturating_sub(1),
            inner: Inner::Segments(segments),
        });
    }

    // No SEEKTABLE — fall back to linear (FLAC is lossless CBR for a given encoding; this is
    // an approximation based on the STREAMINFO total_samples and the actual file size when known).
    let total_secs = if total_samples > 0 {
        total_samples as f64 / sample_rate as f64
    } else {
        0.0
    };
    let audio_bytes = total_size.unwrap_or(probe.len() as u64).saturating_sub(audio_start) as f64;
    let byte_rate = if total_secs > 0.0 {
        audio_bytes / total_secs
    } else {
        FALLBACK_BYTE_RATE
    };

    tracing::debug!("✅ FLAC index parsed (mode=linear)");
    Ok(ContainerIndex {
        init_end_byte: audio_start.saturating_sub(1),
        inner: Inner::Linear {
            byte_rate,
            block_align: 1,
        },
    })
}
