//! Consolidated cache layer.
//!
//! `CacheLayer` bundles all three domain caches (videos, downloads, playlists)
//! into a single struct so the `Downloader` only needs one `Option<Arc<CacheLayer>>`.

use crate::cache::config::CacheConfig;
use crate::cache::files::DownloadCache;
use crate::cache::playlist::PlaylistCache;
use crate::cache::video::VideoCache;
use crate::error::Result;

/// Consolidated cache layer combining video, download, and playlist caches.
///
/// Created from a `CacheConfig` and stored as `Option<Arc<CacheLayer>>` on the
/// `Downloader`. Each sub-cache uses its own TTL from the config, falling back
/// to its domain-specific default.
#[derive(Debug)]
pub struct CacheLayer {
    /// Video metadata cache (tiered L1/L2).
    pub videos: VideoCache,
    /// Downloaded file metadata cache (tiered L1/L2).
    pub downloads: DownloadCache,
    /// Playlist metadata cache (tiered L1/L2).
    pub playlists: PlaylistCache,
}

impl CacheLayer {
    /// Build a `CacheLayer` from a `CacheConfig`.
    ///
    /// # Arguments
    ///
    /// * `config` - The cache configuration specifying directories, TTLs, and backend settings.
    ///
    /// # Returns
    ///
    /// A new `CacheLayer` with all three domain caches initialized.
    ///
    /// # Errors
    ///
    /// Returns an error if any backend initialization fails.
    pub async fn from_config(config: &CacheConfig) -> Result<Self> {
        tracing::debug!(config = %config, "⚙️ Building cache layer from config");

        let videos = VideoCache::new(config, config.video_ttl).await?;
        let downloads = DownloadCache::new(config, config.download_ttl).await?;
        let playlists = PlaylistCache::new(config, config.playlist_ttl).await?;

        tracing::debug!("✅ Cache layer initialized");

        Ok(Self {
            videos,
            downloads,
            playlists,
        })
    }

    /// Clean expired entries across all caches.
    ///
    /// # Errors
    ///
    /// Returns an error if cleanup fails for any sub-cache.
    pub async fn clean(&self) -> Result<()> {
        tracing::debug!("⚙️ Cleaning all caches");

        self.videos.clean().await?;
        self.downloads.clean().await?;
        self.playlists.clean().await?;

        Ok(())
    }
}
