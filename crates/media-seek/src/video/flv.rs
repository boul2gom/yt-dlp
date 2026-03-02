//! FLV (Flash Video) container index parsing via the AMF0 `onMetaData` keyframes table.
//!
//! A well-formed FLV file starts with a Script tag whose payload is an AMF0-encoded
//! `onMetaData` object. This object typically contains two parallel arrays:
//! `keyframes.times` (in seconds) and `keyframes.filepositions` (byte offsets).

use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

/// AMF0 type markers.
const AMF_NUMBER: u8 = 0x00;
const AMF_BOOL: u8 = 0x01;
const AMF_STRING: u8 = 0x02;
const AMF_OBJECT: u8 = 0x03;
const AMF_NULL: u8 = 0x05;
const AMF_UNDEFINED: u8 = 0x06;
const AMF_REFERENCE: u8 = 0x07;
const AMF_ECMA_ARRAY: u8 = 0x08;
const AMF_OBJECT_END: u8 = 0x09;
const AMF_STRICT_ARRAY: u8 = 0x0A;
const AMF_DATE: u8 = 0x0B;
const AMF_LONG_STRING: u8 = 0x0C;

/// FLV file header size: "FLV" (3) + version (1) + flags (1) + header_size (4).
const FLV_HEADER_SIZE: usize = 9;
/// FLV tag header size in bytes.
const TAG_HEADER_SIZE: usize = 11;
/// Size of the "previous tag size" field between tags.
const PREV_TAG_SIZE_LEN: usize = 4;
/// FLV Script (metadata) tag type.
const TAG_TYPE_SCRIPT: u8 = 18;

/// Parses an FLV stream and returns a `ContainerIndex`.
///
/// Locates the first Script tag, decodes its AMF0 `onMetaData` payload, and
/// extracts the `keyframes.times` and `keyframes.filepositions` arrays.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the FLV stream.
///
/// # Errors
///
/// Returns `Error::ParseFailed` when the FLV header is missing, the Script tag
/// is not found, or the keyframes table cannot be decoded.
pub(crate) fn parse(probe: &[u8]) -> Result<ContainerIndex> {
    tracing::debug!(probe_len = probe.len(), "⚙️ Parsing FLV stream");
    // FLV header: "FLV" (3) + version (1) + flags (1) + header_size (4) = 9 bytes
    if probe.len() < FLV_HEADER_SIZE || &probe[0..3] != b"FLV" {
        return Err(Error::parse("not an FLV stream"));
    }
    let header_size = u32::from_be_bytes(probe[5..9].try_into().unwrap()) as usize;
    let mut pos = header_size + PREV_TAG_SIZE_LEN;

    while pos + TAG_HEADER_SIZE <= probe.len() {
        let tag_type = probe[pos];
        let data_size = u24_be(&probe[pos + 1..pos + 4]) as usize;
        pos += TAG_HEADER_SIZE;
        let tag_end = pos + data_size;
        if tag_end > probe.len() {
            break;
        }

        if tag_type == TAG_TYPE_SCRIPT
            && let Ok(index) = parse_script_tag(&probe[pos..tag_end])
        {
            if let Inner::Segments(ref segs) = index.inner {
                tracing::debug!(keyframes = segs.len(), "✅ FLV index parsed");
            }
            return Ok(index);
        }

        pos = tag_end + PREV_TAG_SIZE_LEN;
    }

    Err(Error::parse(
        "FLV onMetaData Script tag not found or no keyframes table",
    ))
}

/// Reads a 3-byte big-endian unsigned integer.
fn u24_be(b: &[u8]) -> u32 {
    ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32
}

