//! AAC ADTS (Audio Data Transport Stream) index via frame scan.
//!
//! ADTS streams are a sequence of variable-length frames. Each frame begins with
//! a 7-or-9-byte sync word (`0xFFF*`). This module scans the first 128 frames to
//! compute an average frame size and sample rate, then returns a `Linear` index.

use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner};

/// Number of ADTS frames to scan for average size computation.
const SCAN_FRAMES: usize = 128;

/// Parses an AAC ADTS stream and returns a `ContainerIndex`.
///
/// Scans up to `SCAN_FRAMES` frames from `probe` to derive the average frame size
/// and extract the sample rate from the ADTS header.
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
    let (sample_rate, avg_frame_size) =
        scan_frames(probe).ok_or_else(|| Error::parse("no valid ADTS sync frames found in probe"))?;

    if sample_rate == 0 || avg_frame_size == 0 {
        return Err(Error::parse("ADTS scan yielded zero sample_rate or frame_size"));
    }

    // ADTS frames contain 1024 PCM samples each (for AAC-LC)
    let samples_per_frame = 1024.0f64;
    let byte_rate = avg_frame_size as f64 * (sample_rate as f64 / samples_per_frame);

    tracing::debug!("✅ ADTS index parsed");
    Ok(ContainerIndex {
        init_end_byte: 0,
        inner: Inner::Linear {
            byte_rate,
            block_align: 1,
        },
    })
}

/// ADTS sample rate table indexed by sampling_frequency_index (4 bits).
const SAMPLE_RATES: [u32; 16] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350, 0, 0, 0,
];

/// Scans up to `SCAN_FRAMES` ADTS frames and returns `(sample_rate, avg_frame_size_bytes)`.
fn scan_frames(data: &[u8]) -> Option<(u32, usize)> {
    let mut pos = 0usize;
    let mut total_size = 0usize;
    let mut count = 0usize;
    let mut found_sample_rate = 0u32;

    while pos + 7 <= data.len() && count < SCAN_FRAMES {
        // Sync word: 12 bits all-1 (0xFFF)
        if data[pos] != 0xFF || (data[pos + 1] & 0xF0) != 0xF0 {
            pos += 1;
            continue;
        }

        // ID bit (bit 3 of byte 1): 0 = MPEG-4, 1 = MPEG-2
        // protection_absent (bit 0 of byte 1): 1 = no CRC (7-byte header), 0 = CRC (9-byte)
        let protection_absent = data[pos + 1] & 0x01 != 0;
        let header_size = if protection_absent { 7 } else { 9 };
        if pos + header_size > data.len() {
            break;
        }

        // sampling_frequency_index: bits 7-4 of byte 2, shifted by profile bits
        // Byte layout: byte2 = [profile:2][sf_idx:4][private:1][channel:3 hi bit]
        let sf_idx = ((data[pos + 2] >> 2) & 0x0F) as usize;
        let sample_rate = SAMPLE_RATES[sf_idx];

        // frame_length: bits spanning bytes 3-5
        // byte3: [channel 2 lsb][copy:1][home:1][dag:1][orig:1][frame_len hi 2]
        // byte4: [frame_len mid 8]
        // byte5: [frame_len lo 3][fullness hi 5]
        let frame_length = (((data[pos + 3] & 0x03) as usize) << 11)
            | ((data[pos + 4] as usize) << 3)
            | ((data[pos + 5] >> 5) as usize);

        if frame_length < header_size {
            pos += 1;
            continue;
        }

        if sample_rate > 0 && found_sample_rate == 0 {
            found_sample_rate = sample_rate;
        }

        total_size += frame_length;
        count += 1;
        pos += frame_length;
    }

    if count == 0 {
        return None;
    }

    Some((found_sample_rate, total_size / count))
}
