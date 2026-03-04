use yt_dlp::cache::backend::VideoBackend;
use yt_dlp::cache::backend::redis::RedisVideoCache;

fn redis_url() -> Option<String> {
    std::env::var("REDIS_URL").ok()
}

// ---------------------------------------------------------------------------
// VideoBackend CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn video_put_and_get() {
    let Some(url) = redis_url() else {
        eprintln!("REDIS_URL not set, skipping Redis test");
        return;
    };

    let cache = RedisVideoCache::new(url, Some(60))
        .await
        .expect("cache creation failed");

    let video = crate::common::fixtures::load_video_fixture();
    let cache_key = "https://youtube.com/watch?v=redis_test";

    cache
        .put(cache_key.to_string(), video.clone())
        .await
        .expect("put failed");

    let retrieved = cache.get(cache_key).await.expect("get failed");
    assert!(retrieved.is_some());

    let retrieved_video = retrieved.unwrap();
    assert_eq!(retrieved_video.id, video.id);
    assert_eq!(retrieved_video.title, video.title);

    // Cleanup
    cache.remove(cache_key).await.expect("remove failed");
}

#[tokio::test]
async fn video_get_miss_returns_none() {
    let Some(url) = redis_url() else {
        eprintln!("REDIS_URL not set, skipping Redis test");
        return;
    };

    let cache = RedisVideoCache::new(url, Some(60))
        .await
        .expect("cache creation failed");

    let result = cache
        .get("https://nonexistent.com/redis_miss")
        .await
        .expect("get failed");
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// Remove
// ---------------------------------------------------------------------------

#[tokio::test]
async fn video_remove() {
    let Some(url) = redis_url() else {
        eprintln!("REDIS_URL not set, skipping Redis test");
        return;
    };

    let cache = RedisVideoCache::new(url, Some(60))
        .await
        .expect("cache creation failed");

    let video = crate::common::fixtures::load_video_fixture();
    let cache_key = "https://youtube.com/watch?v=redis_remove";

    cache.put(cache_key.to_string(), video).await.expect("put failed");
    assert!(cache.get(cache_key).await.expect("get failed").is_some());

    cache.remove(cache_key).await.expect("remove failed");
    assert!(cache.get(cache_key).await.expect("get failed").is_none());
}

// ---------------------------------------------------------------------------
// Clean
// ---------------------------------------------------------------------------

#[tokio::test]
async fn clean_does_not_error() {
    let Some(url) = redis_url() else {
        eprintln!("REDIS_URL not set, skipping Redis test");
        return;
    };

    let cache = RedisVideoCache::new(url, Some(60))
        .await
        .expect("cache creation failed");

    cache.clean().await.expect("clean failed");
}
