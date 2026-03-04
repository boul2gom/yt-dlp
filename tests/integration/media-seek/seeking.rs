use crate::common;

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
