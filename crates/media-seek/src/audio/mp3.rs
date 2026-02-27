//! MP3 container index parsing via Xing/VBRI VBR headers or CBR calculation.
//!
//! For VBR streams: the Xing/Info or VBRI header contains a TOC (Table of Contents)
//! with 100 equidistant percentage entries mapping time to byte position.
//! For CBR streams: byte offset is derived directly from the constant bitrate.

use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

/// Size of each MP3 frame sync lookup window.
const SYNC_SEARCH_LIMIT: usize = 8192;

/// Parses an MP3 stream and returns a `ContainerIndex`.
///
/// Skips any ID3v2 header, locates the first MPEG sync frame, then checks for
/// a Xing/Info or VBRI VBR header. Falls back to CBR calculation if none is found.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the MP3 stream.
///
/// # Errors
///
/// Returns `Error::ParseFailed` when no sync frame is found within the probe.
pub(crate) fn parse(probe: &[u8]) -> Result<ContainerIndex> {
    tracing::debug!(probe_len = probe.len(), "⚙️ Parsing MP3 stream");
    let audio_start = skip_id3(probe);
    let frame_start =
        find_sync_frame(probe, audio_start).ok_or_else(|| Error::parse("no MP3 sync frame found in probe"))?;

    let frame = &probe[frame_start..];
    if frame.len() < 4 {
        return Err(Error::parse("MP3 frame header truncated"));
    }

    let (sample_rate, channels, bitrate_bps, _frame_size) =
        parse_frame_header(frame).ok_or_else(|| Error::parse("invalid MP3 frame header"))?;

    // Check for Xing/Info header at the standard offset after the frame header
    let xing_offset = xing_header_offset(channels);
    if frame.len() >= xing_offset + 4 {
        let tag = &frame[xing_offset..xing_offset + 4];
        if tag == b"Xing" || tag == b"Info" {
            let result = parse_xing(frame, xing_offset, sample_rate, frame_start as u64, probe.len() as u64);
            tracing::debug!("✅ MP3 index parsed (mode=xing)");
            return result;
        }
    }

    // Check for VBRI header (always at offset 36 after frame header start)
    const VBRI_OFFSET: usize = 36;
    if frame.len() >= VBRI_OFFSET + 4 && &frame[VBRI_OFFSET..VBRI_OFFSET + 4] == b"VBRI" {
        let result = parse_vbri(frame, VBRI_OFFSET, sample_rate, frame_start as u64, probe.len() as u64);
        tracing::debug!("✅ MP3 index parsed (mode=vbri)");
        return result;
    }

    // CBR: use constant bitrate for a Linear index
    if bitrate_bps == 0 {
        return Err(Error::parse("MP3 CBR bitrate is zero"));
    }
    let byte_rate = bitrate_bps as f64 / 8.0;
    tracing::debug!("✅ MP3 index parsed (mode=cbr)");
    Ok(ContainerIndex {
        init_end_byte: frame_start as u64,
        inner: Inner::Linear {
            byte_rate,
            block_align: 1,
        },
    })
}

/// Returns the byte offset of the first ID3v2-free audio data.
fn skip_id3(data: &[u8]) -> usize {
    if data.len() < 10 || &data[0..3] != b"ID3" {
        return 0;
    }
    // ID3v2 size is encoded as four 7-bit bytes (syncsafe integer)
    let size = ((data[6] as u32) << 21) | ((data[7] as u32) << 14) | ((data[8] as u32) << 7) | (data[9] as u32);
    // 10-byte ID3 header + optional 10-byte footer
    let footer_flag = data[5] & 0x10 != 0;
    let total = 10 + size as usize + if footer_flag { 10 } else { 0 };
    total.min(data.len())
}

/// Finds the byte position of the first valid MPEG sync frame in `data[start..]`.
fn find_sync_frame(data: &[u8], start: usize) -> Option<usize> {
    let limit = (start + SYNC_SEARCH_LIMIT).min(data.len().saturating_sub(3));
    for i in start..limit {
        if data[i] == 0xFF {
            let b1 = data[i + 1];
            // Sync word: 0xFF + 0xE* (MPEG-1/2) with layer bits indicating MP3
            // Layer: bits 2-1 of byte 1: 01 = Layer III
            if (b1 & 0xE0) == 0xE0 && ((b1 >> 1) & 0x03) == 0x01 {
                // Verify this has a plausible bitrate (bits 7-4 of byte 2 not 0b1111 or 0b0000)
                if i + 3 < data.len() {
                    let bi = (data[i + 2] >> 4) & 0x0F;
                    if bi != 0 && bi != 15 {
                        return Some(i);
                    }
                }
            }
        }
    }
    None
}

