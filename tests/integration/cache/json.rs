use std::path::Path;

use yt_dlp::cache::backend::json::{JsonPlaylistCache, JsonVideoCache};
use yt_dlp::cache::backend::{FileBackend, PlaylistBackend, VideoBackend};
use yt_dlp::cache::video::CachedType;
use yt_dlp::cache::{CachedFile, CachedThumbnail};
use yt_dlp::utils::current_timestamp;

// ---------------------------------------------------------------------------
// VideoBackend CRUD with persistence
// ---------------------------------------------------------------------------

#[tokio::test]
async fn video_put_and_get() {
    let (_dir, cache) = crate::common::cache::json::video().await;

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
    let (_dir, cache) = crate::common::cache::json::video().await;

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
    let (dir, cache) = crate::common::cache::json::video().await;

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
    let (_dir, cache) = crate::common::cache::json::video().await;

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
    let (_dir, cache) = crate::common::cache::json::video().await;

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
    let (_dir, cache) = crate::common::cache::json::video().await;

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

// ---------------------------------------------------------------------------
// JsonVideoCache::get_by_id
// ---------------------------------------------------------------------------

#[tokio::test]
async fn video_get_by_id_returns_cached_video() {
    let (_dir, cache) = crate::common::cache::json::video().await;

    let video = crate::common::fixtures::load_video_fixture();
    let url = "https://youtube.com/watch?v=getbyid_test";

    cache.put(url.to_string(), video.clone()).await.expect("put failed");

    let result = cache.get_by_id(&video.id).await.expect("get_by_id failed");
    assert_eq!(result.id, video.id);
    assert_eq!(result.url, url);
}

#[tokio::test]
async fn video_get_by_id_miss_returns_error() {
    let (_dir, cache) = crate::common::cache::json::video().await;

    let result = cache.get_by_id("nonexistent_id").await;
    assert!(result.is_err(), "get_by_id on unknown id should return error");
}

#[tokio::test]
async fn video_get_by_id_expired_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = JsonVideoCache::new(dir.path().to_path_buf(), Some(1)) // 1s TTL
        .await
        .expect("cache creation failed");

    let video = crate::common::fixtures::load_video_fixture();
    let url = "https://youtube.com/watch?v=getbyid_ttl";

    cache.put(url.to_string(), video.clone()).await.expect("put failed");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let result = cache.get_by_id(&video.id).await;
    assert!(result.is_err(), "get_by_id should fail after TTL expiry");
}

// ---------------------------------------------------------------------------
// JsonPlaylistCache CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn playlist_put_and_get() {
    let (_dir, cache) = crate::common::cache::json::playlist().await;

    let playlist = crate::common::fixtures::load_playlist_fixture();
    let url = "https://youtube.com/playlist?list=json_pl_test";

    cache.put(url.to_string(), playlist.clone()).await.expect("put failed");

    let result = cache.get(url).await.expect("get failed");
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, playlist.id);
}

#[tokio::test]
async fn playlist_get_miss_returns_none() {
    let (_dir, cache) = crate::common::cache::json::playlist().await;

    let result = cache.get("https://nonexistent.com/playlist").await.expect("get failed");
    assert!(result.is_none());
}

#[tokio::test]
async fn playlist_invalidate_removes_entry() {
    let (_dir, cache) = crate::common::cache::json::playlist().await;

    let playlist = crate::common::fixtures::load_playlist_fixture();
    let url = "https://youtube.com/playlist?list=invalidate_test";

    cache.put(url.to_string(), playlist).await.expect("put failed");
    assert!(cache.get(url).await.expect("get").is_some());

    cache.invalidate(url).await.expect("invalidate failed");
    assert!(cache.get(url).await.expect("get after invalidate").is_none());
}

#[tokio::test]
async fn playlist_get_by_id_returns_playlist() {
    let (_dir, cache) = crate::common::cache::json::playlist().await;

    let playlist = crate::common::fixtures::load_playlist_fixture();
    let url = "https://youtube.com/playlist?list=getbyid_pl";

    cache.put(url.to_string(), playlist.clone()).await.expect("put failed");

    let result = cache.get_by_id(&playlist.id).await.expect("get_by_id failed");
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, playlist.id);
}

#[tokio::test]
async fn playlist_get_by_id_miss_returns_none() {
    let (_dir, cache) = crate::common::cache::json::playlist().await;

    let result = cache.get_by_id("nonexistent_pl_id").await.expect("get_by_id failed");
    assert!(result.is_none());
}

#[tokio::test]
async fn playlist_clean_does_not_error() {
    let (_dir, cache) = crate::common::cache::json::playlist().await;

    cache.clean().await.expect("clean failed");
}

