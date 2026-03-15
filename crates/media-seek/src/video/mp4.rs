//! ISO Base Media File Format (MP4 / M4A) container index parsing.
//!
//! Supports two MP4 variants:
//!
//! - **fMP4 via SIDX box** — fragmented MP4 streams that include a `sidx`
//!   (Segment Index Box). Each SIDX entry maps to one fragment; entries are
//!   accumulated into absolute byte offsets and presentation timestamps for
//!   [`ContainerIndex`].
//!
//! - **Classic MP4 via moov box** — non-fragmented MP4/M4A files. The parser
//!   traverses `moov → trak → mdia → minf → stbl` to extract chunk offsets
//!   (`stco`/`co64`), time-to-sample mapping (`stts`), sample-to-chunk mapping
//!   (`stsc`), and keyframe sample indices (`stss`). Segments are produced at
//!   keyframe boundaries for video tracks, or at chunk boundaries for audio tracks.

use crate::RangeFetcher;
use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

// ==================== fMP4 / SIDX constants ====================

/// Size of the initial fetch window when searching for SIDX beyond the probe (512 KB).
const SIDX_FETCH_WINDOW: u64 = 524_288;

// ==================== Classic MP4 / moov constants ====================

/// Size of the FullBox header: version (1 byte) + flags (3 bytes).
const FULL_BOX_HEADER: usize = 4;

/// 4-byte handler type identifying a video track in the `hdlr` box.
const HANDLER_VIDEO: &[u8; 4] = b"vide";

/// 4-byte handler type identifying an audio track in the `hdlr` box.
const HANDLER_AUDIO: &[u8; 4] = b"soun";

/// Byte offset of the handler type within the `hdlr` box payload
/// (after the FullBox header + 4-byte pre_defined field).
const HDLR_HANDLER_TYPE_OFFSET: usize = FULL_BOX_HEADER + 4;

/// Minimum `mdhd` payload size for version 0 (creation, modification, timescale, duration: 4×u32).
const MDHD_V0_MIN: usize = 16;

/// Minimum `mdhd` payload size for version 1 (creation 8B + modification 8B + timescale 4B + …).
const MDHD_V1_MIN: usize = 24;

/// Offset of the timescale field in a version-0 `mdhd` payload.
const MDHD_V0_TIMESCALE_OFFSET: usize = 12;

/// Offset of the timescale field in a version-1 `mdhd` payload.
const MDHD_V1_TIMESCALE_OFFSET: usize = 20;

/// Minimum `stts` payload size: FullBox header + 4-byte entry_count.
const STTS_MIN: usize = FULL_BOX_HEADER + 4;

/// Size of a single `stts` entry: sample_count (u32) + sample_delta (u32).
const STTS_ENTRY_SIZE: usize = 8;

/// Minimum `stco` payload size: FullBox header + 4-byte entry_count.
const STCO_MIN: usize = FULL_BOX_HEADER + 4;

/// Size of a single `stco` chunk offset (u32).
const STCO_ENTRY_SIZE: usize = 4;

/// Size of a single `co64` chunk offset (u64).
const CO64_ENTRY_SIZE: usize = 8;

/// Minimum `stsc` payload size: FullBox header + 4-byte entry_count.
const STSC_MIN: usize = FULL_BOX_HEADER + 4;

/// Size of a single `stsc` entry: first_chunk + samples_per_chunk + sample_description_index (3×u32).
const STSC_ENTRY_SIZE: usize = 12;

/// Minimum `stss` payload size: FullBox header + 4-byte entry_count.
const STSS_MIN: usize = FULL_BOX_HEADER + 4;

/// Size of a single `stss` sample number entry (u32).
const STSS_ENTRY_SIZE: usize = 4;

/// Minimum `stsz` payload size: FullBox header + sample_size (u32) + sample_count (u32).
const STSZ_MIN: usize = FULL_BOX_HEADER + 8;

/// Size of a single variable-length `stsz` entry (u32).
const STSZ_VAR_ENTRY_SIZE: usize = 4;

// ==================== Private intermediate structs ====================

/// One run-length entry from the Time-to-Sample (`stts`) box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SttsEntry {
    /// Number of consecutive samples with this duration.
    count: u32,
    /// Duration (in media timescale units) of each sample in this run.
    duration: u32,
}

/// One entry from the Sample-to-Chunk (`stsc`) box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StscEntry {
    /// 1-based index of the first chunk to which this entry applies.
    first_chunk: u32,
    /// Number of samples per chunk in this range.
    samples_per_chunk: u32,
}

/// All seek-relevant tables collected from a track's `stbl` box.
#[derive(Debug)]
struct MoovTables {
    /// Media timescale (ticks per second) from the `mdhd` box.
    timescale: u32,
    /// Run-length encoded time-to-sample mapping.
    stts: Vec<SttsEntry>,
    /// Absolute byte offsets for each chunk (from `stco` or `co64`).
    chunk_offsets: Vec<u64>,
    /// Sample-to-chunk mapping (run-length encoded).
    stsc: Vec<StscEntry>,
    /// Sync-sample (keyframe) numbers (1-based). Empty if no `stss` box.
    stss: Vec<u32>,
    /// Per-chunk total byte sizes (computed from `stsz` during parsing).
    chunk_sizes: Vec<u64>,
}

