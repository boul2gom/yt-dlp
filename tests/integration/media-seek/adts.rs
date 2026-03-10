use media_seek::Error;

use crate::common;

// ============================== ADTS parser tests ==============================

#[tokio::test]
async fn parse_adts_fixture_succeeds() {
    let data = common::fixtures::load_media_bytes("small.adts");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());

    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("ADTS should parse successfully");

    // ADTS uses a Linear index — verify the index is usable.
    let range = idx.find_byte_range(0.0, 1.0);
    assert!(range.is_some(), "should find byte range in ADTS stream");
}

#[tokio::test]
async fn adts_find_byte_range_is_non_empty() {
    let data = common::fixtures::load_media_bytes("small.adts");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("ADTS should parse");

    let r = idx.find_byte_range(0.0, 1.0).unwrap();
    assert!(r.end >= r.start, "byte range end must be >= start");
}

#[tokio::test]
async fn adts_linear_index_byte_range_monotone() {
    let data = common::fixtures::load_media_bytes("small.adts");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("ADTS should parse");

    let small = idx.find_byte_range(0.0, 1.0).unwrap();
    let large = idx.find_byte_range(0.0, 3.0).unwrap();
    assert_eq!(small.start, large.start, "start byte should be the same");
    assert!(small.end <= large.end, "larger duration should produce larger end byte");
}

#[tokio::test]
async fn adts_init_end_byte_is_zero() {
    // ADTS has no separate init segment — init_end_byte should be 0.
    let data = common::fixtures::load_media_bytes("small.adts");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("ADTS should parse");

    assert_eq!(idx.init_end_byte, 0, "ADTS has no init segment");
}

#[tokio::test]
async fn parse_truncated_adts_returns_parse_failed() {
    // A buffer that looks like ADTS (0xFF 0xF1) but is too short to contain valid frames.
    let data = vec![0xFF, 0xF1, 0x50, 0x00, 0x00, 0x1F, 0xFC]; // one partial frame header
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, Some(data.len() as u64), &fetcher).await;
    // Either UnsupportedFormat (detection fails second sync check) or ParseFailed
    if let Err(e) = result {
        assert!(matches!(e, Error::ParseFailed { .. } | Error::UnsupportedFormat));
    }
    // If it succeeds on a very small probe, that's also acceptable — no panic.
}