#[tokio::test]
async fn playlist_ttl_expires_entry() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache = JsonPlaylistCache::new(dir.path().to_path_buf(), Some(1)) // 1s TTL
        .await
        .expect("cache creation failed");

    let playlist = crate::common::fixtures::load_playlist_fixture();
    let url = "https://youtube.com/playlist?list=ttl_test";

    cache.put(url.to_string(), playlist).await.expect("put failed");
    assert!(cache.get(url).await.expect("get").is_some());

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let result = cache.get(url).await.expect("get after TTL");
    assert!(result.is_none(), "playlist entry should be expired after TTL");
}

#[tokio::test]
async fn playlist_clear_all_removes_all_entries() {
    let (_dir, cache) = crate::common::cache::json::playlist().await;

    let mut pl_a = crate::common::fixtures::load_playlist_fixture();
    pl_a.id = "pl_clear_a".to_string();
    let mut pl_b = crate::common::fixtures::load_playlist_fixture();
    pl_b.id = "pl_clear_b".to_string();

    cache
        .put("https://youtube.com/playlist?list=clear_a".to_string(), pl_a)
        .await
        .expect("put a");
    cache
        .put("https://youtube.com/playlist?list=clear_b".to_string(), pl_b)
        .await
        .expect("put b");

    cache.clear_all().await.expect("clear_all failed");

    let result_a = cache
        .get("https://youtube.com/playlist?list=clear_a")
        .await
        .expect("get a");
    let result_b = cache
        .get("https://youtube.com/playlist?list=clear_b")
        .await
        .expect("get b");
    assert!(result_a.is_none(), "entry a should be gone after clear_all");
    assert!(result_b.is_none(), "entry b should be gone after clear_all");
}

// ---------------------------------------------------------------------------
// JsonFileCache helpers
// ---------------------------------------------------------------------------

fn make_test_source(dir: &Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, b"fake media content for cache test").unwrap();
    path
}

fn make_cached_file(format_id: &str, video_id: &str) -> CachedFile {
    CachedFile {
        id: format!("filehash_{}", format_id),
        filename: format!("{}.mp4", format_id),
        relative_path: format!("files/video/{}.mp4", format_id),
        video_id: Some(video_id.to_string()),
        file_type: CachedType::Format.to_string(),
        format_id: Some(format_id.to_string()),
        format_json: None,
        video_quality: None,
        audio_quality: None,
        video_codec: None,
        audio_codec: None,
        language_code: None,
        filesize: 34,
        mime_type: "video/mp4".to_string(),
        cached_at: current_timestamp(),
    }
}

fn make_cached_thumbnail(id: &str, video_id: &str) -> CachedThumbnail {
    CachedThumbnail {
        id: id.to_string(),
        filename: format!("{}.jpg", id),
        relative_path: format!("thumbnails/{}.jpg", id),
        video_id: video_id.to_string(),
        filesize: 4,
        mime_type: "image/jpeg".to_string(),
        width: None,
        height: None,
        cached_at: current_timestamp(),
    }
}

// ---------------------------------------------------------------------------
// JsonFileCache — file CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn file_put_and_get_by_hash() {
    let (dir, cache) = crate::common::cache::json::file().await;

    let src = make_test_source(dir.path(), "source_file.mp4");
    let file = make_cached_file("fmt_hash_test", "vid_hash_test");
    let hash = file.id.clone();

    cache.put(file.clone(), &src).await.expect("put failed");

    let result = cache.get_by_hash(&hash).await.expect("get_by_hash failed");
    assert!(result.is_some(), "expected cache hit after put");

    let (cached, path) = result.unwrap();
    assert_eq!(cached.format_id, file.format_id);
    assert!(path.exists(), "cached file should exist on disk");
}

#[tokio::test]
async fn file_get_by_hash_miss_returns_none() {
    let (_dir, cache) = crate::common::cache::json::file().await;

    let result = cache.get_by_hash("nonexistent_hash").await.expect("get_by_hash failed");
    assert!(result.is_none());
}

#[tokio::test]
async fn file_put_and_get_by_video_and_format() {
    let (dir, cache) = crate::common::cache::json::file().await;

    let src = make_test_source(dir.path(), "src_vf.mp4");
    let file = make_cached_file("fmt_vf", "vid_vf");

    cache.put(file.clone(), &src).await.expect("put failed");

    let result = cache
        .get_by_video_and_format("vid_vf", "fmt_vf")
        .await
        .expect("get_by_video_and_format failed");
    assert!(result.is_some(), "expected hit after put");
    assert_eq!(result.unwrap().0.video_id, Some("vid_vf".to_string()));
}

#[tokio::test]
async fn file_get_by_video_and_format_miss() {
    let (dir, cache) = crate::common::cache::json::file().await;

    let src = make_test_source(dir.path(), "src_miss.mp4");
    let file = make_cached_file("fmt_miss", "vid_miss");
    cache.put(file, &src).await.expect("put failed");

    // Wrong video_id — should not match
    let result = cache
        .get_by_video_and_format("wrong_video_id", "fmt_miss")
        .await
        .expect("get failed");
    assert!(result.is_none());
}

