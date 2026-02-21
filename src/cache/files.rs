//! Async-safe cache implementation for downloaded files using generic backend.
//!
//! This module provides a fully async cache implementation that uses the configured
//! backend (JSON or SQLite) to store files and metadata.

use crate::cache::backend::{FileBackend, FileBackendEnum};

use crate::cache::current_timestamp;
use crate::cache::video::{CachedFile, CachedThumbnail, CachedType};
use crate::error::Result;
use crate::model::format::Format;
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
use crate::model::thumbnail::Thumbnail;
use crate::model::utils::serde::{serialize_json, serialize_json_opt};
use crate::utils::validation::sanitize_filename;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

/// Structure for storing video metadata in cache.
#[derive(Debug)]
pub struct DownloadCache {
    backend: FileBackendEnum,
}

impl DownloadCache {
    /// Creates a new download cache with the specified cache directory and TTL.
    ///
    /// # Arguments
    ///
    /// * `cache_path` - The path to the cache directory.
    /// * `ttl` - The time-to-live for cache entries in seconds (optional, defaults to 7 days).
    ///
    /// # Returns
    ///
    /// A new `DownloadCache` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache backend initialization fails or no backend is enabled.
    pub async fn new(cache_path: impl Into<PathBuf>, ttl: Option<u64>) -> Result<Self> {
        let cache_dir: PathBuf = cache_path.into();

        tracing::debug!(
            cache_dir = ?cache_dir,
            ttl = ttl.unwrap_or(7 * 24 * 60 * 60),
            "Creating download cache"
        );

        let backend = FileBackendEnum::new(cache_dir, ttl).await?;
        Ok(Self { backend })
    }

    /// Calculates the SHA-256 hash of a file.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the file to hash.
    ///
    /// # Returns
    ///
    /// The SHA-256 hash as a hex string.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub async fn calculate_file_hash(file_path: impl Into<PathBuf>) -> Result<String> {
        let file_path: PathBuf = file_path.into();

        tracing::debug!(file_path = ?file_path, "Calculating SHA-256 hash for file");

        let mut file = File::open(&file_path).await?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await?;

        let mut hasher = Sha256::new();
        hasher.update(&buffer);
        let hash = hasher.finalize();

        let hash_str: String = hash.iter().map(|b| format!("{:02x}", b)).collect();

        tracing::debug!(
            file_path = ?file_path,
            hash = %hash_str,
            file_size = buffer.len(),
            "Calculated file hash"
        );

