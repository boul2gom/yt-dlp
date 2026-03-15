use yt_dlp::cache::backend::VideoBackend;
use yt_dlp::cache::backend::memory::MokaVideoCache;

// ---------------------------------------------------------------------------
// VideoBackend CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn video_put_and_get() {
    let cache = crate::common::cache::memory::video().await;

    let video = crate::common::fixtures::load_video_fixture();
    let url = "https://youtube.com/watch?v=test_video_123";

    cache.put(url.to_string(), video.clone()).await.expect("put failed");

    let retrieved = cache.get(url).await.expect("get failed");
    assert!(retrieved.is_some());

    let retrieved_video = retrieved.unwrap();
    assert_eq!(retrieved_video.id, video.id);
    assert_eq!(retrieved_video.title, video.title);
}

#[tokio::test]
async fn video_get_miss_returns_none() {
    let cache = crate::common::cache::memory::video().await;

    let result = cache.get("https://nonexistent.com/video").await.expect("get failed");
    assert!(result.is_none());
}

#[tokio::test]
async fn video_remove() {
    let cache = crate::common::cache::memory::video().await;

    let video = crate::common::fixtures::load_video_fixture();
    let url = "https://youtube.com/watch?v=removable";

    cache.put(url.to_string(), video).await.expect("put failed");
    assert!(cache.get(url).await.expect("get failed").is_some());

    cache.remove(url).await.expect("remove failed");
    assert!(cache.get(url).await.expect("get failed").is_none());
}

// ---------------------------------------------------------------------------
// PlaylistBackend CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn playlist_put_and_get() {
    use yt_dlp::cache::backend::PlaylistBackend;

    let cache = crate::common::cache::memory::playlist().await;

    let playlist = crate::common::fixtures::load_playlist_fixture();
    let url = "https://youtube.com/playlist?list=test";

    cache.put(url.to_string(), playlist.clone()).await.expect("put failed");

    let retrieved = cache.get(url).await.expect("get failed");
    assert!(retrieved.is_some());

    let retrieved_playlist = retrieved.unwrap();
    assert_eq!(retrieved_playlist.id, playlist.id);
}

#[tokio::test]
async fn playlist_get_miss_returns_none() {
    use yt_dlp::cache::backend::PlaylistBackend;

    let cache = crate::common::cache::memory::playlist().await;

    let result = cache.get("https://nonexistent.com/playlist").await.expect("get failed");
    assert!(result.is_none());
}

#[tokio::test]
async fn playlist_invalidate() {
    use yt_dlp::cache::backend::PlaylistBackend;

    let cache = crate::common::cache::memory::playlist().await;

    let playlist = crate::common::fixtures::load_playlist_fixture();
    let url = "https://youtube.com/playlist?list=inv";

    cache.put(url.to_string(), playlist).await.expect("put failed");
    assert!(cache.get(url).await.expect("get failed").is_some());

    cache.invalidate(url).await.expect("invalidate failed");
    assert!(cache.get(url).await.expect("get failed").is_none());
}

// ---------------------------------------------------------------------------
// TTL expiry
// ---------------------------------------------------------------------------

#[tokio::test]
async fn video_ttl_expires_entries() {
    let cache = MokaVideoCache::new("/tmp/test-cache".into(), Some(1)) // 1 second TTL
        .await
        .expect("cache creation failed");

    let video = crate::common::fixtures::load_video_fixture();
    let url = "https://youtube.com/watch?v=ttl_test";

    cache.put(url.to_string(), video).await.expect("put failed");
    assert!(cache.get(url).await.expect("get failed").is_some());

    // Wait for TTL to expire
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let result = cache.get(url).await.expect("get failed");
    assert!(result.is_none(), "entry should have expired");
}

// ---------------------------------------------------------------------------
// Clean
// ---------------------------------------------------------------------------

#[tokio::test]
async fn clean_does_not_error() {
    let cache = crate::common::cache::memory::video().await;

    cache.clean().await.expect("clean failed");
}
