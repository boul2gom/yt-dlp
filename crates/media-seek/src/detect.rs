//! Container format detection from leading magic bytes.

use crate::audio::flac::FLAC_MARKER;
use crate::audio::mp3::ID3V2_MAGIC;
use crate::audio::ogg::OGG_CAPTURE;
use crate::audio::pcm::{AIFC_SUBTYPE, AIFF_SUBTYPE, FORM_MAGIC, RIFF_MAGIC, WAV_SUBTYPE};
use crate::video::avi::AVI_SUBTYPE;
use crate::video::flv::FLV_MAGIC;
use crate::video::ts::{TS_PKT_SIZE, TS_SYNC};
use crate::video::webm::EBML_MAGIC;

/// Minimum probe length to check the ISOBMFF box type at offset 4.
const ISOBMFF_PROBE_MIN: usize = 8;

/// ADTS sync word: the first 12 bits of every AAC ADTS frame are all-ones.
/// Second byte is masked with this to check the upper nibble.
const ADTS_SYNC_SECOND_BYTE_MASK: u8 = 0xF6;

/// ADTS sync pattern for the second byte after masking.
/// Matches both MPEG-4 AAC (0xF1) and MPEG-2 AAC (0xF0) → both & 0xF6 == 0xF0.
const ADTS_SYNC_SECOND_BYTE_PATTERN: u8 = 0xF0;

/// Minimum ADTS header length (no CRC).
const ADTS_MIN_HEADER: usize = 7;

/// Bitmask for MPEG audio sync + layer bits in the second byte.
/// Applied to the second byte of a bare-sync MPEG audio frame (after 0xFF).
/// Isolates sync (bits 7-5) and layer (bits 2-1); ignores version and protection_absent.
const MPEG_SYNC_LAYER_MASK: u8 = 0xE6;

/// Expected pattern for MPEG Layer III after masking (sync=111, layer bits=01).
const MPEG_SYNC_PATTERN_L3: u8 = 0xE2;

/// Expected pattern for MPEG Layer II after masking (sync=111, layer bits=10).
const MPEG_SYNC_PATTERN_L2: u8 = 0xE4;

/// Expected pattern for MPEG Layer I after masking (sync=111, layer bits=11).
const MPEG_SYNC_PATTERN_L1: u8 = 0xE6;

/// Minimum probe length required to confirm TS via three consecutive sync bytes.
const TS_THREE_SYNC_LEN: usize = TS_PKT_SIZE * 2 + 1;

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
/// Formats are tested in order of decreasing likelihood for yt-dlp downloads:
/// ISOBMFF/MP4, WebM, MP3, OGG, TS, ADTS, MPEG-audio, WAV/AVI (RIFF), FLAC, FLV, AIFF (FORM).
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

    // ISOBMFF / MP4 — most common yt-dlp format (YouTube video + M4A audio)
    if let Some(fmt) = detect_isobmff(probe) {
        return Some(fmt);
    }

    // WebM / EBML — YouTube WebM/VP9/Opus
    if probe.starts_with(EBML_MAGIC) {
        return Some(Format::Webm);
    }

    // MP3 with ID3v2 tag — common audio
    if probe.starts_with(ID3V2_MAGIC) {
        return Some(Format::Mp3);
    }

    // OGG — Opus audio
    if probe.starts_with(OGG_CAPTURE) {
        return Some(Format::Ogg);
    }

    // MPEG-TS — HLS live streams
    if let Some(fmt) = detect_ts_sync(probe) {
        return Some(fmt);
    }

    // ADTS (raw AAC) — YouTube audio-only streams
    if let Some(fmt) = detect_adts(probe) {
        return Some(fmt);
    }

    // Bare MPEG audio sync (MP3 without ID3 tag)
    if let Some(fmt) = detect_mpeg_audio(probe) {
        return Some(fmt);
    }

    // RIFF containers: WAV and AVI
    if let Some(fmt) = detect_riff_container(probe) {
        return Some(fmt);
    }

    // FLAC — lossless audio
    if probe.starts_with(FLAC_MARKER) {
        return Some(Format::Flac);
    }

    // FLV — legacy Flash video
    if probe.starts_with(FLV_MAGIC) {
        return Some(Format::Flv);
    }

    // FORM / AIFF — Apple audio (rare)
    detect_form_container(probe)
}

