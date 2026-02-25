//! Download file cache wrapper with tiered L1 (Moka) + L2 (persistent) lookup.
//!
//! Provides the `DownloadCache` which manages cached download files, thumbnails,
//! and subtitles with higher-level convenience methods built on top of the
//! `FileBackend` trait.

use crate::cache::backend::FileBackend;
#[cfg(has_persistent_cache)]
use crate::cache::backend::PersistentFileBackend;
#[cfg(feature = "cache-memory")]
use crate::cache::backend::memory::MokaFileCache;
use crate::cache::video::{CachedFile, CachedThumbnail, CachedType};
use crate::cache::{AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality};
use crate::error::Result;
use crate::model::format::Format;
use crate::model::utils;
use crate::utils::current_timestamp;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;

/// Guesses a MIME type from a file extension.
fn guess_mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("mp4") | Some("m4v") => "video/mp4",
        Some("mkv") => "video/x-matroska",
        Some("webm") => "video/webm",
        Some("mp3") => "audio/mpeg",
        Some("m4a") => "audio/mp4",
        Some("ogg") | Some("oga") => "audio/ogg",
        Some("opus") => "audio/opus",
        Some("flac") => "audio/flac",
        Some("wav") => "audio/wav",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("srt") => "text/plain",
        Some("vtt") => "text/vtt",
        Some("ass") | Some("ssa") => "text/x-ssa",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

/// Download file cache manager with tiered L1 (Moka) + L2 (persistent) lookup.
///
/// On `get_*`: L1 → miss → L2 → backfill L1.
/// On `put_*`: write to both layers.
#[derive(Debug)]
pub struct DownloadCache {
    #[cfg(feature = "cache-memory")]
    memory: MokaFileCache,
    #[cfg(has_persistent_cache)]
    persistent: PersistentFileBackend,
}

impl DownloadCache {
    /// Default TTL: 7 days
    const DEFAULT_TTL: u64 = 7 * 24 * 60 * 60;

    /// Create a new DownloadCache with default TTL.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory where cache data will be stored.
    /// * `redis_url` - Connection URL for Redis backend (only when `cache-redis` is enabled).
    ///
    /// # Returns
    ///
    /// A new `DownloadCache` instance with default TTL (7 days).
    ///
    /// # Errors
    ///
    /// Returns an error if the backend initialization fails.
    pub async fn new(
        cache_dir: impl Into<PathBuf>,
        #[cfg(feature = "cache-redis")] redis_url: Option<&str>,
        ttl: Option<u64>,
    ) -> Result<Self> {
        let cache_dir = cache_dir.into();
        let ttl_secs = ttl.unwrap_or(Self::DEFAULT_TTL);

        tracing::debug!(cache_dir = ?cache_dir, ttl = ttl_secs, "⚙️ Creating download cache");

        Ok(Self {
            #[cfg(feature = "cache-memory")]
            memory: MokaFileCache::new(cache_dir.clone(), Some(ttl_secs)).await?,
            #[cfg(has_persistent_cache)]
            persistent: PersistentFileBackend::new(
                cache_dir,
                #[cfg(feature = "cache-redis")]
                redis_url,
                Some(ttl_secs),
            )
            .await?,
        })
    }

    /// Retrieve a file by its content hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - The SHA-256 hash of the file content.
    ///
    /// # Returns
    ///
    /// The cached file metadata and its path, or `None` if not found.
    pub async fn get_by_hash(&self, hash: &str) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(hash = hash, "🔍 Looking up file by hash");

        // L1: Moka
        #[cfg(feature = "cache-memory")]
        if let Some(result) = self.memory.get_by_hash(hash).await {
            tracing::debug!(hash = hash, "✅ File cache hit (L1 memory)");
            return Some(result);
        }

        // L2: persistent
        #[cfg(has_persistent_cache)]
        if let Some(result) = self.persistent.get_by_hash(hash).await {
            tracing::debug!(hash = hash, "✅ File cache hit (L2 persistent)");

            // Backfill L1
            #[cfg(feature = "cache-memory")]
            {
                let path = std::path::Path::new(&result.0.relative_path);
                let _ = self.memory.put(result.0.clone(), path).await;
            }

            return Some(result);
        }

