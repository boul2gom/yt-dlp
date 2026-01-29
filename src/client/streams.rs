//! Tools for fetching video streams from YouTube.

use crate::download::Fetcher;
use crate::error::Error;
use crate::executor::Executor;
use crate::model::Video;
use crate::model::format::Format;
use crate::model::playlist::{Playlist, PlaylistDownloadProgress};
#[cfg(feature = "cache")]
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
use crate::{Youtube, utils};
use std::fmt::Display;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

impl Youtube {
    /// Fetch the video information from the given URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to fetch.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video information could not be fetched.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
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
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_video_infos(&self, url: String) -> crate::error::Result<Video> {
        // Check if the video is in the cache
        #[cfg(feature = "cache")]
        if let Some(cache) = &self.cache
            && let Some(video) = cache.get(&url).await?
        {
            #[cfg(feature = "tracing")]
            tracing::debug!("Using cached video information for {}", url);
            return Ok(video);
        }

        // If the video is not in the cache, retrieve it from YouTube
        let download_args = vec!["--no-progress", "--dump-json", &url];

        let mut final_args = self.args.clone();
        final_args.append(&mut utils::to_owned(download_args));

        let executor = Executor {
            executable_path: self.libraries.youtube.clone(),
            timeout: self.timeout,
            args: final_args,
        };

        let output = executor.execute().await?;
        let mut video: Video = serde_json::from_str(&output.stdout).map_err(|e| Error::Json {
            context: "Failed to parse video metadata".to_string(),
            source: e,
        })?;

        // Set the video ID on each format for caching purposes
        for format in &mut video.formats {
            format.video_id = Some(video.id.clone());
        }

        // Put the video in the cache if caching is enabled
        #[cfg(feature = "cache")]
        if let Some(cache) = &self.cache {
            #[cfg(feature = "tracing")]
            tracing::debug!("Caching video information for {}", url);

            if let Err(_e) = cache.put(url.clone(), video.clone()).await {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache video information: {}", _e);
            }
        }

        Ok(video)
    }

    /// Fetch the video from the given URL, download it (video with audio) and returns its path.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download.
    /// * `output` - The name of the file to save the video to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video could not be fetched or downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
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
    /// let video_path = fetcher.download_video_from_url(url, "my-video.mp4").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video_from_url(
        &self,
        url: String,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video from URL: {}", url);

