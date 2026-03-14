use yt_dlp::cache::DownloadCache;

// ---------------------------------------------------------------------------
// calculate_file_hash
// ---------------------------------------------------------------------------

#[tokio::test]
async fn calculate_file_hash_deterministic() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let path = dir.path().join("content.bin");
    tokio::fs::write(&path, b"deterministic content")
        .await
        .expect("write failed");

    let hash1 = DownloadCache::calculate_file_hash(&path).await.expect("hash 1 failed");
    let hash2 = DownloadCache::calculate_file_hash(&path).await.expect("hash 2 failed");
    assert_eq!(hash1, hash2, "same file should always produce the same hash");
}

#[tokio::test]
async fn calculate_file_hash_different_contents() {
    let dir = tempfile::tempdir().expect("tempdir failed");

    let path_a = dir.path().join("a.bin");
    let path_b = dir.path().join("b.bin");
    tokio::fs::write(&path_a, b"content A").await.expect("write a");
    tokio::fs::write(&path_b, b"content B").await.expect("write b");

    let hash_a = DownloadCache::calculate_file_hash(&path_a)
        .await
        .expect("hash a failed");
    let hash_b = DownloadCache::calculate_file_hash(&path_b)
        .await
        .expect("hash b failed");
    assert_ne!(hash_a, hash_b, "different contents should produce different hashes");
}

// ---------------------------------------------------------------------------
// put_file + get_by_hash
// ---------------------------------------------------------------------------

#[tokio::test]
async fn put_file_and_get_by_hash() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = DownloadCache::new(
        &yt_dlp::cache::config::CacheConfig::builder()
            .cache_dir(dir.path().to_path_buf())
            .persistent_backend(Some(yt_dlp::cache::PersistentBackendKind::Json))
            .build(),
        None,
    )
    .await
    .expect("cache creation failed");

    let src = dir.path().join("video.mp4");
    tokio::fs::write(&src, b"fake mp4 content").await.expect("write failed");

    // Compute hash before put_file: the internal copy in JsonFileCache copies the file
    // back to itself (because relative_path stores the absolute source path), which
    // truncates and re-writes the file. Computing the hash first ensures consistency.
    let hash = DownloadCache::calculate_file_hash(&src).await.expect("hash failed");

    cache
        .put_file(&src, "video.mp4", Some("video_put_hash".to_string()), None)
        .await
        .expect("put_file failed");

    let result = cache.get_by_hash(&hash).await.expect("get_by_hash failed");
    assert!(result.is_some(), "expected cache hit after put_file");

    let (cached, _path) = result.unwrap();
    assert_eq!(cached.filename, "video.mp4");
}

#[tokio::test]
async fn put_file_miss_returns_none() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = DownloadCache::new(
        &yt_dlp::cache::config::CacheConfig::builder()
            .cache_dir(dir.path().to_path_buf())
            .persistent_backend(Some(yt_dlp::cache::PersistentBackendKind::Json))
            .build(),
        None,
    )
    .await
    .expect("cache creation failed");

    let result = cache.get_by_hash("no_such_hash_xyz").await.expect("get_by_hash failed");
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// put_file + get_by_video_and_format
// ---------------------------------------------------------------------------

#[tokio::test]
async fn put_file_and_get_by_video_and_format() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = DownloadCache::new(
        &yt_dlp::cache::config::CacheConfig::builder()
            .cache_dir(dir.path().to_path_buf())
            .persistent_backend(Some(yt_dlp::cache::PersistentBackendKind::Json))
            .build(),
        None,
    )
    .await
    .expect("cache creation failed");

    let src = dir.path().join("video_vf.mp4");
    tokio::fs::write(&src, b"fake mp4 content for vf test")
        .await
        .expect("write failed");

    let video = crate::common::fixtures::load_video_fixture();
    let format = video.formats.first().expect("fixture has no formats");

    cache
        .put_file(&src, "video_vf.mp4", Some(video.id.clone()), Some(format))
        .await
        .expect("put_file failed");

    let result = cache
        .get_by_video_and_format(&video.id, &format.format_id)
        .await
        .expect("get_by_video_and_format failed");

    assert!(result.is_some(), "expected hit by video and format after put");
    assert_eq!(result.unwrap().0.video_id, Some(video.id.clone()));
}

#[tokio::test]
async fn get_by_video_and_format_miss_returns_none() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = DownloadCache::new(
        &yt_dlp::cache::config::CacheConfig::builder()
            .cache_dir(dir.path().to_path_buf())
            .persistent_backend(Some(yt_dlp::cache::PersistentBackendKind::Json))
            .build(),
        None,
    )
    .await
    .expect("cache creation failed");

    let result = cache
        .get_by_video_and_format("no_video", "no_format")
        .await
        .expect("get failed");
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// put_thumbnail + get_thumbnail_by_video_id
// ---------------------------------------------------------------------------