// ==================== Public entry point ====================

/// Attempts to fetch a window just beyond `probe` and parse any SIDX boxes found there.
///
/// Returns `Some(result)` when SIDX boxes are found in the fetched window, or `None`
/// when `total_size` is unknown, the probe already covers the full stream, or no SIDX
/// boxes are present in the fetched window. Fetch errors are logged and treated as `None`.
async fn try_sidx_beyond_probe<F: RangeFetcher>(
    probe: &[u8],
    total_size: Option<u64>,
    fetcher: &F,
) -> Option<Result<ContainerIndex>> {
    let total = total_size?;
    let fetch_start = probe.len() as u64;
    if fetch_start >= total {
        return None;
    }
    let fetch_end = (fetch_start + SIDX_FETCH_WINDOW - 1).min(total - 1);
    tracing::debug!(fetch_start, fetch_end, "⚙️ No SIDX/moov in probe, fetching next window");
    let extra = match fetcher.fetch(fetch_start, fetch_end).await {
        Ok(data) => data,
        Err(e) => {
            tracing::debug!(err = %e, "⚙️ fetch beyond probe failed, falling through");
            return None;
        }
    };
    let sidx_list = find_all_sidx(&extra);
    if sidx_list.is_empty() {
        return None;
    }
    let result = parse_all_sidx(&sidx_list, fetch_start);
    if let Ok(ref idx) = result
        && let Inner::Segments(ref segs) = idx.inner
    {
        tracing::debug!(segments = segs.len(), "✅ fMP4 SIDX index parsed (fetched)");
    }
    Some(result)
}

/// Parses an MP4/M4A/ISO BMFF stream and returns a [`ContainerIndex`].
///
/// First checks for fMP4 SIDX boxes (collecting all chained SIDX boxes); if none
/// are found in the probe, attempts to fetch a window beyond the probe to locate
/// SIDX. Falls back to classic moov-based parsing when no SIDX is found.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the MP4/M4A stream (512 KB recommended).
/// * `total_size` - Total stream size in bytes (used for out-of-probe SIDX fetch).
/// * `fetcher` - Provides additional byte ranges when SIDX lies beyond the probe.
///
/// # Errors
///
/// - [`Error::ParseFailed`] when the SIDX or moov box is present but malformed.
/// - [`Error::IndexNotFound`] when neither a SIDX box nor a `moov` box is found.
/// - [`Error::FetchFailed`] when a required extra Range request fails.
pub(crate) async fn parse<F: RangeFetcher>(
    probe: &[u8],
    total_size: Option<u64>,
    fetcher: &F,
) -> Result<ContainerIndex> {
    tracing::debug!(probe_len = probe.len(), "⚙️ Parsing MP4/ISOBMFF stream");

    // Fast path: collect all SIDX boxes from the probe.
    let sidx_list = find_all_sidx(probe);
    if !sidx_list.is_empty() {
        let result = parse_all_sidx(&sidx_list, 0);
        if let Ok(ref idx) = result
            && let Inner::Segments(ref segs) = idx.inner
        {
            tracing::debug!(
                sidx_count = sidx_list.len(),
                segments = segs.len(),
                "✅ fMP4 SIDX index parsed"
            );
        }
        return result;
    }

    // If the probe has no moov either, try fetching beyond the probe for SIDX.
    if find_box(probe, b"moov").is_none() {
        if let Some(result) = try_sidx_beyond_probe(probe, total_size, fetcher).await {
            return result;
        }
        return Err(Error::index_not_found("no SIDX or moov box found in probe"));
    }

    // Classic MP4: locate the moov box.
    tracing::debug!("⚙️ No SIDX found, attempting classic moov-based MP4 parsing");
    let (moov_payload, _moov_end) =
        find_box(probe, b"moov").ok_or_else(|| Error::index_not_found("no SIDX or moov box found in probe"))?;

    let result = parse_moov(moov_payload);
    if let Ok(ref idx) = result
        && let Inner::Segments(ref segs) = idx.inner
    {
        tracing::debug!(segments = segs.len(), "✅ classic MP4 moov index parsed");
    }
    result
}

// ==================== Generic ISOBMFF box locator ====================

/// Finds the first box of `box_type` in `data` using a linear flat traversal.
///
/// Returns `(payload_slice, end_pos)` where `payload_slice` starts immediately
/// after the box header and `end_pos` is the byte position in `data` just after
/// the end of the box.
fn find_box<'a>(data: &'a [u8], box_type: &[u8; 4]) -> Option<(&'a [u8], usize)> {
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size32 = u32::from_be_bytes(data[pos..pos + 4].try_into().ok()?) as usize;

        let (header_len, box_size) = if size32 == 1 {
            if pos + 16 > data.len() {
                return None;
            }
            let extended = u64::from_be_bytes(data[pos + 8..pos + 16].try_into().ok()?) as usize;
            (16, extended)
        } else if size32 == 0 {
            (8, data.len() - pos)
        } else if size32 < 8 {
            return None;
        } else {
            (8, size32)
        };

        if pos + 4 + 4 <= data.len() && &data[pos + 4..pos + 8] == box_type {
            let end = (pos + box_size).min(data.len());
            return Some((&data[pos + header_len..end], pos + box_size));
        }
        pos += box_size;
    }
    None
}

