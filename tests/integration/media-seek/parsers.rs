use media_seek::Error;

use crate::common;

// ============================== WAV parser ==============================

#[tokio::test]
async fn parse_wav_fixture_succeeds() {
    let data = common::fixtures::load_media_bytes("small.wav");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("WAV should parse successfully");

    let full = idx.find_byte_range(0.0, 1.0);
    assert!(full.is_some(), "full range should be found");

    let half = idx.find_byte_range(0.0, 0.5);
    assert!(half.is_some(), "half range should be found");

    let full_r = full.unwrap();
    let half_r = half.unwrap();
    assert!(half_r.end <= full_r.end);
}

// ============================== AIFF parser ==============================

#[tokio::test]
async fn parse_aiff_fixture_succeeds() {
    let data = common::fixtures::load_media_bytes("small.aiff");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("AIFF should parse successfully");

    let range = idx.find_byte_range(0.0, 0.01);
    assert!(range.is_some());
    let r = range.unwrap();
    assert!(r.end >= r.start);
}

// ============================== FLAC parser ==============================

#[tokio::test]
async fn parse_flac_fixture_succeeds() {
    let data = common::fixtures::load_media_bytes("small.flac");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("FLAC should parse successfully");

    let range = idx.find_byte_range(0.0, 0.1);
    assert!(range.is_some());
}

// ============================== MP3 parser ==============================

#[tokio::test]
async fn parse_mp3_fixture_succeeds() {
    let data = common::fixtures::load_media_bytes("small.mp3");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("MP3 should parse successfully");

    let range = idx.find_byte_range(0.0, 0.5);
    assert!(range.is_some());
}

// ============================== OGG (Opus) parser ==============================

#[tokio::test]
async fn parse_ogg_opus_fixture_succeeds() {
    let data = common::fixtures::load_media_bytes("small.ogg");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    // small.ogg is Opus-encoded — the parser handles Opus natively.
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("OGG Opus should parse successfully");

    let range = idx.find_byte_range(0.0, 1.0);
    assert!(range.is_some(), "should find byte range in OGG Opus");
}

// ============================== OGG (Vorbis) parser ==============================

#[tokio::test]
async fn parse_ogg_vorbis_fixture_succeeds() {
    let data = common::fixtures::load_media_bytes("small_vorbis.ogg");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("OGG Vorbis should parse successfully");

    let range = idx.find_byte_range(0.0, 2.0);
    assert!(range.is_some(), "should find byte range in OGG Vorbis");
    let r = range.unwrap();
    assert!(r.end >= r.start);
}

// ============================== WebM parser ==============================

#[tokio::test]
async fn parse_webm_fixture_succeeds() {
    let data = common::fixtures::load_media_bytes("small.webm");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("WebM should parse successfully");

    let range = idx.find_byte_range(0.0, 1.0);
    assert!(range.is_some(), "should find byte range in WebM");
}

// ============================== Classic MP4 (moov) parser ==============================

#[tokio::test]
async fn parse_classic_mp4_fixture_succeeds() {
    let data = common::fixtures::load_media_bytes("small.mp4");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("classic MP4 should parse successfully");

    // Must have at least one segment.
    let range = idx.find_byte_range(0.0, 2.0);
    assert!(range.is_some(), "should find byte range in classic MP4");
    let r = range.unwrap();
    assert!(r.start < r.end, "byte range must be non-empty");
    // init_end_byte should be before the first chunk.
    assert!(idx.init_end_byte > 0, "init_end_byte should be set");
}

#[tokio::test]
async fn parse_m4a_fixture_succeeds() {
    let data = common::fixtures::load_media_bytes("small.m4a");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("M4A should parse successfully");

    let range = idx.find_byte_range(0.0, 1.0);
    assert!(range.is_some(), "should find byte range in M4A");
}

// ============================== fMP4 / SIDX parser ==============================

#[tokio::test]
async fn parse_fmp4_with_sidx_succeeds() {
    let data = common::fixtures::load_media_bytes("small_fmp4.mp4");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("fMP4 with SIDX should parse successfully");

    let range = idx.find_byte_range(0.0, 2.0);
    assert!(range.is_some(), "should find byte range in fMP4");
    let r = range.unwrap();
    assert!(r.start < r.end, "fMP4 byte range must be non-empty");
}

// ============================== FLV parser ==============================

#[tokio::test]
async fn parse_flv_fixture_is_detected() {
    let data = common::fixtures::load_media_bytes("small.flv");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    // small.flv may lack onMetaData keyframes — allow ParseFailed but not UnsupportedFormat.
    if let Err(ref e) = result {
        assert!(
            !matches!(e, media_seek::Error::UnsupportedFormat),
            "FLV should be detected: {e}"
        );
    }
}

// ============================== ADTS parser ==============================

#[tokio::test]
async fn parse_adts_fixture_succeeds() {
    let data = common::fixtures::load_media_bytes("small.adts");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("ADTS should parse successfully");

    let range = idx.find_byte_range(0.0, 1.0);
    assert!(range.is_some(), "should find byte range in ADTS");
    let r = range.unwrap();
    assert!(r.end >= r.start);
}

// ============================== MPEG-TS parser ==============================

#[tokio::test]
async fn parse_ts_fixture_is_detected() {
    let data = common::fixtures::load_media_bytes("small.ts");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    // small.ts may not contain a PCR PID detectable by the binary search — allow ParseFailed.
    if let Err(ref e) = result {
        assert!(
            !matches!(e, media_seek::Error::UnsupportedFormat),
            "TS should be detected: {e}"
        );
    }
}

// ============================== OGG: codec error messages ==============================

/// An OGG stream with a Theora identification header must produce ParseFailed
/// with a message that names the codec, not a generic "unsupported format" error.
#[tokio::test]
async fn parse_ogg_theora_codec_returns_named_parse_failed() {
    // Construct a minimal OGG page with a Theora identification packet.
    // OGG page header (28 bytes minimum) + segment table (1 entry) + payload.
    let mut data = Vec::new();
    // OGG capture + stream_structure_version
    data.extend_from_slice(b"OggS");
    data.push(0x00); // stream_structure_version
    data.push(0x02); // header_type (BOS)
    // granule_position (8 bytes, little-endian 0)
    data.extend_from_slice(&[0u8; 8]);
    // serial_number (4 bytes)
    data.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    // page_sequence_number (4 bytes)
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    // checksum (4 bytes, zero — not validated in parse)
    data.extend_from_slice(&[0x00; 4]);
    // page_segments count = 1
    data.push(0x01);
    // segment_table: 1 segment of 42 bytes (Theora identification header is 42 bytes)
    data.push(42);

    // Theora identification packet: magic \x80theora + version + dimensions
    data.extend_from_slice(b"\x80theora");
    data.extend_from_slice(&[0x03, 0x02, 0x01]); // version 3.2.1
    data.extend_from_slice(&[0x00; 33]); // remaining fields (ignored)

    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;

    assert!(result.is_err(), "Theora OGG should fail to parse");
    let e = result.unwrap_err();
    assert!(
        matches!(e, Error::ParseFailed { .. }),
        "Theora OGG should return ParseFailed, got: {e}"
    );
    // The error message should mention Theora, not just be a generic "sample rate" message.
    assert!(
        e.to_string().to_lowercase().contains("theora"),
        "ParseFailed message should mention Theora: {e}"
    );
}
