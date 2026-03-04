use media_seek::Error;

use crate::common;

#[tokio::test]
async fn parse_empty_file_fails() {
    let data: Vec<u8> = vec![];
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, None, &fetcher).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn parse_random_bytes_fails() {
    let data = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE];
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, None, &fetcher).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::UnsupportedFormat));
}

#[tokio::test]
async fn parse_truncated_wav_fails() {
    let mut data = b"RIFF".to_vec();
    data.extend_from_slice(&100u32.to_le_bytes());
    data.extend_from_slice(b"WAVE");
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, None, &fetcher).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::ParseFailed { .. }));
}

#[tokio::test]
async fn parse_truncated_flac_fails() {
    let data = b"fLaC".to_vec();
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, None, &fetcher).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::ParseFailed { .. }));
}

#[tokio::test]
async fn parse_wrong_format_bytes_for_mp4() {
    let mut data = vec![0x00, 0x00, 0x00, 0x0C]; // size = 12
    data.extend_from_slice(b"ftyp");
    data.extend_from_slice(&[0x00; 4]);
    let fetcher = common::media_seek::MockRangeFetcher::new(data.clone());
    let result = media_seek::parse(&data, None, &fetcher).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::ParseFailed { .. }));
}
