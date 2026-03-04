use yt_dlp::cache::backend::VideoBackend;
use yt_dlp::cache::backend::json::JsonVideoCache;

// ---------------------------------------------------------------------------
// VideoBackend CRUD with persistence
// ---------------------------------------------------------------------------

#[tokio::test]
async fn video_put_and_get() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = JsonVideoCache::new(dir.path().to_path_buf(), Some(3600))
        .await
        .expect("cache creation failed");

    let video = crate::common::fixtures::load_video_fixture();
    let url = "https://youtube.com/watch?v=json_test";

    cache.put(url.to_string(), video.clone()).await.expect("put failed");

    let retrieved = cache.get(url).await.expect("get failed");
    assert!(retrieved.is_some());

    let retrieved_video = retrieved.unwrap();
    assert_eq!(retrieved_video.id, video.id);
    assert_eq!(retrieved_video.title, video.title);
}

#[tokio::test]
async fn video_get_miss_returns_none() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = JsonVideoCache::new(dir.path().to_path_buf(), Some(3600))
        .await
        .expect("cache creation failed");

    let result = cache.get("https://nonexistent.com/video").await.expect("get failed");
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// Persistence across re-creation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn persists_across_reopen() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let url = "https://youtube.com/watch?v=persist_test";

    {
        let cache = JsonVideoCache::new(dir.path().to_path_buf(), Some(3600))
            .await
            .expect("cache creation failed");
        let video = crate::common::fixtures::load_video_fixture();
        cache.put(url.to_string(), video).await.expect("put failed");
    }

    // Re-create from same directory
    let cache = JsonVideoCache::new(dir.path().to_path_buf(), Some(3600))
        .await
        .expect("cache recreation failed");

    let result = cache.get(url).await.expect("get failed");
    assert!(result.is_some(), "data should persist across re-open");
    assert_eq!(result.unwrap().id, "gXtp6C-3JKo");
}

// ---------------------------------------------------------------------------
// Files created on disk
// ---------------------------------------------------------------------------

#[tokio::test]
async fn files_created_on_disk() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = JsonVideoCache::new(dir.path().to_path_buf(), Some(3600))
        .await
        .expect("cache creation failed");

    let video = crate::common::fixtures::load_video_fixture();
    cache
        .put("https://youtube.com/watch?v=disk".to_string(), video)
        .await
        .expect("put failed");

    // Verify at least some file was created in the cache directory
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read_dir failed")
        .filter_map(|e| e.ok())
        .collect();
    assert!(!entries.is_empty(), "expected files created in cache dir");
}

// ---------------------------------------------------------------------------
// Remove
// ---------------------------------------------------------------------------

#[tokio::test]
async fn video_remove() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = JsonVideoCache::new(dir.path().to_path_buf(), Some(3600))
        .await
        .expect("cache creation failed");

    let video = crate::common::fixtures::load_video_fixture();
    let url = "https://youtube.com/watch?v=remove_json";

    cache.put(url.to_string(), video).await.expect("put failed");
    assert!(cache.get(url).await.expect("get failed").is_some());

    cache.remove(url).await.expect("remove failed");
    assert!(cache.get(url).await.expect("get failed").is_none());
}

// ---------------------------------------------------------------------------
// Clean
// ---------------------------------------------------------------------------

#[tokio::test]
async fn clean_does_not_error() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = JsonVideoCache::new(dir.path().to_path_buf(), Some(3600))
        .await
        .expect("cache creation failed");

    cache.clean().await.expect("clean failed");
}