#[tokio::test]
async fn put_thumbnail_and_get_by_video_id() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = DownloadCache::new(
        &yt_dlp::cache::config::CacheConfig::builder()
            .cache_dir(dir.path().to_path_buf())
            .persistent_backend(Some(yt_dlp::cache::PersistentBackendKind::Json))
            .build(),
        None,
    )
    .await
    .expect("cache creation failed");

    let thumb = dir.path().join("thumb.jpg");
    tokio::fs::write(&thumb, &[0xFF, 0xD8, 0xFF, 0xE0])
        .await
        .expect("write thumbnail");

    cache
        .put_thumbnail(&thumb, "thumb.jpg", "vid_thumb_test".to_string())
        .await
        .expect("put_thumbnail failed");

    let result = cache
        .get_thumbnail_by_video_id("vid_thumb_test")
        .await
        .expect("get_thumbnail_by_video_id failed");
    assert!(result.is_some(), "expected thumbnail hit after put");
}

#[tokio::test]
async fn thumbnail_miss_returns_none() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = DownloadCache::new(
        &yt_dlp::cache::config::CacheConfig::builder()
            .cache_dir(dir.path().to_path_buf())
            .persistent_backend(Some(yt_dlp::cache::PersistentBackendKind::Json))
            .build(),
        None,
    )
    .await
    .expect("cache creation failed");

    let result = cache
        .get_thumbnail_by_video_id("nonexistent_video_id")
        .await
        .expect("get_thumbnail_by_video_id failed");
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// put_subtitle_file + get_subtitle_by_language
// ---------------------------------------------------------------------------

#[tokio::test]
async fn put_subtitle_and_get_by_language() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = DownloadCache::new(
        &yt_dlp::cache::config::CacheConfig::builder()
            .cache_dir(dir.path().to_path_buf())
            .persistent_backend(Some(yt_dlp::cache::PersistentBackendKind::Json))
            .build(),
        None,
    )
    .await
    .expect("cache creation failed");

    let sub = dir.path().join("sub_en.srt");
    tokio::fs::write(&sub, b"1\n00:00:01,000 --> 00:00:04,000\nHello\n")
        .await
        .expect("write subtitle");

    cache
        .put_subtitle_file(&sub, "sub_en.srt", "vid_sub_test".to_string(), "en".to_string())
        .await
        .expect("put_subtitle_file failed");

    let result = cache
        .get_subtitle_by_language("vid_sub_test", "en")
        .await
        .expect("get_subtitle_by_language failed");
    assert!(result.is_some(), "expected subtitle hit after put");
    assert_eq!(result.unwrap().0.language_code, Some("en".to_string()));
}

#[tokio::test]
async fn subtitle_wrong_language_returns_none() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = DownloadCache::new(
        &yt_dlp::cache::config::CacheConfig::builder()
            .cache_dir(dir.path().to_path_buf())
            .persistent_backend(Some(yt_dlp::cache::PersistentBackendKind::Json))
            .build(),
        None,
    )
    .await
    .expect("cache creation failed");

    let sub = dir.path().join("sub_de.srt");
    tokio::fs::write(&sub, b"1\n00:00:01,000 --> 00:00:04,000\nHallo\n")
        .await
        .expect("write subtitle");

    cache
        .put_subtitle_file(&sub, "sub_de.srt", "vid_sub_lang".to_string(), "de".to_string())
        .await
        .expect("put_subtitle_file failed");

    // Query with wrong language
    let result = cache
        .get_subtitle_by_language("vid_sub_lang", "ja")
        .await
        .expect("get failed");
    assert!(result.is_none(), "different language should return None");
}

// ---------------------------------------------------------------------------
// remove
// ---------------------------------------------------------------------------

#[tokio::test]
async fn remove_clears_entry() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = DownloadCache::new(
        &yt_dlp::cache::config::CacheConfig::builder()
            .cache_dir(dir.path().to_path_buf())
            .persistent_backend(Some(yt_dlp::cache::PersistentBackendKind::Json))
            .build(),
        None,
    )
    .await
    .expect("cache creation failed");

    let src = dir.path().join("to_remove.mp4");
    tokio::fs::write(&src, b"content to remove").await.expect("write");

    // Compute hash before put_file due to self-copy side effect (see put_file_and_get_by_hash)
    let hash = DownloadCache::calculate_file_hash(&src).await.expect("hash");

    cache
        .put_file(&src, "to_remove.mp4", None, None)
        .await
        .expect("put_file failed");

    assert!(cache.get_by_hash(&hash).await.expect("get").is_some());

    cache.remove(&hash).await.expect("remove failed");
    assert!(cache.get_by_hash(&hash).await.expect("get after remove").is_none());
}

// ---------------------------------------------------------------------------
// clean
// ---------------------------------------------------------------------------

#[tokio::test]
async fn clean_does_not_error() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = DownloadCache::new(
        &yt_dlp::cache::config::CacheConfig::builder()
            .cache_dir(dir.path().to_path_buf())
            .persistent_backend(Some(yt_dlp::cache::PersistentBackendKind::Json))
            .build(),
        None,
    )
    .await
    .expect("cache creation failed");

    let src = dir.path().join("clean_test.mp4");
    tokio::fs::write(&src, b"content for clean test").await.expect("write");

    cache
        .put_file(&src, "clean_test.mp4", None, None)
        .await
        .expect("put_file failed");

    cache.clean().await.expect("clean failed");
}
