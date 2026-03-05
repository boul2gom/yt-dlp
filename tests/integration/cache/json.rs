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

// ---------------------------------------------------------------------------
// TTL expiry
// ---------------------------------------------------------------------------

#[tokio::test]
async fn video_ttl_expires_entry() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = JsonVideoCache::new(dir.path().to_path_buf(), Some(1)) // 1s TTL
        .await
        .expect("cache creation failed");

    let video = crate::common::fixtures::load_video_fixture();
    let url = "https://youtube.com/watch?v=ttl_json_test";

    cache.put(url.to_string(), video).await.expect("put failed");
    assert!(cache.get(url).await.expect("get").is_some());

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let result = cache.get(url).await.expect("get after TTL");
    assert!(result.is_none(), "entry should be expired after TTL");
}

// ---------------------------------------------------------------------------
// Multiple distinct keys are independent
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multiple_distinct_keys_independent() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = JsonVideoCache::new(dir.path().to_path_buf(), Some(3600))
        .await
        .expect("cache creation failed");

    let mut video_a = crate::common::fixtures::load_video_fixture();
    video_a.id = "multi_id_a".to_string();
    let mut video_b = crate::common::fixtures::load_video_fixture();
    video_b.id = "multi_id_b".to_string();
    let url_a = "https://youtube.com/watch?v=multi_a";
    let url_b = "https://youtube.com/watch?v=multi_b";

    cache.put(url_a.to_string(), video_a).await.expect("put a");
    cache.put(url_b.to_string(), video_b).await.expect("put b");

    assert!(cache.get(url_a).await.expect("get a").is_some());
    assert!(cache.get(url_b).await.expect("get b").is_some());

    cache.remove(url_a).await.expect("remove a");
    assert!(cache.get(url_a).await.expect("get a after remove").is_none());
    assert!(
        cache.get(url_b).await.expect("get b after remove").is_some(),
        "b should be unaffected"
    );
}