// ==================== fMP4 / SIDX parsing ====================

/// Collects all `sidx` boxes from `data` in a single linear traversal.
///
/// Returns a list of `(body, sidx_end)` pairs where `body` is the box payload
/// and `sidx_end` is the byte position immediately after the box in `data`.
fn find_all_sidx(data: &[u8]) -> Vec<(&[u8], usize)> {
    let mut result = Vec::new();
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size32 = u32::from_be_bytes(match data[pos..pos + 4].try_into() {
            Ok(b) => b,
            Err(_) => break,
        }) as usize;

        let (header_len, box_size) = if size32 == 1 {
            if pos + 16 > data.len() {
                break;
            }
            let extended = u64::from_be_bytes(data[pos + 8..pos + 16].try_into().unwrap()) as usize;
            (16, extended)
        } else if size32 == 0 {
            (8, data.len() - pos)
        } else if size32 < 8 {
            break;
        } else {
            (8, size32)
        };

        if pos + 4 + 4 <= data.len() && &data[pos + 4..pos + 8] == b"sidx" {
            let end = (pos + box_size).min(data.len());
            result.push((&data[pos + header_len..end], pos + box_size));
        }
        pos += box_size;
    }
    result
}

/// Parses all SIDX boxes in `sidx_list` where byte offsets in `data` are relative
/// to `base_offset` within the stream.
fn parse_all_sidx(sidx_list: &[(&[u8], usize)], base_offset: u64) -> Result<ContainerIndex> {
    let mut all_segments: Vec<SegmentEntry> = Vec::new();

    for &(sidx, sidx_end_in_data) in sidx_list {
        let sidx_end_in_stream = base_offset + sidx_end_in_data as u64;
        let mut segs = parse_sidx(sidx, sidx_end_in_stream as usize)?;
        // Adjust byte_offset to be stream-absolute (parse_sidx already uses sidx_end_in_stream).
        all_segments.append(&mut segs);
    }

    if all_segments.is_empty() {
        return Err(Error::parse("all SIDX boxes were empty"));
    }

    // Sort by byte_offset to handle non-ordered SIDX boxes.
    all_segments.sort_unstable_by_key(|s| s.byte_offset);

    let init_end_byte = all_segments
        .first()
        .map(|s| s.byte_offset.saturating_sub(1))
        .unwrap_or(0);

    Ok(ContainerIndex {
        init_end_byte,
        inner: Inner::Segments(all_segments),
    })
}

/// Parses a SIDX box payload and returns a `Vec<SegmentEntry>`.
///
/// SIDX layout (ISO 14496-12 §8.16.3):
///   - 1 byte  version
///   - 3 bytes flags
///   - 4 bytes reference_ID
///   - 4 bytes timescale
///   - version 0: 4+4 bytes earliest_presentation_time + first_offset
///   - version 1: 8+8 bytes earliest_presentation_time + first_offset
///   - 2 bytes reserved
///   - 2 bytes reference_count
///   - reference_count × 12 bytes entries
///
/// `sidx_end_in_stream` is the absolute stream byte offset immediately after this
/// SIDX box; `first_offset` in the SIDX header is added to it to compute the first
/// referenced fragment's byte offset.
fn parse_sidx(sidx: &[u8], sidx_end_in_stream: usize) -> Result<Vec<SegmentEntry>> {
    if sidx.len() < 12 {
        return Err(Error::parse("SIDX box too short for header"));
    }

    let version = sidx[0];
    let timescale = u32::from_be_bytes(
        sidx[8..12]
            .try_into()
            .map_err(|_| Error::parse("SIDX timescale truncated"))?,
    ) as f64;
    if timescale == 0.0 {
        return Err(Error::parse("SIDX timescale is zero"));
    }

    let (earliest_pts, first_offset, header_end) = if version == 0 {
        if sidx.len() < 20 {
            return Err(Error::parse("SIDX v0 too short"));
        }
        let ept = u32::from_be_bytes(sidx[12..16].try_into().unwrap()) as u64;
        let first_offset = u32::from_be_bytes(sidx[16..20].try_into().unwrap()) as u64;
        (ept, first_offset, 20usize)
    } else {
        if sidx.len() < 28 {
            return Err(Error::parse("SIDX v1 too short"));
        }
        let ept = u64::from_be_bytes(sidx[12..20].try_into().unwrap());
        let first_offset = u64::from_be_bytes(sidx[20..28].try_into().unwrap());
        (ept, first_offset, 28usize)
    };

    if sidx.len() < header_end + 4 {
        return Err(Error::parse("SIDX reference_count truncated"));
    }
    let ref_count = u16::from_be_bytes(sidx[header_end + 2..header_end + 4].try_into().unwrap()) as usize;
    let entries_start = header_end + 4;

    if sidx.len() < entries_start + ref_count * 12 {
        return Err(Error::parse("SIDX entries truncated"));
    }

    let mut segments = Vec::with_capacity(ref_count);
    let mut current_pts = earliest_pts as f64;
    let mut current_byte = sidx_end_in_stream as u64 + first_offset;

    for i in 0..ref_count {
        let off = entries_start + i * 12;
        let word0 = u32::from_be_bytes(sidx[off..off + 4].try_into().unwrap());
        let subseg_duration = u32::from_be_bytes(sidx[off + 4..off + 8].try_into().unwrap()) as f64;
        let ref_size = (word0 & 0x7FFF_FFFF) as u64;

        let start_secs = current_pts / timescale;
        let end_secs = (current_pts + subseg_duration) / timescale;

        segments.push(SegmentEntry {
            start_secs,
            end_secs,
            byte_offset: current_byte,
            byte_size: ref_size,
        });

        current_pts += subseg_duration;
        current_byte += ref_size;
    }

    Ok(segments)
}

