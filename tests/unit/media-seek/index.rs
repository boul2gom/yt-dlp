use crate::common;

// ============================== ContainerIndex find_byte_range ==============================

#[tokio::test]
async fn wav_linear_index_find_range() {
    let data = common::fixtures::load_media_bytes("small.wav");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("WAV parse should succeed");

    let range = idx.find_byte_range(0.0, 0.01);
    assert!(range.is_some(), "WAV find_byte_range should return Some");
    let range = range.unwrap();
    assert!(range.end >= range.start, "end must be >= start");
}

#[tokio::test]
async fn wav_linear_index_zero_duration() {
    let data = common::fixtures::load_media_bytes("small.wav");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("WAV parse should succeed");

    let range = idx.find_byte_range(0.0, 0.0);
    assert!(range.is_some());
    let r = range.unwrap();
    assert!(r.end >= r.start);
}

#[tokio::test]
async fn aiff_linear_index_find_range() {
    let data = common::fixtures::load_media_bytes("small.aiff");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("AIFF parse should succeed");

    let range = idx.find_byte_range(0.0, 0.005);
    assert!(range.is_some());
    let r = range.unwrap();
    assert!(r.end >= r.start);
}

#[tokio::test]
async fn mp3_index_find_range() {
    let data = common::fixtures::load_media_bytes("small.mp3");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("MP3 parse should succeed");

    let range = idx.find_byte_range(0.0, 0.1);
    assert!(range.is_some());
    let r = range.unwrap();
    assert!(r.end >= r.start);
}

#[tokio::test]
async fn container_index_init_end_byte() {
    let data = common::fixtures::load_media_bytes("small.wav");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let idx = media_seek::parse(&data, Some(data.len() as u64), &fetcher)
        .await
        .expect("WAV parse should succeed");

    assert!(idx.init_end_byte < data.len() as u64);
}

// ============================== SegmentEntry ==============================

#[test]
fn segment_entry_debug() {
    let entry = media_seek::index::SegmentEntry {
        start_secs: 0.0,
        end_secs: 5.0,
        byte_offset: 1000,
        byte_size: 5000,
    };
    let debug = format!("{:?}", entry);
    assert!(debug.contains("SegmentEntry"));
    assert!(debug.contains("1000"));
}

#[test]
fn segment_entry_clone() {
    let entry = media_seek::index::SegmentEntry {
        start_secs: 1.0,
        end_secs: 2.0,
        byte_offset: 500,
        byte_size: 200,
    };
    let cloned = entry.clone();
    assert!((entry.start_secs - cloned.start_secs).abs() < f64::EPSILON);
    assert_eq!(entry.byte_offset, cloned.byte_offset);
    assert_eq!(entry.byte_size, cloned.byte_size);
}
