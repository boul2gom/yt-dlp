use std::collections::HashSet;

use yt_dlp::client::proxy::{ProxyConfig, ProxyType};

// ============================== ProxyConfig ==============================

#[test]
fn proxy_config_new() {
    let config = ProxyConfig::new(ProxyType::Http, "http://proxy.example.com:8080");
    assert_eq!(*config.proxy_type(), ProxyType::Http);
    assert_eq!(config.url(), "http://proxy.example.com:8080");
    assert!(config.username().is_none());
    assert!(config.password().is_none());
    assert!(config.no_proxy().is_empty());
}

#[test]
fn proxy_config_with_auth() {
    let config = ProxyConfig::new(ProxyType::Socks5, "socks5://proxy:1080").with_auth("user", "pass");
    assert_eq!(config.username(), Some("user"));
    assert_eq!(config.password(), Some("pass"));
}

#[test]
fn proxy_config_with_no_proxy() {
    let config = ProxyConfig::new(ProxyType::Http, "http://proxy:8080")
        .with_no_proxy(vec!["localhost".to_string(), "127.0.0.1".to_string()]);
    assert_eq!(config.no_proxy().len(), 2);
    assert_eq!(config.no_proxy()[0], "localhost");
}

#[test]
fn proxy_config_build_url_no_auth() {
    let config = ProxyConfig::new(ProxyType::Http, "http://proxy.example.com:8080");
    assert_eq!(config.build_url(), "http://proxy.example.com:8080");
}

#[test]
fn proxy_config_build_url_with_auth() {
    let config = ProxyConfig::new(ProxyType::Http, "http://proxy.example.com:8080").with_auth("user", "pass");
    let url = config.build_url();
    assert_eq!(url, "http://user:pass@proxy.example.com:8080");
}

#[test]
fn proxy_config_build_url_encodes_special_chars() {
    let config = ProxyConfig::new(ProxyType::Http, "http://proxy:8080").with_auth("user@domain", "p@ss:word");
    let url = config.build_url();
    assert!(url.contains("user%40domain"));
    assert!(url.contains("p%40ss%3Aword"));
}

#[test]
fn proxy_config_build_url_no_scheme() {
    let config = ProxyConfig::new(ProxyType::Http, "proxy:8080").with_auth("user", "pass");
    let url = config.build_url();
    assert_eq!(url, "user:pass@proxy:8080");
}

#[test]
fn proxy_config_ytdlp_arg_no_auth() {
    let config = ProxyConfig::new(ProxyType::Http, "http://proxy:8080");
    assert_eq!(config.to_ytdlp_arg(), "http://proxy:8080");
}

#[test]
fn proxy_config_ytdlp_arg_with_auth() {
    let config = ProxyConfig::new(ProxyType::Socks5, "socks5://proxy:1080").with_auth("user", "pass");
    assert_eq!(config.to_ytdlp_arg(), "socks5://user:pass@proxy:1080");
}

#[test]
fn proxy_config_to_reqwest_proxy_http() {
    let config = ProxyConfig::new(ProxyType::Http, "http://proxy:8080");
    assert!(config.to_reqwest_proxy().is_ok());
}

#[test]
fn proxy_config_to_reqwest_proxy_https() {
    let config = ProxyConfig::new(ProxyType::Https, "https://proxy:8443");
    assert!(config.to_reqwest_proxy().is_ok());
}

#[test]
fn proxy_config_to_reqwest_proxy_socks5() {
    let config = ProxyConfig::new(ProxyType::Socks5, "socks5://proxy:1080");
    assert!(config.to_reqwest_proxy().is_ok());
}

#[test]
fn proxy_config_chaining() {
    let config = ProxyConfig::new(ProxyType::Http, "http://proxy:8080")
        .with_auth("user", "pass")
        .with_no_proxy(vec!["localhost".to_string()]);
    assert_eq!(config.username(), Some("user"));
    assert_eq!(config.no_proxy().len(), 1);
}

// ============================== ProxyType ==============================

#[test]
fn proxy_type_display_http() {
    assert_eq!(format!("{}", ProxyType::Http), "Http");
}

#[test]
fn proxy_type_display_https() {
    assert_eq!(format!("{}", ProxyType::Https), "Https");
}

#[test]
fn proxy_type_display_socks5() {
    assert_eq!(format!("{}", ProxyType::Socks5), "Socks5");
}

#[test]
fn proxy_type_eq() {
    assert_eq!(ProxyType::Http, ProxyType::Http);
    assert_ne!(ProxyType::Http, ProxyType::Https);
    assert_ne!(ProxyType::Https, ProxyType::Socks5);
}

#[test]
fn proxy_type_hash() {
    let mut set = HashSet::new();
    set.insert(ProxyType::Http);
    set.insert(ProxyType::Https);
    set.insert(ProxyType::Socks5);
    assert_eq!(set.len(), 3);
    assert!(set.contains(&ProxyType::Http));
}

// ============================== ProxyConfig Display ==============================

#[test]
fn proxy_config_display_no_auth() {
    let config = ProxyConfig::new(ProxyType::Http, "http://proxy:8080");
    let display = format!("{}", config);
    assert_eq!(display, "ProxyConfig(type=Http, url=http://proxy:8080, auth=false)");
}

#[test]
fn proxy_config_display_with_auth() {
    let config = ProxyConfig::new(ProxyType::Socks5, "socks5://proxy:1080").with_auth("user", "pass");
    let display = format!("{}", config);
    assert_eq!(display, "ProxyConfig(type=Socks5, url=socks5://proxy:1080, auth=true)");
}

#[test]
fn proxy_config_debug() {
    let config = ProxyConfig::new(ProxyType::Http, "http://proxy:8080");
    let debug = format!("{:?}", config);
    assert!(debug.contains("ProxyConfig"));
    assert!(debug.contains("Http"));
}
