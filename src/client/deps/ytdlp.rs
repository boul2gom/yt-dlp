//! Fetch the latest release of 'yt-dlp' from GitHub.

use std::fmt;

use crate::client::deps::github::GitHubFetcher;
use crate::client::deps::{Asset, WantedRelease};
use crate::error::Result;
use crate::utils::platform::{Architecture, Platform};

/// The yt-dlp fetcher is responsible for fetching the yt-dlp binary for the current platform and architecture.
///
/// # Architecture
///
/// Uses `GitHubFetcher` internally to find the correct `yt-dlp` binary asset (e.g., `yt-dlp_linux`, `yt-dlp.exe`)
/// from the official or custom repository.
#[derive(Debug)]
pub struct YoutubeFetcher {
    fetcher: GitHubFetcher,
}

impl YoutubeFetcher {
    /// Create a new fetcher for the given GitHub repository.
    ///
    /// # Arguments
    ///
    /// * `owner` - The GitHub repository owner
    /// * `repo` - The GitHub repository name
    ///
    /// # Returns
    ///
    /// A new `YoutubeFetcher` instance
    pub fn new(owner: impl Into<String>, repo: impl Into<String>) -> Self {
        let owner = owner.into();
        let repo = repo.into();

        tracing::debug!(
            owner = %owner,
            repo = %repo,
            "⚙️ Creating new YoutubeFetcher"
        );

        Self {
            fetcher: GitHubFetcher::new(owner, repo),
        }
    }

    /// Fetch the yt-dlp binary for the current platform and architecture.
    ///
    /// # Arguments
    ///
    /// * `auth_token` - Optional GitHub authentication token
    ///
    /// # Returns
    ///
    /// A `WantedRelease` containing download information
    ///
    /// # Errors
    ///
    /// Returns an error if the release cannot be fetched or no compatible binary is found
    pub async fn fetch_release(&self, auth_token: Option<String>) -> Result<WantedRelease> {
        tracing::debug!(
            has_token = auth_token.is_some(),
            "📦 Fetching yt-dlp release for current platform"
        );

        self.fetcher.fetch_release(auth_token, Self::select_asset).await
    }

    /// Select the correct asset from the release for the given platform and architecture.
    ///
    /// # Arguments
    ///
    /// * `release` - The GitHub release to select from
    /// * `platform` - The target platform
    /// * `architecture` - The target architecture
    ///
    /// # Returns
    ///
    /// The matching asset, or None if no compatible binary is found
    fn select_asset<'a>(
        release: &'a crate::client::deps::Release,
        platform: &Platform,
        architecture: &Architecture,
    ) -> Option<&'a Asset> {
        tracing::debug!(
            platform = ?platform,
            architecture = ?architecture,
            asset_count = release.assets.len(),
            "⚙️ Selecting yt-dlp asset for platform"
        );

        let base_name = "yt-dlp";
        release.assets.iter().find(|asset| {
            let name = &asset.name;
            match (platform, architecture) {
                (Platform::Windows, Architecture::X64) => name == &format!("{}.exe", base_name),
                (Platform::Windows, Architecture::X86) => name == &format!("{}_x86.exe", base_name),

                (Platform::Linux, Architecture::X64) => name == &format!("{}_linux", base_name),
                (Platform::Linux, Architecture::Armv7l) => name == &format!("{}_linux_armv7l", base_name),
                (Platform::Linux, Architecture::Aarch64) => name == &format!("{}_linux_aarch64", base_name),

                (Platform::Mac, _) => name == &format!("{}_macos", base_name),

                _ => false,
            }
        })
    }
}

impl fmt::Display for YoutubeFetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "YoutubeFetcher(fetcher={})", self.fetcher)
    }
}
