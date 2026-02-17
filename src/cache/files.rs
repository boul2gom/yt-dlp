//! Async-safe cache implementation for downloaded files using generic backend.
//!
//! This module provides a fully async cache implementation that uses the configured
//! backend (JSON or SQLite) to store files and metadata.

use crate::cache::backend::FileBackend;
#[cfg(feature = "cache-json")]
use crate::cache::backend::json::JsonFileCache;
#[cfg(feature = "cache-sqlite")]
use crate::cache::backend::sqlite::SqliteFileCache;

use crate::cache::video::{CachedFile, CachedThumbnail, CachedType};
use crate::error::Result;
use crate::model::format::Format;
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
use crate::model::thumbnail::Thumbnail;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

/// Structure for storing video metadata in cache.
pub struct DownloadCache {
    backend: Box<dyn FileBackend>,
}

impl std::fmt::Debug for DownloadCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DownloadCache")
    }
}

impl DownloadCache {
    /// Creates a new download cache with the specified cache directory and TTL.
    ///
    /// # Arguments
    ///
    /// * `cache_path` - The path to the cache directory.
    /// * `ttl` - The time-to-live for cache entries in seconds (optional, defaults to 7 days).
    pub async fn new(
        cache_path: impl AsRef<Path> + std::fmt::Debug,
        ttl: Option<u64>,
    ) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating download cache at {:?}", cache_path);

        let cache_dir = cache_path.as_ref().to_path_buf();
        #[allow(unused_assignments)]
        let mut backend: Option<Box<dyn FileBackend>> = None;

        #[cfg(feature = "cache-sqlite")]
        {
            backend = Some(Box::new(
                SqliteFileCache::new(cache_dir.clone(), ttl).await?,
            ));
        }

        #[cfg(all(feature = "cache-json", not(feature = "cache-sqlite")))]
        {
            backend = Some(Box::new(JsonFileCache::new(cache_dir.clone(), ttl).await?));
        }

        // Fallback or default if only cache-json is implicit
        if backend.is_none() {
            #[cfg(feature = "cache-json")]
            {
                backend = Some(Box::new(JsonFileCache::new(cache_dir.clone(), ttl).await?));
            }
        }

