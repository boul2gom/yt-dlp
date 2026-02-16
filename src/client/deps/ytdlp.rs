//! Fetch the latest release of 'yt-dlp' from GitHub.

use crate::client::deps::github::GitHubFetcher;
use crate::client::deps::{Asset, WantedRelease};
use crate::error::Result;
use crate::utils::platform::{Architecture, Platform};
use std::fmt;

/// The yt-dlp fetcher is responsible for fetching the yt-dlp binary for the current platform and architecture.
#[derive(Debug)]
pub struct YoutubeFetcher {
    fetcher: GitHubFetcher,
}

impl fmt::Display for YoutubeFetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "YoutubeFetcher({})", self.fetcher)
    }
}

impl YoutubeFetcher {
    /// Create a new fetcher for the given GitHub repository.
    pub fn new(owner: impl AsRef<str>, repo: impl AsRef<str>) -> Self {
        Self {
            fetcher: GitHubFetcher::new(owner, repo),
        }
    }

    /// Fetch the yt-dlp binary for the current platform and architecture.
    pub async fn fetch_release(&self, auth_token: Option<String>) -> Result<WantedRelease> {
        self.fetcher
            .fetch_release(auth_token, Self::select_asset)
            .await
    }

    /// Select the correct asset from the release for the given platform and architecture.
    fn select_asset<'a>(
        release: &'a crate::client::deps::Release,
        platform: &Platform,
        architecture: &Architecture,
    ) -> Option<&'a Asset> {
        let base_name = "yt-dlp";
        release.assets.iter().find(|asset| {
            let name = &asset.name;
            match (platform, architecture) {
                (Platform::Windows, Architecture::X64) => {
                    name.contains(&format!("{}.exe", base_name))
                }
                (Platform::Windows, Architecture::X86) => {
                    name.contains(&format!("{}_x86.exe", base_name))
                }

                (Platform::Linux, Architecture::X64) => {
                    name.contains(&format!("{}_linux", base_name))
                }
                (Platform::Linux, Architecture::Armv7l) => {
                    name.contains(&format!("{}_linux_armv7l", base_name))
                }
                (Platform::Linux, Architecture::Aarch64) => {
                    name.contains(&format!("{}_linux_aarch64", base_name))
                }

                (Platform::Mac, _) => name.contains(&format!("{}_macos", base_name)),

                _ => false,
            }
        })
    }
}
