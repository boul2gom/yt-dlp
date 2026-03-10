use crate::common;

// ============================== MP3 parser ==============================

#[tokio::test]
async fn parse_mp3_fixture_succeeds() {
    let data = common::fixtures::load_media_bytes("small.mp3");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("MP3 should parse successfully");

    let range = idx.find_byte_range(0.0, 1.0);
    assert!(range.is_some(), "should find byte range in MP3");
}

// ============================== ID3v2 tag handling ==============================

/// Verifies that a MP3 prefixed with two consecutive (stacked) ID3v2 headers
/// is parsed correctly: the second ID3v2 tag must not be mistaken for audio.
#[tokio::test]
async fn parse_mp3_stacked_id3v2_tags_succeeds() {
    let mp3_data = common::fixtures::load_media_bytes("small.mp3");

    // Build a minimal ID3v2 tag (version 2.3, no flags, zero-size body).
    // Header: "ID3" + version(2) + flags(1) + syncsafe_size(4)
    let empty_id3v2 = {
        let mut tag = b"ID3\x03\x00\x00".to_vec(); // "ID3", v2.3, no footer flag
        tag.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // syncsafe size = 0
        tag
    };

    // Prepend two stacked ID3v2 tags in front of the real MP3 data.
    let mut data = empty_id3v2.clone();
    data.extend_from_slice(&empty_id3v2);
    data.extend_from_slice(&mp3_data);

    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    assert!(result.is_ok(), "stacked ID3v2 MP3 should parse: {:?}", result.err());

    let idx = result.unwrap();
    let range = idx.find_byte_range(0.0, 1.0);
    assert!(range.is_some(), "should find byte range after stacked ID3v2 tags");
}

/// A single ID3v2 tag followed by MP3 audio — the baseline case.
#[tokio::test]
async fn parse_mp3_single_id3v2_tag_succeeds() {
    let mp3_data = common::fixtures::load_media_bytes("small.mp3");

    let mut id3v2 = b"ID3\x03\x00\x00".to_vec();
    id3v2.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // syncsafe size = 0

    let mut data = id3v2;
    data.extend_from_slice(&mp3_data);

    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    assert!(result.is_ok(), "single ID3v2 + MP3 should parse: {:?}", result.err());
}

// ============================== VBRI table fallback ==============================

/// Builds a minimal MPEG-1 Layer III frame with an embedded VBRI header whose
/// `entry_bytes` field is 0 (non-conformant). Verifies that the parser falls back
/// to a CBR Linear index rather than returning ParseFailed.
#[tokio::test]
async fn parse_mp3_vbri_invalid_entry_bytes_falls_back_to_cbr() {
    // Minimal MPEG-1, 128 kbps, 44100 Hz, stereo frame header: FF FB 90 00
    // FF = sync, FB = MPEG1 Layer3 no-CRC, 90 = 128kbps 44100Hz, 00 = stereo joint
    let mut frame: Vec<u8> = vec![0xFF, 0xFB, 0x90, 0x00];

    // Pad with zeros for side information (32 bytes for MPEG-1 stereo)
    frame.extend_from_slice(&[0x00u8; 32]);

    // VBRI header starts at offset 36 (4 frame header + 32 side info)
    // Layout: "VBRI"(4) + version(2) + delay(2) + quality(2) + bytes_total(4)
    //       + frames_total(4) + table_size(2) + table_scale(2) + entry_bytes(2) + frames_per_entry(2)
    frame.extend_from_slice(b"VBRI");
    frame.extend_from_slice(&[0x00, 0x01]); // version = 1
    frame.extend_from_slice(&[0x00, 0x00]); // delay
    frame.extend_from_slice(&[0x00, 0x00]); // quality
    frame.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]); // bytes_total = 65536
    frame.extend_from_slice(&[0x00, 0x00, 0x01, 0x00]); // frames_total = 256
    frame.extend_from_slice(&[0x00, 0x05]); // table_size = 5 entries
    frame.extend_from_slice(&[0x00, 0x01]); // table_scale = 1
    frame.extend_from_slice(&[0x00, 0x00]); // entry_bytes = 0 ← INVALID
    frame.extend_from_slice(&[0x00, 0x01]); // frames_per_entry = 1

    // Pad to make the frame look like a complete 417-byte MPEG frame (128kbps/44100)
    frame.resize(417, 0x00);

    let fetcher = common::media_seek::MockRangeFetcher::new(frame.clone());
    let result = media_seek::parse(&frame, Some(frame.len() as u64), &fetcher).await;

    // Should succeed with a fallback Linear index, not ParseFailed.
    assert!(
        result.is_ok(),
        "VBRI with entry_bytes=0 should fall back to CBR, got: {:?}",
        result.err()
    );
    let idx = result.unwrap();
    // Linear index means find_byte_range always returns Some.
    let range = idx.find_byte_range(0.0, 1.0);
    assert!(range.is_some(), "CBR fallback index should be seekable");
}

