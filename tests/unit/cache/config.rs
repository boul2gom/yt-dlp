use std::path::PathBuf;

use yt_dlp::cache::CacheConfig;

// ============================== CacheConfig builder ==============================

#[test]
fn build_cache_config_with_minimal_args_succeeds() {
    let config = CacheConfig::builder().cache_dir(PathBuf::from("/tmp/cache")).build();
    assert_eq!(config.cache_dir, PathBuf::from("/tmp/cache"));
    assert!(config.redis_url.is_none());
    assert!(config.video_ttl.is_none());
    assert!(config.playlist_ttl.is_none());
    assert!(config.download_ttl.is_none());
}

#[test]
fn build_cache_config_with_all_fields_sets_correctly() {
    let config = CacheConfig::builder()
        .cache_dir(PathBuf::from("/var/cache"))
        .redis_url(Some("redis://127.0.0.1/".to_string()))
        .video_ttl(Some(86400))
        .playlist_ttl(Some(21600))
        .download_ttl(Some(604800))
        .build();

    assert_eq!(config.cache_dir, PathBuf::from("/var/cache"));
    assert_eq!(config.redis_url.as_deref(), Some("redis://127.0.0.1/"));
    assert_eq!(config.video_ttl, Some(86400));
    assert_eq!(config.playlist_ttl, Some(21600));
    assert_eq!(config.download_ttl, Some(604800));
}

// ============================== CacheConfig Display ==============================

#[test]
fn display_cache_config_without_redis_shows_none() {
    let config = CacheConfig::builder().cache_dir(PathBuf::from("/tmp/cache")).build();
    let display = format!("{}", config);
    assert!(display.contains("CacheConfig"));
    assert!(display.contains("none")); // redis_url = none
}

#[test]
fn display_cache_config_with_redis_url_shows_url() {
    let config = CacheConfig::builder()
        .cache_dir(PathBuf::from("/var/cache"))
        .redis_url(Some("redis://localhost/".to_string()))
        .build();
    let display = format!("{}", config);
    assert!(display.contains("redis://localhost/"));
}

#[test]
fn display_cache_config_contains_cache_dir_path() {
    let config = CacheConfig::builder().cache_dir(PathBuf::from("/my/cache/dir")).build();
    let display = format!("{}", config);
    assert!(display.contains("my/cache/dir") || display.contains("/my/cache/dir"));
}

// ============================== CacheConfig Clone ==============================

#[test]
fn clone_cache_config_preserves_all_fields() {
    let config = CacheConfig::builder()
        .cache_dir(PathBuf::from("/tmp/cache"))
        .video_ttl(Some(3600))
        .build();
    let cloned = config.clone();
    assert_eq!(cloned.cache_dir, config.cache_dir);
    assert_eq!(cloned.video_ttl, config.video_ttl);
}