// ==================== Classic MP4 / moov parsing ====================

/// Parses a `moov` box payload and returns a [`ContainerIndex`].
///
/// Iterates all `trak` boxes, preferring the first video (`vide`) track.
/// Falls back to the first audio (`soun`) track if no video track is found.
fn parse_moov(moov_payload: &[u8]) -> Result<ContainerIndex> {
    let mut video_trak: Option<&[u8]> = None;
    let mut audio_trak: Option<&[u8]> = None;

    // Walk all trak boxes at the moov level.
    let mut pos = 0usize;
    while pos < moov_payload.len() {
        if let Some((trak_payload, trak_end)) = find_box(&moov_payload[pos..], b"trak") {
            let handler = classify_trak(trak_payload);
            match handler.as_ref() {
                Some(h) if h == HANDLER_VIDEO && video_trak.is_none() => video_trak = Some(trak_payload),
                Some(h) if h == HANDLER_AUDIO && audio_trak.is_none() => audio_trak = Some(trak_payload),
                _ => {}
            }
            pos += trak_end;
        } else {
            break;
        }
    }

    let trak = video_trak
        .or(audio_trak)
        .ok_or_else(|| Error::parse("no usable trak box found in moov"))?;

    let tables = extract_stbl_tables(trak)?;

    if tables.chunk_offsets.is_empty() {
        return Err(Error::parse("no chunk offsets found in stco/co64"));
    }

    let segments = build_segments(&tables);
    if segments.is_empty() {
        return Err(Error::parse("no segments could be built from moov tables"));
    }

    // init segment = everything before the first media chunk
    let init_end_byte = tables.chunk_offsets[0].saturating_sub(1);

    tracing::debug!(
        chunks = tables.chunk_offsets.len(),
        keyframes = tables.stss.len(),
        segments = segments.len(),
        "⚙️ classic MP4 tables extracted"
    );

    Ok(ContainerIndex {
        init_end_byte,
        inner: Inner::Segments(segments),
    })
}

/// Returns the 4-byte handler type from the `hdlr` box inside `trak_payload`.
///
/// Traverses `trak → mdia → hdlr` and reads the handler type at `HDLR_HANDLER_TYPE_OFFSET`.
fn classify_trak(trak_payload: &[u8]) -> Option<[u8; 4]> {
    let (mdia, _) = find_box(trak_payload, b"mdia")?;
    let (hdlr, _) = find_box(mdia, b"hdlr")?;
    if hdlr.len() < HDLR_HANDLER_TYPE_OFFSET + 4 {
        return None;
    }
    hdlr[HDLR_HANDLER_TYPE_OFFSET..HDLR_HANDLER_TYPE_OFFSET + 4]
        .try_into()
        .ok()
}

/// Raw payload slices for each seek-relevant box found inside an `stbl` box.
struct RawStblBoxes<'a> {
    stts: Option<&'a [u8]>,
    stco: Option<&'a [u8]>,
    co64: Option<&'a [u8]>,
    stsc: Option<&'a [u8]>,
    stss: Option<&'a [u8]>,
    stsz: Option<&'a [u8]>,
}

/// Decodes the size fields from an ISOBMFF box header at `data[pos..]`.
///
/// Returns `(header_len, box_size)` where `header_len` is the number of bytes
/// consumed by the header (8 for normal boxes, 16 for extended-size boxes) and
/// `box_size` is the total number of bytes in the box including the header.
/// Returns `None` when the box header is invalid or truncated.
fn decode_box_header(data: &[u8], pos: usize) -> Option<(usize, usize)> {
    let size32 = u32::from_be_bytes(data[pos..pos + 4].try_into().ok()?) as usize;
    if size32 == 1 {
        if pos + 16 > data.len() {
            return None;
        }
        let ext = u64::from_be_bytes(data[pos + 8..pos + 16].try_into().ok()?) as usize;
        Some((16, ext))
    } else if size32 == 0 {
        Some((8, data.len() - pos))
    } else if size32 < 8 {
        None
    } else {
        Some((8, size32))
    }
}

