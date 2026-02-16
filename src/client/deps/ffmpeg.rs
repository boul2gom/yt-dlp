//! Fetch the latest release of 'ffmpeg' from static builds.

use crate::client::deps::WantedRelease;
use crate::client::deps::github::GitHubFetcher;
use crate::error::{Error, Result};
use crate::utils::fs;
use crate::utils::platform::{Architecture, Platform};
use std::fmt;
use std::path::{Path, PathBuf};

/// Information about FFmpeg binary extraction based on platform
#[derive(Debug, Clone)]
struct Extraction {
    /// Path to the executable within the extracted archive
    executable_path: PathBuf,
    /// File extension for the binary
    binary_extension: String,
}

/// The ffmpeg fetcher is responsible for fetching the ffmpeg binary for the current platform and architecture.
/// It can also extract the binary from the downloaded archive.
///
/// # Example
///
/// ```rust, no_run
/// # use yt_dlp::client::deps::ffmpeg::BuildFetcher;
/// # use std::path::PathBuf;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let path = PathBuf::from("ffmpeg-release.zip");
/// let fetcher = BuildFetcher::new();
///
/// let release = fetcher.fetch_binary().await?;
/// release.download(path.clone()).await?;
///
/// fetcher.extract_binary(path).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, Default)]
pub struct BuildFetcher;

impl fmt::Display for BuildFetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BuildFetcher")
    }
}

impl BuildFetcher {
    /// Create a new fetcher for ffmpeg.
    pub fn new() -> Self {
        Self
    }

    /// Fetch the ffmpeg binary for the current platform and architecture.
    pub async fn fetch_binary(&self) -> Result<WantedRelease> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Fetching ffmpeg binary");

        let platform = Platform::detect();
        let architecture = Architecture::detect();

        self.fetch_binary_for_platform(platform, architecture).await
    }

    /// Fetch the ffmpeg binary for the given platform and architecture.
    ///
    /// # Arguments
    ///
    /// * `platform` - The platform to fetch the binary for.
    /// * `architecture` - The architecture to fetch the binary for.
    pub async fn fetch_binary_for_platform(
        &self,
        platform: Platform,
        architecture: Architecture,
    ) -> Result<WantedRelease> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Fetching ffmpeg binary for platform: {:?}, architecture: {:?}",
            platform,
            architecture
        );

        match platform {
            Platform::Windows | Platform::Linux | Platform::Mac => {
                let fetcher = GitHubFetcher::new("boul2gom", "ffmpeg-builds");
                fetcher
                    .fetch_release_for_platform(
                        platform,
                        architecture,
                        None,
                        |release, platform, architecture| {
                            let os_str = platform.to_string();
                            let arch_str = architecture.to_string();

                            let target_name = format!("ffmpeg-{}-{}.zip", os_str, arch_str);

                            release
                                .assets
                                .iter()
                                .find(|asset| asset.name == target_name)
                        },
                    )
                    .await
            }
            _ => Err(Error::Unknown(format!(
                "Unsupported platform for FFmpeg: {:?}",
                platform
            ))),
        }
    }

    /// Get extraction information for the given platform and architecture
    fn get_extraction_info(
        &self,
        platform: &Platform,
        architecture: &Architecture,
    ) -> Option<Extraction> {
        match (platform, architecture) {
            (Platform::Windows, _) => Some(Extraction {
                executable_path: PathBuf::from("ffmpeg.exe"),
                binary_extension: "exe".to_string(),
            }),

            (Platform::Mac, _) => Some(Extraction {
                executable_path: PathBuf::from("ffmpeg"),
                binary_extension: "".to_string(),
            }),

            (Platform::Linux, _) => Some(Extraction {
                executable_path: PathBuf::from("ffmpeg"),
                binary_extension: "".to_string(),
            }),

            _ => None,
        }
    }

    /// Extract the ffmpeg binary from the downloaded archive, for the current platform and architecture.
    /// The resulting binary will be placed in the same directory as the archive.
    /// The archive will be deleted after the binary has been extracted.
    pub async fn extract_binary(
        &self,
        archive: impl AsRef<Path> + std::fmt::Debug,
    ) -> Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Extracting ffmpeg binary from archive: {:?}",
            archive.as_ref()
        );

        let platform = Platform::detect();
        let architecture = Architecture::detect();

        self.extract_binary_for_platform(archive, platform, architecture)
            .await
    }

    /// Extract the ffmpeg binary from the downloaded archive, for the given platform and architecture.
    /// The resulting binary will be placed in the same directory as the archive.
    /// The archive will be deleted after the binary has been extracted.
    ///
    /// # Arguments
    ///
    /// * `archive` - The path to the downloaded archive.
    /// * `platform` - The platform to extract the binary for.
    /// * `architecture` - The architecture to extract the binary for.
    pub async fn extract_binary_for_platform(
        &self,
        archive: impl AsRef<Path> + std::fmt::Debug,
        platform: Platform,
        architecture: Architecture,
    ) -> Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Extracting ffmpeg binary for platform: {:?}, architecture: {:?}, from archive: {:?}",
            platform,
            architecture,
            archive.as_ref()
        );

        let archive_path = archive.as_ref().to_path_buf();
        let destination = archive_path.with_extension("");

        let extraction_info =
            self.get_extraction_info(&platform, &architecture)
                .ok_or(Error::NoBinaryRelease {
                    binary: "ffmpeg".to_string(),
                    platform: platform.clone(),
                    architecture: architecture.clone(),
                })?;

        self.extract_archive(archive_path, destination, extraction_info, platform)
            .await
    }

    /// Extract the archive and move the binary to the correct location
    async fn extract_archive(
        &self,
        archive: PathBuf,
        destination: PathBuf,
        extraction_info: Extraction,
        platform: Platform,
    ) -> Result<PathBuf> {
        fs::extract_zip(&archive, &destination).await?;

        // Get the parent directory of the destination
        let parent = fs::try_parent(&destination)?;

        // Construct paths
        let binary_name = format!(
            "ffmpeg{}",
            if !extraction_info.binary_extension.is_empty() {
                format!(".{}", extraction_info.binary_extension)
            } else {
                "".to_string()
            }
        );
        let final_binary_path = parent.join(&binary_name);

        // In these flat zips, the binary should be directly in the destination folder
        let extracted_binary = destination.join(&extraction_info.executable_path);

        if !extracted_binary.exists() {
            // Fallback: list dir to see what's there (debugging purposes mostly, or slightly nested fallback)
            // But user insisted it's flat.
            return Err(Error::Unknown(format!(
                "Could not find ffmpeg binary at expected path: {:?}. Archive content might not be flat.",
                extracted_binary
            )));
        }

        // Copy the executable to the final location
        tokio::fs::copy(&extracted_binary, &final_binary_path).await?;

        // Clean up
        tokio::fs::remove_dir_all(destination).await?;
        tokio::fs::remove_file(archive).await?;

        // Set executable permissions on Unix platforms
        if matches!(platform, Platform::Mac | Platform::Linux) {
            fs::set_executable(final_binary_path.clone()).await?;
        }

        Ok(final_binary_path)
    }
}
