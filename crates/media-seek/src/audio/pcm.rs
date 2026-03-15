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
pub(crate) const RIFF_MAGIC: &[u8; 4] = b"RIFF";
/// RF64 magic bytes for large WAV files (ITU-R BS.2088).
pub(crate) const RF64_MAGIC: &[u8; 4] = b"RF64";
/// BW64 magic bytes for large broadcast WAV files (EBU R 148).
pub(crate) const BW64_MAGIC: &[u8; 4] = b"BW64";
/// IFF FORM magic bytes — AIFF and AIFC container prefix.
pub(crate) const FORM_MAGIC: &[u8; 4] = b"FORM";
/// AIFF sub-type identifier at offset 8 of a FORM container.
pub(crate) const AIFF_SUBTYPE: &[u8; 4] = b"AIFF";
/// AIFC sub-type identifier at offset 8 of a FORM container.
pub(crate) const AIFC_SUBTYPE: &[u8; 4] = b"AIFC";
/// WAV sub-type identifier at offset 8 of a RIFF container.
pub(crate) const WAV_SUBTYPE: &[u8; 4] = b"WAVE";

// AIFC uncompressed compression types (PCM formula applies to all of these).
const AIFC_NONE: &[u8; 4] = b"NONE"; // raw big-endian twos-complement
const AIFC_TWOS: &[u8; 4] = b"twos"; // big-endian twos-complement (alias)
const AIFC_SOWT: &[u8; 4] = b"sowt"; // little-endian twos-complement
const AIFC_FL32: &[u8; 4] = b"fl32"; // IEEE 754 float 32-bit
const AIFC_FL64: &[u8; 4] = b"fl64"; // IEEE 754 float 64-bit

/// Minimum RIFF/IFF header size: magic (4) + size (4) + form type (4).
const CONTAINER_HEADER_SIZE: usize = 12;

/// Minimum chunk header size: chunk_id (4) + chunk_size (4).
const CHUNK_HEADER_SIZE: usize = 8;

/// Minimum `fmt ` chunk payload size for all WAV sub-formats.
const WAV_FMT_MIN_SIZE: usize = 16;

/// Sentinel value used by RF64/BW64 to indicate the real size is in the `ds64` chunk.
const RF64_SENTINEL_SIZE: usize = 0xFFFF_FFFF;

/// Minimum `ds64` chunk payload size (riff_size + data_size + sample_count + table_length).
const DS64_MIN_PAYLOAD: usize = 28;

/// Minimum standard AIFF `COMM` chunk payload size in bytes.
const AIFF_COMM_MIN_SIZE: usize = 18;

/// Minimum AIFC `COMM` chunk payload size (standard 18 + 4-byte compressionType).
const AIFC_COMM_MIN_SIZE: usize = 22;

/// Byte offset of the `compressionType` field within an AIFC `COMM` payload.
const AIFC_CODEC_OFFSET: usize = 18;

/// Byte length of the `compressionType` field in an AIFC `COMM` payload.
const AIFC_CODEC_LEN: usize = 4;

/// Byte offset of the `channels` field within a `COMM` payload.
const COMM_CHANNELS_OFFSET: usize = 0;

/// Byte offset of the `sampleSize` (bits_per_sample) field within a `COMM` payload.
const COMM_BITS_PER_SAMPLE_OFFSET: usize = 6;

/// Byte offset of the 80-bit IEEE 754 extended sample rate within a `COMM` payload.
const COMM_SAMPLE_RATE_OFFSET: usize = 8;

/// Byte length of the 80-bit IEEE 754 extended sample rate field.
const COMM_SAMPLE_RATE_LEN: usize = 10;

/// Byte offset of the `offset` field within an `SSND` chunk payload (before audio data).
const SSND_OFFSET_FIELD: usize = 0;

/// Combined size of the `offset` (4) and `blockSize` (4) fields at the start of an SSND payload.
const SSND_HEADER_FIELDS: u64 = 8;

