//! The fetchers for required dependencies.

use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;

use derive_more::Constructor;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::client::deps::ffmpeg::BuildFetcher;
use crate::client::deps::ytdlp::YoutubeFetcher;
use crate::download::Fetcher;
use crate::error::Result;
use crate::utils::fs;
use crate::{ternary, utils};

pub mod ffmpeg;
pub mod github;
pub mod ytdlp;

/// Installs required libraries.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::client::deps::LibraryInstaller;
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

impl fmt::Display for LibraryInstaller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LibraryInstaller(destination={})", self.destination.display())
    }
}

/// The installed libraries.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::client::deps::Libraries;
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

impl fmt::Display for Libraries {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Libraries(youtube={}, ffmpeg={})",
            self.youtube.display(),
            self.ffmpeg.display()
        )
    }
}

impl LibraryInstaller {
    /// Install yt-dlp from the main repository.
    ///
    /// # Arguments
    ///
    /// * `custom_name` - Optional custom name for the executable.
    pub async fn install_youtube(&self, custom_name: Option<String>) -> Result<PathBuf> {
        self.install_youtube_from_repo("yt-dlp", "yt-dlp", None, custom_name)
            .await
    }

    /// Install yt-dlp from a custom repository, assuming releases assets are named correctly.
    ///
    /// # Arguments
    ///
    /// * `owner` - The owner of the repository.
    /// * `repo` - The name of the repository.
    /// * `auth_token` - Optional GitHub token to avoid rate limits.
    /// * `custom_name` - Optional custom name for the executable.
    pub async fn install_youtube_from_repo(
        &self,
        owner: impl Into<String>,
        repo: impl Into<String>,
        auth_token: Option<String>,
        custom_name: Option<String>,
    ) -> Result<PathBuf> {
        let owner: String = owner.into();
        let repo: String = repo.into();

        tracing::debug!(
            owner = %owner,
            repo = %repo,
            custom_name = ?custom_name,
            destination = ?self.destination,
            "📦 Installing yt-dlp from repository"
        );

        fs::create_dir(self.destination.clone()).await?;

        let fetcher = YoutubeFetcher::new(owner, repo);

        let name = custom_name.unwrap_or(String::from("yt-dlp"));
        let path = self.destination.join(utils::find_executable(&name));

        let release = fetcher.fetch_release(auth_token).await?;
        release.download(path.clone()).await?;
        fs::set_executable(path.clone()).await?;

        Ok(path)
    }

    /// Install ffmpeg from static builds.
    ///
    /// # Arguments
    ///
    /// * `custom_name` - Optional custom name for the executable.
    pub async fn install_ffmpeg(&self, custom_name: Option<String>) -> Result<PathBuf> {
        tracing::debug!(
            custom_name = ?custom_name,
            destination = ?self.destination,
            "📦 Installing ffmpeg from static builds"
        );

        fs::create_dir(self.destination.clone()).await?;

        let fetcher = BuildFetcher::new();
        let archive = self.destination.join("ffmpeg-release.zip");

        let release = fetcher.fetch_binary().await?;
        release.download(archive.clone()).await?;
        let path = fetcher.extract_binary(archive).await?;

        if let Some(name) = custom_name {
            let new_path = self.destination.join(utils::find_executable(&name));
            tokio::fs::rename(&path, &new_path).await?;

            return Ok(new_path);
        }

        Ok(path)
    }
}

impl Libraries {
    /// Install the required dependencies.
    pub async fn install_dependencies(&self) -> Result<Self> {
        tracing::info!(
            youtube_path = ?self.youtube,
            ffmpeg_path = ?self.ffmpeg,
            "📦 Installing required dependencies"
        );

        let (youtube, ffmpeg) = tokio::join!(self.install_youtube(), self.install_ffmpeg());

        Ok(Self::new(youtube?, ffmpeg?))
    }

    /// Install the required dependencies with an authentication token.
    ///
    /// # Arguments
    ///
    /// * `auth_token` - The authentication token to use for downloading the dependencies.
    pub async fn install_dependencies_with_token(&self, auth_token: impl Into<String>) -> Result<Self> {
        tracing::info!(
            youtube_path = ?self.youtube,
            ffmpeg_path = ?self.ffmpeg,
            has_token = true,
            "📦 Installing required dependencies with authentication token"
        );

        let token = auth_token.into();
        let youtube = self.install_youtube_with_token(token.clone()).await?;
        let ffmpeg = self.install_ffmpeg_with_token(token).await?;

        Ok(Self::new(youtube, ffmpeg))
    }

    /// Install yt-dlp.
    pub async fn install_youtube(&self) -> Result<PathBuf> {
        self.install_youtube_internal(None).await
    }

    /// Install yt-dlp with an authentication token.
    pub async fn install_youtube_with_token(&self, auth_token: impl Into<String>) -> Result<PathBuf> {
        self.install_youtube_internal(Some(auth_token.into())).await
    }