/// Traverses the children of an `stbl` box and returns raw payload slices for each
/// seek-relevant box type (`stts`, `stco`, `co64`, `stsc`, `stss`, `stsz`).
///
/// # Arguments
///
/// * `stbl` - The `stbl` box payload (immediately after the box header).
fn walk_stbl_boxes(stbl: &[u8]) -> RawStblBoxes<'_> {
    let mut raw = RawStblBoxes {
        stts: None,
        stco: None,
        co64: None,
        stsc: None,
        stss: None,
        stsz: None,
    };
    let mut pos = 0usize;
    while pos + 8 <= stbl.len() {
        let Some((header_len, box_size)) = decode_box_header(stbl, pos) else {
            break;
        };
        let box_end = (pos + box_size).min(stbl.len());
        let payload = &stbl[pos + header_len..box_end];
        match &stbl[pos + 4..pos + 8] {
            b"stts" => raw.stts = Some(payload),
            b"stco" => raw.stco = raw.stco.or(Some(payload)),
            b"co64" => raw.co64 = raw.co64.or(Some(payload)),
            b"stsc" => raw.stsc = Some(payload),
            b"stss" => raw.stss = Some(payload),
            b"stsz" => raw.stsz = Some(payload),
            _ => {}
        }
        pos += box_size;
    }
    raw
}

/// Traverses `trak → mdia → {mdhd, minf → stbl}` and collects all seek tables.
fn extract_stbl_tables(trak_payload: &[u8]) -> Result<MoovTables> {
    let (mdia, _) = find_box(trak_payload, b"mdia").ok_or_else(|| Error::parse("no mdia box in trak"))?;

    let timescale = parse_mdhd(mdia)?;

    let (minf, _) = find_box(mdia, b"minf").ok_or_else(|| Error::parse("no minf box in mdia"))?;

    let (stbl, _) = find_box(minf, b"stbl").ok_or_else(|| Error::parse("no stbl box in minf"))?;

    let raw = walk_stbl_boxes(stbl);

    let stts = raw.stts.map(parse_stts).unwrap_or_default();
    let chunk_offsets = raw
        .stco
        .map(parse_stco)
        .or_else(|| raw.co64.map(parse_co64))
        .unwrap_or_default();
    let stsc = raw.stsc.map(parse_stsc).unwrap_or_default();
    let stss = raw.stss.map(parse_stss).unwrap_or_default();

    if stts.is_empty() {
        return Err(Error::parse("no stts box found in stbl"));
    }
    if stsc.is_empty() {
        return Err(Error::parse("no stsc box found in stbl"));
    }

    let chunk_count = chunk_offsets.len();
    let chunk_sizes = raw
        .stsz
        .map(|p| parse_stsz_chunk_sizes(p, &stsc, chunk_count))
        .unwrap_or_else(|| vec![0u64; chunk_count]);

    Ok(MoovTables {
        timescale,
        stts,
        chunk_offsets,
        stsc,
        stss,
        chunk_sizes,
    })
}

// ==================== stbl table parsers ====================

/// Extracts the media timescale from the `mdhd` box inside `mdia_payload`.
fn parse_mdhd(mdia_payload: &[u8]) -> Result<u32> {
    let (mdhd, _) = find_box(mdia_payload, b"mdhd").ok_or_else(|| Error::parse("no mdhd box in mdia"))?;
    if mdhd.is_empty() {
        return Err(Error::parse("mdhd box empty"));
    }
    let version = mdhd[0];
    let timescale = if version == 0 {
        if mdhd.len() < MDHD_V0_MIN {
            return Err(Error::parse("mdhd v0 too short"));
        }
        u32::from_be_bytes(
            mdhd[MDHD_V0_TIMESCALE_OFFSET..MDHD_V0_TIMESCALE_OFFSET + 4]
                .try_into()
                .unwrap(),
        )
    } else {
        if mdhd.len() < MDHD_V1_MIN {
            return Err(Error::parse("mdhd v1 too short"));
        }
        u32::from_be_bytes(
            mdhd[MDHD_V1_TIMESCALE_OFFSET..MDHD_V1_TIMESCALE_OFFSET + 4]
                .try_into()
                .unwrap(),
        )
    };
    if timescale == 0 {
        return Err(Error::parse("mdhd timescale is zero"));
    }
    Ok(timescale)
}

/// Parses the `stts` (Time-to-Sample) box payload into a run-length entry list.
fn parse_stts(payload: &[u8]) -> Vec<SttsEntry> {
    if payload.len() < STTS_MIN {
        return Vec::new();
    }
    let count = u32::from_be_bytes(payload[FULL_BOX_HEADER..FULL_BOX_HEADER + 4].try_into().unwrap()) as usize;
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let off = STTS_MIN + i * STTS_ENTRY_SIZE;
        if off + STTS_ENTRY_SIZE > payload.len() {
            break;
        }
        let sample_count = u32::from_be_bytes(payload[off..off + 4].try_into().unwrap());
        let sample_delta = u32::from_be_bytes(payload[off + 4..off + 8].try_into().unwrap());
        entries.push(SttsEntry {
            count: sample_count,
            duration: sample_delta,
        });
    }
    entries
}

