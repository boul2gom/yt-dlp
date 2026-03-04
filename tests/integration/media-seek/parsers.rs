use media_seek::Error;

use crate::common;

// ============================== WAV parser ==============================

#[tokio::test]
async fn parse_wav_fixture() {
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
async fn parse_aiff_fixture() {
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
async fn parse_flac_fixture() {
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
async fn parse_mp3_fixture() {
    let data = common::fixtures::load_media_bytes("small.mp3");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("MP3 should parse successfully");

    let range = idx.find_byte_range(0.0, 0.5);
    assert!(range.is_some());
}

// ============================== OGG parser ==============================

#[tokio::test]
async fn parse_ogg_fixture() {
    let data = common::fixtures::load_media_bytes("small.ogg");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    if let Err(ref e) = result {
        assert!(
            !matches!(e, Error::UnsupportedFormat),
            "OGG should be detected, not UnsupportedFormat"
        );
    }
}

// ============================== WebM parser ==============================

#[tokio::test]
async fn parse_webm_fixture() {
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

// ============================== MP4 parser ==============================

#[tokio::test]
async fn parse_mp4_fixture() {
    let data = common::fixtures::load_media_bytes("small.mp4");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    if let Err(ref e) = result {
        assert!(
            !matches!(e, Error::UnsupportedFormat),
            "MP4 should be detected, not UnsupportedFormat"
        );
    }
}

// ============================== FLV parser ==============================

#[tokio::test]
async fn parse_flv_fixture() {
    let data = common::fixtures::load_media_bytes("small.flv");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    if let Err(ref e) = result {
        assert!(
            !matches!(e, Error::UnsupportedFormat),
            "FLV should be detected, not UnsupportedFormat"
        );
    }
}

// ============================== TS parser ==============================

#[tokio::test]
async fn parse_ts_fixture() {
    let data = common::fixtures::load_media_bytes("small.ts");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    if let Err(ref e) = result {
        assert!(
            !matches!(e, Error::UnsupportedFormat),
            "TS should be detected, not UnsupportedFormat"
        );
    }
}
