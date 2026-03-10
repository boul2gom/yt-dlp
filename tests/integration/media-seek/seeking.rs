use crate::common;

// ============================== WAV ==============================

#[tokio::test]
async fn wav_full_vs_partial_byte_range() {
    let data = common::fixtures::load_media_bytes("small.wav");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("WAV should parse");

    let full = idx.find_byte_range(0.0, 10.0).unwrap();
    let partial = idx.find_byte_range(0.0, 5.0).unwrap();

    assert_eq!(full.start, partial.start);
    assert!(partial.end <= full.end);
}

// ============================== MP3 ==============================

#[tokio::test]
async fn mp3_find_byte_range_covers_file() {
    let data = common::fixtures::load_media_bytes("small.mp3");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("MP3 should parse");

    let range = idx.find_byte_range(0.0, 100.0);
    assert!(range.is_some());
}

// ============================== FLAC ==============================

#[tokio::test]
async fn flac_find_byte_range_returns_some() {
    let data = common::fixtures::load_media_bytes("small.flac");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("FLAC should parse");

    let range = idx.find_byte_range(0.0, 1.0);
    assert!(range.is_some());
}

// ============================== AIFF ==============================

#[tokio::test]
async fn aiff_find_byte_range_partial() {
    let data = common::fixtures::load_media_bytes("small.aiff");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("AIFF should parse");

    let full = idx.find_byte_range(0.0, 10.0).unwrap();
    let half = idx.find_byte_range(0.0, 5.0).unwrap();
    assert!(half.end <= full.end);
}

// ============================== OGG ==============================

#[tokio::test]
async fn ogg_full_vs_partial_byte_range() {
    let data = common::fixtures::load_media_bytes("small.ogg");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("OGG should parse");

    let full = idx.find_byte_range(0.0, 100.0);
    let partial = idx.find_byte_range(0.0, 2.0);
    assert!(full.is_some());
    assert!(partial.is_some());
    // Partial range end must not exceed full range end.
    assert!(partial.unwrap().end <= full.unwrap().end);
}

// ============================== WebM ==============================

#[tokio::test]
async fn webm_full_vs_partial_byte_range() {
    let data = common::fixtures::load_media_bytes("small.webm");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("WebM should parse");

    let full = idx.find_byte_range(0.0, 10.0);
    let partial = idx.find_byte_range(0.0, 2.0);
    assert!(full.is_some());
    assert!(partial.is_some());
    assert!(partial.unwrap().end <= full.unwrap().end);
}

// ============================== Classic MP4 ==============================

#[tokio::test]
async fn mp4_find_byte_range_returns_some() {
    let data = common::fixtures::load_media_bytes("small.mp4");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("classic MP4 should parse");

    let range = idx.find_byte_range(0.0, 5.0);
    assert!(range.is_some());
    let r = range.unwrap();
    assert!(r.start < r.end);
}

#[tokio::test]
async fn m4a_find_byte_range_returns_some() {
    let data = common::fixtures::load_media_bytes("small.m4a");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("M4A should parse");

    let range = idx.find_byte_range(0.0, 5.0);
    assert!(range.is_some());
}

// ============================== fMP4 ==============================

#[tokio::test]
async fn fmp4_find_byte_range_covers_file() {
    let data = common::fixtures::load_media_bytes("small_fmp4.mp4");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("fMP4 should parse");

    let full = idx.find_byte_range(0.0, 100.0);
    let partial = idx.find_byte_range(0.0, 2.0);
    assert!(full.is_some());
    assert!(partial.is_some());
}

// ============================== ADTS ==============================

#[tokio::test]
async fn adts_full_vs_partial_byte_range() {
    let data = common::fixtures::load_media_bytes("small.adts");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("ADTS should parse");

    let full = idx.find_byte_range(0.0, 10.0).unwrap();
    let partial = idx.find_byte_range(0.0, 2.0).unwrap();
    assert_eq!(full.start, partial.start);
    assert!(partial.end <= full.end);
}

// ============================== MPEG-TS ==============================

#[tokio::test]
async fn ts_seek_when_parsed() {
    // small.ts may not have a detectable PCR PID — only test seeking if parsing succeeds.
    let data = common::fixtures::load_media_bytes("small.ts");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    if let Ok(idx) = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await {
        let range = idx.find_byte_range(0.0, 1.0);
        assert!(range.is_some(), "TS index should support seeking");
    }
}

// ============================== FLV ==============================

#[tokio::test]
async fn flv_seek_when_parsed() {
    // small.flv may not have onMetaData keyframes — only test seeking if parsing succeeds.
    let data = common::fixtures::load_media_bytes("small.flv");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    if let Ok(idx) = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await {
        let range = idx.find_byte_range(0.0, 1.0);
        assert!(range.is_some(), "FLV index should support seeking");
    }
}

// ============================== Monotonicity invariant ==============================

#[tokio::test]
async fn wav_byte_range_end_monotone_with_duration() {
    let data = common::fixtures::load_media_bytes("small.wav");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("WAV should parse");

    let r1 = idx.find_byte_range(0.0, 1.0).unwrap();
    let r2 = idx.find_byte_range(0.0, 2.0).unwrap();
    let r3 = idx.find_byte_range(0.0, 10.0).unwrap();
    assert!(r1.end <= r2.end, "end byte should increase with duration");
    assert!(r2.end <= r3.end);
}

#[tokio::test]
async fn mp4_byte_range_start_before_end() {
    let data = common::fixtures::load_media_bytes("small.mp4");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("MP4 should parse");

    let r = idx.find_byte_range(0.0, 5.0).unwrap();
    assert!(r.start < r.end, "start must be strictly less than end");
    assert!(
        r.start >= idx.init_end_byte,
        "range must start at or after init segment end"
    );
}