        Ok(hash_str)
    }

    /// Determines the MIME type of a file based on its extension.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the file.
    ///
    /// # Returns
    ///
    /// The MIME type as a string.
    fn determine_mime_type(file_path: impl Into<PathBuf>) -> String {
        let file_path: PathBuf = file_path.into();
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        let mime_type = match extension.to_lowercase().as_str() {
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
        };

        tracing::debug!(
            file_path = ?file_path,
            extension = extension,
            mime_type = %mime_type,
            "Determined MIME type"
        );

        mime_type
    }

    /// Collects basic file info needed for caching: hash, filesize, mime_type, extension.
    ///
    /// # Arguments
    ///
    /// * `source_path` - Path to the source file.
    /// * `filename` - The (possibly sanitized) filename used to derive the extension.
    ///
    /// # Returns
    ///
    /// A tuple `(hash, filesize, mime_type, extension)`.
    ///
    /// # Errors
    ///
    /// Returns an error if the hash calculation or metadata retrieval fails.
    async fn collect_file_info(
        source_path: &Path,
        filename: &str,
    ) -> Result<(String, i64, String, String)> {
        let file_hash = Self::calculate_file_hash(source_path).await?;
        let metadata = tokio::fs::metadata(source_path).await?;
        let filesize = metadata.len() as i64;
        let mime_type = Self::determine_mime_type(source_path);
        let extension = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();

        Ok((file_hash, filesize, mime_type, extension))
    }

    /// Cleans the cache by removing expired entries.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the cleanup operation fails.
    pub async fn clean(&self) -> Result<()> {
        tracing::debug!("Cleaning download cache");

        let result = self.backend.clean().await;

        if result.is_ok() {
            tracing::debug!("Successfully cleaned download cache");
        } else {
            tracing::debug!("Failed to clean download cache");
        }

        result
    }

    /// Gets a file from the cache by hash.
    ///
    /// # Arguments
    ///
    /// * `file_hash` - SHA-256 hash of the file.
    ///
    /// # Returns
    ///
    /// `Some((CachedFile, PathBuf))` if found and not expired, `None` otherwise.
    pub async fn get_by_hash(&self, file_hash: &str) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(hash = file_hash, "Getting file from cache by hash");

        let result = self.backend.get_by_hash(file_hash).await;

        tracing::debug!(
            hash = file_hash,
            found = result.is_some(),
            "File cache lookup by hash completed"
        );

        result
    }

    /// Puts a file in the cache.
    ///
    /// # Arguments
    ///
    /// * `source_path` - Path to the source file to cache.
    /// * `filename` - Original filename.
    /// * `video_id` - Associated video ID (if any).
    /// * `format` - Format information (if any).
    ///
    /// # Returns
    ///
    /// The `CachedFile` metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be cached.
    pub async fn put_file(
        &self,
        source_path: impl Into<PathBuf>,
        filename: impl Into<String>,
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

    /// Puts a file in the cache with quality and codec preferences.
    ///
    /// # Arguments
    ///
    /// * `source_path` - Path to the source file to cache.
    /// * `filename` - Original filename.
    /// * `video_id` - Associated video ID (if any).
    /// * `format` - Format information (if any).
    /// * `video_quality` - Video quality preference.
    /// * `audio_quality` - Audio quality preference.
    /// * `video_codec` - Video codec preference.
    /// * `audio_codec` - Audio codec preference.
    ///
    /// # Returns
    ///
    /// The `CachedFile` metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be cached.
    #[allow(clippy::too_many_arguments)]
    pub async fn put_file_with_preferences(
        &self,
        source_path: impl Into<PathBuf>,
        filename: impl Into<String>,
        video_id: Option<String>,
        format: Option<&Format>,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Result<CachedFile> {
        let source_path: PathBuf = source_path.into();
        let filename: String = filename.into();

        tracing::debug!(
            source_path = ?source_path,
            filename = %filename,
            video_id = ?video_id,
            has_format = format.is_some(),
            video_quality = ?video_quality,
            audio_quality = ?audio_quality,
            video_codec = ?video_codec,
            audio_codec = ?audio_codec,
            "Caching file with preferences"
        );

        let sanitized_filename = sanitize_filename(&filename);
        let (file_hash, filesize, mime_type, extension) =
            Self::collect_file_info(&source_path, &sanitized_filename).await?;

        // We construct the relative path here, but the backend might adjust or ignore it
        // depending on its internal structure. However, our FileBackend trait expects
        // the CachedFile to contain the relative path that the backend *should* use
        // or has used.
        // Actually, looking at implementations, they assume relative_path in CachedFile is authoritative.
        let relative_path = format!("files/{}.{}", file_hash, extension);

        // Prepare CachedFile struct
        let (file_type, format_id, format_json) = if let Some(f) = format {
            (
                serialize_json(&CachedType::Format),
                Some(f.format_id.clone()),
                Some(serialize_json(f)),
            )
        } else {
            (serialize_json(&CachedType::Other), None, None)
        };

        let cached_file = CachedFile {
            id: file_hash.clone(),
            filename: filename.clone(),
            relative_path,
            video_id,
            file_type,
            format_id,
            format_json,
            video_quality: serialize_json_opt(video_quality),
            audio_quality: serialize_json_opt(audio_quality),
            video_codec: serialize_json_opt(video_codec),
            audio_codec: serialize_json_opt(audio_codec),
            language_code: None, // Not currently used for generic files
            filesize,
            mime_type,
            cached_at: current_timestamp(),
        };

        // Delegate to backend
        self.backend.put(cached_file.clone(), &source_path).await?;

        Ok(cached_file)
    }

    /// Gets a file from cache by video ID and format ID.
    ///
    /// # Arguments
    ///
    /// * `video_id` - Video ID to search for.
    /// * `format_id` - Format ID to search for.
    ///
    /// # Returns
    ///
    /// `Some((CachedFile, PathBuf))` if found and not expired, `None` otherwise.
    pub async fn get_by_video_and_format(
        &self,
        video_id: &str,
        format_id: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(
            video_id = video_id,
            format_id = format_id,
            "Getting file from cache by video and format"
        );

        let result = self
            .backend
            .get_by_video_and_format(video_id, format_id)
            .await;

        tracing::debug!(
            video_id = video_id,
            format_id = format_id,
            found = result.is_some(),
            "File cache lookup by video and format completed"
        );

        result
    }

    /// Gets a file from cache by video ID and quality/codec preferences.
    ///
    /// # Arguments
    ///
    /// * `video_id` - Video ID to search for.
    /// * `video_quality` - Video quality preference.
    /// * `audio_quality` - Audio quality preference.
    /// * `video_codec` - Video codec preference.
    /// * `audio_codec` - Audio codec preference.
    ///
    /// # Returns
    ///
    /// `Some((CachedFile, PathBuf))` if found and not expired, `None` otherwise.
    #[cfg(feature = "cache-backend")]
    pub async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(
            video_id = video_id,
            video_quality = ?video_quality,
            audio_quality = ?audio_quality,
            video_codec = ?video_codec,
            audio_codec = ?audio_codec,
            "Getting file from cache by video and preferences"
        );

        let result = self
            .backend
            .get_by_video_and_preferences(
                video_id,
                video_quality,
                audio_quality,
                video_codec,
                audio_codec,
            )
            .await;

        tracing::debug!(
            video_id = video_id,
            found = result.is_some(),
            "File cache lookup by preferences completed"
        );

        result
    }

    /// Puts a thumbnail in the cache.
    ///
    /// # Arguments
    ///
    /// * `source_path` - Path to the source thumbnail file.
    /// * `filename` - Original filename.
    /// * `video_id` - Associated video ID.
    /// * `thumbnail` - Thumbnail metadata.
    ///
    /// # Returns
    ///
    /// The `CachedThumbnail` metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the thumbnail cannot be cached.
    pub async fn put_thumbnail(
        &self,
        source_path: impl Into<PathBuf>,
        filename: impl Into<String>,
        video_id: String,
        thumbnail: &Thumbnail,
    ) -> Result<CachedThumbnail> {
        let source_path: PathBuf = source_path.into();
        let filename: String = filename.into();

        tracing::debug!(
            source_path = ?source_path,
            filename = %filename,
            video_id = %video_id,
            width = ?thumbnail.width,
            height = ?thumbnail.height,
            "Caching thumbnail"
        );

        let (file_hash, filesize, mime_type, extension) =
            Self::collect_file_info(&source_path, &filename).await?;

        let relative_path = format!("thumbnails/{}.{}", file_hash, extension);

        let width = thumbnail.width.map(|w| w as i32);
        let height = thumbnail.height.map(|h| h as i32);

        let cached_thumbnail = CachedThumbnail {
            id: file_hash.clone(),
            filename: filename.clone(),
            relative_path,
            video_id,
            filesize,
            mime_type,
            width,
            height,
            cached_at: current_timestamp(),
        };

        self.backend
            .put_thumbnail(cached_thumbnail.clone(), &source_path)
            .await?;

        Ok(cached_thumbnail)
    }

    /// Gets a thumbnail from cache by video ID.
    ///
    /// # Arguments
    ///
    /// * `video_id` - Video ID to search for.
    ///
    /// # Returns
    ///
    /// `Some((CachedThumbnail, PathBuf))` if found and not expired, `None` otherwise.
    pub async fn get_thumbnail_by_video_id(
        &self,
        video_id: &str,
    ) -> Option<(CachedThumbnail, PathBuf)> {
        tracing::debug!(
            video_id = video_id,
            "Getting thumbnail from cache by video ID"
        );

        let result = self.backend.get_thumbnail_by_video_id(video_id).await;

        tracing::debug!(
            video_id = video_id,
            found = result.is_some(),
            "Thumbnail cache lookup completed"
        );

        result
    }

    /// Gets a subtitle from cache by video ID and language.
    ///
    /// # Arguments
    ///
    /// * `video_id` - Video ID to search for.
    /// * `language` - Language code to search for.
    ///
    /// # Returns
    ///
    /// `Some((CachedFile, PathBuf))` if found and not expired, `None` otherwise.
    pub async fn get_subtitle_by_language(
        &self,
        video_id: &str,
        language: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(
            video_id = video_id,
            language = language,
            "Getting subtitle from cache by video ID and language"
        );

        let result = self
            .backend
            .get_subtitle_by_language(video_id, language)
            .await;

        tracing::debug!(
            video_id = video_id,
            language = language,
            found = result.is_some(),
            "Subtitle cache lookup completed"
        );

        result
    }

    /// Puts a subtitle file in the cache.
    ///
    /// # Arguments
    ///
    /// * `source_path` - Path to the source subtitle file.
    /// * `filename` - Original filename.
    /// * `video_id` - Associated video ID.
    /// * `language` - Language code.
    ///
    /// # Returns
    ///
    /// The `CachedFile` metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the subtitle cannot be cached.
    pub async fn put_subtitle_file(
        &self,
        source_path: impl Into<PathBuf>,
        filename: impl Into<String>,
        video_id: String,
        language: String,
    ) -> Result<CachedFile> {
        let source_path: PathBuf = source_path.into();
        let filename: String = filename.into();

        tracing::debug!(
            source_path = ?source_path,
            filename = %filename,
            video_id = %video_id,
            language = %language,
            "Caching subtitle file"
        );

        let sanitized_filename = sanitize_filename(&filename);
        let (file_hash, filesize, mime_type, extension) =
            Self::collect_file_info(&source_path, &sanitized_filename).await?;

        // Use a distinct path structure for subtitles if desired, or just files/
        // Current impl uses files/hash.ext
        let relative_path = format!("files/{}.{}", file_hash, extension);

        let cached_file = CachedFile {
            id: file_hash.clone(),
            filename: filename.clone(),
            relative_path,
            video_id: Some(video_id),
            file_type: serialize_json(&CachedType::Subtitle),
            format_id: None,
            format_json: None,
            video_quality: None,
            audio_quality: None,
            video_codec: None,
            audio_codec: None,
            language_code: Some(language),
            filesize,
            mime_type,
            cached_at: current_timestamp(),
        };

        // Delegate to backend
        self.backend.put(cached_file.clone(), &source_path).await?;

        Ok(cached_file)
    }
}
