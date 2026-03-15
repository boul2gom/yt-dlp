use yt_dlp::cache::backend::VideoBackend;
use yt_dlp::cache::backend::redb::RedbVideoCache;

// ---------------------------------------------------------------------------
// VideoBackend CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn video_put_and_get() {
    let (_dir, cache) = crate::common::cache::redb::video().await;

    let video = crate::common::fixtures::load_video_fixture();
    let url = "https://youtube.com/watch?v=redb_test";

    cache.put(url.to_string(), video.clone()).await.expect("put failed");

    let retrieved = cache.get(url).await.expect("get failed");
    assert!(retrieved.is_some());

    let retrieved_video = retrieved.unwrap();
    assert_eq!(retrieved_video.id, video.id);
    assert_eq!(retrieved_video.title, video.title);
}

#[tokio::test]
async fn video_get_miss_returns_none() {
    let (_dir, cache) = crate::common::cache::redb::video().await;

    let result = cache.get("https://nonexistent.com/video").await.expect("get failed");
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// Persistence across re-open
// ---------------------------------------------------------------------------

#[tokio::test]
async fn persists_across_reopen() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let url = "https://youtube.com/watch?v=redb_persist";

    {
        let cache = RedbVideoCache::new(dir.path().to_path_buf(), Some(3600))
            .await
            .expect("cache creation failed");
        let video = crate::common::fixtures::load_video_fixture();
        cache.put(url.to_string(), video).await.expect("put failed");
    }

    // Re-create from same directory
    let cache = RedbVideoCache::new(dir.path().to_path_buf(), Some(3600))
        .await
        .expect("cache recreation failed");

    let result = cache.get(url).await.expect("get failed");
    assert!(result.is_some(), "data should persist across re-open");
    assert_eq!(result.unwrap().id, "gXtp6C-3JKo");
}

// ---------------------------------------------------------------------------
// Remove
// ---------------------------------------------------------------------------

#[tokio::test]
async fn video_remove() {
    let (_dir, cache) = crate::common::cache::redb::video().await;

    let video = crate::common::fixtures::load_video_fixture();
    let url = "https://youtube.com/watch?v=redb_remove";

    cache.put(url.to_string(), video).await.expect("put failed");
    assert!(cache.get(url).await.expect("get failed").is_some());

    cache.remove(url).await.expect("remove failed");
    assert!(cache.get(url).await.expect("get failed").is_none());
}

// ---------------------------------------------------------------------------
// Concurrent reads
// ---------------------------------------------------------------------------

#[tokio::test]
async fn concurrent_reads() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = std::sync::Arc::new(
        RedbVideoCache::new(dir.path().to_path_buf(), Some(3600))
            .await
            .expect("cache creation failed"),
    );

    let video = crate::common::fixtures::load_video_fixture();
    let url = "https://youtube.com/watch?v=redb_concurrent";
    cache.put(url.to_string(), video).await.expect("put failed");

    // Multiple concurrent reads sharing a single DB handle should succeed
    let mut handles = Vec::new();
    for _ in 0..5 {
        let u = url.to_string();
        let c = cache.clone();
        handles.push(tokio::spawn(
            async move { c.get(&u).await.expect("concurrent get failed") },
        ));
    }

    for handle in handles {
        let result = handle.await.expect("join failed");
        assert!(result.is_some());
    }
}

// ---------------------------------------------------------------------------
// Clean
// ---------------------------------------------------------------------------

#[tokio::test]
async fn clean_does_not_error() {
    let (_dir, cache) = crate::common::cache::redb::video().await;

    cache.clean().await.expect("clean failed");
}