    async fn install_youtube_internal(&self, auth_token: Option<String>) -> Result<PathBuf> {
        tracing::debug!(
            youtube_path = ?self.youtube,
            has_token = auth_token.is_some(),
            "📦 Installing yt-dlp binary"
        );

        let parent = fs::try_parent(self.youtube.clone())?;
        let installer = LibraryInstaller::new(parent);

        if self.youtube.exists() {
            return Ok(self.youtube.clone());
        }

        let name = utils::find_executable("yt-dlp");
        let file_name = fs::try_name(self.youtube.clone())?;

        let custom_name = ternary!(file_name == name, None, Some(file_name));
        installer
            .install_youtube_from_repo("yt-dlp", "yt-dlp", auth_token, custom_name)
            .await
    }

    /// Install ffmpeg.
    pub async fn install_ffmpeg(&self) -> Result<PathBuf> {
        self.install_ffmpeg_internal(None).await
    }

    /// Install ffmpeg with an authentication token.
    pub async fn install_ffmpeg_with_token(&self, auth_token: impl Into<String>) -> Result<PathBuf> {
        self.install_ffmpeg_internal(Some(auth_token.into())).await
    }

    async fn install_ffmpeg_internal(&self, _auth_token: Option<String>) -> Result<PathBuf> {
        tracing::debug!(
            ffmpeg_path = ?self.ffmpeg,
            "📦 Installing ffmpeg binary"
        );

        let parent = fs::try_parent(self.ffmpeg.clone())?;
        let installer = LibraryInstaller::new(parent);

        if self.ffmpeg.exists() {
            return Ok(self.ffmpeg.clone());
        }

        let name = utils::find_executable("ffmpeg");
        let file_name = fs::try_name(self.ffmpeg.clone())?;

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
        write!(f, "Release(tag={}, assets={})", self.tag_name, self.assets.len())
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
    /// The digest of the asset (if available via API, e.g. sha256:...).
    pub digest: Option<String>,
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Asset(name={}, url={})", self.name, self.download_url)
    }
}

/// A release that has been selected for the current platform.
#[derive(Debug)]
pub struct WantedRelease {
    /// The URL of the release asset.
    pub url: String,
    /// The name of the release asset.
    pub name: String,
    /// The expected SHA256 checksum of the asset.
    pub checksum: Option<String>,
}

impl fmt::Display for WantedRelease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WantedRelease(asset={}, url={}, checksum={})",
            self.name,
            self.url,
            self.checksum.as_deref().unwrap_or("none")
        )
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
    /// This function will return an error if the asset could not be downloaded, written to the destination,
    /// or if the checksum verification fails.
    pub async fn download(&self, destination: impl Into<PathBuf>) -> Result<()> {
        let destination: PathBuf = destination.into();
        tracing::debug!(
            url = %self.url,
            destination = ?destination,
            asset_name = %self.name,
            has_checksum = self.checksum.is_some(),
            "📦 Downloading release asset"
        );

        let fetcher = Fetcher::new(&self.url, None, None)?;
        fetcher.fetch_asset(destination.clone()).await?;

        if let Some(expected_checksum) = &self.checksum {
            tracing::debug!(
                destination = ?destination,
                expected_checksum = %expected_checksum,
                "⚙️ Verifying asset checksum"
            );

            let dest_path = destination.clone();
            let actual_checksum = tokio::task::spawn_blocking(move || {
                let file = File::open(&dest_path)
                    .map_err(|e| crate::error::Error::io_with_path("open file for checksum", dest_path.clone(), e))?;
                let mut reader = BufReader::new(file);
                let mut hasher = Sha256::new();
                let mut buffer = [0; 8192];

                loop {
                    let count = reader.read(&mut buffer).map_err(|e| {
                        crate::error::Error::io_with_path("read file for checksum", dest_path.clone(), e)
                    })?;
                    if count == 0 {
                        break;
                    }
                    hasher.update(&buffer[..count]);
                }

                let result = hasher.finalize();
                Ok::<_, crate::error::Error>(result.iter().fold(String::new(), |mut acc, b| {
                    use std::fmt::Write;
                    let _ = write!(acc, "{:02x}", b);
                    acc
                }))
            })
            .await
            .map_err(|e| crate::error::Error::runtime("checksum computation", e))??;

            if actual_checksum != *expected_checksum {
                // Delete the invalid file
                let _ = tokio::fs::remove_file(&destination).await;
                return Err(crate::error::Error::ChecksumMismatch {
                    path: destination.clone(),
                    expected: expected_checksum.to_string(),
                    actual: actual_checksum.clone(),
                });
            }

            tracing::debug!(
                expected = %expected_checksum,
                actual = %actual_checksum,
                "✅ Checksum verification passed"
            );
        }

        Ok(())
    }
}
