//! Fetch releases and assets from a GitHub repository.

use std::fmt;

use crate::client::deps::{Asset, Release, WantedRelease};
use crate::download::Fetcher;
use crate::error::{Error, Result};
use crate::utils::platform::{Architecture, Platform};

/// The GitHub fetcher is responsible for fetching the latest release of a project from a GitHub repository.
/// It can also select the correct asset for the current platform and architecture.
///
/// # Architecture
///
/// Wraps the GitHub Releases API to fetch release metadata and assets.
/// Handles authentication via tokens to avoid rate limits.
#[derive(Debug)]
pub struct GitHubFetcher {
    /// The owner or organization of the GitHub repository.
    owner: String,
    /// The name of the GitHub repository.
    repo: String,
}

impl GitHubFetcher {
    /// Create a new fetcher for the given GitHub repository.
    ///
    /// # Arguments
    ///
    /// * `owner` - The owner of the GitHub repository.
    /// * `repo` - The name of the GitHub repository.
    ///
    /// # Returns
    ///
    /// A new `GitHubFetcher` instance
    pub fn new(owner: impl Into<String>, repo: impl Into<String>) -> Self {
        let owner = owner.into();
        let repo = repo.into();

        tracing::debug!(
            owner = %owner,
            repo = %repo,
            "⚙️ Creating new GitHubFetcher"
        );

        Self { owner, repo }
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
    pub async fn fetch_release<F>(&self, auth_token: Option<String>, selector: F) -> Result<WantedRelease>
    where
        F: for<'a> Fn(&'a Release, &Platform, &Architecture) -> Option<&'a Asset>,
    {
        tracing::debug!(
            owner = %self.owner,
            repo = %self.repo,
            has_token = auth_token.is_some(),
            platform = ?Platform::detect(),
            architecture = ?Architecture::detect(),
            "📦 Fetching latest release from GitHub"
        );

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
        tracing::debug!(
            owner = %self.owner,
            repo = %self.repo,
            platform = ?platform,
            architecture = ?architecture,
            has_token = auth_token.is_some(),
            "📦 Fetching release for specific platform"
        );

        let release = self.fetch_latest_release(auth_token.clone()).await?;

        tracing::debug!(
            platform = ?platform,
            architecture = ?architecture,
            release_tag = %release.tag_name,
            asset_count = release.assets.len(),
            "⚙️ Selecting asset from release"
        );

        let asset = selector(&release, &platform, &architecture).ok_or(Error::NoBinaryRelease {
            binary: self.repo.clone(),
            platform: platform.clone(),
            architecture: architecture.clone(),
        })?;

        let checksum = self.fetch_checksum(&release, &asset.name).await.ok().flatten();

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
        tracing::debug!(
            owner = %self.owner,
            repo = %self.repo,
            has_token = auth_token.is_some(),
            "📦 Fetching latest release metadata from GitHub API"
        );

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
    ///
    /// # Arguments
    ///
    /// * `release` - The GitHub release
    /// * `asset_name` - Name of the asset to find checksum for
    ///
    /// # Returns
    ///
    /// The SHA256 checksum if available, None otherwise
    ///
    /// # Errors
    ///
    /// Returns an error if checksum parsing fails
    async fn fetch_checksum(&self, release: &Release, asset_name: &str) -> Result<Option<String>> {
        tracing::debug!(
            asset_name = asset_name,
            release_tag = %release.tag_name,
            "⚙️ Looking for checksum in release"
        );
        if let Some(digest) = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .and_then(|a| a.digest.as_ref())
        {
            return if let Some(stripped) = digest.strip_prefix("sha256:") {
                tracing::debug!(
                    asset_name = asset_name,
                    checksum = stripped,
                    "✅ Found SHA256 digest from API"
                );
                Ok(Some(stripped.to_string()))
            } else {
                tracing::debug!(
                    asset_name = asset_name,
                    digest = digest,
                    "✅ Found digest from API (raw format)"
                );
                Ok(Some(digest.clone()))
            };
        }

        tracing::warn!(
            asset_name = asset_name,
            release_tag = %release.tag_name,
            "⚙️ Checksum not found for asset"
        );
        Ok(None)
    }
}

impl fmt::Display for GitHubFetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GitHubFetcher(owner={}, repo={})", self.owner, self.repo)
    }
}
