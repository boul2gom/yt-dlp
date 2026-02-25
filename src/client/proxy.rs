//! Proxy configuration for HTTP/HTTPS/SOCKS5 proxies.
//!
//! This module provides proxy configuration for both reqwest HTTP client
//! and yt-dlp command-line tool.

use std::fmt;

/// Proxy configuration supporting HTTP, HTTPS, and SOCKS5 proxies.
///
/// # Examples
///
/// ```rust,no_run
/// use yt_dlp::client::proxy::{ProxyConfig, ProxyType};
///
/// // Simple HTTP proxy
/// let proxy = ProxyConfig::new(ProxyType::Http, "http://proxy.example.com:8080");
///
/// // SOCKS5 proxy with authentication
/// let proxy = ProxyConfig::new(ProxyType::Socks5, "socks5://proxy.example.com:1080")
///     .with_auth("username", "password");
///
/// // With no-proxy list
/// let proxy = ProxyConfig::new(ProxyType::Http, "http://proxy.example.com:8080")
///     .with_no_proxy(vec!["localhost".to_string(), "127.0.0.1".to_string()]);
/// ```
#[derive(Clone, Debug)]
pub struct ProxyConfig {
    proxy_type: ProxyType,
    url: String,
    username: Option<String>,
    password: Option<String>,
    no_proxy: Vec<String>,
}

/// Type of proxy to use.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum ProxyType {
    /// HTTP proxy
    Http,
    /// HTTPS proxy
    Https,
    /// SOCKS5 proxy
    Socks5,
}

impl ProxyConfig {
    /// Creates a new proxy configuration.
    ///
    /// # Arguments
    ///
    /// * `proxy_type` - The type of proxy (HTTP, HTTPS, or SOCKS5)
    /// * `url` - The proxy URL (e.g., "http://proxy.example.com:8080")
    ///
    /// # Returns
    ///
    /// A new ProxyConfig instance
    pub fn new(proxy_type: ProxyType, url: impl Into<String>) -> Self {
        let url = url.into();

        tracing::debug!(
            proxy_type = ?proxy_type,
            url = %url,
            "🔧 Creating proxy config"
        );

        Self {
            proxy_type,
            url,
            username: None,
            password: None,
            no_proxy: Vec::new(),
        }
    }

    /// Adds authentication credentials to the proxy.
    ///
    /// # Arguments
    ///
    /// * `username` - The proxy username
    /// * `password` - The proxy password
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        let username = username.into();

        tracing::debug!(
            username = %username,
            "🔧 Adding proxy authentication"
        );

        self.username = Some(username);
        self.password = Some(password.into());
        self
    }

    /// Sets the list of domains that should bypass the proxy.
    ///
    /// # Arguments
    ///
    /// * `no_proxy` - List of domains to bypass (e.g., ["localhost", "127.0.0.1"])
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_no_proxy(mut self, no_proxy: Vec<String>) -> Self {
        tracing::debug!(
            no_proxy = ?no_proxy,
            count = no_proxy.len(),
            "🔧 Setting no-proxy list"
        );

        self.no_proxy = no_proxy;
        self
    }

    /// Returns the proxy type.
    ///
    /// # Returns
    ///
    /// A reference to the configured `ProxyType`.
    pub fn proxy_type(&self) -> &ProxyType {
        &self.proxy_type
    }

    /// Returns the proxy URL.
    ///
    /// # Returns
    ///
    /// The proxy URL as a string slice.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns the username if authentication is configured.
    ///
    /// # Returns
    ///
    /// The proxy username, or `None` if no authentication is set.
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    /// Returns the password if authentication is configured.
    ///
    /// # Returns
    ///
    /// The proxy password, or `None` if no authentication is set.
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    /// Returns the no-proxy list.
    ///
    /// # Returns
    ///
    /// A slice of domain strings that should bypass the proxy.
    pub fn no_proxy(&self) -> &[String] {
        &self.no_proxy
    }

    /// Builds the complete proxy URL with authentication if configured.
    ///
    /// # Returns
    ///
    /// The proxy URL with embedded authentication credentials if provided
    pub fn build_url(&self) -> String {
        tracing::debug!(has_auth = self.username.is_some(), "🔧 Building proxy URL");

        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            // URL-encode username and password
            let username_enc = url::form_urlencoded::byte_serialize(username.as_bytes()).collect::<String>();
            let password_enc = url::form_urlencoded::byte_serialize(password.as_bytes()).collect::<String>();

            // Extract scheme and host from URL
            if let Some(idx) = self.url.find("://") {
                let scheme = &self.url[..idx];
                let rest = &self.url[idx + 3..];
                format!("{}://{}:{}@{}", scheme, username_enc, password_enc, rest)
            } else {
                // No scheme, just add auth
                format!("{}:{}@{}", username_enc, password_enc, self.url)
            }
        } else {
            self.url.clone()
        }
    }

    /// Converts to reqwest proxy format.
    ///
    /// # Returns
    ///
    /// Result containing the reqwest Proxy instance
    ///
    /// # Errors
    ///
    /// Returns error if the proxy URL is invalid
    pub fn to_reqwest_proxy(&self) -> reqwest::Result<reqwest::Proxy> {
        let url = self.build_url();

        tracing::debug!(proxy_type = ?self.proxy_type, no_proxy_count = self.no_proxy.len(), "🔧 Converting to reqwest proxy");

        match self.proxy_type {
            ProxyType::Http => reqwest::Proxy::http(&url),
            ProxyType::Https => reqwest::Proxy::https(&url),
            ProxyType::Socks5 => reqwest::Proxy::all(&url),
        }
        .map(|mut proxy| {
            // Add no-proxy domains
            if !self.no_proxy.is_empty() {
                proxy = proxy.no_proxy(reqwest::NoProxy::from_string(&self.no_proxy.join(",")));
            }
            proxy
        })
    }

    /// Converts to yt-dlp proxy argument format.
    ///
    /// # Returns
    ///
    /// The proxy URL in the format expected by yt-dlp's `--proxy` argument.
    /// Includes authentication credentials if configured.
    pub fn to_ytdlp_arg(&self) -> String {
        self.build_url()
    }
}

impl fmt::Display for ProxyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http => f.write_str("Http"),
            Self::Https => f.write_str("Https"),
            Self::Socks5 => f.write_str("Socks5"),
        }
    }
}

impl fmt::Display for ProxyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ProxyConfig(type={}, url={}, auth={})",
            self.proxy_type,
            self.url,
            self.username.is_some()
        )
    }
}
