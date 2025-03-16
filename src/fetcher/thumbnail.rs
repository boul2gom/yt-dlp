//! Tools for fetching thumbnails from YouTube.

use crate::fetcher::Fetcher;
use crate::model::Video;
use crate::Youtube;
use std::path::PathBuf;

impl Youtube {
    /// Downloads the thumbnail of the video from the given URL, usually in the highest resolution available.
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
    /// let thumbnail_path = fetcher.download_thumbnail_from_url(url, "thumbnail.jpg").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
    pub async fn download_thumbnail_from_url(
        &self,
        url: String,
        file_name: impl AsRef<str> + std::fmt::Debug + derive_more::Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading thumbnail from {}", url);

        let video = self.fetch_video_infos(url).await?;

        self.download_thumbnail(&video, file_name).await
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
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
    pub async fn download_thumbnail(
        &self,
        video: &Video,
        file_name: impl AsRef<str> + std::fmt::Debug + derive_more::Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading thumbnail {}", video.title);

        let path = self.output_dir.join(file_name.as_ref());

        let fetcher = Fetcher::new(&video.thumbnail);
        fetcher.fetch_asset(path.clone()).await?;

        Ok(path)
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
    pub async fn download_thumbnail_from_video(
        &self,
        video: &Video,
        file_name: impl AsRef<str> + std::fmt::Debug + derive_more::Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading thumbnail {}", video.title);

        let path = self.output_dir.join(file_name.as_ref());

        let fetcher = Fetcher::new(&video.thumbnail);
        fetcher.fetch_asset(path.clone()).await?;

        Ok(path)
    }
}