/// Detects ISO Base Media File Format by checking the 4-byte box type at offset 4.
fn detect_isobmff(probe: &[u8]) -> Option<Format> {
    let is_long_enough = probe.len() >= ISOBMFF_PROBE_MIN;
    let is_known_box = is_long_enough && ISOBMFF_BOXES.contains(&&probe[4..8]);
    if is_known_box { Some(Format::Mp4) } else { None }
}

/// Detects MPEG-TS by checking for consecutive sync bytes spaced 188 bytes apart.
fn detect_ts_sync(probe: &[u8]) -> Option<Format> {
    if probe.len() >= TS_THREE_SYNC_LEN {
        let is_sync0 = probe[0] == TS_SYNC;
        let is_sync1 = probe[TS_PKT_SIZE] == TS_SYNC;
        let is_sync2 = probe[TS_PKT_SIZE * 2] == TS_SYNC;
        if is_sync0 && is_sync1 && is_sync2 {
            return Some(Format::Ts);
        }
    }
    if probe.len() > TS_PKT_SIZE {
        let is_sync0 = probe[0] == TS_SYNC;
        let is_sync1 = probe[TS_PKT_SIZE] == TS_SYNC;
        if is_sync0 && is_sync1 {
            return Some(Format::Ts);
        }
    }
    None
}

/// Detects AAC ADTS sync from leading probe bytes.
///
/// Verifies the first frame's length field and optionally checks for a second sync word
/// at the computed frame boundary for extra confidence.
fn detect_adts(probe: &[u8]) -> Option<Format> {
    if probe.len() < ADTS_MIN_HEADER {
        return None;
    }
    let is_sync_byte = probe[0] == 0xFF;
    let is_adts_pattern = (probe[1] & ADTS_SYNC_SECOND_BYTE_MASK) == ADTS_SYNC_SECOND_BYTE_PATTERN;
    if !is_sync_byte || !is_adts_pattern {
        return None;
    }
    let frame_length = ((probe[3] as usize & 0x03) << 11) | ((probe[4] as usize) << 3) | ((probe[5] as usize) >> 5);
    if frame_length < ADTS_MIN_HEADER {
        return None;
    }
    // If the probe is long enough, verify the second sync word at the frame boundary.
    if probe.len() > frame_length + 1 {
        let is_next_sync = probe[frame_length] == 0xFF;
        let is_next_adts = (probe[frame_length + 1] & ADTS_SYNC_SECOND_BYTE_MASK) == ADTS_SYNC_SECOND_BYTE_PATTERN;
        if is_next_sync && is_next_adts {
            Some(Format::Adts)
        } else {
            None
        }
    } else {
        Some(Format::Adts)
    }
}

/// Detects bare MPEG audio (Layer I, II, or III) without an ID3 header.
fn detect_mpeg_audio(probe: &[u8]) -> Option<Format> {
    if probe.len() < 2 || probe[0] != 0xFF {
        return None;
    }
    let masked = probe[1] & MPEG_SYNC_LAYER_MASK;
    let is_layer3 = masked == MPEG_SYNC_PATTERN_L3;
    let is_layer2 = masked == MPEG_SYNC_PATTERN_L2;
    let is_layer1 = masked == MPEG_SYNC_PATTERN_L1;
    if is_layer3 || is_layer2 || is_layer1 {
        Some(Format::Mp3)
    } else {
        None
    }
}

/// Detects a RIFF container and discriminates WAV vs AVI via the 4-byte subtype at offset 8.
fn detect_riff_container(probe: &[u8]) -> Option<Format> {
    if !probe.starts_with(RIFF_MAGIC) || probe.len() < 12 {
        return None;
    }
    match &probe[8..12] {
        s if s == WAV_SUBTYPE => Some(Format::Wav),
        s if s == AVI_SUBTYPE => Some(Format::Avi),
        _ => None,
    }
}

/// Detects an IFF FORM container and discriminates AIFF vs AIFC via the subtype at offset 8.
fn detect_form_container(probe: &[u8]) -> Option<Format> {
    if !probe.starts_with(FORM_MAGIC) || probe.len() < 12 {
        return None;
    }
    let subtype = &probe[8..12];
    let is_aiff = subtype == AIFF_SUBTYPE;
    let is_aifc = subtype == AIFC_SUBTYPE;
    if is_aiff || is_aifc { Some(Format::Aiff) } else { None }
}
