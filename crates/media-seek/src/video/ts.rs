//! MPEG-2 Transport Stream container index via PCR binary search.
//!
//! TS streams are a sequence of 188-byte packets beginning with 0x47 (sync byte).
//! This module reads the PAT to find the PMT PID, reads the PMT to find the PCR PID,
//! then binary-searches the stream for PCR timestamps to build a coarse seek index.

use futures_util::future::try_join_all;

use crate::RangeFetcher;
use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

/// MPEG-TS packet size in bytes (as u64 for range arithmetic).
const PKT_SIZE: u64 = 188;
/// MPEG-TS packet size in bytes (as usize for slice indexing in detect).
pub(crate) const TS_PKT_SIZE: usize = 188;
/// Number of seek points to build in the binary search.
const SEEK_POINTS: u64 = 64;
/// Bytes to fetch per binary search probe (must be >= PKT_SIZE, ideally several packets).
const PROBE_WINDOW: u64 = 4096;
/// TS sync byte — first byte of every 188-byte packet.
pub(crate) const TS_SYNC: u8 = 0x47;
/// PAT PID in MPEG-TS.
const PAT_PID: u16 = 0x0000;
/// PCR flag bit in the adaptation field flags byte.
const PCR_FLAG: u8 = 0x10;
/// Minimum adaptation field length to contain a PCR (6 bytes of PCR data + flags byte).
const PCR_AF_MIN_LEN: usize = 7;
/// PCR base-to-27 MHz multiplier.
const PCR_BASE_MULTIPLIER: u64 = 300;
/// MPEG-TS system clock frequency in Hz (27 MHz).
const SYSTEM_CLOCK_HZ: f64 = 27_000_000.0;
/// Deduplication granularity: two PCR values within ~100 ms are considered equal.
const PCR_DEDUP_SCALE: f64 = 10.0;

/// Parses an MPEG-TS stream and returns a `ContainerIndex`.
///
/// Reads the PAT and PMT from `probe` to identify the PCR PID, then performs a
/// binary search over the stream using Range requests to sample PCR timestamps at
/// equally-spaced byte positions.
///
/// # Arguments
///
/// * `probe` - Leading bytes of the TS stream (at least 64 KB recommended to include PAT+PMT).
/// * `total_size` - Total stream size in bytes.
/// * `fetcher` - Provides byte ranges for the binary search.
///
/// # Errors
///
/// Returns `Error::ParseFailed` when PAT/PMT parsing fails, `Error::FetchFailed`
/// on Range request errors.
pub(crate) async fn parse<F>(probe: &[u8], total_size: Option<u64>, fetcher: &F) -> Result<ContainerIndex>
where
    F: RangeFetcher,
{
    tracing::debug!(probe_len = probe.len(), total_size = ?total_size, "⚙️ Parsing MPEG-TS stream");
    let total = total_size.ok_or_else(|| Error::parse("TS PCR search requires total_size"))?;

    // Try the structured PAT→PMT path first; fall back to brute-force scan for
    // audio-only or unusual streams where PAT/PMT may not be in the probe.
    let pcr_pid = find_pcr_pid(probe)
        .or_else(|| find_any_pcr_pid(probe))
        .ok_or_else(|| Error::index_not_found("no PCR PID found in TS probe (audio-only or PAT/PMT absent)"))?;

    // Build coarse seek index: sample SEEK_POINTS equidistant byte positions.
    // Phase 1 (in-probe) and Phase 2 (remote, concurrent) are handled by collect_pcr_points.
    let points = collect_pcr_points(probe, total, pcr_pid, fetcher).await?;

    if points.is_empty() {
        return Err(Error::parse("no PCR timestamps found during TS binary search"));
    }

    // Pre-compute average inter-point duration for the last segment's end estimate.
    let avg_pcr_step = if points.len() >= 2 {
        (points[points.len() - 1].secs - points[0].secs) / (points.len() - 1) as f64
    } else {
        0.0
    };

    let mut segments = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        let start_secs = points[i].secs;
        let byte_offset = points[i].byte_pos;
        let (end_secs, next_byte) = if i + 1 < points.len() {
            (points[i + 1].secs, points[i + 1].byte_pos)
        } else {
            (start_secs + avg_pcr_step, total)
        };
        segments.push(SegmentEntry {
            start_secs,
            end_secs,
            byte_offset,
            byte_size: next_byte.saturating_sub(byte_offset),
        });
    }

    tracing::debug!(points = segments.len(), "✅ TS index parsed");
    Ok(ContainerIndex {
        init_end_byte: 0, // TS has no separate init segment
        inner: Inner::Segments(segments),
    })
}

