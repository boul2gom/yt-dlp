//! MPEG-2 Transport Stream container index via PCR binary search.
//!
//! TS streams are a sequence of 188-byte packets beginning with 0x47 (sync byte).
//! This module reads the PAT to find the PMT PID, reads the PMT to find the PCR PID,
//! then binary-searches the stream for PCR timestamps to build a coarse seek index.

use futures_util::future::try_join_all;

use crate::RangeFetcher;
use crate::error::{Error, Result};
use crate::index::{ContainerIndex, Inner, SegmentEntry};

/// MPEG-TS packet size in bytes.
const PKT_SIZE: u64 = 188;
/// Number of seek points to build in the binary search.
const SEEK_POINTS: u64 = 64;
/// Bytes to fetch per binary search probe (must be >= PKT_SIZE, ideally several packets).
const PROBE_WINDOW: u64 = 4096;

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

    let pcr_pid = find_pcr_pid(probe).ok_or_else(|| Error::parse("could not find PCR PID from PAT/PMT"))?;

    // Build coarse seek index: sample SEEK_POINTS equidistant byte positions.
    // Phase 1: synchronously handle positions that fall within the probe buffer.
    // Phase 2: fetch all out-of-probe positions concurrently (1 RTT instead of 64).
    let mut points: Vec<(f64, u64)> = Vec::new();
    let mut remote_positions: Vec<(u64, u64)> = Vec::new(); // (byte_pos, window_end)

    for i in 0..SEEK_POINTS {
        let byte_pos = (i * total / SEEK_POINTS) & !(PKT_SIZE - 1); // align to packet boundary
        let window_end = (byte_pos + PROBE_WINDOW).min(total);

        if (byte_pos as usize) < probe.len() {
            let end = (window_end as usize).min(probe.len());
            let slice = &probe[byte_pos as usize..end];
            if let Some(pcr_secs) = find_pcr_in_window(slice, pcr_pid) {
                points.push((pcr_secs, align_to_sync(slice, byte_pos)));
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
                points.push((pcr_secs, align_to_sync(chunk, byte_pos)));
            }
        }
    }

    if points.is_empty() {
        return Err(Error::parse("no PCR timestamps found during TS binary search"));
    }

    points.sort_unstable_by(|a, b| a.1.cmp(&b.1));
    points.dedup_by_key(|p| (p.0 * 10.0) as u64); // deduplicate by ~100 ms granularity

    let mut segments = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        let (start_secs, byte_offset) = points[i];
        let (end_secs, next_byte) = if i + 1 < points.len() {
            points[i + 1]
        } else {
            (start_secs, total)
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

/// Reads the PAT from `data` to find the PMT PID, then reads the PMT to find the PCR PID.
fn find_pcr_pid(data: &[u8]) -> Option<u16> {
    let pmt_pid = read_pat(data)?;
    read_pmt_pcr_pid(data, pmt_pid)
}

/// Scans `data` for a PAT packet (PID 0) and returns the first program's PMT PID.
fn read_pat(data: &[u8]) -> Option<u16> {
    let n_pkts = data.len() / PKT_SIZE as usize;
    for i in 0..n_pkts {
        let pkt = &data[i * PKT_SIZE as usize..(i + 1) * PKT_SIZE as usize];
        if pkt[0] != 0x47 {
            continue;
        }
        let pid = (((pkt[1] & 0x1F) as u16) << 8) | pkt[2] as u16;
        if pid != 0x0000 {
            continue;
        }
        // PAT packet — skip adaptation field and point to payload
        let payload_start = pat_payload_start(pkt)?;
        let payload = &pkt[payload_start..];
        // PAT payload: pointer_field (1) + table_id (1) + section_length (2 masked) + ...
        // After section header (8 bytes), programs are 4 bytes each: program_number(2) + PMT_PID(2 masked)
        if payload.len() < 13 {
            continue;
        }
        let pointer = payload[0] as usize;
        let off = 1 + pointer + 8; // skip pointer + PAT table header (8 bytes)
        if off + 4 > payload.len() {
            continue;
        }
        let prog_num = u16::from_be_bytes(payload[off..off + 2].try_into().ok()?);
        if prog_num == 0 {
            // NIT entry — skip
            if off + 8 > payload.len() {
                continue;
            }
            let pmt_pid = (((payload[off + 4] & 0x1F) as u16) << 8) | payload[off + 5] as u16;
            return Some(pmt_pid);
        }
        let pmt_pid = (((payload[off + 2] & 0x1F) as u16) << 8) | payload[off + 3] as u16;
        return Some(pmt_pid);
    }
    None
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
        if pkt[0] != 0x47 {
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

/// Scans `window` for a TS packet carrying a PCR for `pcr_pid` and returns the PCR in seconds.
fn find_pcr_in_window(window: &[u8], pcr_pid: u16) -> Option<f64> {
    let n_pkts = window.len() / PKT_SIZE as usize;
    for i in 0..n_pkts {
        let pkt = &window[i * PKT_SIZE as usize..(i + 1) * PKT_SIZE as usize];
        if pkt[0] != 0x47 {
            continue;
        }
        let pid = (((pkt[1] & 0x1F) as u16) << 8) | pkt[2] as u16;
        if pid != pcr_pid {
            continue;
        }
        // PCR is in the adaptation field
        let adaptation_field_control = (pkt[3] >> 4) & 0x03;
        if adaptation_field_control != 0x02 && adaptation_field_control != 0x03 {
            continue;
        }
        if pkt.len() < 6 {
            continue;
        }
        let af_len = pkt[4] as usize;
        if af_len < 7 {
            continue;
        }
        let af = &pkt[5..5 + af_len];
        // af[0]: flags; bit 4 = PCR_flag
        if af[0] & 0x10 == 0 {
            continue;
        }
        // PCR base (33 bits) + reserved (6 bits) + PCR extension (9 bits)
        // Bytes 1-6 of adaptation field carry the PCR
        if af.len() < 7 {
            continue;
        }
        let base = ((af[1] as u64) << 25)
            | ((af[2] as u64) << 17)
            | ((af[3] as u64) << 9)
            | ((af[4] as u64) << 1)
            | ((af[5] >> 7) as u64);
        let ext = (((af[5] & 0x01) as u64) << 8) | af[6] as u64;
        let pcr_value = base * 300 + ext; // in 27 MHz ticks
        return Some(pcr_value as f64 / 27_000_000.0);
    }
    None
}

/// Returns `byte_pos` aligned to the first sync byte (0x47) found in `window`.
fn align_to_sync(window: &[u8], byte_pos: u64) -> u64 {
    // Find offset of sync byte within window
    for (i, &b) in window.iter().enumerate() {
        if b == 0x47 {
            return byte_pos + i as u64;
        }
    }
    byte_pos
}