/// Parses the `stco` (Chunk Offset) box payload into 64-bit chunk offsets.
fn parse_stco(payload: &[u8]) -> Vec<u64> {
    if payload.len() < STCO_MIN {
        return Vec::new();
    }
    let count = u32::from_be_bytes(payload[FULL_BOX_HEADER..FULL_BOX_HEADER + 4].try_into().unwrap()) as usize;
    let mut offsets = Vec::with_capacity(count);
    for i in 0..count {
        let off = STCO_MIN + i * STCO_ENTRY_SIZE;
        if off + STCO_ENTRY_SIZE > payload.len() {
            break;
        }
        offsets.push(u32::from_be_bytes(payload[off..off + 4].try_into().unwrap()) as u64);
    }
    offsets
}

/// Parses the `co64` (64-bit Chunk Offset) box payload into chunk offsets.
fn parse_co64(payload: &[u8]) -> Vec<u64> {
    if payload.len() < STCO_MIN {
        return Vec::new();
    }
    let count = u32::from_be_bytes(payload[FULL_BOX_HEADER..FULL_BOX_HEADER + 4].try_into().unwrap()) as usize;
    let mut offsets = Vec::with_capacity(count);
    for i in 0..count {
        let off = STCO_MIN + i * CO64_ENTRY_SIZE;
        if off + CO64_ENTRY_SIZE > payload.len() {
            break;
        }
        offsets.push(u64::from_be_bytes(payload[off..off + 8].try_into().unwrap()));
    }
    offsets
}

/// Parses the `stsc` (Sample-to-Chunk) box payload.
fn parse_stsc(payload: &[u8]) -> Vec<StscEntry> {
    if payload.len() < STSC_MIN {
        return Vec::new();
    }
    let count = u32::from_be_bytes(payload[FULL_BOX_HEADER..FULL_BOX_HEADER + 4].try_into().unwrap()) as usize;
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let off = STSC_MIN + i * STSC_ENTRY_SIZE;
        if off + STSC_ENTRY_SIZE > payload.len() {
            break;
        }
        let first_chunk = u32::from_be_bytes(payload[off..off + 4].try_into().unwrap());
        let samples_per_chunk = u32::from_be_bytes(payload[off + 4..off + 8].try_into().unwrap());
        // sample_description_index (bytes 8-11) is not needed for seeking.
        entries.push(StscEntry {
            first_chunk,
            samples_per_chunk,
        });
    }
    entries
}

/// Parses the `stss` (Sync Sample) box payload into a list of 1-based sample numbers.
fn parse_stss(payload: &[u8]) -> Vec<u32> {
    if payload.len() < STSS_MIN {
        return Vec::new();
    }
    let count = u32::from_be_bytes(payload[FULL_BOX_HEADER..FULL_BOX_HEADER + 4].try_into().unwrap()) as usize;
    let mut samples = Vec::with_capacity(count);
    for i in 0..count {
        let off = STSS_MIN + i * STSS_ENTRY_SIZE;
        if off + STSS_ENTRY_SIZE > payload.len() {
            break;
        }
        samples.push(u32::from_be_bytes(payload[off..off + 4].try_into().unwrap()));
    }
    samples
}

/// Accumulates variable-size `stsz` entries into a per-chunk byte total.
fn accumulate_variable_chunk_sizes(
    payload: &[u8],
    stsc: &[StscEntry],
    chunk_count: usize,
    sample_count: usize,
) -> Vec<u64> {
    let data_start = STSZ_MIN;
    let available = (payload.len() - data_start) / STSZ_VAR_ENTRY_SIZE;
    let entry_count = sample_count.min(available);

    let sample_sizes: Vec<u64> = (0..entry_count)
        .map(|i| {
            let off = data_start + i * STSZ_VAR_ENTRY_SIZE;
            u32::from_be_bytes(payload[off..off + 4].try_into().unwrap()) as u64
        })
        .collect();

    let mut sizes = vec![0u64; chunk_count];
    let mut sample_idx = 0usize;
    for chunk_1based in 1..=chunk_count {
        let spc = samples_per_chunk_for(stsc, chunk_1based as u32) as usize;
        for _ in 0..spc {
            if sample_idx < sample_sizes.len() {
                sizes[chunk_1based - 1] += sample_sizes[sample_idx];
                sample_idx += 1;
            }
        }
    }
    sizes
}

