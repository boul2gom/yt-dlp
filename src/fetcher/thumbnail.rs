//! Tools for fetching thumbnails from YouTube.

use crate::Youtube;
use crate::error::Error;
use crate::fetcher::Fetcher;
use crate::model::Video;
#[cfg(feature = "cache")]
use crate::model::thumbnail::Thumbnail;
use std::fmt::Display;
use std::path::PathBuf;

impl Youtube {
    /// Downloads the thumbnail of the video from the given URL, usually in the highest resolution available.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download the thumbnail from.
    /// * `output` - The name of the file to save the thumbnail to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the thumbnail could not be downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let fetcher = Youtube::new(libraries, output_dir)?;
    ///
    /// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    /// let thumbnail_path = fetcher.download_thumbnail_from_url(url, "thumbnail.jpg").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_thumbnail_from_url(
        &self,
        url: String,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading thumbnail from URL {}", url);

        let video = self.fetch_video_infos(url).await?;
        self.download_thumbnail(&video, output).await
    }

    /// Downloads the thumbnail of the video, usually in the highest resolution available.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download the thumbnail from.
    /// * `file_name` - The name of the file to save the thumbnail to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the thumbnail could not be fetched or downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let fetcher = Youtube::new(libraries, output_dir)?;
    ///
    /// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    /// let video = fetcher.fetch_video_infos(url).await?;
    ///
    /// let thumbnail_path = fetcher.download_thumbnail(&video, "thumbnail.jpg").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_thumbnail(
        &self,
        video: &Video,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading thumbnail for {}", video.title);

        let output_str = output.as_ref();
        let path = self.output_dir.join(output_str);

        // Check if the thumbnail is in the cache
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            // Try to find the thumbnail in the cache by video ID
            if let Some((_, cached_path)) = download_cache.get_thumbnail_by_video_id(&video.id) {
                #[cfg(feature = "tracing")]
                tracing::debug!("Using cached thumbnail for video: {}", video.id);

                // Copy the file from the cache to the output directory
                tokio::fs::copy(&cached_path, &path).await?;
                return Ok(path);
            }
        }

        // Get the best thumbnail
        let best_thumbnail = video
            .thumbnails
            .iter()
            .max_by_key(|t| t.width.unwrap_or(0))
            .ok_or(Error::MissingThumbnail)?;

        // Create an optimized fetcher with parallel downloading
        let fetcher = Fetcher::new(&best_thumbnail.url)
            .with_parallel_segments(4) // Use 4 parallel segments for thumbnails
            .with_segment_size(1024 * 1024) // 1 MB per segment
            .with_retry_attempts(3); // 3 attempts in case of failure

        fetcher.fetch_asset(path.clone()).await?;

        // Cache the downloaded thumbnail if caching is enabled
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            #[cfg(feature = "tracing")]
            tracing::debug!("Caching thumbnail for video: {}", video.id);

            if let Err(_e) = download_cache
                .put_thumbnail(&path, output_str, video.id.clone(), best_thumbnail)
                .await
            {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache thumbnail: {}", _e);
            }
        }

        Ok(path)
    }

    pub async fn download_thumbnail_from_video(
        &self,
        video: &Video,
        file_name: impl AsRef<str> + std::fmt::Debug + std::fmt::Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading thumbnail {}", video.title);

        let file_name_str = file_name.as_ref();
        let path = self.output_dir.join(file_name_str);

        // Check if the thumbnail is in the cache
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            // Try to find the thumbnail in the cache by video ID
            if let Some((_, cached_path)) = download_cache.get_thumbnail_by_video_id(&video.id) {
                #[cfg(feature = "tracing")]
                tracing::debug!("Using cached thumbnail for video: {}", video.id);

                // Copy the file from the cache to the output directory
                tokio::fs::copy(&cached_path, &path).await?;
                return Ok(path);
            }
        }

        let fetcher = Fetcher::new(&video.thumbnail);
        fetcher.fetch_asset(path.clone()).await?;

        // Cache the downloaded thumbnail if caching is enabled
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            #[cfg(feature = "tracing")]
            tracing::debug!("Caching thumbnail for video: {}", video.id);

            // Create a simple thumbnail object from the video's thumbnail URL
            let thumbnail = Thumbnail {
                url: video.thumbnail.clone(),
                preference: 0,
                id: "default".to_string(),
                height: None,
                width: None,
                resolution: None,
            };

            // Try to cache the file
            if let Err(_e) = download_cache
                .put_thumbnail(&path, file_name_str, video.id.clone(), &thumbnail)
                .await
            {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache thumbnail: {}", _e);
            }
        }

        Ok(path)
    }
}
