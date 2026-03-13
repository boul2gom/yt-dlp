//! PCM container index for WAV (RIFF WAVE / RF64 / BW64) and AIFF (IFF FORM) streams.
//!
//! WAV files store the byte rate and block alignment directly in the `fmt ` chunk,
//! which works for both PCM and compressed formats (ADPCM, IEEE float, etc.).
//! RF64/BW64 (RIFF64) large-file variants are also supported by accepting the
//! `RF64` / `BW64` magic bytes in addition to `RIFF`.
//!
//! AIFF/AIFC files use a linear PCM index. AIFC compressed formats (µ-law,
//! MACE3/6, ima4, …) are rejected with an explicit error.

use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner};

/// RIFF magic bytes for standard WAV files.
const RIFF_MAGIC: &[u8; 4] = b"RIFF";
/// RF64 magic bytes for large WAV files (ITU-R BS.2088).
const RF64_MAGIC: &[u8; 4] = b"RF64";
/// BW64 magic bytes for large broadcast WAV files (EBU R 148).
const BW64_MAGIC: &[u8; 4] = b"BW64";

// AIFC uncompressed compression types (PCM formula applies to all of these).
const AIFC_NONE: &[u8; 4] = b"NONE"; // raw big-endian twos-complement
const AIFC_TWOS: &[u8; 4] = b"twos"; // big-endian twos-complement (alias)
const AIFC_SOWT: &[u8; 4] = b"sowt"; // little-endian twos-complement
const AIFC_FL32: &[u8; 4] = b"fl32"; // IEEE 754 float 32-bit
const AIFC_FL64: &[u8; 4] = b"fl64"; // IEEE 754 float 64-bit

/// Parses a RIFF WAVE (or RF64/BW64) stream and returns a `ContainerIndex`.
///
/// Reads `byte_rate` and `block_align` directly from the `fmt ` chunk, which
/// are correct for all WAV sub-formats including PCM, IEEE float, ADPCM, and
/// other compressed variants.
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
    // RIFF header: magic (4) + file_size (4) + "WAVE" (4) = 12 bytes
    if probe.len() < 12 {
        return Err(Error::parse("WAV probe too short"));
    }
    let magic = &probe[0..4];
    if magic != RIFF_MAGIC && magic != RF64_MAGIC && magic != BW64_MAGIC {
        return Err(Error::parse("not a RIFF/RF64/BW64 WAVE stream"));
    }
    if &probe[8..12] != b"WAVE" {
        return Err(Error::parse("not a RIFF WAVE stream"));
    }

    let mut pos = 12usize;
    let mut fmt_found = false;
    let mut byte_rate: f64 = 0.0;
    let mut block_align: u64 = 0;
    let mut channels: u16 = 0;
    let mut sample_rate: u32 = 0;
    let mut bits_per_sample: u16 = 0;
    let mut data_offset: u64 = 0;

    while pos + 8 <= probe.len() {
        let chunk_id = &probe[pos..pos + 4];
        let mut chunk_size = u32::from_le_bytes(probe[pos + 4..pos + 8].try_into().unwrap()) as usize;
        pos += 8;

        // For RF64/BW64, a chunk size of 0xFFFFFFFF means the real size is in the ds64 chunk.
        // For seeking we only care about fmt  and data chunk *positions*, so for the ds64 chunk
        // itself we handle it inline; for oversized data chunks the start offset is what matters.
        if chunk_id == b"ds64" && chunk_size >= 28 {
            // ds64 layout (after 8-byte chunk header we already consumed):
            // 8 bytes riff_size_low/high, 8 bytes data_size_low/high, 8 bytes sample_count,
            // 4 bytes table_length + entries — we don't need any of this for linear seeking.
            pos += chunk_size;
            pos = (pos + 1) & !1;
            continue;
        }

        if chunk_id == b"fmt " && chunk_size >= 16 {
            // fmt  layout (at least 16 bytes for PCM; extended formats add more):
            // 2  audio_format (wFormatTag)
            // 2  channels
            // 4  sample_rate
            // 4  byte_rate   (nAvgBytesPerSec) — authoritative for all formats
            // 2  block_align (nBlockAlign)      — authoritative for all formats
            // 2  bits_per_sample
            channels = u16::from_le_bytes(probe[pos + 2..pos + 4].try_into().unwrap());
            sample_rate = u32::from_le_bytes(probe[pos + 4..pos + 8].try_into().unwrap());
            byte_rate = u32::from_le_bytes(probe[pos + 8..pos + 12].try_into().unwrap()) as f64;
            block_align = u16::from_le_bytes(probe[pos + 12..pos + 14].try_into().unwrap()) as u64;
            bits_per_sample = u16::from_le_bytes(probe[pos + 14..pos + 16].try_into().unwrap());
            fmt_found = true;
        } else if chunk_id == b"data" {
            data_offset = pos as u64; // first byte of the audio data
            break;
        }

        // Handle 0xFFFFFFFF size in RF64 for non-ds64 chunks gracefully — skip forward by 0
        // (we cannot know the real size here without the ds64 table) but avoid underflow.
        if chunk_size == 0xFFFF_FFFF {
            chunk_size = 0;
        }

        pos += chunk_size;
        pos = (pos + 1) & !1; // chunks are word-aligned
    }

    if !fmt_found {
        return Err(Error::parse("WAV fmt chunk not found"));
    }
    if channels == 0 || sample_rate == 0 {
        return Err(Error::parse("WAV fmt chunk has zero sample parameters"));
    }

    // Fallback: if nAvgBytesPerSec is zero (malformed file), compute from PCM formula.
    if byte_rate == 0.0 && bits_per_sample > 0 {
        let computed_align = (channels as u64) * (bits_per_sample as u64).div_ceil(8);
        byte_rate = sample_rate as f64 * computed_align as f64;
        block_align = computed_align;
    }
    if byte_rate == 0.0 {
        return Err(Error::parse("WAV fmt chunk has zero byte rate and cannot be computed"));
    }
    if block_align == 0 {
        block_align = 1;
    }

    tracing::debug!("✅ WAV index parsed");
    Ok(ContainerIndex {
        init_end_byte: data_offset.saturating_sub(1),
        inner: Inner::Linear { byte_rate, block_align },
    })
}