/// A sampled PCR timestamp with its aligned byte position.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PcrPoint {
    secs: f64,
    byte_pos: u64,
}

/// Collects, deduplicates, and sorts [`PcrPoint`] samples across the stream.
///
/// Phase 1 reads positions that fall within `probe` synchronously.
/// Phase 2 fetches all out-of-probe positions concurrently via `try_join_all`.
async fn collect_pcr_points<F: RangeFetcher>(
    probe: &[u8],
    total: u64,
    pcr_pid: u16,
    fetcher: &F,
) -> Result<Vec<PcrPoint>> {
    let mut points: Vec<PcrPoint> = Vec::new();
    let mut remote_positions: Vec<(u64, u64)> = Vec::new();

    for i in 0..SEEK_POINTS {
        let byte_pos = (i.saturating_mul(total) / SEEK_POINTS) / PKT_SIZE * PKT_SIZE;
        let window_end = (byte_pos + PROBE_WINDOW).min(total).saturating_sub(1);

        if (byte_pos as usize) < probe.len() {
            let end = (window_end as usize).min(probe.len());
            let slice = &probe[byte_pos as usize..end];
            if let Some(pcr_secs) = find_pcr_in_window(slice, pcr_pid) {
                points.push(PcrPoint {
                    secs: pcr_secs,
                    byte_pos: align_to_sync(slice, byte_pos),
                });
            }
        } else {
            remote_positions.push((byte_pos, window_end));
        }
    }

    if !remote_positions.is_empty() {
        let fetch_futures: Vec<_> = remote_positions
            .iter()
            .map(|&(byte_pos, window_end)| fetcher.fetch(byte_pos, window_end))
            .collect();
        let chunks = try_join_all(fetch_futures).await.map_err(Error::fetch)?;

        for (chunk, &(byte_pos, _)) in chunks.iter().zip(remote_positions.iter()) {
            if let Some(pcr_secs) = find_pcr_in_window(chunk, pcr_pid) {
                points.push(PcrPoint {
                    secs: pcr_secs,
                    byte_pos: align_to_sync(chunk, byte_pos),
                });
            }
        }
    }

    // Deduplicate by PCR time (~100 ms granularity), then sort by byte offset.
    points.sort_unstable_by(|a, b| a.secs.partial_cmp(&b.secs).unwrap_or(std::cmp::Ordering::Equal));
    points.dedup_by_key(|p| (p.secs * PCR_DEDUP_SCALE) as u64);
    points.sort_unstable_by_key(|p| p.byte_pos);

    Ok(points)
}

/// Reads the PAT from `data` to find the PMT PID, then reads the PMT to find the PCR PID.
fn find_pcr_pid(data: &[u8]) -> Option<u16> {
    let pmt_pid = read_pat(data)?;
    read_pmt_pcr_pid(data, pmt_pid)
}