        None
    }

    /// Retrieve a file by video ID and format ID.
    ///
    /// # Arguments
    ///
    /// * `video_id` - The video identifier.
    /// * `format_id` - The format identifier.
    ///
    /// # Returns
    ///
    /// The cached file metadata and its path, or `None` if not found.
    pub async fn get_by_video_and_format(
        &self,
        video_id: &str,
        format_id: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(
            video_id = video_id,
            format_id = format_id,
            "🔍 Looking up file by video and format"
        );

        // L1: Moka
        #[cfg(feature = "cache-memory")]
        if let Some(result) = self
            .memory
            .get_by_video_and_format(video_id, format_id)
            .await
        {
            tracing::debug!(
                video_id = video_id,
                format_id = format_id,
                "✅ File cache hit (L1 memory)"
            );
            return Some(result);
        }

        // L2: persistent
        #[cfg(has_persistent_cache)]
        if let Some(result) = self
            .persistent
            .get_by_video_and_format(video_id, format_id)
            .await
        {
            tracing::debug!(
                video_id = video_id,
                format_id = format_id,
                "✅ File cache hit (L2 persistent)"
            );

            #[cfg(feature = "cache-memory")]
            {
                let path = std::path::Path::new(&result.0.relative_path);
                let _ = self.memory.put(result.0.clone(), path).await;
            }

            return Some(result);
        }

        None
    }

    /// Retrieve a file by video ID and quality/codec preferences.
    ///
    /// # Arguments
    ///
    /// * `video_id` - The video identifier.
    /// * `video_quality` - Preferred video quality.
    /// * `audio_quality` - Preferred audio quality.
    /// * `video_codec` - Preferred video codec.
    /// * `audio_codec` - Preferred audio codec.
    ///
    /// # Returns
    ///
    /// The cached file metadata and its path, or `None` if no match.
    pub async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(video_id = video_id, "🔍 Looking up file by preferences");

        // L1: Moka
        #[cfg(feature = "cache-memory")]
        if let Some(result) = self
            .memory
            .get_by_video_and_preferences(
                video_id,
                video_quality,
                audio_quality,
                video_codec.clone(),
                audio_codec.clone(),
            )
            .await
        {
            tracing::debug!(
                video_id = video_id,
                "✅ File cache hit by preferences (L1 memory)"
            );
            return Some(result);
        }

        // L2: persistent
        #[cfg(has_persistent_cache)]
        if let Some(result) = self
            .persistent
            .get_by_video_and_preferences(
                video_id,
                video_quality,
                audio_quality,
                video_codec,
                audio_codec,
            )
            .await
        {
            tracing::debug!(
                video_id = video_id,
                "✅ File cache hit by preferences (L2 persistent)"
            );

            #[cfg(feature = "cache-memory")]
            {
                let path = std::path::Path::new(&result.0.relative_path);
                let _ = self.memory.put(result.0.clone(), path).await;
            }

            return Some(result);
        }

        None
    }

    /// Retrieve a thumbnail by video ID.
    ///
    /// # Arguments
    ///
    /// * `video_id` - The video identifier.
    ///
    /// # Returns
    ///
    /// The cached thumbnail metadata and its path, or `None` if not found.
    pub async fn get_thumbnail_by_video_id(
        &self,
        video_id: &str,
    ) -> Option<(CachedThumbnail, PathBuf)> {
        tracing::debug!(video_id = video_id, "🔍 Looking up thumbnail by video ID");

        // L1: Moka
        #[cfg(feature = "cache-memory")]
        if let Some(result) = self.memory.get_thumbnail_by_video_id(video_id).await {
            tracing::debug!(video_id = video_id, "✅ Thumbnail cache hit (L1 memory)");
            return Some(result);
        }

        // L2: persistent
        #[cfg(has_persistent_cache)]
        if let Some(result) = self.persistent.get_thumbnail_by_video_id(video_id).await {
            tracing::debug!(
                video_id = video_id,
                "✅ Thumbnail cache hit (L2 persistent)"
            );

            #[cfg(feature = "cache-memory")]
            {
                let path = std::path::Path::new(&result.0.relative_path);
                let _ = self.memory.put_thumbnail(result.0.clone(), path).await;
            }

            return Some(result);
        }

        None
    }

    /// Retrieve a subtitle by video ID and language code.
    ///
    /// # Arguments
    ///
    /// * `video_id` - The video identifier.
    /// * `language` - The language code (e.g., "en", "es").
    ///
    /// # Returns
    ///
    /// The cached subtitle file metadata and its path, or `None` if not found.
    pub async fn get_subtitle_by_language(
        &self,
        video_id: &str,
        language: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(
            video_id = video_id,
            language = language,
            "🔍 Looking up subtitle by language"
        );

        // L1: Moka
        #[cfg(feature = "cache-memory")]
        if let Some(result) = self
            .memory
            .get_subtitle_by_language(video_id, language)
            .await
        {
            tracing::debug!(
                video_id = video_id,
                language = language,
                "✅ Subtitle cache hit (L1 memory)"
            );
            return Some(result);
        }

        // L2: persistent
        #[cfg(has_persistent_cache)]
        if let Some(result) = self
            .persistent
            .get_subtitle_by_language(video_id, language)
            .await
        {
            tracing::debug!(
                video_id = video_id,
                language = language,
                "✅ Subtitle cache hit (L2 persistent)"
            );

            #[cfg(feature = "cache-memory")]
            {
                let path = std::path::Path::new(&result.0.relative_path);
                let _ = self.memory.put(result.0.clone(), path).await;
            }

            return Some(result);
        }

        None
    }

    /// Store a file in the cache (both layers).
    ///
    /// # Arguments
    ///
    /// * `source_path` - Path to the source file.
    /// * `filename` - Display name for the cached file.
    /// * `video_id` - Optional video ID association.
    /// * `format` - Optional format metadata to store alongside the file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be hashed or stored.
    pub async fn put_file(
        &self,
        source_path: &Path,
        filename: impl Into<String>,
        video_id: Option<String>,
        format: Option<&Format>,
    ) -> Result<PathBuf> {
        let file_info = Self::collect_file_info(source_path, filename.into(), video_id, format)?;
        self.put_cached_file(file_info, source_path).await
    }

    /// Store a file in the cache with quality/codec preferences (both layers).
    ///
    /// # Arguments
    ///
    /// * `source_path` - Path to the source file.
    /// * `filename` - Display name for the cached file.
    /// * `video_id` - Optional video ID association.
    /// * `format` - Optional format metadata.
    /// * `video_quality` - Video quality preference used for selection.
    /// * `audio_quality` - Audio quality preference used for selection.
    /// * `video_codec` - Video codec preference used for selection.
    /// * `audio_codec` - Audio codec preference used for selection.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be hashed or stored.
    #[allow(clippy::too_many_arguments)]
    pub async fn put_file_with_preferences(
        &self,
        source_path: &Path,
        filename: impl Into<String>,
        video_id: Option<String>,
        format: Option<&Format>,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Result<PathBuf> {
        let mut file_info =
            Self::collect_file_info(source_path, filename.into(), video_id, format)?;

        file_info.video_quality = utils::serde::serialize_json_opt(video_quality);
        file_info.audio_quality = utils::serde::serialize_json_opt(audio_quality);
        file_info.video_codec = utils::serde::serialize_json_opt(video_codec);
        file_info.audio_codec = utils::serde::serialize_json_opt(audio_codec);

        self.put_cached_file(file_info, source_path).await
    }

    /// Store a thumbnail in the cache (both layers).
    ///
    /// # Arguments
    ///
    /// * `source_path` - Path to the source thumbnail file.
    /// * `filename` - Display name for the cached thumbnail.
    /// * `video_id` - The video ID associated with this thumbnail.
    ///
    /// # Errors
    ///
    /// Returns an error if the thumbnail cannot be stored.
    pub async fn put_thumbnail(
        &self,
        source_path: &Path,
        filename: impl Into<String>,
        video_id: String,
    ) -> Result<PathBuf> {
        let filename = filename.into();
        let hash = Self::calculate_file_hash(source_path).await?;
        let size = tokio::fs::metadata(source_path)
            .await
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        let thumbnail = CachedThumbnail {
            id: hash,
            filename,
            relative_path: source_path.to_string_lossy().to_string(),
            video_id,
            filesize: size,
            mime_type: guess_mime(source_path).to_string(),
            width: None,
            height: None,
            cached_at: current_timestamp(),
        };

        self.put_cached_thumbnail(thumbnail, source_path).await
    }

    /// Store a subtitle file in the cache (both layers).
    ///
    /// # Arguments
    ///
    /// * `source_path` - Path to the source subtitle file.
    /// * `filename` - Display name for the cached subtitle.
    /// * `video_id` - The video ID associated with this subtitle.
    /// * `language_code` - The language code (e.g., "en", "es").
    ///
    /// # Errors
    ///
    /// Returns an error if the subtitle cannot be stored.
    pub async fn put_subtitle_file(
        &self,
        source_path: &Path,
        filename: impl Into<String>,
        video_id: String,
        language_code: String,
    ) -> Result<PathBuf> {
        let filename = filename.into();
        let hash = Self::calculate_file_hash(source_path).await?;
        let size = tokio::fs::metadata(source_path)
            .await
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        let cached_file = CachedFile {
            id: hash,
            filename,
            relative_path: source_path.to_string_lossy().to_string(),
            video_id: Some(video_id),
            file_type: CachedType::Subtitle.to_string(),
            format_id: None,
            format_json: None,
            video_quality: None,
            audio_quality: None,
            video_codec: None,
            audio_codec: None,
            language_code: Some(language_code),
            filesize: size,
            mime_type: "text/plain".to_string(),
            cached_at: current_timestamp(),
        };

        self.put_cached_file(cached_file, source_path).await
    }

    /// Remove a file from the cache (both layers).
    ///
    /// # Arguments
    ///
    /// * `id` - The unique identifier of the cached file.
    ///
    /// # Errors
    ///
    /// Returns an error if the removal operation fails.
    pub async fn remove(&self, id: &str) -> Result<()> {
        tracing::debug!(file_id = id, "⚙️ Removing file from cache");

        #[cfg(feature = "cache-memory")]
        self.memory.remove(id).await?;

        #[cfg(has_persistent_cache)]
        self.persistent.remove(id).await?;

        Ok(())
    }

    /// Clean expired entries (both layers).
    ///
    /// # Errors
    ///
    /// Returns an error if the cleanup operation fails.
    pub async fn clean(&self) -> Result<()> {
        tracing::debug!("⚙️ Cleaning download cache");

        #[cfg(feature = "cache-memory")]
        self.memory.clean().await?;

        #[cfg(has_persistent_cache)]
        self.persistent.clean().await?;

        Ok(())
    }

    /// Calculate the SHA-256 hash of a file.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the file to hash.
    ///
    /// # Returns
    ///
    /// A hex-encoded SHA-256 hash string.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub async fn calculate_file_hash(path: &Path) -> Result<String> {
        let mut file = tokio::fs::File::open(path).await?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = file.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Collect file metadata into a `CachedFile` struct.
    fn collect_file_info(
        source_path: &Path,
        filename: String,
        video_id: Option<String>,
        format: Option<&Format>,
    ) -> Result<CachedFile> {
        let size = std::fs::metadata(source_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        let mime = guess_mime(source_path).to_string();

        let format_id = format.map(|f| f.format_id.clone());
        let format_json = format.and_then(|f| serde_json::to_string(f).ok());

        Ok(CachedFile {
            id: String::new(), // Will be set after hashing
            filename,
            relative_path: source_path.to_string_lossy().to_string(),
            video_id,
            file_type: CachedType::Format.to_string(),
            format_id,
            format_json,
            video_quality: None,
            audio_quality: None,
            video_codec: None,
            audio_codec: None,
            language_code: None,
            filesize: size,
            mime_type: mime,
            cached_at: current_timestamp(),
        })
    }

    /// Internal helper: put a CachedFile to both layers.
    async fn put_cached_file(&self, mut file: CachedFile, source_path: &Path) -> Result<PathBuf> {
        // Calculate hash if not already set
        if file.id.is_empty() {
            file.id = Self::calculate_file_hash(source_path).await?;
        }

        tracing::debug!(
            file_id = file.id,
            filename = file.filename,
            "⚙️ Storing file in cache"
        );

        // L1: Moka (metadata only)
        #[cfg(feature = "cache-memory")]
        let _ = self.memory.put(file.clone(), source_path).await?;

        // L2: persistent (may copy actual file content)
        #[cfg(has_persistent_cache)]
        {
            let path = self.persistent.put(file, source_path).await?;
            return Ok(path);
        }

        #[allow(unreachable_code)]
        Ok(source_path.to_path_buf())
    }

    /// Internal helper: put a CachedThumbnail to both layers.
    async fn put_cached_thumbnail(
        &self,
        thumbnail: CachedThumbnail,
        source_path: &Path,
    ) -> Result<PathBuf> {
        tracing::debug!(
            thumbnail_id = thumbnail.id,
            video_id = thumbnail.video_id,
            "⚙️ Storing thumbnail in cache"
        );

        #[cfg(feature = "cache-memory")]
        let _ = self
            .memory
            .put_thumbnail(thumbnail.clone(), source_path)
            .await?;

        #[cfg(has_persistent_cache)]
        {
            let path = self
                .persistent
                .put_thumbnail(thumbnail, source_path)
                .await?;
            return Ok(path);
        }

        #[allow(unreachable_code)]
        Ok(source_path.to_path_buf())
    }
}