/// Computes the total byte size for each chunk from the `stsz` box.
///
/// Returns a `Vec<u64>` of length `chunk_count` where `[i]` is the byte total
/// of chunk `i + 1` (1-based chunk numbering in ISO 14496-12).
fn parse_stsz_chunk_sizes(payload: &[u8], stsc: &[StscEntry], chunk_count: usize) -> Vec<u64> {
    if payload.len() < STSZ_MIN || chunk_count == 0 {
        return vec![0u64; chunk_count];
    }
    let fixed_size = u32::from_be_bytes(payload[FULL_BOX_HEADER..FULL_BOX_HEADER + 4].try_into().unwrap());
    let sample_count =
        u32::from_be_bytes(payload[FULL_BOX_HEADER + 4..FULL_BOX_HEADER + 8].try_into().unwrap()) as usize;

    if fixed_size > 0 {
        // All samples have the same size — compute per-chunk total directly.
        (1..=chunk_count)
            .map(|c| samples_per_chunk_for(stsc, c as u32) as u64 * fixed_size as u64)
            .collect()
    } else {
        accumulate_variable_chunk_sizes(payload, stsc, chunk_count, sample_count)
    }
}

// ==================== Time / chunk helpers ====================

/// Returns the number of samples per chunk for `chunk_idx` (1-based) via the `stsc` table.
fn samples_per_chunk_for(stsc: &[StscEntry], chunk_idx_1based: u32) -> u32 {
    // Find the last stsc entry whose first_chunk <= chunk_idx_1based.
    let mut spc = stsc.first().map(|e| e.samples_per_chunk).unwrap_or(0);
    for entry in stsc {
        if entry.first_chunk <= chunk_idx_1based {
            spc = entry.samples_per_chunk;
        } else {
            break;
        }
    }
    spc
}

/// Returns the cumulative media-timescale duration at the start of sample `sample_num` (1-based).
///
/// Iterates `stts` runs in O(runs) time without expanding to a full per-sample array.
fn cum_dur_at_sample(stts: &[SttsEntry], sample_num: u32) -> u64 {
    let mut remaining = sample_num.saturating_sub(1); // samples before this one
    let mut cum: u64 = 0;
    for entry in stts {
        if remaining == 0 {
            break;
        }
        let take = remaining.min(entry.count);
        cum = cum.saturating_add((take as u64).saturating_mul(entry.duration as u64));
        remaining -= take;
    }
    cum
}

/// Returns the total number of samples encoded in `stts`.
fn total_samples_from_stts(stts: &[SttsEntry]) -> u64 {
    stts.iter().map(|e| e.count as u64).sum()
}

/// Precomputed run data for one `stsc` entry.
#[derive(Debug, Clone, Copy)]
struct StscRunInfo {
    /// Samples per chunk for this run.
    samples_per_chunk: u32,
    /// Total samples covered by this run.
    samples_in_run: u32,
}

/// Computes [`StscRunInfo`] for entry `i` given the total `chunk_count`.
fn stsc_run_info(stsc: &[StscEntry], i: usize, chunk_count: usize) -> StscRunInfo {
    let next_first_chunk = if i + 1 < stsc.len() {
        stsc[i + 1].first_chunk
    } else {
        chunk_count as u32 + 1
    };
    let spc = stsc[i].samples_per_chunk;
    let chunks_in_run = next_first_chunk.saturating_sub(stsc[i].first_chunk);
    let samples_in_run = (chunks_in_run as u64).saturating_mul(spc as u64) as u32;
    StscRunInfo {
        samples_per_chunk: spc,
        samples_in_run,
    }
}

/// Maps a 1-based sample number to a 0-based chunk index using the `stsc` table.
///
/// Returns `None` if `stsc` or `chunk_offsets` are empty.
fn sample_to_chunk_idx(stsc: &[StscEntry], chunk_offsets: &[u64], sample_num: u32) -> Option<usize> {
    if stsc.is_empty() || chunk_offsets.is_empty() {
        return None;
    }
    let chunk_count = chunk_offsets.len();
    let mut sample_cursor: u32 = 1; // first sample of current chunk
    for i in 0..stsc.len() {
        let run = stsc_run_info(stsc, i, chunk_count);
        let run_end_sample = sample_cursor.saturating_add(run.samples_in_run);
        if sample_num < run_end_sample {
            // sample is in this run
            let offset_into_run = (sample_num - sample_cursor) / run.samples_per_chunk;
            let chunk_1based = stsc[i].first_chunk + offset_into_run;
            return Some(chunk_1based.saturating_sub(1) as usize);
        }
        sample_cursor = run_end_sample;
    }
    // Fallback: last chunk.
    Some(chunk_count.saturating_sub(1))
}

/// Returns the 1-based sample number of the first sample in chunk `chunk_idx` (0-based).
fn first_sample_of_chunk(stsc: &[StscEntry], chunk_idx_0based: usize) -> u32 {
    let chunk_1based = (chunk_idx_0based + 1) as u32;
    let mut sample_cursor: u32 = 1;
    for i in 0..stsc.len() {
        let next_first_chunk = if i + 1 < stsc.len() {
            stsc[i + 1].first_chunk
        } else {
            u32::MAX
        };
        let spc = stsc[i].samples_per_chunk;
        if chunk_1based < next_first_chunk {
            let offset = chunk_1based.saturating_sub(stsc[i].first_chunk);
            return sample_cursor + offset * spc;
        }
        let chunks_in_run = next_first_chunk.saturating_sub(stsc[i].first_chunk);
        sample_cursor += chunks_in_run * spc;
    }
    sample_cursor
}