/// Holds the fields extracted from a WAV `fmt ` and `data` chunk scan.
struct WavChunkResult {
    byte_rate: f64,
    block_align: u64,
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
    data_offset: u64,
}

/// Holds the fields extracted from an AIFF `COMM` and `SSND` chunk scan.
struct AiffChunkResult {
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
    ssnd_offset: u64,
}

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
    if probe.len() < CONTAINER_HEADER_SIZE {
        return Err(Error::parse("WAV probe too short"));
    }
    let magic = &probe[0..4];
    if magic != RIFF_MAGIC && magic != RF64_MAGIC && magic != BW64_MAGIC {
        return Err(Error::parse("not a RIFF/RF64/BW64 WAVE stream"));
    }
    if &probe[8..12] != b"WAVE" {
        return Err(Error::parse("not a RIFF WAVE stream"));
    }

    let result = scan_wav_chunks(probe)?;

    // Fallback: if nAvgBytesPerSec is zero (malformed file), compute from PCM formula.
    let (byte_rate, block_align) = if result.byte_rate == 0.0 && result.bits_per_sample > 0 {
        let computed_align = (result.channels as u64) * (result.bits_per_sample as u64).div_ceil(8);
        (result.sample_rate as f64 * computed_align as f64, computed_align)
    } else {
        (result.byte_rate, result.block_align)
    };

    if byte_rate == 0.0 {
        return Err(Error::parse("WAV fmt chunk has zero byte rate and cannot be computed"));
    }
    let block_align = if block_align == 0 { 1 } else { block_align };

    tracing::debug!("✅ WAV index parsed");
    Ok(ContainerIndex {
        init_end_byte: result.data_offset.saturating_sub(1),
        inner: Inner::Linear { byte_rate, block_align },
    })
}

/// Fields extracted from a WAV `fmt ` chunk payload.
#[derive(Debug)]
struct WavFmtFields {
    channels: u16,
    sample_rate: u32,
    byte_rate: f64,
    block_align: u64,
    bits_per_sample: u16,
}

/// Reads the five key fields from a `fmt ` chunk payload starting at `offset` in `probe`.
///
/// Assumes `probe[offset..]` is at least `WAV_FMT_MIN_SIZE` bytes.
fn parse_fmt_chunk(probe: &[u8], offset: usize) -> WavFmtFields {
    WavFmtFields {
        channels: u16::from_le_bytes(probe[offset + 2..offset + 4].try_into().unwrap()),
        sample_rate: u32::from_le_bytes(probe[offset + 4..offset + 8].try_into().unwrap()),
        byte_rate: u32::from_le_bytes(probe[offset + 8..offset + 12].try_into().unwrap()) as f64,
        block_align: u16::from_le_bytes(probe[offset + 12..offset + 14].try_into().unwrap()) as u64,
        bits_per_sample: u16::from_le_bytes(probe[offset + 14..offset + 16].try_into().unwrap()),
    }
}

