//! Fetch releases and assets from a GitHub repository.

use crate::client::deps::{Asset, Release, WantedRelease};
use crate::download::Fetcher;
use crate::error::{Error, Result};
use crate::utils::platform::{Architecture, Platform};
use std::fmt;

/// The GitHub fetcher is responsible for fetching the latest release of a project from a GitHub repository.
/// It can also select the correct asset for the current platform and architecture.
#[derive(Debug)]
pub struct GitHubFetcher {
    /// The owner or organization of the GitHub repository.
    owner: String,
    /// The name of the GitHub repository.
    repo: String,
}

impl fmt::Display for GitHubFetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GitHubFetcher(owner={}, repo={})", self.owner, self.repo)
    }
}

impl GitHubFetcher {
    /// Create a new fetcher for the given GitHub repository.
    ///
    /// # Arguments
    ///
    /// * `owner` - The owner of the GitHub repository.
    /// * `repo` - The name of the GitHub repository.
    pub fn new(owner: impl AsRef<str>, repo: impl AsRef<str>) -> Self {
        Self {
            owner: owner.as_ref().to_string(),
            repo: repo.as_ref().to_string(),
        }
    }

    /// Fetch the latest release for the current platform.
    ///
    /// # Arguments
    ///
    /// * `auth_token` - An optional GitHub personal access token to authenticate the request.
    ///
    /// # Errors
    ///
    /// This function will return an error if the release could not be fetched or if no asset was found for the current platform.
    /// Fetch the latest release for the current platform.
    ///
    /// # Arguments
    ///
    /// * `auth_token` - An optional GitHub personal access token to authenticate the request.
    /// * `selector` - A closure that selects the correct asset from the release for the given platform and architecture.
    ///
    /// # Errors
    ///
    /// This function will return an error if the release could not be fetched or if no asset was found for the current platform.
    pub async fn fetch_release<F>(
        &self,
        auth_token: Option<String>,
        selector: F,
    ) -> Result<WantedRelease>
    where
        F: for<'a> Fn(&'a Release, &Platform, &Architecture) -> Option<&'a Asset>,
    {
        #[cfg(feature = "tracing")]
        tracing::debug!("Fetching latest release from {}/{}", self.owner, self.repo);

        let platform = Platform::detect();
        let architecture = Architecture::detect();

        self.fetch_release_for_platform(platform, architecture, auth_token, selector)
            .await
    }

    /// Fetch the latest release for the given platform.
    ///
    /// # Arguments
    ///
    /// * `platform` - The platform to fetch the release for.
    /// * `architecture` - The architecture to fetch the release for.
    /// * `auth_token` - An optional GitHub personal access token to authenticate the request.
    /// * `selector` - A closure that selects the correct asset from the release for the given platform and architecture.
    ///
    /// # Errors
    ///
    /// This function will return an error if the release could not be fetched or if no asset was found for the given platform.
    pub async fn fetch_release_for_platform<F>(
        &self,
        platform: Platform,
        architecture: Architecture,
        auth_token: Option<String>,
        selector: F,
    ) -> Result<WantedRelease>
    where
        F: for<'a> Fn(&'a Release, &Platform, &Architecture) -> Option<&'a Asset>,
    {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Fetching latest release for {}/{} for platform: {:?}, architecture: {:?}",
            self.owner,
            self.repo,
            platform,
            architecture
        );

        let release = self.fetch_latest_release(auth_token.clone()).await?;

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Selecting asset for platform: {:?}, architecture: {:?}",
            platform,
            architecture
        );

        let asset = selector(&release, &platform, &architecture).ok_or(Error::NoBinaryRelease {
            binary: self.repo.clone(),
            platform: platform.clone(),
            architecture: architecture.clone(),
        })?;

        let checksum = self
            .fetch_checksum(&release, &asset.name)
            .await
            .ok()
            .flatten();

        Ok(WantedRelease {
            name: asset.name.clone(),
            url: asset.download_url.clone(),
            checksum,
        })
    }

    /// Fetch the latest release of the GitHub repository.
    ///
    /// # Arguments
    ///
    /// * `auth_token` - An optional GitHub personal access token to authenticate the request.
    pub async fn fetch_latest_release(&self, auth_token: Option<String>) -> Result<Release> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Fetching latest release for {}/{}", self.owner, self.repo);

        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            self.owner, self.repo
        );

        let fetcher = Fetcher::new(&url, None, None)?;
        let response = fetcher.fetch_json(auth_token).await?;

        let release: Release = serde_json::from_value(response)?;
        Ok(release)
    }

    /// Fetch the checksum for the given asset from the release.
    async fn fetch_checksum(&self, release: &Release, asset_name: &str) -> Result<Option<String>> {
        if let Some(digest) = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .and_then(|a| a.digest.as_ref())
        {
            if let Some(stripped) = digest.strip_prefix("sha256:") {
                #[cfg(feature = "tracing")]
                tracing::debug!("Found digest from API for {}: {}", asset_name, stripped);
                return Ok(Some(stripped.to_string()));
            } else {
                #[cfg(feature = "tracing")]
                tracing::debug!("Found digest from API for {}: {}", asset_name, digest);
                return Ok(Some(digest.clone()));
            }
        }

        #[cfg(feature = "tracing")]
        tracing::warn!("Checksum not found for {}", asset_name);
        Ok(None)
    }
}
