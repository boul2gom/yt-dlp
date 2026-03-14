use yt_dlp::DownloadStatus;
#[cfg(feature = "cache-json")]
use yt_dlp::cache::PersistentBackendKind;

use crate::common::assertions::assert_file_exists;
use crate::common::fixtures;
use crate::helpers;

/// Enable cache on a Downloader and verify the cache layer is active.
/// Note: download_video_stream + cache triggers an internal block_on in the file
/// cache store, so we test cache setup and manager-level integration instead.
#[tokio::test]
async fn cache_layer_activates_after_setup() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let mut downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    // Verify cache is not active initially
    assert!(downloader.cache().is_none(), "Cache should be None before setup");

    // Ensure cache directory exists before setup
    let cache_dir = tmp.path().join("cache");
    std::fs::create_dir_all(&cache_dir).unwrap();

    let builder = yt_dlp::cache::CacheConfig::builder().cache_dir(cache_dir.clone());
    #[cfg(feature = "cache-json")]
    let builder = builder.persistent_backend(Some(PersistentBackendKind::Json));
    let config = builder.build();
    downloader.set_cache(config).await.expect("Cache setup should succeed");

    // Verify cache is now active
    assert!(downloader.cache().is_some(), "Cache should be active after setup");
}

/// Verify that a download through the manager completes normally when cache is enabled,
/// and that a second download also completes (cache does not interfere with manager).
#[tokio::test]
async fn repeated_manager_download_with_cache() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let mut downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    let cache_dir = tmp.path().join("cache");
    std::fs::create_dir_all(&cache_dir).unwrap();

    let builder = yt_dlp::cache::CacheConfig::builder().cache_dir(cache_dir.clone());
    #[cfg(feature = "cache-json")]
    let builder = builder.persistent_backend(Some(PersistentBackendKind::Json));
    let config = builder.build();
    downloader.set_cache(config).await.expect("Cache setup should succeed");

    // First download
    let url = format!("{}/media/small.bin", server.uri());
    let output1 = tmp.path().join("first.bin");
    let id1 = downloader.download_manager().enqueue(&url, &output1, None).await;
    let status1 = downloader.wait_for_download(id1).await;
    assert!(
        matches!(status1, Some(DownloadStatus::Completed)),
        "First download should complete, got {:?}",
        status1
    );
    assert_file_exists(&output1);

    // Second download of the same URL
    let output2 = tmp.path().join("second.bin");
    let id2 = downloader.download_manager().enqueue(&url, &output2, None).await;
    let status2 = downloader.wait_for_download(id2).await;
    assert!(
        matches!(status2, Some(DownloadStatus::Completed)),
        "Second download should complete, got {:?}",
        status2
    );
    assert_file_exists(&output2);
}

/// Cache integration with direct manager downloads.
/// Manager downloads bypass the cache, so files won't be cached via this path.
/// This test verifies downloads complete regardless of cache configuration.
#[tokio::test]
async fn manager_download_works_with_cache_enabled() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let mut downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    let cache_dir = tmp.path().join("cache");
    std::fs::create_dir_all(&cache_dir).unwrap();

    let builder = yt_dlp::cache::CacheConfig::builder().cache_dir(cache_dir);
    #[cfg(feature = "cache-json")]
    let builder = builder.persistent_backend(Some(PersistentBackendKind::Json));
    let config = builder.build();
    downloader.set_cache(config).await.expect("Cache setup should succeed");

    let url = format!("{}/media/small.bin", server.uri());
    let output = tmp.path().join("manager_cached.bin");

    let id = downloader.download_manager().enqueue(&url, &output, None).await;
    let status = downloader.wait_for_download(id).await;

    assert!(
        matches!(status, Some(DownloadStatus::Completed)),
        "Download should complete with cache enabled, got {:?}",
        status
    );

    assert_file_exists(&output);
}