/// Parses the AMF0 Script tag payload and returns the keyframes `ContainerIndex`.
fn parse_script_tag(data: &[u8]) -> Result<ContainerIndex> {
    // Expect: AMF_STRING "onMetaData" + AMF_ECMA_ARRAY or AMF_OBJECT
    let mut pos = 0usize;

    // First value: string "onMetaData"
    if data.is_empty() {
        return Err(Error::parse("Script tag empty"));
    }
    let (_, consumed) = skip_amf_value(data, pos)?;
    pos += consumed;

    // Second value: the metadata object/ECMA array
    let (times, positions) = extract_keyframes(data, pos)?;

    if times.is_empty() || times.len() != positions.len() {
        return Err(Error::parse("FLV keyframes times/positions arrays empty or mismatched"));
    }

    let mut segments = Vec::with_capacity(times.len());
    for i in 0..times.len() {
        let start_secs = times[i];
        let byte_offset = positions[i] as u64;

        // For the last keyframe, estimate end_secs from the average interval
        // and set byte_size to 0 (unknown extent — callers treat 0 as "until EOF")
        let end_secs = if i + 1 < times.len() {
            times[i + 1]
        } else if i > 0 {
            let avg_interval = (times[i] - times[0]) / i as f64;
            times[i] + avg_interval
        } else {
            times[i]
        };
        let byte_size = if i + 1 < positions.len() {
            (positions[i + 1] as u64).saturating_sub(byte_offset)
        } else {
            0
        };
        segments.push(SegmentEntry {
            start_secs,
            end_secs,
            byte_offset,
            byte_size,
        });
    }

    let init_end_byte = segments.first().map(|s| s.byte_offset.saturating_sub(1)).unwrap_or(0);

    Ok(ContainerIndex {
        init_end_byte,
        inner: Inner::Segments(segments),
    })
}

/// Extracts `keyframes.times` and `keyframes.filepositions` arrays from an AMF0 object.
fn extract_keyframes(data: &[u8], start: usize) -> Result<(Vec<f64>, Vec<f64>)> {
    let mut pos = start;
    if pos >= data.len() {
        return Err(Error::parse("AMF0 object empty"));
    }

    let marker = data[pos];
    pos += 1;

    // Support both ECMA array (0x08) and strict object (0x03)
    if marker == AMF_ECMA_ARRAY {
        if pos + 4 > data.len() {
            return Err(Error::parse("AMF0 ECMA array truncated"));
        }
        pos += 4; // skip approximate count
    } else if marker != AMF_OBJECT {
        return Err(Error::parse("AMF0 metadata is not an object or ECMA array"));
    }

    let mut times: Option<Vec<f64>> = None;
    let mut positions: Option<Vec<f64>> = None;

    loop {
        if pos + 2 > data.len() {
            break;
        }
        let key_len = u16::from_be_bytes(data[pos..pos + 2].try_into().unwrap()) as usize;
        pos += 2;
        if key_len == 0 && pos < data.len() && data[pos] == AMF_OBJECT_END {
            break;
        }
        if pos + key_len > data.len() {
            break;
        }
        let key = &data[pos..pos + key_len];
        pos += key_len;

        if pos >= data.len() {
            break;
        }

        if key == b"keyframes" {
            // Nested object with "times" and "filepositions"
            let (t, fp, consumed) = parse_keyframes_object(data, pos)?;
            pos += consumed;
            times = Some(t);
            positions = Some(fp);
        } else {
            let (_, consumed) = skip_amf_value(data, pos)?;
            pos += consumed;
        }

        if times.is_some() && positions.is_some() {
            break;
        }
    }

    match (times, positions) {
        (Some(t), Some(p)) => Ok((t, p)),
        _ => Err(Error::parse("FLV keyframes object not found in onMetaData")),
    }
}

/// Parses the `keyframes` nested AMF0 object and returns `(times, filepositions, bytes_consumed)`.
fn parse_keyframes_object(data: &[u8], start: usize) -> Result<(Vec<f64>, Vec<f64>, usize)> {
    let mut pos = start;
    if pos >= data.len() {
        return Err(Error::parse("keyframes object truncated"));
    }
    let marker = data[pos];
    pos += 1;
    if marker == AMF_ECMA_ARRAY {
        if pos + 4 > data.len() {
            return Err(Error::parse("keyframes ECMA array size truncated"));
        }
        pos += 4;
    } else if marker != AMF_OBJECT {
        return Err(Error::parse("keyframes value is not an object"));
    }

    let mut times: Option<Vec<f64>> = None;
    let mut filepositions: Option<Vec<f64>> = None;

    loop {
        if pos + 2 > data.len() {
            break;
        }
        let key_len = u16::from_be_bytes(data[pos..pos + 2].try_into().unwrap()) as usize;
        pos += 2;
        if key_len == 0 && pos < data.len() && data[pos] == AMF_OBJECT_END {
            pos += 1;
            break;
        }
        if pos + key_len > data.len() {
            break;
        }
        let key = &data[pos..pos + key_len];
        pos += key_len;

        if pos >= data.len() {
            break;
        }

        if key == b"times" || key == b"filepositions" {
            let (arr, consumed) = read_strict_array(data, pos)?;
            pos += consumed;
            if key == b"times" {
                times = Some(arr);
            } else {
                filepositions = Some(arr);
            }
        } else {
            let (_, consumed) = skip_amf_value(data, pos)?;
            pos += consumed;
        }
    }

    match (times, filepositions) {
        (Some(t), Some(fp)) => Ok((t, fp, pos - start)),
        _ => Err(Error::parse("keyframes object missing times or filepositions")),
    }
}

