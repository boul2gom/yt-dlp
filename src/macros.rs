//! Convenience macros for common operations.
//!
//! This module provides macros that simplify common tasks when working with yt-dlp.

/// Create a Youtube instance with sensible defaults.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::youtube;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let yt = youtube!("libs/yt-dlp", "libs/ffmpeg", "output").await?;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! youtube {
    ($yt_dlp:expr, $ffmpeg:expr, $output:expr) => {{
        let libraries = $crate::client::Libraries::new($yt_dlp, $ffmpeg);
        $crate::Youtube::builder(libraries, $output).build()
    }};

    ($yt_dlp:expr, $ffmpeg:expr, $output:expr, cache: $cache:expr) => {{
        let libraries = $crate::client::Libraries::new($yt_dlp, $ffmpeg);
        $crate::Youtube::builder(libraries, $output)
            .with_cache($cache)
            .build()
    }};
}

/// Quick video download macro.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::{download_video, prelude::*};
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let yt = youtube!("libs/yt-dlp", "libs/ffmpeg", "output").await?;
/// download_video!(yt, "https://youtube.com/watch?v=dQw4w9WgXcQ", "video.mp4").await?;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! download_video {
    ($yt:expr, $url:expr, $output:expr) => {{
        let video = $yt.fetch_video_infos($url).await?;
        $yt.download_video(&video, $output).await
    }};

    ($yt:expr, $url:expr, $output:expr, quality: $quality:expr) => {{
        $yt.download_video_with_quality(
            $url,
            $output,
            $quality,
            $crate::model::selector::VideoCodecPreference::Any,
            $crate::model::selector::AudioQuality::Best,
            $crate::model::selector::AudioCodecPreference::Any,
        )
        .await
    }};
}

/// Quick audio download macro.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::{download_audio, prelude::*};
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let yt = youtube!("libs/yt-dlp", "libs/ffmpeg", "output").await?;
/// download_audio!(yt, "https://youtube.com/watch?v=dQw4w9WgXcQ", "audio.m4a").await?;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! download_audio {
    ($yt:expr, $url:expr, $output:expr) => {{
        let video = $yt.fetch_video_infos($url).await?;
        $yt.download_audio(&video, $output).await
    }};

    ($yt:expr, $url:expr, $output:expr, quality: $quality:expr) => {{
        $yt.download_audio_stream_with_quality(
            $url,
            $output,
            $quality,
            $crate::model::selector::AudioCodecPreference::Any,
        )
        .await
    }};
}

/// Configure yt-dlp arguments easily.
///
/// # Examples
///
/// ```rust,ignore
/// let args = ytdlp_args![
///     "--no-playlist",
///     "--extract-audio",
///     format: "bestvideo+bestaudio"
/// ];
/// ```
#[macro_export]
macro_rules! ytdlp_args {
    ($($arg:expr),* $(,)?) => {{
        vec![$($arg.to_string()),*]
    }};

    ($($key:ident: $value:expr),* $(,)?) => {{
        vec![$(format!("--{}={}", stringify!($key).replace('_', "-"), $value)),*]
    }};
}

/// Create a Libraries instance with automatic binary installation.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::install_libraries;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let libs = install_libraries!("libs").await?;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! install_libraries {
    ($dir:expr) => {{
        use std::path::PathBuf;
        use $crate::client::Libraries;
        use $crate::client::deps::LibraryInstaller;

        let dir = PathBuf::from($dir);
        let yt_dlp = dir.join("yt-dlp");
        let ffmpeg = dir.join("ffmpeg");

        let libraries = Libraries::new(yt_dlp, ffmpeg);
        libraries.install(None).await?;

        Ok::<Libraries, $crate::error::Error>(libraries)
    }};

    ($dir:expr, token: $token:expr) => {{
        use std::path::PathBuf;
        use $crate::client::Libraries;
        use $crate::client::deps::LibraryInstaller;

        let dir = PathBuf::from($dir);
        let yt_dlp = dir.join("yt-dlp");
        let ffmpeg = dir.join("ffmpeg");

        let libraries = Libraries::new(yt_dlp, ffmpeg);
        libraries.install(Some($token)).await?;

        Ok::<Libraries, $crate::error::Error>(libraries)
    }};
}
