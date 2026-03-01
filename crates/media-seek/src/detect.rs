//! Container format detection from leading magic bytes.

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
    if probe.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
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

    // AAC ADTS sync — 0xFFF1 (MPEG-4 AAC) or 0xFFF0 (MPEG-2 AAC) at start
    // Verify frame_length from header bytes 3-5 and check for second sync word
    if probe.len() >= 7 && probe[0] == 0xFF && (probe[1] & 0xF6) == 0xF0 {
        let frame_length =
            ((probe[3] as usize & 0x03) << 11) | ((probe[4] as usize) << 3) | ((probe[5] as usize) >> 5);
        if frame_length >= 7 {
            // If we have enough data, validate the second sync word
            if probe.len() > frame_length + 1 {
                if probe[frame_length] == 0xFF && (probe[frame_length + 1] & 0xF6) == 0xF0 {
                    return Some(Format::Adts);
                }
            } else {
                // Not enough data for second sync, trust the first header
                return Some(Format::Adts);
            }
        }
    }

    // MP3 sync frame without ID3 (0xFF 0xE*  or 0xFF 0xF* with layer bits indicating MP3)
    if probe.len() >= 2 && probe[0] == 0xFF {
        // Bits [15:13] = 111 (sync), bit [12:11] = 01 (MPEG-1/2 layer 3)
        let b1 = probe[1];
        // layer bits in positions [10:9]: 01 = Layer III
        // 0xE0 = sync extension; 0x02 = layer bits for Layer III
        if (b1 & 0xE6) == 0xE2 || (b1 & 0xE6) == 0xE4 {
            return Some(Format::Mp3);
        }
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
    if probe.len() >= 8 {
        let box_type = &probe[4..8];
        const ISOBMFF_BOXES: &[&[u8]] = &[b"ftyp", b"styp", b"moov", b"moof", b"mdat", b"sidx", b"free", b"skip", b"wide"];
        if ISOBMFF_BOXES.contains(&box_type) {
            return Some(Format::Mp4);
        }
    }

    // MPEG-TS: three consecutive 0x47 sync bytes spaced 188 bytes apart
    if probe.len() >= 376 && probe[0] == 0x47 && probe[188] == 0x47 && probe[376] == 0x47 {
        return Some(Format::Ts);
    }
    // Shorter probe — just check first sync byte + second packet start if available
    if probe.len() >= 189 && probe[0] == 0x47 && probe[188] == 0x47 {
        return Some(Format::Ts);
    }

    None
}
