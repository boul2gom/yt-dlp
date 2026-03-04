use media_seek::RangeFetcher;

use crate::common;

#[tokio::test]
async fn mock_fetcher_returns_correct_bytes() {
    let data: Vec<u8> = (0..100).collect();
    let fetcher = common::media_seek::MockRangeFetcher::new(data);

    let bytes = fetcher.fetch(10, 19).await.unwrap();
    assert_eq!(bytes.len(), 10);
    assert_eq!(bytes[0], 10);
    assert_eq!(bytes[9], 19);
}

#[tokio::test]
async fn mock_fetcher_clamps_end() {
    let data = vec![0u8; 50];
    let fetcher = common::media_seek::MockRangeFetcher::new(data);

    let bytes = fetcher.fetch(40, 100).await.unwrap();
    assert_eq!(bytes.len(), 10); // 40..=49
}

#[tokio::test]
async fn mock_fetcher_out_of_bounds_error() {
    let data = vec![0u8; 10];
    let fetcher = common::media_seek::MockRangeFetcher::new(data);

    let result = fetcher.fetch(20, 30).await;
    assert!(result.is_err());
}