/// Brute-force scan: returns the PID of the first TS packet that carries a PCR.
///
/// Used as a fallback when PAT/PMT are absent or the stream is audio-only.
fn find_any_pcr_pid(data: &[u8]) -> Option<u16> {
    let n_pkts = data.len() / PKT_SIZE as usize;
    for i in 0..n_pkts {
        let pkt = &data[i * PKT_SIZE as usize..(i + 1) * PKT_SIZE as usize];
        if pkt[0] != TS_SYNC {
            continue;
        }
        let adaptation_field_control = (pkt[3] >> 4) & 0x03;
        if adaptation_field_control != 0x02 && adaptation_field_control != 0x03 {
            continue;
        }
        if pkt.len() < 6 {
            continue;
        }
        let af_len = pkt[4] as usize;
        if af_len < PCR_AF_MIN_LEN {
            continue;
        }
        let af = &pkt[5..5 + af_len];
        if af[0] & PCR_FLAG == 0 {
            continue;
        }
        let pid = (((pkt[1] & 0x1F) as u16) << 8) | pkt[2] as u16;
        // Ignore null PID (0x1FFF) which carries no real PCR.
        if pid == 0x1FFF {
            continue;
        }
        tracing::debug!(pid, "⚙️ TS PCR PID found by brute-force scan");
        return Some(pid);
    }
    None
}

/// Scans `data` for a PAT packet (PID 0) and returns the first program's PMT PID.
fn read_pat(data: &[u8]) -> Option<u16> {
    let n_pkts = data.len() / PKT_SIZE as usize;
    for i in 0..n_pkts {
        let pkt = &data[i * PKT_SIZE as usize..(i + 1) * PKT_SIZE as usize];
        if pkt[0] != TS_SYNC {
            continue;
        }
        let pid = (((pkt[1] & 0x1F) as u16) << 8) | pkt[2] as u16;
        if pid != PAT_PID {
            continue;
        }
        let payload_start = pat_payload_start(pkt)?;
        let payload = &pkt[payload_start..];
        if let Some(pmt_pid) = extract_pmt_pid_from_pat_payload(payload) {
            return Some(pmt_pid);
        }
    }
    None
}

/// Extracts the PMT PID from a PAT section payload.
///
/// Handles the NIT special case (program_number == 0) by skipping the NIT
/// entry and returning the PID from the next program entry.
///
/// # Arguments
///
/// * `payload` - PAT payload bytes starting at the pointer field.
fn extract_pmt_pid_from_pat_payload(payload: &[u8]) -> Option<u16> {
    // PAT payload: pointer_field (1) + table_id (1) + section_length (2 masked) + ...
    // After section header (8 bytes), programs are 4 bytes each: program_number(2) + PMT_PID(2 masked)
    if payload.len() < 13 {
        return None;
    }
    let pointer = payload[0] as usize;
    let off = 1 + pointer + 8; // skip pointer + PAT table header (8 bytes)
    if off + 4 > payload.len() {
        return None;
    }
    let prog_num = u16::from_be_bytes(payload[off..off + 2].try_into().ok()?);
    if prog_num == 0 {
        // NIT entry — skip and read the next program entry
        if off + 8 > payload.len() {
            return None;
        }
        let next_prog = u16::from_be_bytes(payload[off + 4..off + 6].try_into().ok()?);
        if next_prog == 0 {
            return None;
        }
        let pmt_pid = (((payload[off + 6] & 0x1F) as u16) << 8) | payload[off + 7] as u16;
        return Some(pmt_pid);
    }
    let pmt_pid = (((payload[off + 2] & 0x1F) as u16) << 8) | payload[off + 3] as u16;
    Some(pmt_pid)
}

/// Returns the byte offset of the PAT/PMT payload within a TS packet.
fn pat_payload_start(pkt: &[u8]) -> Option<usize> {
    // byte 1: transport_error, payload_unit_start_indicator, transport_priority, PID[12:8]
    // byte 3: [scrambling:2][adaptation_field_control:2][continuity:4]
    let adaptation_field_control = (pkt[3] >> 4) & 0x03;
    match adaptation_field_control {
        0x01 => Some(4), // payload only
        0x02 => None,    // adaptation only, no payload
        0x03 => {
            // adaptation + payload
            let af_len = pkt[4] as usize;
            Some(5 + af_len)
        }
        _ => None,
    }
}

