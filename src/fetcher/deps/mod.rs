//! The fetchers for required dependencies.

use crate::error::Result;
use crate::fetcher::Fetcher;
use crate::fetcher::deps::ffmpeg::BuildFetcher;
use crate::fetcher::deps::youtube::GitHubFetcher;
use crate::utils::file_system;
use crate::{ternary, utils};
use derive_more::Constructor;
use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

pub mod ffmpeg;
pub mod youtube;

/// Installs required libraries.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::fetcher::deps::LibraryInstaller;
/// # use std::path::PathBuf;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let destination = PathBuf::from("libs");
/// let installer = LibraryInstaller::new(destination);
///
/// let youtube = installer.install_youtube(None).await.unwrap();
/// let ffmpeg = installer.install_ffmpeg(None).await.unwrap();
/// # Ok(())
/// # }
/// ```
#[derive(Constructor, Clone, Debug)]
pub struct LibraryInstaller {
    /// The destination directory for the libraries.
    pub destination: PathBuf,
}

/// The installed libraries.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::fetcher::deps::Libraries;
/// # use std::path::PathBuf;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let destination = PathBuf::from("libs");
///
/// let youtube = destination.join("yt-dlp");
/// let ffmpeg = destination.join("ffmpeg");
///
/// let libraries = Libraries::new(youtube, ffmpeg);
/// # Ok(())
/// # }
/// ```
#[derive(Constructor, Clone, Debug)]
pub struct Libraries {
    /// The path to the installed yt-dlp binary.
    pub youtube: PathBuf,
    /// The path to the installed ffmpeg binary.
    pub ffmpeg: PathBuf,
}

impl LibraryInstaller {
    /// Install yt-dlp from the main repository.
    pub async fn install_youtube(&self, custom_name: Option<String>) -> Result<PathBuf> {
        self.install_youtube_from_repo("yt-dlp", "yt-dlp", None, custom_name)
            .await
    }

    /// Install yt-dlp from a custom repository, assuming releases assets are named correctly.
    pub async fn install_youtube_from_repo(
        &self,
        owner: impl AsRef<str> + std::fmt::Debug + std::fmt::Display,
        repo: impl AsRef<str> + std::fmt::Debug + std::fmt::Display,
        auth_token: Option<String>,
        custom_name: Option<String>,
    ) -> Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Installing yt-dlp from {}/{}, with custom executable name: {:?}",
            owner,
            repo,
            custom_name
        );

        file_system::create_dir(self.destination.clone())?;

        let fetcher = GitHubFetcher::new(owner, repo);

        let name = custom_name.unwrap_or(String::from("yt-dlp"));
        let path = self.destination.join(utils::find_executable(&name));

        let release = fetcher.fetch_release(auth_token).await?;
        release.download(path.clone()).await?;

        Ok(path)
    }

    /// Install ffmpeg from static builds.
    pub async fn install_ffmpeg(&self, custom_name: Option<String>) -> Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Installing ffmpeg with custom executable name: {:?}",
            custom_name
        );

        file_system::create_dir(self.destination.clone())?;

        let fetcher = BuildFetcher::new();
        let archive = self.destination.join("ffmpeg-release.zip");

        let release = fetcher.fetch_binary().await?;
        release.download(archive.clone()).await?;
        let path = fetcher.extract_binary(archive).await?;

        if let Some(name) = custom_name {
            let new_path = self.destination.join(utils::find_executable(&name));
            std::fs::rename(&path, &new_path)?;

            return Ok(new_path);
        }

        Ok(path)
    }
}

impl Libraries {
    /// Install the required dependencies.
    pub async fn install_dependencies(&self) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Installing required dependencies");

        let youtube = self.install_youtube().await?;
        let ffmpeg = self.install_ffmpeg().await?;

        Ok(Self::new(youtube, ffmpeg))
    }

    /// Install yt-dlp.
    pub async fn install_youtube(&self) -> Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Installing yt-dlp");

        let parent = file_system::try_parent(self.youtube.clone())?;
        let installer = LibraryInstaller::new(parent);

        if self.youtube.exists() {
            return Ok(self.youtube.clone());
        }

        let name = utils::find_executable("yt-dlp");
        let file_name = file_system::try_name(self.youtube.clone())?;

        let custom_name = ternary!(file_name == name, None, Some(file_name));
        installer.install_youtube(custom_name).await
    }

    /// Install ffmpeg.
    pub async fn install_ffmpeg(&self) -> Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Installing ffmpeg");

        let parent = file_system::try_parent(self.ffmpeg.clone())?;
        let installer = LibraryInstaller::new(parent);

        if self.ffmpeg.exists() {
            return Ok(self.ffmpeg.clone());
        }

        let name = utils::find_executable("ffmpeg");
        let file_name = file_system::try_name(self.ffmpeg.clone())?;

        let custom_name = ternary!(file_name == name, None, Some(file_name));
        installer.install_ffmpeg(custom_name).await
    }
}

/// A GitHub release.
#[derive(Debug, Deserialize)]
pub struct Release {
    /// The tag name of the release.
    pub tag_name: String,
    /// The assets of the release.
    pub assets: Vec<Asset>,
}

impl fmt::Display for Release {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Release: tag={}, assets={};",
            self.tag_name,
            self.assets.len()
        )
    }
}

/// A release asset.
#[derive(Debug, Deserialize)]
pub struct Asset {
    /// The name of the asset.
    pub name: String,
    /// The download URL of the asset.
    #[serde(rename = "browser_download_url")]
    pub download_url: String,
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Asset: name={}, url={};", self.name, self.download_url)
    }
}

/// A release that has been selected for the current platform.
#[derive(Debug)]
pub struct WantedRelease {
    /// The URL of the release asset.
    pub url: String,
    /// The name of the release asset.
    pub name: String,
}

impl fmt::Display for WantedRelease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WantedRelease: asset={}, url={};", self.name, self.url)
    }
}

impl WantedRelease {
    /// Download the release asset to the given destination.
    ///
    /// # Arguments
    ///
    /// * `destination` - The path to write the asset to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the asset could not be downloaded or written to the destination.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::fetcher::deps::WantedRelease;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let release = WantedRelease {
    ///     asset_name: "yt-dlp".to_string(),
    ///     asset_url: "https://github.com/yt-dlp/yt-dlp/releases/download/2024.10.22/yt-dlp".to_string(),
    /// };
    ///
    /// let destination = PathBuf::from("yt-dlp");
    /// release.download(destination).await?;
    /// # Ok(())
    /// # }
    pub async fn download(&self, destination: impl AsRef<Path> + std::fmt::Debug) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Downloading asset from {} to {}",
            self.url,
            destination.as_ref().display()
        );

        let fetcher = Fetcher::new(&self.url);
        fetcher.fetch_asset(destination).await
    }
}