/// Reads an AMF0 strict array of numbers and returns `(values, bytes_consumed)`.
fn read_strict_array(data: &[u8], start: usize) -> Result<(Vec<f64>, usize)> {
    let mut pos = start;
    if pos >= data.len() || data[pos] != AMF_STRICT_ARRAY {
        return Err(Error::parse("expected AMF0 strict array"));
    }
    pos += 1;
    if pos + 4 > data.len() {
        return Err(Error::parse("strict array count truncated"));
    }
    let count = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;

    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        if pos + 1 > data.len() {
            break;
        }
        if data[pos] != AMF_NUMBER {
            return Err(Error::parse("strict array element is not an AMF0 number"));
        }
        pos += 1;
        if pos + 8 > data.len() {
            return Err(Error::parse("AMF0 number truncated"));
        }
        let bits = u64::from_be_bytes(data[pos..pos + 8].try_into().unwrap());
        values.push(f64::from_bits(bits));
        pos += 8;
    }

    Ok((values, pos - start))
}

/// Skips one AMF0 value and returns `((), bytes_consumed)`.
fn skip_amf_value(data: &[u8], start: usize) -> Result<((), usize)> {
    let mut pos = start;
    if pos >= data.len() {
        return Err(Error::parse("AMF0 value expected but data ended"));
    }
    let t = data[pos];
    pos += 1;
    match t {
        AMF_NUMBER => {
            pos += 8;
        }
        AMF_BOOL => {
            pos += 1;
        }
        AMF_STRING => {
            if pos + 2 > data.len() {
                return Err(Error::parse("AMF0 string length truncated"));
            }
            let len = u16::from_be_bytes(data[pos..pos + 2].try_into().unwrap()) as usize;
            pos += 2 + len;
        }
        AMF_OBJECT => loop {
            if pos + 2 > data.len() {
                break;
            }
            let kl = u16::from_be_bytes(data[pos..pos + 2].try_into().unwrap()) as usize;
            pos += 2;
            if kl == 0 && pos < data.len() && data[pos] == AMF_OBJECT_END {
                pos += 1;
                break;
            }
            pos += kl;
            let (_, n) = skip_amf_value(data, pos)?;
            pos += n;
        },
        AMF_NULL | AMF_UNDEFINED => {}
        AMF_REFERENCE => {
            // 2-byte reference index
            pos += 2;
        }
        AMF_ECMA_ARRAY => {
            if pos + 4 > data.len() {
                return Err(Error::parse("ECMA array count truncated"));
            }
            pos += 4;
            loop {
                if pos + 2 > data.len() {
                    break;
                }
                let kl = u16::from_be_bytes(data[pos..pos + 2].try_into().unwrap()) as usize;
                pos += 2;
                if kl == 0 && pos < data.len() && data[pos] == AMF_OBJECT_END {
                    pos += 1;
                    break;
                }
                pos += kl;
                let (_, n) = skip_amf_value(data, pos)?;
                pos += n;
            }
        }
        AMF_STRICT_ARRAY => {
            if pos + 4 > data.len() {
                return Err(Error::parse("strict array count truncated"));
            }
            let count = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
            pos += 4;
            for _ in 0..count {
                let (_, n) = skip_amf_value(data, pos)?;
                pos += n;
            }
        }
        AMF_DATE => {
            // 8-byte f64 timestamp + 2-byte timezone offset
            pos += 10;
        }
        AMF_LONG_STRING => {
            // 4-byte length prefix + N bytes payload
            if pos + 4 > data.len() {
                return Err(Error::parse("AMF0 long string length truncated"));
            }
            let len = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
            pos += 4 + len;
        }
        _ => {
            // Unknown type — cannot determine length; stop
            return Err(Error::parse(format!("unknown AMF0 type: {:#04x}", t)));
        }
    }
    Ok(((), pos - start))
}