#[tokio::test]
async fn file_remove_cleans_metadata_and_content() {
    let (dir, cache) = crate::common::cache::json::file().await;

    let src = make_test_source(dir.path(), "src_remove.mp4");
    let file = make_cached_file("fmt_remove", "vid_remove");
    let hash = file.id.clone();

    cache.put(file, &src).await.expect("put failed");
    assert!(cache.get_by_hash(&hash).await.expect("get").is_some());

    cache.remove(&hash).await.expect("remove failed");
    assert!(cache.get_by_hash(&hash).await.expect("get after remove").is_none());
}

#[tokio::test]
async fn file_clean_does_not_error() {
    let (dir, cache) = crate::common::cache::json::file().await;

    let src = make_test_source(dir.path(), "src_clean.mp4");
    let file = make_cached_file("fmt_clean", "vid_clean");
    cache.put(file, &src).await.expect("put failed");

    cache.clean().await.expect("clean failed");
}

// ---------------------------------------------------------------------------
// JsonFileCache — thumbnail CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn thumbnail_put_and_get_by_video_id() {
    let (dir, cache) = crate::common::cache::json::file().await;

    // Minimal JPEG bytes
    let src = dir.path().join("thumb.jpg");
    std::fs::write(&src, &[0xFF, 0xD8, 0xFF, 0xE0]).expect("write thumb");

    let thumbnail = make_cached_thumbnail("thumbhash1", "vid_thumb");

    cache
        .put_thumbnail(thumbnail.clone(), &src)
        .await
        .expect("put_thumbnail failed");

    let result = cache
        .get_thumbnail_by_video_id("vid_thumb")
        .await
        .expect("get_thumbnail_by_video_id failed");
    assert!(result.is_some(), "expected thumbnail hit after put");
    assert_eq!(result.unwrap().0.video_id, "vid_thumb");
}

#[tokio::test]
async fn thumbnail_get_miss_returns_none() {
    let (_dir, cache) = crate::common::cache::json::file().await;

    let result = cache
        .get_thumbnail_by_video_id("nonexistent_vid")
        .await
        .expect("get_thumbnail_by_video_id failed");
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// JsonFileCache — subtitle CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn subtitle_put_and_get_by_language() {
    let (dir, cache) = crate::common::cache::json::file().await;

    let src = dir.path().join("sub_en.srt");
    std::fs::write(&src, b"1\n00:00:01,000 --> 00:00:04,000\nHello\n").expect("write subtitle");

    let subtitle = CachedFile {
        id: "subtitlehash_en".to_string(),
        filename: "sub_en.srt".to_string(),
        relative_path: "files/video/sub_en.srt".to_string(),
        video_id: Some("vid_sub".to_string()),
        file_type: CachedType::Subtitle.to_string(),
        format_id: None,
        format_json: None,
        video_quality: None,
        audio_quality: None,
        video_codec: None,
        audio_codec: None,
        language_code: Some("en".to_string()),
        filesize: 38,
        mime_type: "text/plain".to_string(),
        cached_at: current_timestamp(),
    };

    cache.put(subtitle, &src).await.expect("put subtitle failed");

    let result = cache
        .get_subtitle_by_language("vid_sub", "en")
        .await
        .expect("get_subtitle_by_language failed");
    assert!(result.is_some(), "expected subtitle hit after put");
    assert_eq!(result.unwrap().0.language_code, Some("en".to_string()));
}

#[tokio::test]
async fn subtitle_get_wrong_language_returns_none() {
    let (dir, cache) = crate::common::cache::json::file().await;

    let src = dir.path().join("sub_fr.srt");
    std::fs::write(&src, b"1\n00:00:01,000 --> 00:00:04,000\nBonjour\n").expect("write subtitle");

    let subtitle = CachedFile {
        id: "subtitlehash_fr".to_string(),
        filename: "sub_fr.srt".to_string(),
        relative_path: "files/video/sub_fr.srt".to_string(),
        video_id: Some("vid_sub2".to_string()),
        file_type: CachedType::Subtitle.to_string(),
        format_id: None,
        format_json: None,
        video_quality: None,
        audio_quality: None,
        video_codec: None,
        audio_codec: None,
        language_code: Some("fr".to_string()),
        filesize: 41,
        mime_type: "text/plain".to_string(),
        cached_at: current_timestamp(),
    };

    cache.put(subtitle, &src).await.expect("put subtitle");

    // Query with different language
    let result = cache
        .get_subtitle_by_language("vid_sub2", "es")
        .await
        .expect("get_subtitle_by_language failed");
    assert!(result.is_none(), "wrong language should return None");
}
