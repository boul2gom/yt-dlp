use media_seek::Error;

use crate::common;

#[tokio::test]
async fn parse_empty_file_fails() {
    let data: Vec<u8> = vec![];
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, None, &fetcher).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::UnsupportedFormat));
}

#[tokio::test]
async fn parse_random_bytes_returns_unsupported() {
    let data = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE];
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, None, &fetcher).await;
    assert!(matches!(result.unwrap_err(), Error::UnsupportedFormat));
}

#[tokio::test]
async fn parse_truncated_wav_returns_parse_failed() {
    let mut data = b"RIFF".to_vec();
    data.extend_from_slice(&100u32.to_le_bytes());
    data.extend_from_slice(b"WAVE");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, None, &fetcher).await;
    assert!(matches!(result.unwrap_err(), Error::ParseFailed { .. }));
}

#[tokio::test]
async fn parse_truncated_flac_returns_parse_failed() {
    let data = b"fLaC".to_vec();
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, None, &fetcher).await;
    assert!(matches!(result.unwrap_err(), Error::ParseFailed { .. }));
}

#[tokio::test]
async fn parse_truncated_mp3_returns_parse_failed() {
    // ID3 header with no actual MP3 data following
    let mut data = b"ID3".to_vec();
    data.extend_from_slice(&[0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10]); // version, flags, size
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, None, &fetcher).await;
    assert!(matches!(result.unwrap_err(), Error::ParseFailed { .. }));
}

// A minimal MP4 with only an ftyp box — no moov, no sidx — returns IndexNotFound,
// since the format IS detected as MP4 but the seek index cannot be located.
#[tokio::test]
async fn parse_mp4_ftyp_only_returns_index_not_found() {
    let mut data = vec![0x00, 0x00, 0x00, 0x10]; // size = 16
    data.extend_from_slice(b"ftyp");
    data.extend_from_slice(b"isom");
    data.extend_from_slice(&[0x00; 4]); // minor_version
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, None, &fetcher).await;
    assert!(matches!(result.unwrap_err(), Error::IndexNotFound { .. }));
}

// A minimal ftyp+ftyp MP4 with no sidx or moov also returns IndexNotFound.
#[tokio::test]
async fn parse_mp4_no_moov_no_sidx_returns_index_not_found() {
    let mut data = vec![0x00, 0x00, 0x00, 0x0C]; // size = 12
    data.extend_from_slice(b"ftyp");
    data.extend_from_slice(&[0x00; 4]);
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, None, &fetcher).await;
    assert!(matches!(result.unwrap_err(), Error::IndexNotFound { .. }));
}

#[tokio::test]
async fn parse_ogg_without_total_size_returns_parse_failed() {
    // Valid OGG magic but no total_size and only a stub — should fail cleanly.
    let mut data = b"OggS".to_vec();
    data.extend_from_slice(&[0x00; 50]);
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    // No total_size → OGG parser immediately returns ParseFailed
    let result = media_seek::parse(&data, None, &fetcher).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::ParseFailed { .. }));
}

#[tokio::test]
async fn index_not_found_error_displays_reason() {
    let e = Error::IndexNotFound {
        reason: "test reason".to_string(),
    };
    let msg = e.to_string();
    assert!(msg.contains("test reason"), "Display should include reason: {msg}");
}
