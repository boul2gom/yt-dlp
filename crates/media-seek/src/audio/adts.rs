//! AAC ADTS (Audio Data Transport Stream) index via frame scan.
//!
//! ADTS streams are a sequence of variable-length frames. Each frame begins with
//! a 7-or-9-byte sync word (`0xFFF*`). This module scans the first [`SCAN_FRAMES`]
//! frames to compute the total byte count and the total decoded sample count, then
//! derives a `Linear` byte rate from those totals.
//!
//! **HE-AAC / SBR limitation:** the ADTS frame header encodes the base AAC profile
//! (usually LC = 1), not whether Spectral Band Replication (SBR) is active. SBR
//! doubles the effective frame duration (2048 samples per QMF frame instead of 1024),
//! but that information is embedded in the Audio Specific Config, which is not
//! carried in raw ADTS headers. The byte rate estimate will therefore be 2× too high
//! for HE-AAC streams. There is no reliable way to detect SBR from ADTS headers alone.

use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner};

// ==================== Constants ====================

/// Number of ADTS frames to scan for the byte-rate estimate.
const SCAN_FRAMES: usize = 128;

/// Number of PCM samples per AAC-LC ADTS frame (1024 at the input sample rate).
///
/// Note: HE-AAC (SBR) effectively produces 2048 samples per QMF frame, but SBR
/// cannot be detected from the ADTS sync header. This constant is therefore correct
/// for AAC-LC and AAC-Main, and will cause a 2× overestimate for HE-AAC streams.
const AAC_LC_SAMPLES_PER_FRAME: u64 = 1024;

/// ADTS header size without CRC (protection_absent = 1).
const ADTS_HEADER_NO_CRC: usize = 7;

/// ADTS header size with CRC (protection_absent = 0).
const ADTS_HEADER_WITH_CRC: usize = 9;

// ==================== Internal struct ====================

/// Result of scanning ADTS frames for byte-rate estimation.
#[derive(Debug)]
struct AdtsScanResult {
    /// Sample rate from the first valid ADTS frame header.
    sample_rate: u32,
    /// Total bytes across all scanned frames (including headers).
    total_bytes: u64,
    /// Number of frames successfully scanned.
    frame_count: u64,
}

// ==================== Public entry point ====================

/// Parses an AAC ADTS stream and returns a `ContainerIndex`.
///
/// Scans up to [`SCAN_FRAMES`] frames from `probe` to derive the byte rate
/// and extract the sample rate from the ADTS frame headers.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the ADTS stream.
///
/// # Errors
///
/// Returns `Error::ParseFailed` if no valid ADTS sync frame is found.
pub(crate) fn parse(probe: &[u8]) -> Result<ContainerIndex> {
    tracing::debug!(probe_len = probe.len(), "⚙️ Parsing ADTS/AAC stream");
    let scan = scan_frames(probe).ok_or_else(|| Error::parse("no valid ADTS sync frames found in probe"))?;

    if scan.sample_rate == 0 || scan.frame_count == 0 {
        return Err(Error::parse("ADTS scan yielded zero sample_rate or frame_count"));
    }

    // Derive byte rate directly from total bytes and total decoded duration,
    // avoiding intermediate integer truncation from avg_frame_size = total / count.
    let total_duration_secs = scan.frame_count as f64 * AAC_LC_SAMPLES_PER_FRAME as f64 / scan.sample_rate as f64;
    let byte_rate = scan.total_bytes as f64 / total_duration_secs;

    tracing::debug!(
        sample_rate = scan.sample_rate,
        frame_count = scan.frame_count,
        total_bytes = scan.total_bytes,
        byte_rate,
        "✅ ADTS index parsed"
    );
    Ok(ContainerIndex {
        init_end_byte: 0,
        inner: Inner::Linear {
            byte_rate,
            block_align: 1,
        },
    })
}

// ==================== Internal helpers ====================

/// ADTS sample rate table indexed by `sampling_frequency_index` (4 bits, 0–15).
const SAMPLE_RATES: [u32; 16] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350, 0, 0, 0,
];

/// Parsed fields from a single ADTS frame header.
struct AdtsFrameHeader {
    /// Total frame size in bytes (including the header itself).
    frame_length: usize,
    /// Sample rate in Hz decoded from the `sampling_frequency_index` field.
    sample_rate: u32,
}

/// Parses one ADTS frame header starting at `data[pos]`.
///
/// Returns `None` if the sync word is missing, the data is too short, or the
/// computed `frame_length` is smaller than the header itself.
fn parse_adts_frame_header(data: &[u8], pos: usize) -> Option<AdtsFrameHeader> {
    // Need at least 7 bytes for a minimal ADTS header.
    if pos + 7 > data.len() {
        return None;
    }
    // Sync word: 12 bits all-1 (0xFFF*).
    if data[pos] != 0xFF || (data[pos + 1] & 0xF0) != 0xF0 {
        return None;
    }
    // protection_absent (bit 0 of byte 1): 1 = no CRC (7-byte header), 0 = CRC (9-byte).
    let header_size = if data[pos + 1] & 0x01 != 0 {
        ADTS_HEADER_NO_CRC
    } else {
        ADTS_HEADER_WITH_CRC
    };
    if pos + header_size > data.len() {
        return None;
    }
    // sampling_frequency_index: bits 5-2 of byte 2.
    // Byte layout: byte2 = [profile:2][sf_idx:4][private:1][channel hi:1].
    let sf_idx = ((data[pos + 2] >> 2) & 0x0F) as usize;
    let sample_rate = SAMPLE_RATES[sf_idx];
    // frame_length spans bytes 3-5:
    // byte3[1:0] + byte4[7:0] + byte5[7:5] = 13 bits total.
    let frame_length =
        (((data[pos + 3] & 0x03) as usize) << 11) | ((data[pos + 4] as usize) << 3) | ((data[pos + 5] >> 5) as usize);
    if frame_length < header_size {
        return None;
    }
    Some(AdtsFrameHeader {
        frame_length,
        sample_rate,
    })
}

/// Scans up to [`SCAN_FRAMES`] ADTS frames and returns an [`AdtsScanResult`].
///
/// Returns `None` if no valid sync frame is found.
fn scan_frames(data: &[u8]) -> Option<AdtsScanResult> {
    let mut pos = 0usize;
    let mut total_bytes = 0u64;
    let mut frame_count = 0u64;
    let mut found_sample_rate = 0u32;

    while pos + 7 <= data.len() && frame_count < SCAN_FRAMES as u64 {
        let Some(hdr) = parse_adts_frame_header(data, pos) else {
            pos += 1;
            continue;
        };

        if hdr.sample_rate > 0 && found_sample_rate == 0 {
            found_sample_rate = hdr.sample_rate;
        }

        total_bytes += hdr.frame_length as u64;
        frame_count += 1;
        pos += hdr.frame_length;
    }

    if frame_count == 0 {
        return None;
    }

    Some(AdtsScanResult {
        sample_rate: found_sample_rate,
        total_bytes,
        frame_count,
    })
}
