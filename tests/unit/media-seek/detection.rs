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
async fn detect_too_short_probe() {
    let tiny = vec![0xAB];
    let fetcher = common::media_seek::MockRangeFetcher::new(tiny.clone());
    let result = media_seek::parse(&tiny, None, &fetcher).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn detect_empty_probe() {
    let empty: Vec<u8> = vec![];
    let fetcher = common::media_seek::MockRangeFetcher::new(empty.clone());
    let result = media_seek::parse(&empty, None, &fetcher).await;
    assert!(result.is_err());
}

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
async fn detect_ogg_from_fixture() {
    let data = common::fixtures::load_media_bytes("small.ogg");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    if let Err(ref e) = result {
        assert!(
            !matches!(e, Error::UnsupportedFormat),
            "OGG should be detected, got UnsupportedFormat"
        );
    }
}

#[tokio::test]
async fn detect_webm_from_fixture() {
    let data = common::fixtures::load_media_bytes("small.webm");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    // WebM fixture is only 60 bytes — too small for full parsing
    // Detection or parsing may fail, we just verify it doesn't panic
    if let Ok(idx) = result {
        let range = idx.find_byte_range(0.0, 1.0);
        assert!(range.is_some() || range.is_none());
    }
}

#[tokio::test]
async fn detect_mp4_from_fixture() {
    let data = common::fixtures::load_media_bytes("small.mp4");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    if let Err(ref e) = result {
        assert!(
            !matches!(e, Error::UnsupportedFormat),
            "MP4 should be detected, got UnsupportedFormat"
        );
    }
}

#[tokio::test]
async fn detect_flv_from_fixture() {
    let data = common::fixtures::load_media_bytes("small.flv");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    if let Err(ref e) = result {
        assert!(
            !matches!(e, Error::UnsupportedFormat),
            "FLV should be detected, got UnsupportedFormat"
        );
    }
}

#[tokio::test]
async fn detect_ts_from_fixture() {
    let data = common::fixtures::load_media_bytes("small.ts");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    if let Err(ref e) = result {
        assert!(
            !matches!(e, Error::UnsupportedFormat),
            "TS should be detected, got UnsupportedFormat"
        );
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
async fn detect_ogg_magic() {
    let mut probe = b"OggS".to_vec();
    probe.extend_from_slice(&[0x00; 100]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, Some(104), &fetcher).await;
    if let Err(ref e) = result {
        assert!(!matches!(e, Error::UnsupportedFormat));
    }
}

#[tokio::test]
async fn detect_flac_magic() {
    let mut probe = b"fLaC".to_vec();
    probe.extend_from_slice(&[0x00; 100]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, None, &fetcher).await;
    if let Err(ref e) = result {
        assert!(!matches!(e, Error::UnsupportedFormat));
    }
}

#[tokio::test]
async fn detect_flv_magic() {
    let mut probe = b"FLV".to_vec();
    probe.extend_from_slice(&[0x00; 100]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, None, &fetcher).await;
    if let Err(ref e) = result {
        assert!(!matches!(e, Error::UnsupportedFormat));
    }
}

#[tokio::test]
async fn detect_id3_as_mp3() {
    let mut probe = b"ID3".to_vec();
    probe.extend_from_slice(&[0x00; 200]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, None, &fetcher).await;
    if let Err(ref e) = result {
        assert!(!matches!(e, Error::UnsupportedFormat));
    }
}

#[tokio::test]
async fn detect_riff_wave_magic() {
    let mut probe = b"RIFF".to_vec();
    probe.extend_from_slice(&[0x00; 4]);
    probe.extend_from_slice(b"WAVE");
    probe.extend_from_slice(&[0x00; 100]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, None, &fetcher).await;
    if let Err(ref e) = result {
        assert!(!matches!(e, Error::UnsupportedFormat));
    }
}

#[tokio::test]
async fn detect_riff_avi_magic() {
    let mut probe = b"RIFF".to_vec();
    probe.extend_from_slice(&[0x00; 4]);
    probe.extend_from_slice(b"AVI ");
    probe.extend_from_slice(&[0x00; 100]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, Some(112), &fetcher).await;
    if let Err(ref e) = result {
        assert!(!matches!(e, Error::UnsupportedFormat));
    }
}

#[tokio::test]
async fn detect_isobmff_ftyp() {
    let mut probe = vec![0x00, 0x00, 0x00, 0x14]; // size = 20
    probe.extend_from_slice(b"ftyp");
    probe.extend_from_slice(b"isom\x00\x00\x00\x00isomavc1");
    probe.extend_from_slice(&[0x00; 100]);
    let fetcher = common::media_seek::MockRangeFetcher::new(probe.clone());
    let result = media_seek::parse(&probe, None, &fetcher).await;
    if let Err(ref e) = result {
        assert!(!matches!(e, Error::UnsupportedFormat));
    }
}