/// Parses an MPEG frame header and returns (sample_rate, channels, bitrate_bps, frame_size).
fn parse_frame_header(frame: &[u8]) -> Option<(u32, u8, u32, usize)> {
    if frame.len() < 4 {
        return None;
    }
    let b1 = frame[1];
    let b2 = frame[2];
    let b3 = frame[3];

    // MPEG version: bits 4-3 of byte 1
    let mpeg_version = (b1 >> 3) & 0x03; // 11=MPEG1, 10=MPEG2, 00=MPEG2.5
    // Layer: bits 2-1 of byte 1 (01 = Layer III)
    let layer = (b1 >> 1) & 0x03;
    if layer != 1 {
        return None; // Only handle Layer III
    }

    let bitrate_idx = (b2 >> 4) as usize;
    let sr_idx = ((b2 >> 2) & 0x03) as usize;
    let padding = (b2 >> 1) & 0x01;
    let channel_mode = (b3 >> 6) & 0x03; // 3 = mono
    let channels = if channel_mode == 3 { 1u8 } else { 2u8 };

    const BITRATES_MPEG1: [u32; 16] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0];
    const BITRATES_MPEG2: [u32; 16] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0];
    const SAMPLE_RATES_MPEG1: [u32; 4] = [44100, 48000, 32000, 0];
    const SAMPLE_RATES_MPEG2: [u32; 4] = [22050, 24000, 16000, 0];
    const SAMPLE_RATES_MPEG25: [u32; 4] = [11025, 12000, 8000, 0];

    let (bitrate_kbps, sample_rate) = match mpeg_version {
        3 => (BITRATES_MPEG1[bitrate_idx], SAMPLE_RATES_MPEG1[sr_idx]),
        2 | 0 => (
            BITRATES_MPEG2[bitrate_idx],
            if mpeg_version == 2 {
                SAMPLE_RATES_MPEG2[sr_idx]
            } else {
                SAMPLE_RATES_MPEG25[sr_idx]
            },
        ),
        _ => return None,
    };

    if bitrate_kbps == 0 || sample_rate == 0 {
        return None;
    }

    let bitrate_bps = bitrate_kbps * 1000;
    let frame_size = (144 * bitrate_bps / sample_rate + padding as u32) as usize;

    Some((sample_rate, channels, bitrate_bps, frame_size))
}

/// Returns the offset of the Xing header within a frame (after the side information).
fn xing_header_offset(channels: u8) -> usize {
    // MPEG1 stereo: 32 bytes side info; MPEG1 mono: 17 bytes; simplified to mono/stereo only
    4 + if channels == 1 { 17 } else { 32 }
}

/// Parses a Xing/Info VBR header and constructs a segmented `ContainerIndex`.
fn parse_xing(
    frame: &[u8],
    xing_off: usize,
    sample_rate: u32,
    frame_start_byte: u64,
    total_size: u64,
) -> Result<ContainerIndex> {
    let x = xing_off;
    if frame.len() < x + 8 {
        return Err(Error::parse("Xing header truncated"));
    }
    let flags = u32::from_be_bytes(frame[x + 4..x + 8].try_into().unwrap());

    let mut off = x + 8;

    let total_frames = if flags & 0x01 != 0 {
        if frame.len() < off + 4 {
            return Err(Error::parse("Xing total_frames truncated"));
        }
        let f = u32::from_be_bytes(frame[off..off + 4].try_into().unwrap());
        off += 4;
        f
    } else {
        0
    };

    let total_bytes = if flags & 0x02 != 0 {
        if frame.len() < off + 4 {
            return Err(Error::parse("Xing total_bytes truncated"));
        }
        let b = u32::from_be_bytes(frame[off..off + 4].try_into().unwrap()) as u64;
        off += 4;
        b
    } else {
        // Use the actual file size as a fallback
        total_size.saturating_sub(frame_start_byte)
    };

    let toc: Option<[u8; 100]> = if flags & 0x04 != 0 {
        if frame.len() < off + 100 {
            return Err(Error::parse("Xing TOC truncated"));
        }
        let mut t = [0u8; 100];
        t.copy_from_slice(&frame[off..off + 100]);
        Some(t)
    } else {
        None
    };

    if total_frames == 0 {
        // No frame count — fall back to a linear approximation using total_bytes
        let samples_per_frame = 1152u64; // MP3 Layer III
        let duration_secs = total_frames as f64 * samples_per_frame as f64 / sample_rate as f64;
        let byte_rate = if duration_secs > 0.0 {
            total_bytes as f64 / duration_secs
        } else {
            128_000.0 / 8.0
        };
        return Ok(ContainerIndex {
            init_end_byte: frame_start_byte,
            inner: Inner::Linear {
                byte_rate,
                block_align: 1,
            },
        });
    }

    let samples_per_frame = 1152u64;
    let total_duration = total_frames as f64 * samples_per_frame as f64 / sample_rate as f64;

    if let Some(toc) = toc {
        // Convert the 100-entry TOC into SegmentEntry slices
        let mut segments = Vec::with_capacity(100);
        for i in 0..100usize {
            let pct = toc[i] as f64 / 256.0;
            let byte_offset = frame_start_byte + (pct * total_bytes as f64) as u64;
            let start_secs = i as f64 * total_duration / 100.0;
            let end_secs = (i + 1) as f64 * total_duration / 100.0;
            let next_pct = if i + 1 < 100 { toc[i + 1] as f64 / 256.0 } else { 1.0 };
            let next_byte = frame_start_byte + (next_pct * total_bytes as f64) as u64;
            segments.push(SegmentEntry {
                start_secs,
                end_secs,
                byte_offset,
                byte_size: next_byte.saturating_sub(byte_offset),
            });
        }
        return Ok(ContainerIndex {
            init_end_byte: frame_start_byte,
            inner: Inner::Segments(segments),
        });
    }

    // No TOC — linear using average bitrate derived from total bytes/duration
    let byte_rate = total_bytes as f64 / total_duration;
    Ok(ContainerIndex {
        init_end_byte: frame_start_byte,
        inner: Inner::Linear {
            byte_rate,
            block_align: 1,
        },
    })
}

