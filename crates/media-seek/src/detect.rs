//! Container format detection from leading magic bytes.

/// EBML header magic bytes — identifies WebM / Matroska containers.
const EBML_MAGIC: [u8; 4] = [0x1A, 0x45, 0xDF, 0xA3];

/// MPEG-TS sync byte — first byte of every 188-byte TS packet.
pub(crate) const TS_SYNC_BYTE: u8 = 0x47;

/// Standard MPEG-TS packet size in bytes.
const TS_PACKET_SIZE: usize = 188;

/// Minimum probe length required to confirm TS via three consecutive sync bytes.
const TS_THREE_SYNC_LEN: usize = TS_PACKET_SIZE * 2 + 1;

/// ADTS sync word: the first 12 bits of every AAC ADTS frame are all-ones.
/// Second byte is masked with this to check the upper nibble.
const ADTS_SYNC_SECOND_BYTE_MASK: u8 = 0xF6;

/// ADTS sync pattern for the second byte after masking.
/// Matches both MPEG-4 AAC (0xF1) and MPEG-2 AAC (0xF0) → both & 0xF6 == 0xF0.
const ADTS_SYNC_SECOND_BYTE_PATTERN: u8 = 0xF0;

/// Minimum ADTS header length (no CRC).
const ADTS_MIN_HEADER: usize = 7;

/// Bitmask for MPEG audio sync + version + layer bits in the second byte.
/// Used to detect MP3 (Layer III) sync frames without ID3.
const MP3_SYNC_MASK: u8 = 0xE6;

/// Expected pattern for MPEG-1 Layer III after masking (sync=111, version=1x, layer=01).
/// Only Layer III (MP3) matches — Layer II (0xE4) is intentionally excluded.
const MP3_SYNC_PATTERN_MPEG1_L3: u8 = 0xE2;

/// ISO Base Media File Format box types recognized at offset 4.
const ISOBMFF_BOXES: &[&[u8]] = &[
    b"ftyp", b"styp", b"moov", b"moof", b"mdat", b"sidx", b"free", b"skip", b"wide",
];

/// Recognised container formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    /// ISO Base Media File Format (MP4, M4A, M4V, MOV, fMP4 …)
    Mp4,
    /// WebM / Matroska (EBML magic)
    Webm,
    /// MPEG Audio Layer 3 (ID3-tagged or bare sync frame)
    Mp3,
    /// Ogg container (OGG page sync)
    Ogg,
    /// Free Lossless Audio Codec
    Flac,
    /// RIFF WAVE (PCM audio)
    Wav,
    /// Audio Interchange File Format
    Aiff,
    /// AAC Audio Data Transport Stream
    Adts,
    /// Flash Video
    Flv,
    /// RIFF AVI
    Avi,
    /// MPEG-2 Transport Stream (188-byte packets starting with 0x47)
    Ts,
}

/// Detects the container format from the leading bytes of a stream.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the stream (at least 12 bytes recommended; more is better).
///
/// # Returns
///
/// `Some(Format)` when magic bytes are recognized, `None` for MHTML multipart or unknown data.
pub(crate) fn detect(probe: &[u8]) -> Option<Format> {
    if probe.len() < 4 {
        return None;
    }

    // EBML magic — WebM / Matroska
    if probe.starts_with(&EBML_MAGIC) {
        return Some(Format::Webm);
    }

    // OGG page sync
    if probe.starts_with(b"OggS") {
        return Some(Format::Ogg);
    }

    // FLAC stream marker
    if probe.starts_with(b"fLaC") {
        return Some(Format::Flac);
    }

    // FLV signature
    if probe.starts_with(b"FLV") {
        return Some(Format::Flv);
    }

    // ID3 tag (MP3 with ID3v2 header)
    if probe.starts_with(b"ID3") {
        return Some(Format::Mp3);
    }

    // AAC ADTS sync — 0xFFF1 (MPEG-4 AAC) or 0xFFF0 (MPEG-2 AAC)
    // Verify frame_length from header bytes 3-5 and check for second sync word
    if probe.len() >= ADTS_MIN_HEADER
        && probe[0] == 0xFF
        && (probe[1] & ADTS_SYNC_SECOND_BYTE_MASK) == ADTS_SYNC_SECOND_BYTE_PATTERN
    {
        let frame_length = ((probe[3] as usize & 0x03) << 11) | ((probe[4] as usize) << 3) | ((probe[5] as usize) >> 5);
        if frame_length >= ADTS_MIN_HEADER {
            if probe.len() > frame_length + 1 {
                if probe[frame_length] == 0xFF
                    && (probe[frame_length + 1] & ADTS_SYNC_SECOND_BYTE_MASK) == ADTS_SYNC_SECOND_BYTE_PATTERN
                {
                    return Some(Format::Adts);
                }
            } else {
                return Some(Format::Adts);
            }
        }
    }

    // MP3 sync frame without ID3 (0xFF 0xE* with layer bits indicating Layer III only)
    if probe.len() >= 2 && probe[0] == 0xFF && (probe[1] & MP3_SYNC_MASK) == MP3_SYNC_PATTERN_MPEG1_L3 {
        return Some(Format::Mp3);
    }

    // RIFF container — discriminate WAV vs AVI via WAVE/AVI subtype at offset 8
    if probe.starts_with(b"RIFF") && probe.len() >= 12 {
        let subtype = &probe[8..12];
        if subtype == b"WAVE" {
            return Some(Format::Wav);
        }
        if subtype == b"AVI " {
            return Some(Format::Avi);
        }
    }

    // FORM container — AIFF / AIFC
    if probe.starts_with(b"FORM") && probe.len() >= 12 {
        let subtype = &probe[8..12];
        if subtype == b"AIFF" || subtype == b"AIFC" {
            return Some(Format::Aiff);
        }
    }

    // ISO Base Media File Format — check 4-byte box type at offset 4
    if probe.len() >= 8 && ISOBMFF_BOXES.contains(&&probe[4..8]) {
        return Some(Format::Mp4);
    }

    // MPEG-TS: three consecutive sync bytes spaced 188 bytes apart
    if probe.len() >= TS_THREE_SYNC_LEN
        && probe[0] == TS_SYNC_BYTE
        && probe[TS_PACKET_SIZE] == TS_SYNC_BYTE
        && probe[TS_PACKET_SIZE * 2] == TS_SYNC_BYTE
    {
        return Some(Format::Ts);
    }
    // Shorter probe — two consecutive sync bytes
    if probe.len() > TS_PACKET_SIZE && probe[0] == TS_SYNC_BYTE && probe[TS_PACKET_SIZE] == TS_SYNC_BYTE {
        return Some(Format::Ts);
    }

    None
}
