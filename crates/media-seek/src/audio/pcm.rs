//! PCM container index for WAV (RIFF WAVE) and AIFF (IFF FORM) streams.
//!
//! Both formats store uncompressed linear PCM audio. The byte offset for any
//! timestamp is computed exactly from the sample rate, channel count, and
//! bit depth read from the format chunk.

use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner};

/// Parses a RIFF WAVE stream and returns a `ContainerIndex`.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the WAV stream.
///
/// # Errors
///
/// Returns `Error::ParseFailed` if the `fmt ` chunk is missing or malformed.
pub(crate) fn parse_wav(probe: &[u8]) -> Result<ContainerIndex> {
    tracing::debug!(probe_len = probe.len(), "⚙️ Parsing WAV stream");
    // RIFF header: "RIFF" (4) + file_size (4) + "WAVE" (4) = 12 bytes
    if probe.len() < 12 || &probe[0..4] != b"RIFF" || &probe[8..12] != b"WAVE" {
        return Err(Error::parse("not a RIFF WAVE stream"));
    }

    let mut pos = 12usize;
    let mut fmt_found = false;
    let mut channels = 0u16;
    let mut sample_rate = 0u32;
    let mut bits_per_sample = 0u16;
    let mut data_offset: u64 = 0;

    while pos + 8 <= probe.len() {
        let chunk_id = &probe[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(probe[pos + 4..pos + 8].try_into().unwrap()) as usize;
        pos += 8;
        let chunk_end = pos + chunk_size;

        if chunk_id == b"fmt " && chunk_size >= 16 {
            // PCM format chunk: 2 bytes audio_format (1=PCM), 2 channels, 4 sample_rate,
            // 4 byte_rate, 2 block_align, 2 bits_per_sample
            channels = u16::from_le_bytes(probe[pos + 2..pos + 4].try_into().unwrap());
            sample_rate = u32::from_le_bytes(probe[pos + 4..pos + 8].try_into().unwrap());
            bits_per_sample = u16::from_le_bytes(probe[pos + 14..pos + 16].try_into().unwrap());
            fmt_found = true;
        } else if chunk_id == b"data" {
            data_offset = pos as u64; // first byte of the PCM data
            break;
        }

        pos = (chunk_end + 1) & !1; // chunks are word-aligned
    }

    if !fmt_found {
        return Err(Error::parse("WAV fmt chunk not found"));
    }
    if channels == 0 || sample_rate == 0 || bits_per_sample == 0 {
        return Err(Error::parse("WAV fmt chunk has zero sample parameters"));
    }

    let block_align = (channels as u64) * (bits_per_sample as u64).div_ceil(8);
    let byte_rate = sample_rate as f64 * block_align as f64;

    tracing::debug!("✅ WAV index parsed");
    Ok(ContainerIndex {
        init_end_byte: data_offset.saturating_sub(1),
        inner: Inner::Linear { byte_rate, block_align },
    })
}

/// Parses an IFF FORM/AIFF or FORM/AIFC stream and returns a `ContainerIndex`.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the AIFF stream.
///
/// # Errors
///
/// Returns `Error::ParseFailed` if the `COMM` chunk is missing or malformed.
pub(crate) fn parse_aiff(probe: &[u8]) -> Result<ContainerIndex> {
    tracing::debug!(probe_len = probe.len(), "⚙️ Parsing AIFF stream");
    // FORM header: "FORM" (4) + size (4) + "AIFF"/"AIFC" (4) = 12 bytes
    if probe.len() < 12 || &probe[0..4] != b"FORM" {
        return Err(Error::parse("not a FORM IFF stream"));
    }
    let form_type = &probe[8..12];
    if form_type != b"AIFF" && form_type != b"AIFC" {
        return Err(Error::parse("not an AIFF/AIFC stream"));
    }

    let mut pos = 12usize;
    let mut comm_found = false;
    let mut channels = 0u16;
    let mut sample_rate = 0u32;
    let mut bits_per_sample = 0u16;
    let mut ssnd_offset: u64 = 0;

    while pos + 8 <= probe.len() {
        let chunk_id = &probe[pos..pos + 4];
        let chunk_size = u32::from_be_bytes(probe[pos + 4..pos + 8].try_into().unwrap()) as usize;
        pos += 8;
        let chunk_end = pos + chunk_size;

        if chunk_id == b"COMM" && chunk_size >= 18 {
            channels = u16::from_be_bytes(probe[pos..pos + 2].try_into().unwrap());
            // 4 bytes num_sample_frames, then 2 bytes bit_depth, then 10-byte 80-bit extended SR
            bits_per_sample = u16::from_be_bytes(probe[pos + 6..pos + 8].try_into().unwrap());
            sample_rate = read_ieee754_extended(&probe[pos + 8..pos + 18]);
            comm_found = true;
        } else if chunk_id == b"SSND" {
            // SSND: 4 byte offset field + 4 byte block size field, then PCM data
            let ssnd_data_offset = u32::from_be_bytes(probe[pos..pos + 4].try_into().unwrap()) as u64;
            ssnd_offset = pos as u64 + 8 + ssnd_data_offset; // skip offset+block fields
            break;
        }

        pos = (chunk_end + 1) & !1;
    }

    if !comm_found {
        return Err(Error::parse("AIFF COMM chunk not found"));
    }
    if channels == 0 || sample_rate == 0 || bits_per_sample == 0 {
        return Err(Error::parse("AIFF COMM chunk has zero sample parameters"));
    }

    let block_align = (channels as u64) * (bits_per_sample as u64).div_ceil(8);
    let byte_rate = sample_rate as f64 * block_align as f64;

    tracing::debug!("✅ AIFF index parsed");
    Ok(ContainerIndex {
        init_end_byte: ssnd_offset.saturating_sub(1),
        inner: Inner::Linear { byte_rate, block_align },
    })
}

/// Converts an 80-bit IEEE 754 extended precision float to a `u32` sample rate.
///
/// The 10-byte representation: 1-bit sign, 15-bit exponent, 64-bit mantissa (no hidden bit).
fn read_ieee754_extended(data: &[u8]) -> u32 {
    if data.len() < 10 {
        return 0;
    }
    let exponent = (((data[0] & 0x7F) as i32) << 8) | data[1] as i32;
    let mantissa = u64::from_be_bytes(data[2..10].try_into().unwrap());
    if exponent == 0 {
        return 0;
    }
    let shift = 63 - (exponent - 16383);
    if !(0..=32).contains(&shift) {
        return 0;
    }
    (mantissa >> shift) as u32
}