        let video = self.fetch_video_infos(url).await?;
        self.download_video(&video, output).await
    }

    /// Fetch the video, download it (video with audio) and returns its path.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download.
    /// * `output` - The name of the file to save the video to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video could not be downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
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
    /// let video_path = fetcher.download_video(&video, "my-video.mp4").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video(
        &self,
        video: &Video,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video {}", video.title);

        cfg_if::cfg_if! {
            if #[cfg(feature = "cache")] {
                let output_str = output.as_ref();
                let path = self.output_dir.join(output_str);
            }
        }

        // Check if the video is in the cache
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            // Try to find the video in the cache by its ID
            if let Some((_, cached_path)) = download_cache.get_by_hash(&video.id).await {
                #[cfg(feature = "tracing")]
                tracing::debug!("Caching downloaded video with ID: {}", video.id);

                // Copy the file from the cache to the output directory
                tokio::fs::copy(&cached_path, &path).await?;
                return Ok(path);
            }
        }

        let best_video = video
            .best_video_format()
            .ok_or(Error::Unknown(format!("Missing format: {}", "video")))?;

        let best_audio = video
            .best_audio_format()
            .ok_or(Error::Unknown(format!("Missing format: {}", "audio")))?;

        // Create temporary names for audio and video files
        let audio_name = format!("temp_audio_{}.m4a", video.id);
        let video_name = format!("temp_video_{}.mp4", video.id);

        // Download audio and video streams in parallel
        let (audio_result, video_result) = tokio::join!(
            self.download_format(best_audio, &audio_name),
            self.download_format(best_video, &video_name)
        );

        // Check the results
        let _audio_path = audio_result?;
        let _video_path = video_result?;

        // Combine audio and video streams
        let output_path = self
            .combine_audio_and_video(&audio_name, &video_name, output.as_ref())
            .await?;

        // Clean up temporary files
        if let Err(_e) = tokio::fs::remove_file(&_video_path).await {
            #[cfg(feature = "tracing")]
            tracing::warn!("Failed to remove temporary video file: {}", _e);
        }
        if let Err(_e) = tokio::fs::remove_file(&_audio_path).await {
            #[cfg(feature = "tracing")]
            tracing::warn!("Failed to remove temporary audio file: {}", _e);
        }

        // Cache the downloaded file if caching is enabled
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            #[cfg(feature = "tracing")]
            tracing::debug!("Caching downloaded video with ID: {}", video.id);

            if let Err(_e) = download_cache
                .put_file(&path, output_str, Some(video.id.clone()), None)
                .await
            {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache downloaded video: {}", _e);
            }
        }

        Ok(output_path)
    }

    /// Fetch the video from the given URL, download it and returns its path.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download.
    /// * `output` - The name of the file to save the video to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video could not be fetched or downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
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
    /// let video_path = fetcher.download_video_stream_from_url(url, "my-video-stream.mp4").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video_stream_from_url(
        &self,
        url: String,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading audio stream from URL: {}", url);

        let video = self.fetch_video_infos(url).await?;

        self.download_video_stream(&video, output).await
    }

    /// Download the video only, and returns its path.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download.
    /// * `output` - The name of the file to save the video to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video could not be fetched or downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
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
    /// let video_path = fetcher.download_video_stream(&video, "my-video-stream.mp4").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video_stream(
        &self,
        video: &Video,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video stream {}", video.title);

        let best_video = video
            .best_video_format()
            .ok_or(Error::Unknown(format!("Missing format: {}", "video")))?;

        self.download_format(best_video, output).await
    }

    /// Fetch the audio stream from the given URL, download it and returns its path.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download.
    /// * `output` - The name of the file to save the audio to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video could not be fetched or downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
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
    /// let audio_path = fetcher.download_audio_stream_from_url(url, "my-audio.mp3").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_audio_stream_from_url(
        &self,
        url: String,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading audio stream from URL: {}", url);

        let video = self.fetch_video_infos(url).await?;
        self.download_audio_stream(&video, output).await
    }

    /// Fetch the audio stream, download it and returns its path.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download the audio from.
    /// * `output` - The name of the file to save the audio to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the audio could not be downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
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
    /// let audio_path = fetcher.download_audio_stream(&video, "my-audio.mp3").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_audio_stream(
        &self,
        video: &Video,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading audio stream {}", video.title);

        let output_str = output.as_ref();

        // Check if we have a cached audio file for this video
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            let path = self.output_dir.join(output_str);

            // Try to find an audio format in the cache by video ID
            let best_audio = video
                .best_audio_format()
                .ok_or(Error::Unknown(format!("Missing format: {}", "audio")))?;

            if let Some((_, cached_path)) = download_cache
                .get_by_video_and_format(&video.id, &best_audio.format_id)
                .await
            {
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Using cached audio: {} (format: {})",
                    video.id,
                    best_audio.format_id
                );

                // Copy the file from the cache to the output directory
                tokio::fs::copy(&cached_path, &path).await?;
                return Ok(path);
            }
        }

        let best_audio = video
            .best_audio_format()
            .ok_or(Error::Unknown(format!("Missing format: {}", "audio")))?;

        let temp_output = format!("temp_{}", output_str);
        let temp_path = self.download_format(best_audio, &temp_output).await?;

        // Post-process the audio file with ffmpeg to ensure compatibility with players
        let output_path = self.output_dir.join(output_str);

        let temp = temp_path
            .to_str()
            .ok_or(Error::Unknown("Invalid temp path".to_string()))?;
        let output_str_path = output_path
            .to_str()
            .ok_or(Error::Unknown("Invalid output path".to_string()))?;

    
        let args = if output_str.ends_with(".webm"){
            vec!["-i", temp, "-c:a", "copy", output_str_path]
        } else if output_str.ends_with(".m4a"){
            vec!["-i", temp, "-c:a", "aac", "-b:a", "192k", output_str_path]
        }else if output_str.ends_with(".mp3"){
            vec!["-i", temp, "-c:a", "libmp3lame", "-b:a", "192k", output_str_path]
        }else {
            return Err(Error::Unknown("Unsupported output format".into()));
        };

        let executor = Executor {
            executable_path: self.libraries.ffmpeg.clone(),
            timeout: self.timeout,
            args: utils::to_owned(args),
        };

        executor.execute().await?;

        // Clean up temporary file (logs error internally if tracing is enabled)
        let _ = utils::fs::remove_temp_file(temp_path).await;

        // Cache the processed audio file
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            #[cfg(feature = "tracing")]
            tracing::debug!("Caching format with ID: {}", best_audio.format_id);

            if let Err(_e) = download_cache
                .put_file(
                    &output_path,
                    output_str,
                    Some(video.id.clone()),
                    Some(best_audio),
                )
                .await
            {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache format: {}", _e);
            }
        }

        Ok(output_path)
    }

    /// Downloads a format.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `format` - The format to download.
    /// * `output` - The name of the file to save the format to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video could not be downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
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
    /// let video_format = video.best_video_format().unwrap();
    /// let format_path = fetcher.download_format(&video_format, "my-video-stream.mp4").await?;
    ///
    /// let audio_format = video.worst_audio_format().unwrap();
    /// let audio_path = fetcher.download_format(&audio_format, "my-audio-stream.mp3").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_format(
        &self,
        format: &Format,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading format {}", format.format_id);

        let output_path = self.output_dir.join(output.as_ref());

        // Use the internal function to download the format without preferences
        cfg_if::cfg_if! {
            if #[cfg(feature = "cache")] {
                self.download_format_internal(format, &output_path, None, None, None, None).await
            } else {
                self.download_format_internal(format, &output_path).await
            }
        }
    }

    /// Downloads a format with specific quality and codec preferences.
    ///
    /// This method allows fine-grained control over the download process by specifying
    /// quality and codec preferences for both video and audio components of the format.
    ///
    /// # Arguments
    ///
    /// * `format` - The format to download
    /// * `output` - The name of the output file
    /// * `video_quality` - Optional video quality preference
    /// * `audio_quality` - Optional audio quality preference
    /// * `video_codec` - Optional video codec preference
    /// * `audio_codec` - Optional audio codec preference
    ///
    /// # Returns
    ///
    /// * `PathBuf` - The path to the downloaded format
    ///
    /// # Errors
    ///
    /// This function will return an error if the video could not be downloaded.
    pub async fn download_format_with_preferences(
        &self,
        format: &Format,
        output: impl AsRef<str> + std::fmt::Debug + Display,
        #[cfg(feature = "cache")] video_quality: Option<VideoQuality>,
        #[cfg(feature = "cache")] audio_quality: Option<AudioQuality>,
        #[cfg(feature = "cache")] video_codec: Option<VideoCodecPreference>,
        #[cfg(feature = "cache")] audio_codec: Option<AudioCodecPreference>,
    ) -> crate::error::Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());

        // Use the internal function to download the format with preferences
        cfg_if::cfg_if! {
            if #[cfg(feature = "cache")] {
                self.download_format_internal(
                    format,
                    &output_path,
                    video_quality,
                    audio_quality,
                    video_codec,
                    audio_codec,
                )
                .await
            } else {
                self.download_format_internal(format, &output_path).await
            }
        }
    }

    /// Internal function that handles downloading a format with or without preferences
    ///
    /// This function avoids code duplication between download_format and download_format_with_preferences
    async fn download_format_internal(
        &self,
        format: &Format,
        path: &PathBuf,
        #[cfg(feature = "cache")] video_quality: Option<VideoQuality>,
        #[cfg(feature = "cache")] audio_quality: Option<AudioQuality>,
        #[cfg(feature = "cache")] video_codec: Option<VideoCodecPreference>,
        #[cfg(feature = "cache")] audio_codec: Option<AudioCodecPreference>,
    ) -> crate::error::Result<PathBuf> {
        // Check if we have specific preferences
        #[cfg(feature = "cache")]
        let has_preferences = video_quality.is_some()
            || audio_quality.is_some()
            || video_codec.is_some()
            || audio_codec.is_some();

        // Check if the format is in the cache
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache
            && let Some(video_id) = format.video_id.as_ref()
        {
            // First try to find by exact format ID
            if let Some((_, cached_path)) = download_cache
                .get_by_video_and_format(video_id, &format.format_id)
                .await
            {
                #[cfg(feature = "tracing")]
                tracing::debug!("Using cached format by ID: {}", format.format_id);

                // Copy the file from the cache to the output directory
                tokio::fs::copy(&cached_path, path).await?;
                return Ok(path.clone());
            }

            // Then try to find by preferences if they exist
            if has_preferences
                && let Some((_, cached_path)) = download_cache
                    .get_by_video_and_preferences(
                        video_id,
                        video_quality,
                        audio_quality,
                        video_codec.clone(),
                        audio_codec.clone(),
                    )
                    .await
            {
                #[cfg(feature = "tracing")]
                tracing::debug!("Using cached format by preferences");

                // Copy the file from the cache to the output directory
                tokio::fs::copy(&cached_path, path).await?;
                return Ok(path.clone());
            }
        }

        // Check if URL is available
        let url = format
            .download_info
            .url
            .clone()
            .ok_or_else(|| Error::FormatNoUrl {
                video_id: format
                    .video_id
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                format_id: format.format_id.clone(),
            })?;

        // Create an optimized fetcher with parallel downloading
        let fetcher = Fetcher::new(&url, self.proxy.as_ref())
            .with_parallel_segments(8) // Use 8 parallel segments
            .with_segment_size(1024 * 1024 * 5) // 5 MB per segment
            .with_retry_attempts(3); // 3 attempts in case of failure

        fetcher.fetch_asset(path.clone()).await?;

        // Don't add metadata for video or audio streams that will be combined later
        // Only add metadata for standalone formats that contain both
        // audio and video, or for audio-only formats intended for direct use
        self.add_metadata_if_needed(path, format).await?;

        // Cache the downloaded file if caching is enabled
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            let output_str = path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or_default()
                .to_string();

            #[cfg(feature = "tracing")]
            tracing::debug!("Caching format with ID: {}", format.format_id);

            // Use the appropriate function depending on whether we have preferences or not
            if has_preferences {
                if let Some(video_id) = format.video_id.as_ref()
                    && let Err(_e) = download_cache
                        .put_file_with_preferences(
                            path,
                            output_str,
                            Some(video_id.clone()),
                            Some(format),
                            video_quality,
                            audio_quality,
                            video_codec,
                            audio_codec,
                        )
                        .await
                {
                    #[cfg(feature = "tracing")]
                    tracing::warn!("Failed to cache format with preferences: {}", _e);
                }
            } else if let Err(_e) = download_cache
                .put_file(path, output_str, format.video_id.clone(), Some(format))
                .await
            {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache format: {}", _e);
            }
        }

        Ok(path.clone())
    }

    /// Adds format metadata based on the format type (audio-only, video-only, or both)
    /// This function is extracted to avoid code duplication
    async fn add_metadata_if_needed(
        &self,
        path: impl AsRef<Path>,
        format: &Format,
    ) -> crate::error::Result<()> {
        let format_type = format.format_type();
        let is_standalone_format = format_type.is_audio_and_video() || format_type.is_audio();

        if is_standalone_format {
            if let Some(video_id) = format.video_id.as_ref() {
                #[cfg(feature = "tracing")]
                tracing::debug!("Adding metadata to standalone format file");

                // Try to get video metadata from cache
                #[cfg(feature = "cache")]
                if let Some(cache) = &self.cache
                    && let Ok(cached_video) = cache.get_by_id(video_id).await
                    && let Ok(video) = cached_video.video()
                {
                    // Add metadata with format information
                    if let Err(_e) = crate::metadata::MetadataManager::add_metadata_with_format(
                        path.as_ref(),
                        &video,
                        None,
                        Some(format),
                    )
                    .await
                    {
                        #[cfg(feature = "tracing")]
                        tracing::warn!("Failed to add metadata: {}", _e);
                    }
                }

                #[cfg(not(feature = "cache"))]
                {
                    #[cfg(feature = "tracing")]
                    tracing::debug!("Cache feature disabled, cannot retrieve video metadata");
                    let _ = video_id; // Suppress unused warning
                }
            }
        } else {
            #[cfg(feature = "tracing")]
            tracing::debug!(
                "Skipping metadata for non-standalone format: will be added after combining"
            );
        }

        Ok(())
    }

    /// Retrieve a video by its ID, checking the cache first if available
    ///
    /// # Arguments
    ///
    /// * `video_id` - The ID of the video to find
    ///
    /// # Returns
    ///
    /// * `Option<Video>` - The video if found, None otherwise
    pub async fn get_video_by_id(&self, video_id: &str) -> Option<Video> {
        // First check if the video is in the cache
        #[cfg(feature = "cache")]
        if let Some(cache) = &self.cache
            && let Ok(cached_video) = cache.get_by_id(video_id).await
            && let Ok(video) = cached_video.video()
        {
            #[cfg(feature = "tracing")]
            tracing::debug!("Using cached video data for ID: {}", video_id);
            return Some(video);
        }

        // If not in cache, try to fetch it using the ID-based URL
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Video not found in cache, trying to fetch it using ID: {}",
            video_id
        );

        let url = format!("https://www.youtube.com/watch?v={}", video_id);

        self.fetch_video_infos(url).await.ok().or({
            #[cfg(feature = "tracing")]
            tracing::warn!("Failed to fetch video by ID: {}", video_id);
            None
        })
    }

    /// Lists all available subtitle languages for a video.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to get subtitle languages from
    ///
    /// # Returns
    ///
    /// A vector of language codes that have subtitles available
    pub fn list_subtitle_languages(&self, video: &Video) -> Vec<String> {
        video.subtitles.keys().cloned().collect()
    }

    /// Checks if a video has subtitles in a specific language.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to check
    /// * `language_code` - The language code to check for (e.g., "en", "fr")
    ///
    /// # Returns
    ///
    /// true if subtitles are available in the specified language
    pub fn has_subtitle_language(&self, video: &Video, language_code: &str) -> bool {
        video.subtitles.contains_key(language_code)
    }

    /// Downloads a subtitle file for a specific language.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download subtitles from
    /// * `language_code` - The language code of the subtitle (e.g., "en", "fr")
    /// * `output` - The output filename
    ///
    /// # Errors
    ///
    /// Returns an error if the subtitle language is not available or download fails
    ///
    /// # Returns
    ///
    /// The path to the downloaded subtitle file
    pub async fn download_subtitle(
        &self,
        video: &Video,
        language_code: impl AsRef<str>,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        let language_code = language_code.as_ref();

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Downloading subtitle for video {} in language {}",
            video.id,
            language_code
        );

        let output_path = self.output_dir.join(output.as_ref());

        // Check if subtitle is in the cache
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache
            && let Some((_, cached_path)) = download_cache
                .get_subtitle_by_language(&video.id, language_code)
                .await
        {
            #[cfg(feature = "tracing")]
            tracing::debug!(
                "Using cached subtitle for video {} in language {}",
                video.id,
                language_code
            );

            // Copy the file from the cache to the output directory
            tokio::fs::copy(&cached_path, &output_path).await?;
            return Ok(output_path);
        }

        // Get subtitles for the language
        let subtitles =
            video
                .subtitles
                .get(language_code)
                .ok_or_else(|| Error::SubtitleNotAvailable {
                    video_id: video.id.clone(),
                    language: language_code.to_string(),
                })?;

        // Prefer SRT format, then VTT, then any available format
        let subtitle = subtitles
            .iter()
            .find(|s| s.is_format(&crate::model::caption::Extension::Srt))
            .or_else(|| {
                subtitles
                    .iter()
                    .find(|s| s.is_format(&crate::model::caption::Extension::Vtt))
            })
            .or_else(|| subtitles.first())
            .ok_or_else(|| Error::SubtitleNotAvailable {
                video_id: video.id.clone(),
                language: language_code.to_string(),
            })?;

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Downloading subtitle from {} to {:?}",
            subtitle.url,
            output_path
        );

        // Download the subtitle file
        let fetcher = Fetcher::new(&subtitle.url, self.proxy.as_ref());
        fetcher.fetch_asset(&output_path).await?;

        // Cache the downloaded subtitle
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            #[cfg(feature = "tracing")]
            tracing::debug!(
                "Caching subtitle for video {} in language {}",
                video.id,
                language_code
            );

            if let Err(_e) = download_cache
                .put_subtitle_file(
                    &output_path,
                    output.as_ref(),
                    video.id.clone(),
                    language_code.to_string(),
                )
                .await
            {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache subtitle: {}", _e);
            }
        }

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Successfully downloaded subtitle for language {} to {:?}",
            language_code,
            output_path
        );

        Ok(output_path)
    }

    /// Downloads all available subtitles for a video.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download subtitles from
    /// * `output_dir` - The directory to save subtitle files
    ///
    /// # Errors
    ///
    /// Returns an error if any subtitle download fails
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded subtitle files
    pub async fn download_all_subtitles(
        &self,
        video: &Video,
        output_dir: impl AsRef<Path>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading all subtitles for video {}", video.id);

        let output_dir = output_dir.as_ref();
        let mut downloaded_files = Vec::new();

        for (language_code, subtitles) in &video.subtitles {
            if let Some(subtitle) = subtitles.first() {
                let filename = format!(
                    "{}.{}.{}",
                    video.id,
                    language_code,
                    subtitle.file_extension()
                );
                let output_path = output_dir.join(&filename);

                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Downloading subtitle for language {} from {}",
                    language_code,
                    subtitle.url
                );

                let fetcher = Fetcher::new(&subtitle.url, self.proxy.as_ref());
                fetcher.fetch_asset(&output_path).await?;
                downloaded_files.push(output_path);
            }
        }

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Successfully downloaded {} subtitle files",
            downloaded_files.len()
        );

        Ok(downloaded_files)
    }

    /// Embeds subtitle files into a video file using ffmpeg.
    ///
    /// # Arguments
    ///
    /// * `video_path` - The path to the video file
    /// * `subtitle_paths` - Paths to the subtitle files to embed
    /// * `output` - The output filename for the video with embedded subtitles
    ///
    /// # Errors
    ///
    /// Returns an error if ffmpeg execution fails
    ///
    /// # Returns
    ///
    /// The path to the output video file with embedded subtitles
    pub async fn embed_subtitles_in_video(
        &self,
        video_path: impl AsRef<Path>,
        subtitle_paths: &[PathBuf],
        output: impl AsRef<str>,
    ) -> crate::error::Result<PathBuf> {
        self.embed_subtitles_with_languages(video_path, subtitle_paths, &[], output)
            .await
    }

    /// Embeds subtitle files into a video file with language metadata using ffmpeg.
    ///
    /// # Arguments
    ///
    /// * `video_path` - The path to the video file
    /// * `subtitle_paths` - Paths to the subtitle files to embed
    /// * `language_codes` - Language codes for each subtitle (e.g., "en", "fr"). Must match subtitle_paths length.
    /// * `output` - The output filename for the video with embedded subtitles
    ///
    /// # Errors
    ///
    /// Returns an error if ffmpeg execution fails or if subtitle_paths and language_codes lengths don't match
    ///
    /// # Returns
    ///
    /// The path to the output video file with embedded subtitles
    pub async fn embed_subtitles_with_languages(
        &self,
        video_path: impl AsRef<Path>,
        subtitle_paths: &[PathBuf],
        language_codes: &[&str],
        output: impl AsRef<str>,
    ) -> crate::error::Result<PathBuf> {
        let video_path = video_path.as_ref();
        let output_path = self.output_dir.join(output.as_ref());

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Embedding {} subtitles into video {:?}",
            subtitle_paths.len(),
            video_path
        );

        // Build ffmpeg command
        let mut args = vec!["-i".to_string(), video_path.to_string_lossy().to_string()];

        // Add each subtitle file as input
        for subtitle_path in subtitle_paths {
            args.push("-i".to_string());
            args.push(subtitle_path.to_string_lossy().to_string());
        }

        // Map video and audio streams
        args.push("-map".to_string());
        args.push("0:v".to_string());
        args.push("-map".to_string());
        args.push("0:a".to_string());

        // Map subtitle streams
        for i in 0..subtitle_paths.len() {
            args.push("-map".to_string());
            args.push(format!("{}:s", i + 1));
        }

        // Add language metadata for each subtitle stream
        for (i, &language_code) in language_codes.iter().enumerate() {
            if i < subtitle_paths.len() {
                // Set language metadata for subtitle stream
                args.push(format!("-metadata:s:s:{}", i));
                args.push(format!("language={}", language_code));

                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Setting language {} for subtitle stream {}",
                    language_code,
                    i
                );
            }
        }

        // Copy codecs
        args.push("-c".to_string());
        args.push("copy".to_string());

        // Output file
        args.push(output_path.to_string_lossy().to_string());

        #[cfg(feature = "tracing")]
        tracing::debug!("Running ffmpeg with args: {:?}", args);

        let executor = Executor {
            executable_path: self.libraries.ffmpeg.clone(),
            timeout: self.timeout,
            args,
        };

        executor.execute().await?;

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Successfully embedded subtitles into video at {:?}",
            output_path
        );

        Ok(output_path)
    }

    /// Fetches playlist information from a URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the playlist to fetch
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist could not be fetched
    ///
    /// # Returns
    ///
    /// The playlist metadata
    pub async fn fetch_playlist_infos(&self, url: String) -> crate::error::Result<Playlist> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Fetching playlist information from {}", url);

        // Check if the playlist is in the cache
        #[cfg(feature = "cache")]
        if let Some(cache) = &self.playlist_cache
            && let Some(playlist) = cache.get(&url).await?
        {
            #[cfg(feature = "tracing")]
            tracing::debug!("Using cached playlist information for {}", url);
            return Ok(playlist);
        }

        // Use --flat-playlist to get just the playlist metadata without downloading videos
        let playlist_args = vec![
            "--flat-playlist",
            "--dump-single-json",
            "--no-progress",
            &url,
        ];

        let mut final_args = self.args.clone();
        final_args.append(&mut utils::to_owned(playlist_args));

        let executor = Executor {
            executable_path: self.libraries.youtube.clone(),
            timeout: self.timeout,
            args: final_args,
        };

        let output = executor.execute().await?;
        let mut playlist: Playlist =
            serde_json::from_str(&output.stdout).map_err(|e| Error::Json {
                context: "Failed to parse playlist metadata".to_string(),
                source: e,
            })?;

        // Store the URL in the playlist for caching purposes
        playlist.url = Some(url.clone());

        // Cache the playlist if caching is enabled
        #[cfg(feature = "cache")]
        if let Some(cache) = &self.playlist_cache {
            #[cfg(feature = "tracing")]
            tracing::debug!("Caching playlist information for {}", url);

            if let Err(_e) = cache.put(url.clone(), playlist.clone()).await {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache playlist information: {}", _e);
            }
        }

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Successfully fetched playlist {} with {} videos",
            playlist.id,
            playlist.entry_count()
        );

        Ok(playlist)
    }

    /// Downloads all videos from a playlist.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The playlist to download
    /// * `output_pattern` - The output filename pattern (use %(playlist_index)s, %(title)s placeholders)
    ///
    /// # Errors
    ///
    /// Returns an error if any video download fails
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded videos
    pub async fn download_playlist(
        &self,
        playlist: &Playlist,
        output_pattern: impl AsRef<str>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Downloading playlist {} with {} videos using parallel mode",
            playlist.id,
            playlist.entry_count()
        );

        // Use None to let download_playlist_parallel use its default concurrent limit
        // The limit is already configured in the download manager based on the speed profile
        let max_concurrent = None;

        // Use parallel download mode by default for better performance
        let results = self
            .download_playlist_parallel(playlist, output_pattern, max_concurrent)
            .await?;

        // Convert results to a simple Vec<PathBuf>, filtering out errors
        // and collecting only successful downloads
        let mut downloaded_files = Vec::new();
        let mut errors = Vec::new();

        for (_idx, result) in results.into_iter().enumerate() {
            match result {
                Ok(path) => downloaded_files.push(path),
                Err(e) => {
                    #[cfg(feature = "tracing")]
                    tracing::error!("Failed to download video at index {}: {}", _idx, e);
                    errors.push(e);
                }
            }
        }

        // If there were any errors, return the first one
        // (to maintain backward compatibility with the previous sequential behavior)
        if !errors.is_empty() && downloaded_files.is_empty() {
            return Err(errors.into_iter().next().unwrap());
        }

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Successfully downloaded {} out of {} videos from playlist {}",
            downloaded_files.len(),
            playlist.entry_count(),
            playlist.id
        );

        Ok(downloaded_files)
    }

    /// Downloads all videos from a playlist in parallel.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The playlist to download
    /// * `output_pattern` - The output filename pattern (use %(playlist_index)s, %(title)s placeholders)
    /// * `max_concurrent` - Maximum number of concurrent downloads (defaults to 3)
    ///
    /// # Errors
    ///
    /// Returns an error containing all failed downloads if any occur
    ///
    /// # Returns
    ///
    /// A vector of results for each video download
    pub async fn download_playlist_parallel(
        &self,
        playlist: &Playlist,
        output_pattern: impl AsRef<str>,
        max_concurrent: Option<usize>,
    ) -> crate::error::Result<Vec<crate::error::Result<PathBuf>>> {
        self.download_playlist_parallel_with_progress::<fn(PlaylistDownloadProgress)>(
            playlist,
            output_pattern,
            max_concurrent,
            None,
        )
        .await
    }

    /// Downloads all videos from a playlist in parallel with progress tracking.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The playlist to download
    /// * `output_pattern` - The output filename pattern (use %(playlist_index)s, %(title)s placeholders)
    /// * `max_concurrent` - Maximum number of concurrent downloads (defaults to 3)
    /// * `progress_callback` - Optional callback to receive progress updates
    ///
    /// # Errors
    ///
    /// Returns an error containing all failed downloads if any occur
    ///
    /// # Returns
    ///
    /// A vector of results for each video download
    pub async fn download_playlist_parallel_with_progress<F>(
        &self,
        playlist: &Playlist,
        output_pattern: impl AsRef<str>,
        max_concurrent: Option<usize>,
        progress_callback: Option<F>,
    ) -> crate::error::Result<Vec<crate::error::Result<PathBuf>>>
    where
        F: Fn(PlaylistDownloadProgress) + Send + Sync + 'static,
    {
        use futures_util::stream::{FuturesUnordered, StreamExt};

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Downloading playlist {} with {} videos in parallel (max {} concurrent)",
            playlist.id,
            playlist.entry_count(),
            max_concurrent.unwrap_or(3)
        );

        let max_concurrent = max_concurrent.unwrap_or(3);
        let total_videos = playlist.entry_count();
        let mut completed = 0usize;
        let mut results = Vec::new();
        let mut tasks = FuturesUnordered::new();
        let mut entry_iter = playlist.entries.iter().peekable();

        let output_pattern = output_pattern.as_ref().to_string();
        let progress_callback = progress_callback.map(Arc::new);

        loop {
            // Spawn tasks up to max_concurrent limit
            while tasks.len() < max_concurrent {
                if let Some(entry) = entry_iter.next() {
                    if !entry.is_available() {
                        #[cfg(feature = "tracing")]
                        tracing::warn!(
                            "Skipping unavailable video: {} ({})",
                            entry.title,
                            entry.id
                        );

                        let entry_clone = entry.clone();
                        completed += 1;

                        // Call progress callback for unavailable video
                        if let Some(callback) = &progress_callback {
                            callback(PlaylistDownloadProgress {
                                entry: entry_clone.clone(),
                                result: Err(format!("Video {} is not available", entry_clone.id)),
                                completed,
                                total: total_videos,
                            });
                        }

                        results.push(Err(Error::Unknown(format!(
                            "Video {} is not available",
                            entry.id
                        ))));
                        continue;
                    }

                    let entry = entry.clone();
                    let output_pattern = output_pattern.clone();
                    let youtube = self.clone();
                    let _callback = progress_callback.clone();

                    let task = tokio::spawn(async move {
                        #[cfg(feature = "tracing")]
                        tracing::debug!(
                            "Downloading video {} from playlist (index: {})",
                            entry.id,
                            entry.index.unwrap_or(0)
                        );

                        // Fetch full video info
                        let video_result = youtube.fetch_video_infos(entry.url.clone()).await;
                        let video = match video_result {
                            Ok(v) => v,
                            Err(e) => return (entry, Err(e)),
                        };

                        // Generate filename from pattern
                        let filename = output_pattern
                            .replace("%(playlist_index)s", &entry.index.unwrap_or(0).to_string())
                            .replace("%(title)s", &entry.title)
                            .replace("%(id)s", &entry.id);

                        // Download the video
                        let download_result = youtube.download_video(&video, &filename).await;

                        #[cfg(feature = "tracing")]
                        if download_result.is_ok() {
                            tracing::info!(
                                "Downloaded video from playlist: {} (index: {})",
                                entry.title,
                                entry.index.unwrap_or(0)
                            );
                        }

                        (entry, download_result)
                    });

                    tasks.push(task);
                } else {
                    // No more entries to spawn
                    break;
                }
            }

            // If no tasks running and no more entries, we're done
            if tasks.is_empty() {
                break;
            }

            // Wait for next task to complete
            if let Some(result) = tasks.next().await {
                completed += 1;

                match result {
                    Ok((entry, download_result)) => {
                        // Call progress callback
                        if let Some(callback) = &progress_callback {
                            let result_for_progress = download_result
                                .as_ref()
                                .map(|p| p.clone())
                                .map_err(|e| e.to_string());

                            callback(PlaylistDownloadProgress {
                                entry,
                                result: result_for_progress,
                                completed,
                                total: total_videos,
                            });
                        }

                        results.push(download_result);
                    }
                    Err(e) => {
                        results.push(Err(Error::Unknown(format!("Task join error: {}", e))));
                    }
                }
            }
        }

        #[cfg(feature = "tracing")]
        {
            let successful = results.iter().filter(|r| r.is_ok()).count();
            tracing::info!(
                "Downloaded {}/{} videos from playlist {} in parallel",
                successful,
                playlist.entry_count(),
                playlist.id
            );
        }

        Ok(results)
    }

    /// Downloads specific videos from a playlist by their indices.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The playlist to download from
    /// * `indices` - The indices of videos to download (0-based)
    /// * `output_pattern` - The output filename pattern
    ///
    /// # Errors
    ///
    /// Returns an error if any video download fails
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded videos
    pub async fn download_playlist_items(
        &self,
        playlist: &Playlist,
        indices: &[usize],
        output_pattern: impl AsRef<str>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Downloading {} specific videos from playlist {}",
            indices.len(),
            playlist.id
        );

        let mut downloaded_files = Vec::new();

        for &index in indices {
            if let Some(entry) = playlist.get_entry_by_index(index) {
                if !entry.is_available() {
                    #[cfg(feature = "tracing")]
                    tracing::warn!(
                        "Skipping unavailable video at index {}: {}",
                        index,
                        entry.title
                    );
                    continue;
                }

                // Fetch full video info
                let video = self.fetch_video_infos(entry.url.clone()).await?;

                // Generate filename from pattern
                let filename = output_pattern
                    .as_ref()
                    .replace("%(playlist_index)s", &index.to_string())
                    .replace("%(title)s", &entry.title)
                    .replace("%(id)s", &entry.id);

                // Download the video
                let video_path = self.download_video(&video, &filename).await?;
                downloaded_files.push(video_path);

                #[cfg(feature = "tracing")]
                tracing::info!("Downloaded video at index {}: {}", index, entry.title);
            } else {
                #[cfg(feature = "tracing")]
                tracing::warn!("Index {} is out of bounds for playlist", index);
            }
        }

        Ok(downloaded_files)
    }

    /// Downloads a range of videos from a playlist.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The playlist to download from
    /// * `start` - The starting index (0-based, inclusive)
    /// * `end` - The ending index (0-based, inclusive)
    /// * `output_pattern` - The output filename pattern
    ///
    /// # Errors
    ///
    /// Returns an error if any video download fails
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded videos
    pub async fn download_playlist_range(
        &self,
        playlist: &Playlist,
        start: usize,
        end: usize,
        output_pattern: impl AsRef<str>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Downloading videos {}-{} from playlist {}",
            start,
            end,
            playlist.id
        );

        let entries = playlist.get_entries_in_range(start, end);
        let mut downloaded_files = Vec::new();

        for entry in entries {
            if !entry.is_available() {
                #[cfg(feature = "tracing")]
                tracing::warn!("Skipping unavailable video: {} ({})", entry.title, entry.id);
                continue;
            }

            // Fetch full video info
            let video = self.fetch_video_infos(entry.url.clone()).await?;

            // Generate filename from pattern
            let filename = output_pattern
                .as_ref()
                .replace("%(playlist_index)s", &entry.index.unwrap_or(0).to_string())
                .replace("%(title)s", &entry.title)
                .replace("%(id)s", &entry.id);

            // Download the video
            let video_path = self.download_video(&video, &filename).await?;
            downloaded_files.push(video_path);

            #[cfg(feature = "tracing")]
            tracing::info!("Downloaded video: {}", entry.title);
        }

        Ok(downloaded_files)
    }

    /// Downloads a partial range of a video using hybrid approach (yt-dlp with ffmpeg fallback).
    ///
    /// This method first attempts to use yt-dlp's --download-sections feature.
    /// If that fails, it falls back to downloading the full video and extracting
    /// the desired segment using ffmpeg.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download
    /// * `range` - The partial range to download (time or chapter based)
    /// * `output` - The output filename
    ///
    /// # Errors
    ///
    /// Returns an error if both yt-dlp and ffmpeg approaches fail
    ///
    /// # Returns
    ///
    /// The path to the downloaded partial video file
    pub async fn download_video_partial(
        &self,
        video: &crate::model::Video,
        range: &crate::download::partial::PartialRange,
        output: impl AsRef<str>,
    ) -> crate::error::Result<PathBuf> {
        // Convert chapter ranges to time ranges if needed
        let time_range = if range.needs_chapter_metadata() {
            if !video.chapters.is_empty() {
                range
                    .to_time_range(&video.chapters)
                    .ok_or_else(|| Error::Unknown("Chapter index out of bounds".to_string()))?
            } else {
                return Err(Error::Unknown(
                    "Video does not have chapter information".to_string(),
                ));
            }
        } else {
            range.clone()
        };

        let output_path = self.output_dir.join(output.as_ref());

        // Try yt-dlp approach first
        match self
            .try_download_partial_ytdlp(video, &time_range, &output_path)
            .await
        {
            Ok(path) => {
                #[cfg(feature = "tracing")]
                tracing::info!("Successfully downloaded partial video using yt-dlp");
                Ok(path)
            }
            Err(_e) => {
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    "yt-dlp partial download failed: {}, trying ffmpeg fallback",
                    _e
                );

                // Fallback to ffmpeg approach
                self.download_partial_ffmpeg(video, &time_range, &output_path)
                    .await
            }
        }
    }

    /// Attempts to download a partial video using yt-dlp's --download-sections.
    async fn try_download_partial_ytdlp(
        &self,
        video: &crate::model::Video,
        range: &crate::download::partial::PartialRange,
        output_path: &Path,
    ) -> crate::error::Result<PathBuf> {
        let output_str = output_path.to_str().ok_or_else(|| Error::PathValidation {
            path: output_path.to_path_buf(),
            reason: "Invalid UTF-8 in path".to_string(),
        })?;

        let download_sections_arg = range.to_ytdlp_arg();
        let video_url = format!("https://www.youtube.com/watch?v={}", video.id);

        let download_args = vec![
            "--no-progress",
            "--download-sections",
            &download_sections_arg,
            "-o",
            output_str,
            &video_url,
        ];

        let mut final_args = self.args.clone();
        final_args.append(&mut utils::to_owned(download_args));

        let executor = Executor {
            executable_path: self.libraries.youtube.clone(),
            timeout: self.timeout,
            args: final_args,
        };

        executor.execute().await?;
        Ok(output_path.to_path_buf())
    }

    /// Downloads full video and extracts partial range using ffmpeg.
    async fn download_partial_ffmpeg(
        &self,
        video: &crate::model::Video,
        range: &crate::download::partial::PartialRange,
        output_path: &Path,
    ) -> crate::error::Result<PathBuf> {
        // Get time range
        let (start_time, end_time) = range
            .get_times()
            .ok_or_else(|| Error::Unknown("Cannot extract times from range".to_string()))?;

        // Download full video to temporary file
        let temp_filename = format!("temp_full_{}.mp4", crate::utils::fs::random_filename(8));
        let temp_path = self.download_video(video, &temp_filename).await?;

        // Extract segment using ffmpeg
        let output_str = output_path.to_str().ok_or_else(|| Error::PathValidation {
            path: output_path.to_path_buf(),
            reason: "Invalid UTF-8 in path".to_string(),
        })?;

        let temp_str = temp_path.to_str().ok_or_else(|| Error::PathValidation {
            path: temp_path.clone(),
            reason: "Invalid UTF-8 in path".to_string(),
        })?;

        let start_str = format!("{:.3}", start_time);
        let duration = end_time - start_time;
        let duration_str = format!("{:.3}", duration);

        let args = vec![
            "-i",
            temp_str,
            "-ss",
            &start_str,
            "-t",
            &duration_str,
            "-c",
            "copy",
            "-avoid_negative_ts",
            "1",
            output_str,
        ];

        let executor = Executor {
            executable_path: self.libraries.ffmpeg.clone(),
            timeout: self.timeout,
            args: utils::to_owned(args),
        };

        executor.execute().await?;

        // Clean up temporary file
        tokio::fs::remove_file(&temp_path).await.ok();

        Ok(output_path.to_path_buf())
    }
}