// ============================== total_size threading ==============================

/// Verifies that the Xing fallback `total_bytes` uses the actual file size
/// (from `total_size`) rather than `probe.len()`. When the Xing header does not
/// include the `XING_FLAG_BYTES` flag, the parser must fall back to `total_size`.
/// This test constructs a Xing frame without BYTES flag, passes a large `total_size`,
/// and checks that the resulting `byte_rate` reflects the large file size.
#[tokio::test]
async fn parse_mp3_xing_fallback_uses_actual_total_size() {
    // Build a minimal MPEG-1, 128 kbps, 44100 Hz, stereo frame: FF FB 90 64
    // 0x90 = 128kbps, 44100Hz (sr_idx=0), 0x64 = stereo+padding
    let mut frame: Vec<u8> = vec![0xFF, 0xFB, 0x90, 0x64];

    // Pad side information (32 bytes for MPEG-1 stereo)
    frame.extend_from_slice(&[0x00u8; 32]);

    // Xing header at offset 36 (4 + 32 side info):
    // "Xing" + flags(4) + [no BYTES field] + [no FRAMES field] + TOC(100 bytes linear)
    // flags = 0x04 (only TOC present, no frames, no bytes)
    frame.extend_from_slice(b"Xing");
    frame.extend_from_slice(&[0x00, 0x00, 0x00, 0x05]); // flags: FRAMES + TOC
    // FRAMES field (required by flag 0x01):
    frame.extend_from_slice(&[0x00, 0x00, 0x01, 0x00]); // total_frames = 256
    // TOC: 100 bytes, linearly increasing 0..=99 (scaled by 256/100)
    let toc: Vec<u8> = (0u8..100).map(|i| (i as u16 * 256 / 100) as u8).collect();
    frame.extend_from_slice(&toc);

    // Pad to a plausible frame size
    frame.resize(1000, 0x00);

    // Provide a large total_size (10 MB) — much larger than frame/probe size.
    let large_total_size: u64 = 10 * 1024 * 1024;
    let fetcher = common::media_seek::MockRangeFetcher::new(frame.clone());
    let result = media_seek::parse(&frame, Some(large_total_size), &fetcher).await;

    assert!(result.is_ok(), "Xing MP3 should parse: {:?}", result.err());
    let idx = result.unwrap();

    // The TOC-based segment byte offsets should be proportional to 10 MB,
    // not to the probe size (1000 bytes). The 50%-mark byte should be ~5 MB.
    let mid_range = idx.find_byte_range(0.0, 100.0);
    assert!(mid_range.is_some());
    // At least verify the index is seekable and non-trivial.
    let r = mid_range.unwrap();
    assert!(
        r.end > 1000,
        "byte range end should reflect large file size, not probe size: end={}",
        r.end
    );
}

