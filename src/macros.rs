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
/// # async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
/// let yt = youtube!("libs/yt-dlp", "libs/ffmpeg", "output").await?;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! youtube {
    ($yt_dlp:expr, $ffmpeg:expr, $output:expr) => {{
        let libraries =
            $crate::client::Libraries::new(std::path::PathBuf::from($yt_dlp), std::path::PathBuf::from($ffmpeg));
        $crate::Downloader::builder(libraries, $output).build()
    }};

    ($yt_dlp:expr, $ffmpeg:expr, $output:expr, cache: $cache:expr) => {{
        let libraries =
            $crate::client::Libraries::new(std::path::PathBuf::from($yt_dlp), std::path::PathBuf::from($ffmpeg));
        $crate::Downloader::builder(libraries, $output)
            .with_cache($cache)
            .build()
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
/// let libs = install_libraries!("libs")?;
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
        libraries.install_dependencies().await?;

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
        libraries.install_dependencies_with_token($token).await?;

        Ok::<Libraries, $crate::error::Error>(libraries)
    }};
}

/// A macro to mimic the ternary operator in Rust.
#[macro_export]
macro_rules! ternary {
    ($condition:expr, $true:expr, $false:expr) => {
        if $condition { $true } else { $false }
    };
}