/// Parses a VBRI VBR header and constructs a segmented `ContainerIndex`.
fn parse_vbri(
    frame: &[u8],
    vbri_off: usize,
    sample_rate: u32,
    frame_start_byte: u64,
    _total_size: u64,
) -> Result<ContainerIndex> {
    // VBRI layout: 4 tag + 2 version + 2 delay + 2 quality + 4 bytes_total + 4 frames_total +
    //              2 table_size + 2 table_scale + 2 entry_bytes + 2 frames_per_entry + table
    let v = vbri_off;
    if frame.len() < v + 26 {
        return Err(Error::parse("VBRI header truncated"));
    }
    let total_bytes = u32::from_be_bytes(frame[v + 10..v + 14].try_into().unwrap()) as u64;
    let total_frames = u32::from_be_bytes(frame[v + 14..v + 18].try_into().unwrap());
    let table_size = u16::from_be_bytes(frame[v + 18..v + 20].try_into().unwrap()) as usize;
    let table_scale = u16::from_be_bytes(frame[v + 20..v + 22].try_into().unwrap()) as u64;
    let entry_bytes = u16::from_be_bytes(frame[v + 22..v + 24].try_into().unwrap()) as usize;
    let frames_per_entry = u16::from_be_bytes(frame[v + 24..v + 26].try_into().unwrap()) as u64;

    let table_start = v + 26;
    if frame.len() < table_start + table_size * entry_bytes || entry_bytes == 0 || entry_bytes > 4 {
        return Err(Error::parse("VBRI table truncated or invalid entry_bytes"));
    }

    let samples_per_frame = 1152u64;
    let total_duration = total_frames as f64 * samples_per_frame as f64 / sample_rate as f64;
    let _ = total_bytes; // might be 0; use offsets directly

    let mut segments = Vec::with_capacity(table_size);
    let mut byte_cursor = frame_start_byte;
    for i in 0..table_size {
        let off = table_start + i * entry_bytes;
        let mut entry_val = 0u64;
        for j in 0..entry_bytes {
            entry_val = (entry_val << 8) | frame[off + j] as u64;
        }
        let chunk_bytes = entry_val * table_scale;
        let start_secs = i as f64 * frames_per_entry as f64 * samples_per_frame as f64 / sample_rate as f64;
        let end_secs = (i + 1) as f64 * frames_per_entry as f64 * samples_per_frame as f64 / sample_rate as f64;
        segments.push(SegmentEntry {
            start_secs,
            end_secs: end_secs.min(total_duration),
            byte_offset: byte_cursor,
            byte_size: chunk_bytes,
        });
        byte_cursor += chunk_bytes;
    }

    Ok(ContainerIndex {
        init_end_byte: frame_start_byte,
        inner: Inner::Segments(segments),
    })
}