/// Verifies that an ID3v1 trailing tag is subtracted from the total_size
/// when the probe covers the entire file (total_size == probe.len()).
/// We build a fake MP3: real CBR audio + ID3v1 trailer.
/// Without ID3v1 stripping the parser would compute a slightly inflated byte_rate.
/// With stripping the init_end_byte should remain before the ID3v1 region.
#[tokio::test]
async fn parse_mp3_id3v1_trailer_stripped_when_probe_covers_file() {
    let mut mp3_data = common::fixtures::load_media_bytes("small.mp3");

    // Append a fake ID3v1 tag: "TAG" + 125 zero bytes
    mp3_data.extend_from_slice(b"TAG");
    mp3_data.extend_from_slice(&[0u8; 125]); // total: 128 bytes

    let total_size = mp3_data.len() as u64;
    let fetcher = common::media_seek::MockRangeFetcher::new(mp3_data.clone());
    let result = media_seek::parse(&mp3_data, Some(total_size), &fetcher).await;

    assert!(result.is_ok(), "MP3 + ID3v1 should parse: {:?}", result.err());
    let idx = result.unwrap();

    // The ID3v1 tag is at [total_size - 128, total_size).
    // A correct implementation does not include these bytes in the audio range.
    // Specifically: find_byte_range(0, very_large) must not overshoot the audio end.
    let range = idx.find_byte_range(0.0, 9999.0);
    assert!(range.is_some());
    let r = range.unwrap();
    // The end byte should be ≤ total_size - 128 (the ID3v1 boundary)
    assert!(
        r.end <= total_size.saturating_sub(128),
        "byte range should not include ID3v1 trailer: end={}, audio_end={}",
        r.end,
        total_size.saturating_sub(128)
    );
}

// ============================== Free-format (bi==0) ==============================

/// Builds a synthetic free-format MPEG-1 Layer III stream: all frames have bi==0
/// and a fixed frame size of 200 bytes, containing valid sync patterns.
/// Verifies that the parser detects the frame size and returns a seekable Linear index.
#[tokio::test]
async fn parse_mp3_free_format_detects_frame_size() {
    const FRAME_SIZE: usize = 200;
    // MPEG-1 Layer III, bi==0, 44100 Hz, stereo: FF E2 00 C0
    // FF E2 = sync + MPEG1 + Layer3; 00 = bi=0, sr_idx=0 (44100); C0 = stereo
    let sync_header = [0xFF, 0xE2, 0x00, 0xC0];

    // Build 4 frames: each is FRAME_SIZE bytes, starting with sync_header + zeros
    let mut data = Vec::with_capacity(FRAME_SIZE * 4);
    for _ in 0..4 {
        data.extend_from_slice(&sync_header);
        data.resize(data.len() + (FRAME_SIZE - 4), 0x00);
    }

    let total_size = data.len() as u64;
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(total_size), &fetcher).await;

    assert!(
        result.is_ok(),
        "free-format MP3 with detectable frame size should parse: {:?}",
        result.err()
    );
    let idx = result.unwrap();
    let range = idx.find_byte_range(0.0, 1.0);
    assert!(range.is_some(), "free-format index should be seekable");
}

/// A probe containing only a single free-format frame (no second sync word) must
/// return ParseFailed with a message that mentions "free-format".
#[tokio::test]
async fn parse_mp3_free_format_too_short_returns_parse_failed() {
    // Single free-format frame: sync + 20 bytes of zeros (too short to find a second sync)
    let mut data: Vec<u8> = vec![0xFF, 0xE2, 0x00, 0xC0];
    data.resize(28, 0x00); // 28 bytes total — far too short to detect frame size

    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;

    assert!(result.is_err(), "probe too short for free-format detection should fail");
    let e = result.unwrap_err();
    assert!(
        matches!(e, media_seek::Error::ParseFailed { .. }),
        "should be ParseFailed, got: {e}"
    );
    let msg = e.to_string().to_lowercase();
    assert!(
        msg.contains("free-format") || msg.contains("free_format"),
        "error should mention free-format: {e}"
    );
}

// ============================== ID3v1 via fetch (probe < total_size) ==============================