/// Scans WAV chunks starting at byte 12 (after the RIFF/RF64/BW64 + WAVE header).
///
/// Returns the parsed fields from the `fmt ` chunk and the byte offset of the `data` chunk.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the WAV stream (must be at least 12 bytes).
///
/// # Errors
///
/// Returns `Error::ParseFailed` if the `fmt ` chunk is not found, or if the
/// chunk has zero `channels` or `sample_rate`.
fn scan_wav_chunks(probe: &[u8]) -> Result<WavChunkResult> {
    let mut pos = CONTAINER_HEADER_SIZE;
    let mut fmt_found = false;
    let mut byte_rate: f64 = 0.0;
    let mut block_align: u64 = 0;
    let mut channels: u16 = 0;
    let mut sample_rate: u32 = 0;
    let mut bits_per_sample: u16 = 0;
    let mut data_offset: u64 = 0;

    while pos + CHUNK_HEADER_SIZE <= probe.len() {
        let chunk_id = &probe[pos..pos + 4];
        let mut chunk_size = u32::from_le_bytes(probe[pos + 4..pos + 8].try_into().unwrap()) as usize;
        pos += CHUNK_HEADER_SIZE;

        // For RF64/BW64, a chunk size of 0xFFFFFFFF means the real size is in the ds64 chunk.
        // For seeking we only care about fmt  and data chunk *positions*, so for the ds64 chunk
        // itself we handle it inline; for oversized data chunks the start offset is what matters.
        if chunk_id == b"ds64" && chunk_size >= DS64_MIN_PAYLOAD {
            // ds64 layout (after 8-byte chunk header we already consumed):
            // 8 bytes riff_size_low/high, 8 bytes data_size_low/high, 8 bytes sample_count,
            // 4 bytes table_length + entries — we don't need any of this for linear seeking.
            pos += chunk_size;
            pos = (pos + 1) & !1;
            continue;
        }

        if chunk_id == b"fmt " && chunk_size >= WAV_FMT_MIN_SIZE {
            // fmt  layout (at least 16 bytes for PCM; extended formats add more):
            // 2  audio_format (wFormatTag)
            // 2  channels
            // 4  sample_rate
            // 4  byte_rate   (nAvgBytesPerSec) — authoritative for all formats
            // 2  block_align (nBlockAlign)      — authoritative for all formats
            // 2  bits_per_sample
            let fmt = parse_fmt_chunk(probe, pos);
            channels = fmt.channels;
            sample_rate = fmt.sample_rate;
            byte_rate = fmt.byte_rate;
            block_align = fmt.block_align;
            bits_per_sample = fmt.bits_per_sample;
            fmt_found = true;
        } else if chunk_id == b"data" {
            data_offset = pos as u64; // first byte of the audio data
            break;
        }

        // Handle 0xFFFFFFFF size in RF64 for non-ds64 chunks gracefully — skip forward by 0
        // (we cannot know the real size here without the ds64 table) but avoid underflow.
        if chunk_size == RF64_SENTINEL_SIZE {
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

    Ok(WavChunkResult {
        byte_rate,
        block_align,
        channels,
        sample_rate,
        bits_per_sample,
        data_offset,
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
    if probe.len() < CONTAINER_HEADER_SIZE || &probe[0..4] != b"FORM" {
        return Err(Error::parse("not a FORM IFF stream"));
    }
    let form_type = &probe[8..12];
    if form_type != b"AIFF" && form_type != b"AIFC" {
        return Err(Error::parse("not an AIFF/AIFC stream"));
    }
    let is_aifc = form_type == b"AIFC";

    let result = scan_aiff_chunks(probe, is_aifc)?;

    let block_align = (result.channels as u64) * (result.bits_per_sample as u64).div_ceil(8);
    let byte_rate = result.sample_rate as f64 * block_align as f64;

    tracing::debug!("✅ AIFF index parsed");
    Ok(ContainerIndex {
        init_end_byte: result.ssnd_offset.saturating_sub(1),
        inner: Inner::Linear { byte_rate, block_align },
    })
}

/// Validates that an AIFC `compressionType` is one of the supported uncompressed codecs.
///
/// # Arguments
///
/// * `codec` - The 4-byte compression type field from an AIFC `COMM` chunk.
///
/// # Errors
///
/// Returns `Error::ParseFailed` when `codec` names a compressed AIFC codec.
fn validate_aifc_codec(codec: &[u8]) -> Result<()> {
    if codec == AIFC_NONE || codec == AIFC_TWOS || codec == AIFC_SOWT || codec == AIFC_FL32 || codec == AIFC_FL64 {
        return Ok(());
    }
    Err(Error::parse(format!(
        "AIFC compressed codec {:?} not supported for seeking",
        std::str::from_utf8(codec).unwrap_or("????")
    )))
}

/// Parses the `COMM` chunk payload and returns `(channels, sample_rate, bits_per_sample)`.
///
/// # Errors
///
/// Returns `Error::ParseFailed` when the chunk is smaller than the required minimum or
/// (for AIFC) when the codec is a compressed type.
fn parse_comm_chunk(probe: &[u8], pos: usize, chunk_size: usize, is_aifc: bool) -> Result<(u16, u32, u16)> {
    // Standard AIFF COMM: 18 bytes minimum.
    // AIFC COMM: 22 bytes minimum (18 standard + 4 compressionType).
    let min_size = if is_aifc {
        AIFC_COMM_MIN_SIZE
    } else {
        AIFF_COMM_MIN_SIZE
    };
    if chunk_size < min_size {
        return Err(Error::parse("AIFF COMM chunk too small"));
    }

    let channels = u16::from_be_bytes(
        probe[pos + COMM_CHANNELS_OFFSET..pos + COMM_CHANNELS_OFFSET + 2]
            .try_into()
            .unwrap(),
    );
    // 4 bytes num_sample_frames, then 2 bytes bit_depth, then 10-byte 80-bit extended SR
    let bits_per_sample = u16::from_be_bytes(
        probe[pos + COMM_BITS_PER_SAMPLE_OFFSET..pos + COMM_BITS_PER_SAMPLE_OFFSET + 2]
            .try_into()
            .unwrap(),
    );
    let sample_rate = read_ieee754_extended(
        &probe[pos + COMM_SAMPLE_RATE_OFFSET..pos + COMM_SAMPLE_RATE_OFFSET + COMM_SAMPLE_RATE_LEN],
    );

    if is_aifc {
        // AIFC COMM has 4-byte compressionType immediately after the 18-byte base.
        let codec = &probe[pos + AIFC_CODEC_OFFSET..pos + AIFC_CODEC_OFFSET + AIFC_CODEC_LEN];
        validate_aifc_codec(codec)?;
    }

    Ok((channels, sample_rate, bits_per_sample))
}

/// Scans AIFF/AIFC chunks starting at byte 12 (after the FORM + type header).
///
/// Returns the parsed fields from the `COMM` chunk and the absolute byte offset of
/// the first audio sample inside the `SSND` chunk.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the AIFF/AIFC stream.
/// * `is_aifc` - `true` when the form type is `AIFC`; enables codec validation.
///
/// # Errors
///
/// Returns `Error::ParseFailed` if the `COMM` chunk is not found, malformed, has
/// zero sample parameters, or (for AIFC) uses a compressed codec.
fn scan_aiff_chunks(probe: &[u8], is_aifc: bool) -> Result<AiffChunkResult> {
    let mut pos = CONTAINER_HEADER_SIZE;
    let mut comm_found = false;
    let mut channels = 0u16;
    let mut sample_rate = 0u32;
    let mut bits_per_sample = 0u16;
    let mut ssnd_offset: u64 = 0;

    while pos + CHUNK_HEADER_SIZE <= probe.len() {
        let chunk_id = &probe[pos..pos + 4];
        let chunk_size = u32::from_be_bytes(probe[pos + 4..pos + 8].try_into().unwrap()) as usize;
        pos += CHUNK_HEADER_SIZE;
        let chunk_end = pos + chunk_size;

        if chunk_id == b"COMM" {
            (channels, sample_rate, bits_per_sample) = parse_comm_chunk(probe, pos, chunk_size, is_aifc)?;
            comm_found = true;
        } else if chunk_id == b"SSND" {
            // SSND: 4 byte offset field + 4 byte block size field, then PCM data
            let ssnd_data_offset =
                u32::from_be_bytes(probe[pos + SSND_OFFSET_FIELD..pos + 4].try_into().unwrap()) as u64;
            ssnd_offset = pos as u64 + SSND_HEADER_FIELDS + ssnd_data_offset; // skip offset+block fields
            break;
        }

        pos = (chunk_end + 1) & !1;
    }

    if !comm_found {
        return Err(Error::parse("AIFF COMM chunk not found"));
    }
    if channels == 0 {
        return Err(Error::parse("AIFF COMM chunk has zero channels"));
    }
    if sample_rate == 0 || bits_per_sample == 0 {
        return Err(Error::parse("AIFF COMM chunk has zero sample parameters"));
    }

    Ok(AiffChunkResult {
        channels,
        sample_rate,
        bits_per_sample,
        ssnd_offset,
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
