use media_seek::Error;

use crate::common;

// ============================== Format detection via parse() ==============================

#[tokio::test]
async fn detect_unsupported_returns_error() {
    let garbage = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
    let fetcher = common::media_seek::MockRangeFetcher::new(garbage.clone());
    let result = media_seek::parse(&garbage, None, &fetcher).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::UnsupportedFormat));
}

#[tokio::test]
async fn detect_too_short_probe_returns_error() {
    let tiny = vec![0xAB];
    let fetcher = common::media_seek::MockRangeFetcher::new(tiny.clone());
    let result = media_seek::parse(&tiny, None, &fetcher).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn detect_empty_probe_returns_error() {
    let empty: Vec<u8> = vec![];
    let fetcher = common::media_seek::MockRangeFetcher::new(empty.clone());
    let result = media_seek::parse(&empty, None, &fetcher).await;
    assert!(result.is_err());
}

// ============================== Fixture-based format detection ==============================

#[tokio::test]
async fn detect_wav_from_fixture() {
    let data = common::fixtures::load_media_bytes("small.wav");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    assert!(result.is_ok(), "WAV parse failed: {:?}", result.err());
}

#[tokio::test]
async fn detect_aiff_from_fixture() {
    let data = common::fixtures::load_media_bytes("small.aiff");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    assert!(result.is_ok(), "AIFF parse failed: {:?}", result.err());
}

#[tokio::test]
async fn detect_flac_from_fixture() {
    let data = common::fixtures::load_media_bytes("small.flac");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    assert!(result.is_ok(), "FLAC parse failed: {:?}", result.err());
}

#[tokio::test]
async fn detect_mp3_from_fixture() {
    let data = common::fixtures::load_media_bytes("small.mp3");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    assert!(result.is_ok(), "MP3 parse failed: {:?}", result.err());
}

#[tokio::test]
async fn detect_ogg_opus_from_fixture() {
    // small.ogg is Opus-encoded; the OGG parser handles Opus natively.
    let data = common::fixtures::load_media_bytes("small.ogg");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    assert!(result.is_ok(), "OGG Opus parse failed: {:?}", result.err());
}

#[tokio::test]
async fn detect_ogg_vorbis_from_fixture() {
    let data = common::fixtures::load_media_bytes("small_vorbis.ogg");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    assert!(result.is_ok(), "OGG Vorbis parse failed: {:?}", result.err());
}

#[tokio::test]
async fn detect_webm_from_fixture() {
    // small.webm is rebuilt from original_video.webm with proper SeekHead+Cues.
    let data = common::fixtures::load_media_bytes("small.webm");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    assert!(result.is_ok(), "WebM parse failed: {:?}", result.err());
}

#[tokio::test]
async fn detect_classic_mp4_from_fixture() {
    // small.mp4 is a classic (non-fragmented) MP4 with a moov box.
    let data = common::fixtures::load_media_bytes("small.mp4");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    assert!(result.is_ok(), "classic MP4 parse failed: {:?}", result.err());
}

#[tokio::test]
async fn detect_fmp4_from_fixture() {
    let data = common::fixtures::load_media_bytes("small_fmp4.mp4");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    assert!(result.is_ok(), "fMP4 parse failed: {:?}", result.err());
}

#[tokio::test]
async fn detect_flv_from_fixture() {
    let data = common::fixtures::load_media_bytes("small.flv");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    // small.flv may lack onMetaData keyframes — allow ParseFailed but not UnsupportedFormat.
    if let Err(ref e) = result {
        assert!(!matches!(e, Error::UnsupportedFormat), "FLV should be detected: {e}");
    }
}

#[tokio::test]
async fn detect_adts_from_fixture() {
    let data = common::fixtures::load_media_bytes("small.adts");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    assert!(result.is_ok(), "ADTS parse failed: {:?}", result.err());
}

#[tokio::test]
async fn detect_ts_from_fixture() {
    let data = common::fixtures::load_media_bytes("small.ts");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    // small.ts may not contain a PCR PID detectable by the binary search — allow ParseFailed.
    if let Err(ref e) = result {
        assert!(!matches!(e, Error::UnsupportedFormat), "TS should be detected: {e}");
    }
}

// ============================== Magic byte patterns ==============================

#[tokio::test]
async fn detect_ebml_magic_as_webm() {
    let mut probe = vec![0x1A, 0x45, 0xDF, 0xA3];
    probe.extend_from_slice(&[0x00; 100]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, None, &fetcher).await;
    if let Err(ref e) = result {
        assert!(!matches!(e, Error::UnsupportedFormat));
    }
}

#[tokio::test]
async fn detect_ogg_magic_is_not_unsupported() {
    let mut probe = b"OggS".to_vec();
    probe.extend_from_slice(&[0x00; 100]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, Some(104), &fetcher).await;
    if let Err(ref e) = result {
        assert!(
            !matches!(e, Error::UnsupportedFormat),
            "OGG magic should be detected: {e}"
        );
    }
}

#[tokio::test]
async fn detect_flac_magic_is_not_unsupported() {
    let mut probe = b"fLaC".to_vec();
    probe.extend_from_slice(&[0x00; 100]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, None, &fetcher).await;
    if let Err(ref e) = result {
        assert!(
            !matches!(e, Error::UnsupportedFormat),
            "FLAC magic should be detected: {e}"
        );
    }
}

#[tokio::test]
async fn detect_flv_magic_is_not_unsupported() {
    let mut probe = b"FLV".to_vec();
    probe.extend_from_slice(&[0x00; 100]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, None, &fetcher).await;
    if let Err(ref e) = result {
        assert!(
            !matches!(e, Error::UnsupportedFormat),
            "FLV magic should be detected: {e}"
        );
    }
}

#[tokio::test]
async fn detect_id3_as_mp3() {
    let mut probe = b"ID3".to_vec();
    probe.extend_from_slice(&[0x00; 200]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, None, &fetcher).await;
    if let Err(ref e) = result {
        assert!(
            !matches!(e, Error::UnsupportedFormat),
            "ID3 should be detected as MP3: {e}"
        );
    }
}

#[tokio::test]
async fn detect_riff_wave_magic_is_not_unsupported() {
    let mut probe = b"RIFF".to_vec();
    probe.extend_from_slice(&[0x00; 4]);
    probe.extend_from_slice(b"WAVE");
    probe.extend_from_slice(&[0x00; 100]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, None, &fetcher).await;
    if let Err(ref e) = result {
        assert!(
            !matches!(e, Error::UnsupportedFormat),
            "WAVE magic should be detected: {e}"
        );
    }
}

#[tokio::test]
async fn detect_riff_avi_magic_is_not_unsupported() {
    let mut probe = b"RIFF".to_vec();
    probe.extend_from_slice(&[0x00; 4]);
    probe.extend_from_slice(b"AVI ");
    probe.extend_from_slice(&[0x00; 100]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, Some(112), &fetcher).await;
    if let Err(ref e) = result {
        assert!(
            !matches!(e, Error::UnsupportedFormat),
            "AVI magic should be detected: {e}"
        );
    }
}

#[tokio::test]
async fn detect_isobmff_ftyp_is_not_unsupported() {
    let mut probe = vec![0x00, 0x00, 0x00, 0x14]; // size = 20
    probe.extend_from_slice(b"ftyp");
    probe.extend_from_slice(b"isom\x00\x00\x00\x00isomavc1");
    probe.extend_from_slice(&[0x00; 100]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, None, &fetcher).await;
    // ftyp-only is detected as MP4 but IndexNotFound — NOT UnsupportedFormat
    if let Err(ref e) = result {
        assert!(!matches!(e, Error::UnsupportedFormat), "ftyp should be detected: {e}");
        assert!(
            matches!(e, Error::IndexNotFound { .. }),
            "ftyp-only should be IndexNotFound: {e}"
        );
    }
}