/// When the probe does not cover the end of the file, the parser fetches the last 128 bytes
/// to detect an ID3v1 tag. This test constructs a MockRangeFetcher where the full file
/// (including an ID3v1 trailer) is available, but passes only the first half as the probe.
/// The ID3v1 must be detected via fetch and subtracted from the total_size.
#[tokio::test]
async fn parse_mp3_id3v1_detected_via_fetch_when_probe_shorter_than_file() {
    let mut full_file = common::fixtures::load_media_bytes("small.mp3");

    // Append a fake ID3v1 tag: "TAG" + 125 zero bytes
    full_file.extend_from_slice(b"TAG");
    full_file.extend_from_slice(&[0u8; 125]);

    let total_size = full_file.len() as u64;

    // Only provide the first half of the file as the probe.
    let probe_len = full_file.len() / 2;
    let probe = &full_file[..probe_len];

    // The fetcher has the complete file, so the ID3v1 tail fetch will succeed.
    let fetcher = common::media_seek::MockRangeFetcher::new(full_file.clone());
    let result = media_seek::parse(probe, Some(total_size), &fetcher).await;

    assert!(
        result.is_ok(),
        "MP3 with ID3v1 (detected via fetch) should parse: {:?}",
        result.err()
    );
    let idx = result.unwrap();

    // With ID3v1 subtracted, the audio ends at total_size - 128.
    // The byte range for a very long duration must not exceed that boundary.
    let range = idx.find_byte_range(0.0, 9999.0);
    assert!(range.is_some());
    let r = range.unwrap();
    assert!(
        r.end <= total_size.saturating_sub(128),
        "byte range should not include ID3v1 trailer: end={}, audio_end={}",
        r.end,
        total_size.saturating_sub(128)
    );
}

// ============================== Layer I and Layer II support ==============================

/// A minimal synthetic MPEG-1 Layer II frame (64 kbps, 44100 Hz, stereo).
/// Layer II frame header byte 2: b1 = 0xFF E4 xx → masked & 0xE6 = 0xE4 (Layer II pattern).
/// 0xFF = sync, E4 = sync(111)+MPEG-1(11)+LayerII(10)+no_crc(1), 0x60 = 64kbps + 44100Hz sr
#[tokio::test]
async fn parse_mpeg_layer2_cbr_succeeds() {
    // MPEG-1 Layer II, 64 kbps, 44100 Hz, stereo: FF E4 60 C0
    // FF E4 = sync + MPEG-1 + Layer II + no CRC
    // 0x60 = bitrate_idx=6 (64kbps for L2) + sr_idx=0 (44100) + no padding
    // 0xC0 = stereo joint
    let mut frame: Vec<u8> = vec![0xFF, 0xE4, 0x60, 0xC0];
    frame.resize(417, 0x00); // ~417 bytes at 64kbps/44100

    let fetcher = common::media_seek::MockRangeFetcher::new(frame.clone());
    let result = media_seek::parse(&frame, Some(frame.len() as u64), &fetcher).await;

    assert!(result.is_ok(), "MPEG Layer II should parse as CBR: {:?}", result.err());
    let idx = result.unwrap();
    let range = idx.find_byte_range(0.0, 1.0);
    assert!(range.is_some(), "Layer II index should be seekable");
}

/// A minimal synthetic MPEG-1 Layer I frame (32 kbps, 44100 Hz, stereo).
/// Layer I frame header: 0xFF E6 xx → masked & 0xE6 = 0xE6 (Layer I pattern).
/// 0xFF = sync, E6 = sync(111)+MPEG-1(11)+LayerI(11)+no_crc(0), 0x20 = 32kbps + 44100Hz
#[tokio::test]
async fn parse_mpeg_layer1_cbr_succeeds() {
    // MPEG-1 Layer I, 32 kbps, 44100 Hz, stereo: FF E6 20 C0
    // FF E6 = sync + MPEG-1 + Layer I + no CRC (protection_absent=0)
    // 0x22 = bitrate_idx=2 (64kbps for L1, actually let's use idx=1=32kbps) + sr=0 (44100)
    // Wait: MPEG-1 L1 bitrate_idx=1 → 32kbps; sr_idx=0 → 44100
    // byte2: bi(4)|sr(2)|padding(1)|private(1) = 0001 00 0 0 = 0x10
    // byte3: channel_mode(2)|... = 11 000000 = 0xC0 (joint stereo)
    let mut frame: Vec<u8> = vec![0xFF, 0xE6, 0x10, 0xC0];
    // Layer I frame is padded to 48 bytes per slot × 12 slots = 52 bytes minimum
    frame.resize(52, 0x00);

    let fetcher = common::media_seek::MockRangeFetcher::new(frame.clone());
    let result = media_seek::parse(&frame, Some(frame.len() as u64), &fetcher).await;

    assert!(result.is_ok(), "MPEG Layer I should parse as CBR: {:?}", result.err());
    let idx = result.unwrap();
    let range = idx.find_byte_range(0.0, 1.0);
    assert!(range.is_some(), "Layer I index should be seekable");
}