/// Scans `data` for a PMT packet with the given PID and returns the PCR PID.
fn read_pmt_pcr_pid(data: &[u8], pmt_pid: u16) -> Option<u16> {
    let n_pkts = data.len() / PKT_SIZE as usize;
    for i in 0..n_pkts {
        let pkt = &data[i * PKT_SIZE as usize..(i + 1) * PKT_SIZE as usize];
        if pkt[0] != TS_SYNC {
            continue;
        }
        let pid = (((pkt[1] & 0x1F) as u16) << 8) | pkt[2] as u16;
        if pid != pmt_pid {
            continue;
        }
        let payload_start = pat_payload_start(pkt)?;
        let payload = &pkt[payload_start..];
        // PMT header: pointer(1) + table_id(1) + section_length(2) + program_number(2) +
        //             version/current(1) + section_number(1) + last_section(1) + PCR_PID(2) + ...
        if payload.len() < 13 {
            continue;
        }
        let pointer = payload[0] as usize;
        let off = 1 + pointer + 8; // skip to PCR_PID field
        if off + 2 > payload.len() {
            continue;
        }
        let pcr_pid = (((payload[off] & 0x1F) as u16) << 8) | payload[off + 1] as u16;
        return Some(pcr_pid);
    }
    None
}

/// Extracts a PCR timestamp (in seconds) from a single TS packet.
///
/// Returns `None` when the packet does not carry a PCR for `pcr_pid`.
fn extract_pcr_from_packet(pkt: &[u8], pcr_pid: u16) -> Option<f64> {
    if pkt[0] != TS_SYNC {
        return None;
    }
    let pid = (((pkt[1] & 0x1F) as u16) << 8) | pkt[2] as u16;
    if pid != pcr_pid {
        return None;
    }
    // PCR is in the adaptation field
    let adaptation_field_control = (pkt[3] >> 4) & 0x03;
    if adaptation_field_control != 0x02 && adaptation_field_control != 0x03 {
        return None;
    }
    if pkt.len() < 6 {
        return None;
    }
    let af_len = pkt[4] as usize;
    if af_len < PCR_AF_MIN_LEN {
        return None;
    }
    let af = &pkt[5..5 + af_len];
    if af[0] & PCR_FLAG == 0 {
        return None;
    }
    // PCR base (33 bits) + reserved (6 bits) + PCR extension (9 bits)
    let base = ((af[1] as u64) << 25)
        | ((af[2] as u64) << 17)
        | ((af[3] as u64) << 9)
        | ((af[4] as u64) << 1)
        | ((af[5] >> 7) as u64);
    let ext = (((af[5] & 0x01) as u64) << 8) | af[6] as u64;
    let pcr_value = base * PCR_BASE_MULTIPLIER + ext;
    Some(pcr_value as f64 / SYSTEM_CLOCK_HZ)
}

/// Scans `window` for a TS packet carrying a PCR for `pcr_pid` and returns the PCR in seconds.
fn find_pcr_in_window(window: &[u8], pcr_pid: u16) -> Option<f64> {
    let n_pkts = window.len() / PKT_SIZE as usize;
    for i in 0..n_pkts {
        let pkt = &window[i * PKT_SIZE as usize..(i + 1) * PKT_SIZE as usize];
        if let Some(pcr) = extract_pcr_from_packet(pkt, pcr_pid) {
            return Some(pcr);
        }
    }
    None
}

/// Returns `byte_pos` aligned to the first validated sync byte (0x47) in `window`.
///
/// A candidate sync byte is validated by checking that the next packet boundary
/// (188 bytes later) also carries 0x47, preventing false positives on payload data.
fn align_to_sync(window: &[u8], byte_pos: u64) -> u64 {
    for (i, &b) in window.iter().enumerate() {
        if b == TS_SYNC {
            let next = i + PKT_SIZE as usize;
            // If next packet is in bounds, verify its sync byte too.
            if next >= window.len() || window[next] == TS_SYNC {
                return byte_pos + i as u64;
            }
        }
    }
    byte_pos
}
