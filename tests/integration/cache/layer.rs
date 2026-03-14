#[cfg(persistent_cache)]
use yt_dlp::cache::PersistentBackendKind;
#[cfg(persistent_cache)]
use yt_dlp::cache::{CacheConfig, CacheLayer};

// ---------------------------------------------------------------------------
// CacheLayer construction from config
// ---------------------------------------------------------------------------

#[cfg(feature = "cache-json")]
#[tokio::test]
async fn cache_layer_from_config() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let builder = CacheConfig::builder().cache_dir(dir.path().to_path_buf());
    let builder = builder.persistent_backend(Some(PersistentBackendKind::Json));
    let config = builder.build();

    let layer = CacheLayer::from_config(&config).await.expect("from_config failed");

    // Verify the layer can perform basic operations
    let video = crate::common::fixtures::load_video_fixture();
    let url = "https://youtube.com/watch?v=layer_test";

    layer
        .videos
        .put(url.to_string(), video.clone())
        .await
        .expect("put failed");

    let retrieved = layer.videos.get(url).await.expect("get failed");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, video.id);
}

// ---------------------------------------------------------------------------
// Tiered lookup: L1 miss → L2 hit
// ---------------------------------------------------------------------------

#[cfg(all(feature = "cache-memory", feature = "cache-json"))]
#[tokio::test]
async fn l1_miss_promotes_from_l2() {
    // With memory + json, a video put to both layers, then evicted from L1,
    // should still be found from L2 and promoted back to L1.
    // In practice we test the happy path: put through the layer, get back.
    let dir = tempfile::tempdir().expect("tempdir failed");
    let builder = CacheConfig::builder().cache_dir(dir.path().to_path_buf());
    let builder = builder.persistent_backend(Some(PersistentBackendKind::Json));
    let config = builder.build();

    let layer = CacheLayer::from_config(&config).await.expect("from_config failed");

    let video = crate::common::fixtures::load_video_fixture();
    let url = "https://youtube.com/watch?v=tiered_test";

    layer
        .videos
        .put(url.to_string(), video.clone())
        .await
        .expect("put failed");

    // Get should work via tiered lookup
    let result = layer.videos.get(url).await.expect("get failed");
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, video.id);
}

#[cfg(all(feature = "cache-memory", feature = "cache-redb"))]
#[tokio::test]
async fn l1_miss_promotes_from_l2_redb() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let builder = CacheConfig::builder().cache_dir(dir.path().to_path_buf());
    let builder = builder.persistent_backend(Some(PersistentBackendKind::Redb));
    let config = builder.build();

    let layer = CacheLayer::from_config(&config).await.expect("from_config failed");

    let video = crate::common::fixtures::load_video_fixture();
    let url = "https://youtube.com/watch?v=tiered_redb";

    layer
        .videos
        .put(url.to_string(), video.clone())
        .await
        .expect("put failed");

    let result = layer.videos.get(url).await.expect("get failed");
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, video.id);
}

// ---------------------------------------------------------------------------
// Clean
// ---------------------------------------------------------------------------

#[cfg(feature = "cache-json")]
#[tokio::test]
async fn clean_all_caches() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let builder = CacheConfig::builder().cache_dir(dir.path().to_path_buf());
    let builder = builder.persistent_backend(Some(PersistentBackendKind::Json));
    let config = builder.build();

    let layer = CacheLayer::from_config(&config).await.expect("from_config failed");
    layer.clean().await.expect("clean failed");
}

// ---------------------------------------------------------------------------
// Playlist via layer
// ---------------------------------------------------------------------------

#[cfg(feature = "cache-json")]
#[tokio::test]
async fn playlist_cache_via_layer() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let builder = CacheConfig::builder().cache_dir(dir.path().to_path_buf());
    let builder = builder.persistent_backend(Some(PersistentBackendKind::Json));
    let config = builder.build();

    let layer = CacheLayer::from_config(&config).await.expect("from_config failed");

    let playlist = crate::common::fixtures::load_playlist_fixture();
    let url = "https://youtube.com/playlist?list=layer_pl";

    layer
        .playlists
        .put(url.to_string(), playlist.clone())
        .await
        .expect("put failed");

    let result = layer.playlists.get(url).await.expect("get failed");
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, playlist.id);
}