// ==================== Segment builder ====================

/// Builds a `Vec<SegmentEntry>` from the collected `MoovTables`.
///
/// Uses keyframe-based segmentation (Mode 1) when the `stss` table is non-empty,
/// otherwise falls back to chunk-based segmentation (Mode 2) for audio tracks.
fn build_segments(tables: &MoovTables) -> Vec<SegmentEntry> {
    let timescale = tables.timescale as f64;
    let total_samples = total_samples_from_stts(&tables.stts);
    let total_secs = if timescale > 0.0 {
        let total_dur = tables
            .stts
            .iter()
            .map(|e| e.count as u64 * e.duration as u64)
            .sum::<u64>();
        total_dur as f64 / timescale
    } else {
        0.0
    };

    if !tables.stss.is_empty() {
        build_keyframe_segments(tables, timescale, total_secs, total_samples)
    } else {
        build_chunk_segments(tables, timescale, total_secs)
    }
}

/// Computes the byte size of a keyframe segment from `chunk_idx` to `next_chunk_idx`.
///
/// # Arguments
///
/// * `tables` - The collected moov seek tables.
/// * `chunk_idx` - 0-based index of the first chunk in this segment.
/// * `next_chunk_idx` - 0-based index of the first chunk of the next keyframe segment,
///   or `None` when this is the last segment.
fn compute_segment_byte_size(tables: &MoovTables, chunk_idx: usize, next_chunk_idx: Option<usize>) -> u64 {
    match next_chunk_idx {
        Some(nc) if nc > chunk_idx => tables.chunk_offsets[nc].saturating_sub(tables.chunk_offsets[chunk_idx]),
        Some(_) => {
            // Next keyframe is in the same chunk — use the chunk size.
            tables.chunk_sizes.get(chunk_idx).copied().unwrap_or(0)
        }
        None => {
            // Last keyframe segment: sum the remaining chunk sizes.
            tables.chunk_sizes[chunk_idx..].iter().sum()
        }
    }
}

/// Fills in `end_secs` for each segment from the next segment's `start_secs`.
///
/// The last segment receives `total_secs` as its end time.
fn fixup_segment_end_times(segments: &mut [SegmentEntry], total_secs: f64) {
    if let Some(last) = segments.last_mut() {
        last.end_secs = total_secs;
    }
    for i in (0..segments.len().saturating_sub(1)).rev() {
        let next_start = segments[i + 1].start_secs;
        segments[i].end_secs = next_start;
    }
}

/// Mode 1: keyframe-based segments using the `stss` sync-sample table.
fn build_keyframe_segments(
    tables: &MoovTables,
    timescale: f64,
    total_secs: f64,
    _total_samples: u64,
) -> Vec<SegmentEntry> {
    let mut segments: Vec<SegmentEntry> = Vec::with_capacity(tables.stss.len());

    for (ki, &kf_sample) in tables.stss.iter().enumerate() {
        let start_secs = cum_dur_at_sample(&tables.stts, kf_sample) as f64 / timescale;

        let chunk_idx = match sample_to_chunk_idx(&tables.stsc, &tables.chunk_offsets, kf_sample) {
            Some(c) => c,
            None => continue,
        };
        let byte_offset = tables.chunk_offsets[chunk_idx];

        // Determine end byte: span all chunks up to (but not including) the next keyframe's chunk.
        let next_chunk_idx = tables
            .stss
            .get(ki + 1)
            .and_then(|&next_kf| sample_to_chunk_idx(&tables.stsc, &tables.chunk_offsets, next_kf));

        let byte_size = compute_segment_byte_size(tables, chunk_idx, next_chunk_idx);

        segments.push(SegmentEntry {
            start_secs,
            end_secs: 0.0, // fixed up below
            byte_offset,
            byte_size,
        });
    }

    fixup_segment_end_times(&mut segments, total_secs);
    segments
}

/// Mode 2: one segment per chunk (audio-only or video without stss).
fn build_chunk_segments(tables: &MoovTables, timescale: f64, total_secs: f64) -> Vec<SegmentEntry> {
    let chunk_count = tables.chunk_offsets.len();
    let mut segments = Vec::with_capacity(chunk_count);

    for c in 0..chunk_count {
        let first_sample = first_sample_of_chunk(&tables.stsc, c);
        let start_secs = cum_dur_at_sample(&tables.stts, first_sample) as f64 / timescale;
        let byte_offset = tables.chunk_offsets[c];
        let byte_size = tables.chunk_sizes.get(c).copied().unwrap_or(0);

        segments.push(SegmentEntry {
            start_secs,
            end_secs: 0.0, // fixed up below
            byte_offset,
            byte_size,
        });
    }

    fixup_segment_end_times(&mut segments, total_secs);
    segments
}
