//! Fetch the latest release of 'ffmpeg' from static builds.

use crate::error::{Error, Result};
use crate::fetcher::deps::{Asset, WantedRelease};
use crate::utils::file_system;
use crate::utils::platform::{Architecture, Platform};
use std::fmt;
use std::path::{Path, PathBuf};

/// URL templates for FFmpeg builds based on platform and architecture
#[derive(Debug, Clone)]
struct Url;

impl Url {
    fn windows() -> &'static str {
        "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
    }

    fn macos_intel() -> &'static str {
        "https://www.osxexperts.net/ffmpeg71intel.zip"
    }

    fn macos_arm() -> &'static str {
        "https://www.osxexperts.net/ffmpeg71arm.zip"
    }

    fn linux(arch: &str) -> String {
        format!(
            "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-{}-static.tar.xz",
            arch
        )
    }
}

/// Information about FFmpeg binary extraction based on platform
#[derive(Debug, Clone)]
struct Extraction {
    /// Path to the executable within the extracted archive
    executable_path: PathBuf,
    /// Name of the extracted directory (for Linux)
    extracted_dir: Option<String>,
    /// File extension for the binary
    binary_extension: String,
}

/// The ffmpeg fetcher is responsible for fetching the ffmpeg binary for the current platform and architecture.
/// It can also extract the binary from the downloaded archive.
///
/// # Example
///
/// ```rust, no_run
/// # use yt_dlp::fetcher::deps::ffmpeg::BuildFetcher;
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

        let asset = self
            .select_asset(&platform, &architecture)
            .ok_or(Error::Binary(platform, architecture))?;

        Ok(WantedRelease {
            url: asset.download_url.clone(),
            name: asset.name.clone(),
        })
    }

    /// Select the correct ffmpeg asset for the given platform and architecture.
    ///
    /// # Arguments
    ///
    /// * `platform` - The platform to select the asset for.
    /// * `architecture` - The architecture to select the asset for.
    pub fn select_asset(&self, platform: &Platform, architecture: &Architecture) -> Option<Asset> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Selecting ffmpeg asset for platform: {:?}, architecture: {:?}",
            platform,
            architecture
        );

        match (platform, architecture) {
            (Platform::Windows, _) => {
                let url = Url::windows().to_string();
                let name = url.split('/').next_back()?.to_string();
                Some(Asset {
                    name,
                    download_url: url,
                })
            }

            (Platform::Mac, Architecture::X64) => {
                let url = Url::macos_intel().to_string();
                let name = url.split('/').next_back()?.to_string();
                Some(Asset {
                    name,
                    download_url: url,
                })
            }
            (Platform::Mac, Architecture::Aarch64) => {
                let url = Url::macos_arm().to_string();
                let name = url.split('/').next_back()?.to_string();
                Some(Asset {
                    name,
                    download_url: url,
                })
            }

            (Platform::Linux, Architecture::X64) => {
                let url = Url::linux("amd64");
                let name = url.split('/').next_back()?.to_string();
                Some(Asset {
                    name,
                    download_url: url,
                })
            }
            (Platform::Linux, Architecture::X86) => {
                let url = Url::linux("i686");
                let name = url.split('/').next_back()?.to_string();
                Some(Asset {
                    name,
                    download_url: url,
                })
            }
            (Platform::Linux, Architecture::Armv7l) => {
                let url = Url::linux("armhf");
                let name = url.split('/').next_back()?.to_string();
                Some(Asset {
                    name,
                    download_url: url,
                })
            }
            (Platform::Linux, Architecture::Aarch64) => {
                let url = Url::linux("arm64");
                let name = url.split('/').next_back()?.to_string();
                Some(Asset {
                    name,
                    download_url: url,
                })
            }

            _ => None,
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
                executable_path: PathBuf::from("ffmpeg-7.1.1-essentials_build/bin/ffmpeg.exe"),
                extracted_dir: None,
                binary_extension: "exe".to_string(),
            }),

            (Platform::Mac, _) => Some(Extraction {
                executable_path: PathBuf::from("ffmpeg"),
                extracted_dir: None,
                binary_extension: "".to_string(),
            }),

            (Platform::Linux, Architecture::X64) => Some(Extraction {
                executable_path: PathBuf::from("ffmpeg"),
                extracted_dir: Some("ffmpeg-7.0.2-amd64-static".to_string()),
                binary_extension: "".to_string(),
            }),
            (Platform::Linux, Architecture::X86) => Some(Extraction {
                executable_path: PathBuf::from("ffmpeg"),
                extracted_dir: Some("ffmpeg-7.0.2-i686-static".to_string()),
                binary_extension: "".to_string(),
            }),
            (Platform::Linux, Architecture::Armv7l) => Some(Extraction {
                executable_path: PathBuf::from("ffmpeg"),
                extracted_dir: Some("ffmpeg-7.0.2-armhf-static".to_string()),
                binary_extension: "".to_string(),
            }),
            (Platform::Linux, Architecture::Aarch64) => Some(Extraction {
                executable_path: PathBuf::from("ffmpeg"),
                extracted_dir: Some("ffmpeg-7.0.2-arm64-static".to_string()),
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

        let extraction_info = self
            .get_extraction_info(&platform, &architecture)
            .ok_or(Error::Binary(platform.clone(), architecture.clone()))?;

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
        // Extract the archive based on platform
        match platform {
            Platform::Windows | Platform::Mac => {
                file_system::extract_zip(&archive, &destination).await?;
            }
            Platform::Linux => {
                file_system::extract_tar_xz(&archive, &destination).await?;
            }
            _ => return Err(Error::Binary(platform.clone(), Architecture::detect())),
        }

        // Get the parent directory of the destination
        let parent = file_system::try_parent(&destination)?;

        // Construct paths
        let binary_name = format!(
            "ffmpeg{}",
            if !extraction_info.binary_extension.is_empty() {
                format!(".{}", extraction_info.binary_extension)
            } else {
                "".to_string()
            }
        );
        let binary = parent.join(binary_name);

        // Find the executable path
        let executable = if let Some(extracted_dir) = extraction_info.extracted_dir {
            destination
                .join(extracted_dir)
                .join(extraction_info.executable_path)
        } else {
            destination.join(extraction_info.executable_path)
        };

        // Copy the executable to the final location
        tokio::fs::copy(executable, binary.clone()).await?;

        // Clean up
        tokio::fs::remove_dir_all(destination).await?;
        tokio::fs::remove_file(archive).await?;

        // Set executable permissions on Unix platforms
        if matches!(platform, Platform::Mac | Platform::Linux) {
            file_system::set_executable(binary.clone())?;
        }

        Ok(binary)
    }
}