/// Parses an IFF FORM/AIFF or FORM/AIFC stream and returns a `ContainerIndex`.
///
/// For AIFC files, only uncompressed compression types (`NONE`, `twos`, `sowt`,
/// `fl32`, `fl64`) are supported. Compressed AIFC (µ-law, a-law, MACE, ima4, …)
/// returns an error.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the AIFF stream.
///
/// # Errors
///
/// Returns `Error::ParseFailed` if the `COMM` chunk is missing, malformed, or
/// uses an unsupported compressed AIFC codec.
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
    let is_aifc = form_type == b"AIFC";

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

        if chunk_id == b"COMM" {
            // Standard AIFF COMM: 18 bytes minimum.
            // AIFC COMM: 22 bytes minimum (18 standard + 4 compressionType).
            let min_size = if is_aifc { 22 } else { 18 };
            if chunk_size >= min_size {
                channels = u16::from_be_bytes(probe[pos..pos + 2].try_into().unwrap());
                // 4 bytes num_sample_frames, then 2 bytes bit_depth, then 10-byte 80-bit extended SR
                bits_per_sample = u16::from_be_bytes(probe[pos + 6..pos + 8].try_into().unwrap());
                sample_rate = read_ieee754_extended(&probe[pos + 8..pos + 18]);

                if is_aifc {
                    // AIFC COMM has 4-byte compressionType immediately after the 18-byte base.
                    let codec = &probe[pos + 18..pos + 22];
                    if codec != AIFC_NONE
                        && codec != AIFC_TWOS
                        && codec != AIFC_SOWT
                        && codec != AIFC_FL32
                        && codec != AIFC_FL64
                    {
                        return Err(Error::parse(format!(
                            "AIFC compressed codec {:?} not supported for seeking",
                            std::str::from_utf8(codec).unwrap_or("????")
                        )));
                    }
                }
                comm_found = true;
            }
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
    if !(0..=63).contains(&shift) {
        return 0;
    }
    (mantissa >> shift) as u32
}