        if let Some(b) = backend {
            Ok(Self { backend: b })
        } else {
            // This happens if no feature is enabled, but we should probably default to JSON if technically possible,
            // or panic/error if cargo features are messed up.
            // Given `cache` implies `cache-json`, this branch shouldn't be reached if `cache` is on.
            Err(crate::error::Error::Unknown(
                "No cache backend enabled".to_string(),
            ))
        }
    }

    /// Calculates the SHA-256 hash of a file.
    pub async fn calculate_file_hash(
        file_path: impl AsRef<Path> + std::fmt::Debug,
    ) -> Result<String> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Calculating hash for file {:?}", file_path);

        let mut file = File::open(&file_path).await?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await?;

        let mut hasher = Sha256::new();
        hasher.update(&buffer);
        let hash = hasher.finalize();

        Ok(hash.iter().map(|b| format!("{:02x}", b)).collect())
    }

    /// Sanitize a filename to prevent path traversal attacks
    fn sanitize_filename(filename: &str) -> String {
        filename
            .replace("..", "")
            .replace(['/', '\\', ':'], "")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
            .collect()
    }

    /// Determines the MIME type of a file based on its extension.
    fn determine_mime_type(file_path: impl AsRef<Path>) -> String {
        let extension = file_path
            .as_ref()
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        match extension.to_lowercase().as_str() {
            "mp4" => "video/mp4".to_string(),
            "webm" => "video/webm".to_string(),
            "mp3" => "audio/mpeg".to_string(),
            "m4a" => "audio/mp4".to_string(),
            "jpg" | "jpeg" => "image/jpeg".to_string(),
            "png" => "image/png".to_string(),
            "vtt" => "text/vtt".to_string(),
            "srt" => "application/x-subrip".to_string(),
            "ass" | "ssa" => "text/x-ssa".to_string(),
            _ => "application/octet-stream".to_string(),
        }
    }

    /// Cleans the cache by removing expired entries.
    pub async fn clean(&self) -> Result<()> {
        self.backend.clean().await
    }

    /// Gets a file from the cache by hash.
    pub async fn get_by_hash(&self, file_hash: &str) -> Option<(CachedFile, PathBuf)> {
        self.backend.get_by_hash(file_hash).await
    }

    /// Puts a file in the cache.
    pub async fn put_file(
        &self,
        source_path: impl AsRef<Path> + std::fmt::Debug,
        filename: impl AsRef<str> + std::fmt::Debug,
        video_id: Option<String>,
        format: Option<&Format>,
    ) -> Result<CachedFile> {
        self.put_file_with_preferences(
            source_path,
            filename,
            video_id,
            format,
            None,
            None,
            None,
            None,
        )
        .await
    }

    /// Puts a file in the cache with preferences.
    #[allow(clippy::too_many_arguments)]
    pub async fn put_file_with_preferences(
        &self,
        source_path: impl AsRef<Path> + std::fmt::Debug,
        filename: impl AsRef<str> + std::fmt::Debug,
        video_id: Option<String>,
        format: Option<&Format>,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Result<CachedFile> {
        let source_path = source_path.as_ref();
        #[cfg(feature = "tracing")]
        tracing::debug!("Caching file {:?}", source_path);

        let file_hash = Self::calculate_file_hash(source_path).await?;
        let metadata = tokio::fs::metadata(source_path).await?;
        let filesize = metadata.len() as i64;
        let mime_type = Self::determine_mime_type(source_path);

        let filename_str = filename.as_ref();
        let sanitized_filename = Self::sanitize_filename(filename_str);
        let extension = Path::new(&sanitized_filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // We construct the relative path here, but the backend might adjust or ignore it
        // depending on its internal structure. However, our FileBackend trait expects
        // the CachedFile to contain the relative path that the backend *should* use
        // or has used.
        // Actually, looking at implementations, they assume relative_path in CachedFile is authoritative.
        let relative_path = format!("files/{}.{}", file_hash, extension);

        // Prepare CachedFile struct
        let (file_type, format_id, format_json) = if let Some(f) = format {
            (
                serde_json::to_string(&CachedType::Format).unwrap_or_default(),
                Some(f.format_id.clone()),
                Some(serde_json::to_string(f).unwrap_or_default()),
            )
        } else {
            (
                serde_json::to_string(&CachedType::Other).unwrap_or_default(),
                None,
                None,
            )
        };

        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let video_quality_str =
            video_quality.map(|vq| serde_json::to_string(&vq).unwrap_or_default());
        let audio_quality_str =
            audio_quality.map(|aq| serde_json::to_string(&aq).unwrap_or_default());
        let video_codec_str = video_codec
            .clone()
            .map(|vc| serde_json::to_string(&vc).unwrap_or_default());
        let audio_codec_str = audio_codec
            .clone()
            .map(|ac| serde_json::to_string(&ac).unwrap_or_default());

        let cached_file = CachedFile {
            id: file_hash.clone(),
            filename: filename_str.to_string(),
            relative_path,
            video_id,
            file_type,
            format_id,
            format_json,
            video_quality: video_quality_str,
            audio_quality: audio_quality_str,
            video_codec: video_codec_str,
            audio_codec: audio_codec_str,
            language_code: None, // Not currently used for generic files
            filesize,
            mime_type,
            cached_at,
        };

        // Delegate to backend
        self.backend.put(cached_file.clone(), source_path).await?;

        Ok(cached_file)
    }

    /// Gets a file from cache by video ID and format ID.
    pub async fn get_by_video_and_format(
        &self,
        video_id: &str,
        format_id: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        self.backend
            .get_by_video_and_format(video_id, format_id)
            .await
    }

    /// Gets a file from cache by video ID and preferences.
    #[cfg(feature = "cache")]
    pub async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Option<(CachedFile, PathBuf)> {
        self.backend
            .get_by_video_and_preferences(
                video_id,
                video_quality,
                audio_quality,
                video_codec,
                audio_codec,
            )
            .await
    }

    // For non-cache feature build (if get_by_video_and_preferences is not available in trait)
    // we can omit it or shim it. But trait has #[cfg(feature = "cache")] on that method too.
    // So we should gate it here too.

    /// Puts a thumbnail in the cache.
    pub async fn put_thumbnail(
        &self,
        source_path: impl AsRef<Path> + std::fmt::Debug,
        filename: impl AsRef<str> + std::fmt::Debug,
        video_id: String,
        thumbnail: &Thumbnail,
    ) -> Result<CachedThumbnail> {
        let source_path = source_path.as_ref();
        #[cfg(feature = "tracing")]
        tracing::debug!("Caching thumbnail {:?}", source_path);

        let file_hash = Self::calculate_file_hash(source_path).await?;
        let metadata = tokio::fs::metadata(source_path).await?;
        let filesize = metadata.len() as i64;
        let mime_type = Self::determine_mime_type(source_path);

        let filename_str = filename.as_ref();
        let extension = Path::new(filename_str)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        let relative_path = format!("thumbnails/{}.{}", file_hash, extension);

        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let width = thumbnail.width.map(|w| w as i32);
        let height = thumbnail.height.map(|h| h as i32);

        let cached_thumbnail = CachedThumbnail {
            id: file_hash.clone(),
            filename: filename_str.to_string(),
            relative_path,
            video_id,
            filesize,
            mime_type,
            width,
            height,
            cached_at,
        };

        self.backend
            .put_thumbnail(cached_thumbnail.clone(), source_path)
            .await?;

        Ok(cached_thumbnail)
    }

    /// Gets a thumbnail from cache by video ID.
    pub async fn get_thumbnail_by_video_id(
        &self,
        video_id: &str,
    ) -> Option<(CachedThumbnail, PathBuf)> {
        self.backend.get_thumbnail_by_video_id(video_id).await
    }

    /// Gets a subtitle from cache by video ID and language.
    pub async fn get_subtitle_by_language(
        &self,
        video_id: &str,
        language: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        self.backend
            .get_subtitle_by_language(video_id, language)
            .await
    }

    /// Puts a subtitle file in the cache.
    pub async fn put_subtitle_file(
        &self,
        source_path: impl AsRef<Path> + std::fmt::Debug,
        filename: impl AsRef<str> + std::fmt::Debug,
        video_id: String,
        language: String,
    ) -> Result<CachedFile> {
        let source_path = source_path.as_ref();
        #[cfg(feature = "tracing")]
        tracing::debug!("Caching subtitle file {:?}", source_path);

        let file_hash = Self::calculate_file_hash(source_path).await?;
        let metadata = tokio::fs::metadata(source_path).await?;
        let filesize = metadata.len() as i64;
        let mime_type = Self::determine_mime_type(source_path);

        let filename_str = filename.as_ref();
        let sanitized_filename = Self::sanitize_filename(filename_str);
        let extension = Path::new(&sanitized_filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // Use a distinct path structure for subtitles if desired, or just files/
        // Current impl uses files/hash.ext
        let relative_path = format!("files/{}.{}", file_hash, extension);

        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cached_file = CachedFile {
            id: file_hash.clone(),
            filename: filename_str.to_string(),
            relative_path,
            video_id: Some(video_id),
            file_type: serde_json::to_string(&CachedType::Subtitle).unwrap_or_default(),
            format_id: None,
            format_json: None,
            video_quality: None,
            audio_quality: None,
            video_codec: None,
            audio_codec: None,
            language_code: Some(language),
            filesize,
            mime_type,
            cached_at,
        };

        // Delegate to backend
        self.backend.put(cached_file.clone(), source_path).await?;

        Ok(cached_file)
    }
}
