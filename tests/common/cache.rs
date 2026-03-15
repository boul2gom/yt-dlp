//! Factory helpers for constructing cache backend instances in tests.
//!
//! Each sub-module corresponds to one backend feature and exposes simple
//! async factory functions so test bodies don't repeat the
//! `tempdir + XxxCache::new(…).await.expect(…)` boilerplate.

#[cfg(feature = "cache-json")]
pub mod json {
    use tempfile::TempDir;
    use yt_dlp::cache::backend::json::{JsonFileCache, JsonPlaylistCache, JsonVideoCache};

    pub async fn video() -> (TempDir, JsonVideoCache) {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let cache = JsonVideoCache::new(dir.path().to_path_buf(), Some(3600))
            .await
            .expect("JsonVideoCache creation failed");
        (dir, cache)
    }

    pub async fn playlist() -> (TempDir, JsonPlaylistCache) {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let cache = JsonPlaylistCache::new(dir.path().to_path_buf(), Some(3600))
            .await
            .expect("JsonPlaylistCache creation failed");
        (dir, cache)
    }

    pub async fn file() -> (TempDir, JsonFileCache) {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let cache = JsonFileCache::new(dir.path().to_path_buf(), Some(3600))
            .await
            .expect("JsonFileCache creation failed");
        (dir, cache)
    }
}

#[cfg(feature = "cache-redb")]
pub mod redb {
    use tempfile::TempDir;
    use yt_dlp::cache::backend::redb::{RedbFileCache, RedbPlaylistCache, RedbVideoCache};

    pub async fn video() -> (TempDir, RedbVideoCache) {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let cache = RedbVideoCache::new(dir.path().to_path_buf(), Some(3600))
            .await
            .expect("RedbVideoCache creation failed");
        (dir, cache)
    }

    pub async fn playlist() -> (TempDir, RedbPlaylistCache) {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let cache = RedbPlaylistCache::new(dir.path().to_path_buf(), Some(3600))
            .await
            .expect("RedbPlaylistCache creation failed");
        (dir, cache)
    }

    pub async fn file() -> (TempDir, RedbFileCache) {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let cache = RedbFileCache::new(dir.path().to_path_buf(), Some(3600))
            .await
            .expect("RedbFileCache creation failed");
        (dir, cache)
    }
}

#[cfg(feature = "cache-memory")]
pub mod memory {
    use yt_dlp::cache::backend::memory::{MokaPlaylistCache, MokaVideoCache};

    pub async fn video() -> MokaVideoCache {
        MokaVideoCache::new("/tmp/test-cache".into(), Some(3600))
            .await
            .expect("MokaVideoCache creation failed")
    }

    pub async fn playlist() -> MokaPlaylistCache {
        MokaPlaylistCache::new("/tmp/test-cache".into(), Some(3600))
            .await
            .expect("MokaPlaylistCache creation failed")
    }
}
