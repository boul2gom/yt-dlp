use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use yt_dlp::extractor::youtube::{FormatPreset, PlayerClient};
use yt_dlp::extractor::{ExtractorBase, ExtractorConfig, ExtractorName, VideoExtractor, Youtube};

// ============================== ExtractorName ==============================

#[test]
fn extractor_name_youtube_display() {
    assert_eq!(format!("{}", ExtractorName::Youtube), "Youtube");
}

#[test]
fn extractor_name_generic_with_name_display() {
    let name = ExtractorName::Generic(Some("vimeo".to_string()));
    assert_eq!(format!("{}", name), "Generic(name=vimeo)");
}

#[test]
fn extractor_name_generic_none_display() {
    let name = ExtractorName::Generic(None);
    assert_eq!(format!("{}", name), "Generic");
}

#[test]
fn extractor_name_eq() {
    assert_eq!(ExtractorName::Youtube, ExtractorName::Youtube);
    assert_ne!(ExtractorName::Youtube, ExtractorName::Generic(None));
    assert_eq!(
        ExtractorName::Generic(Some("vimeo".to_string())),
        ExtractorName::Generic(Some("vimeo".to_string()))
    );
    assert_ne!(
        ExtractorName::Generic(Some("vimeo".to_string())),
        ExtractorName::Generic(Some("tiktok".to_string()))
    );
}

#[test]
fn extractor_name_hash() {
    let mut set = HashSet::new();
    set.insert(ExtractorName::Youtube);
    set.insert(ExtractorName::Generic(None));
    set.insert(ExtractorName::Generic(Some("vimeo".to_string())));
    assert_eq!(set.len(), 3);
}

// ============================== Youtube::supports_url ==============================

#[test]
fn supports_url_standard_watch() {
    assert!(Youtube::supports_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
}

#[test]
fn supports_url_short_link() {
    assert!(Youtube::supports_url("https://youtu.be/dQw4w9WgXcQ"));
}

#[test]
fn supports_url_nocookie() {
    assert!(Youtube::supports_url(
        "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ"
    ));
}

#[test]
fn supports_url_mobile() {
    assert!(Youtube::supports_url("https://m.youtube.com/watch?v=dQw4w9WgXcQ"));
}

#[test]
fn supports_url_music() {
    assert!(Youtube::supports_url("https://music.youtube.com/watch?v=dQw4w9WgXcQ"));
}

#[test]
fn supports_url_ytsearch() {
    assert!(Youtube::supports_url("ytsearch10:rust programming"));
}

#[test]
fn supports_url_ytplaylist() {
    assert!(Youtube::supports_url("ytplaylist:PLtest123"));
}

#[test]
fn supports_url_playlist() {
    assert!(Youtube::supports_url("https://www.youtube.com/playlist?list=PLtest123"));
}

#[test]
fn supports_url_channel() {
    assert!(Youtube::supports_url("https://www.youtube.com/channel/UC123"));
}

#[test]
fn supports_url_rejects_vimeo() {
    assert!(!Youtube::supports_url("https://vimeo.com/123456"));
}

#[test]
fn supports_url_rejects_tiktok() {
    assert!(!Youtube::supports_url("https://www.tiktok.com/@user/video/123"));
}

#[test]
fn supports_url_rejects_notyoutube() {
    assert!(!Youtube::supports_url("https://notyoutube.com/watch?v=abc"));
}

#[test]
fn supports_url_http() {
    assert!(Youtube::supports_url("http://youtube.com/watch?v=abc"));
}

#[test]
fn supports_url_case_insensitive() {
    assert!(Youtube::supports_url("https://YOUTUBE.COM/watch?v=abc"));
}

// ============================== Youtube::new ==============================

#[test]
fn youtube_new() {
    let yt = Youtube::new(PathBuf::from("yt-dlp"));
    assert_eq!(format!("{:?}", yt), format!("{:?}", yt)); // just check it doesn't panic
}

#[test]
fn youtube_name() {
    let yt = Youtube::new(PathBuf::from("yt-dlp"));
    assert_eq!(yt.name(), ExtractorName::Youtube);
}

#[test]
fn youtube_supports_url_instance_method() {
    let yt = Youtube::new(PathBuf::from("yt-dlp"));
    assert!(yt.supports_url("https://www.youtube.com/watch?v=abc"));
    assert!(!yt.supports_url("https://vimeo.com/123"));
}

#[test]
fn youtube_with_player_client() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_player_client(PlayerClient::Android);
    let debug = format!("{:?}", yt);
    assert!(debug.contains("Android"));
}

#[test]
fn youtube_skip_dash_manifest() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.skip_dash_manifest(true);
    let debug = format!("{:?}", yt);
    assert!(debug.contains("true"));
}

#[test]
fn youtube_with_format_preset() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_format_preset(FormatPreset::AudioOnly);
    let debug = format!("{:?}", yt);
    assert!(debug.contains("AudioOnly"));
}

// ============================== FormatPreset ==============================

#[test]
fn format_preset_debug() {
    assert_eq!(format!("{:?}", FormatPreset::Best), "Best");
    assert_eq!(format!("{:?}", FormatPreset::Premium), "Premium");
    assert_eq!(format!("{:?}", FormatPreset::High), "High");
    assert_eq!(format!("{:?}", FormatPreset::Medium), "Medium");
    assert_eq!(format!("{:?}", FormatPreset::Low), "Low");
    assert_eq!(format!("{:?}", FormatPreset::AudioOnly), "AudioOnly");
    assert_eq!(format!("{:?}", FormatPreset::ModernCodecs), "ModernCodecs");
    assert_eq!(format!("{:?}", FormatPreset::LegacyCompatible), "LegacyCompatible");
}

#[test]
fn format_preset_custom() {
    let preset = FormatPreset::Custom("best[height<=360]".to_string());
    assert_eq!(format!("{:?}", preset), "Custom(\"best[height<=360]\")");
}

#[test]
fn format_preset_eq() {
    assert_eq!(FormatPreset::Best, FormatPreset::Best);
    assert_ne!(FormatPreset::Best, FormatPreset::Low);
    assert_eq!(
        FormatPreset::Custom("abc".to_string()),
        FormatPreset::Custom("abc".to_string())
    );
}

// ============================== PlayerClient ==============================

#[test]
fn player_client_debug() {
    assert_eq!(format!("{:?}", PlayerClient::Android), "Android");
    assert_eq!(format!("{:?}", PlayerClient::IOS), "IOS");
    assert_eq!(format!("{:?}", PlayerClient::Web), "Web");
    assert_eq!(format!("{:?}", PlayerClient::TvEmbedded), "TvEmbedded");
}

#[test]
fn player_client_eq() {
    assert_eq!(PlayerClient::Android, PlayerClient::Android);
    assert_ne!(PlayerClient::Android, PlayerClient::Web);
}

#[test]
fn player_client_copy() {
    let client = PlayerClient::IOS;
    let copy = client;
    assert_eq!(client, copy);
}

// ============================== ExtractorConfig ==============================

#[test]
fn youtube_extractor_config_with_arg() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_arg("--no-check-certificate".to_string());
    // Verify it doesn't panic and the arg is stored
    let debug = format!("{:?}", yt);
    assert!(debug.contains("no-check-certificate"));
}

#[test]
fn youtube_extractor_config_with_timeout() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_timeout(Duration::from_secs(120));
    let debug = format!("{:?}", yt);
    assert!(debug.contains("120"));
}

#[test]
fn youtube_clone() {
    let yt = Youtube::new(PathBuf::from("yt-dlp"));
    let clone = yt.clone();
    assert_eq!(format!("{:?}", yt), format!("{:?}", clone));
}

// ============================== build_base_args (tests PlayerClient::as_arg indirectly) ==============================

#[test]
fn build_base_args_default() {
    let yt = Youtube::new(PathBuf::from("yt-dlp"));
    let args = yt.build_base_args();
    assert!(args.contains(&"--no-progress".to_string()));
    assert!(args.contains(&"--dump-json".to_string()));
    // No extractor-args or format preset by default
    assert!(!args.iter().any(|a| a.contains("--extractor-args")));
    assert!(!args.iter().any(|a| a == "-f"));
}

#[test]
fn build_base_args_with_player_client_android() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_player_client(PlayerClient::Android);
    let args = yt.build_base_args();
    assert!(args.contains(&"--extractor-args".to_string()));
    let ea_value = args.iter().find(|a| a.contains("youtube:")).unwrap();
    assert!(ea_value.contains("player_client=android"));
}

#[test]
fn build_base_args_with_player_client_ios() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_player_client(PlayerClient::IOS);
    let args = yt.build_base_args();
    let ea_value = args.iter().find(|a| a.contains("youtube:")).unwrap();
    assert!(ea_value.contains("player_client=ios"));
}

#[test]
fn build_base_args_with_player_client_web() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_player_client(PlayerClient::Web);
    let args = yt.build_base_args();
    let ea_value = args.iter().find(|a| a.contains("youtube:")).unwrap();
    assert!(ea_value.contains("player_client=web"));
}

#[test]
fn build_base_args_with_player_client_tv_embedded() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_player_client(PlayerClient::TvEmbedded);
    let args = yt.build_base_args();
    let ea_value = args.iter().find(|a| a.contains("youtube:")).unwrap();
    assert!(ea_value.contains("player_client=tv_embedded"));
}

#[test]
fn build_base_args_with_skip_dash() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.skip_dash_manifest(true);
    let args = yt.build_base_args();
    let ea_value = args.iter().find(|a| a.contains("youtube:")).unwrap();
    assert!(ea_value.contains("skip=dash"));
}

#[test]
fn build_base_args_with_player_client_and_skip_dash() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_player_client(PlayerClient::Android);
    yt.skip_dash_manifest(true);
    let args = yt.build_base_args();
    let ea_value = args.iter().find(|a| a.contains("youtube:")).unwrap();
    assert!(ea_value.contains("player_client=android"));
    assert!(ea_value.contains("skip=dash"));
    assert!(ea_value.contains(';'));
}

// ============================== build_base_args (tests FormatPreset::to_format_selector indirectly) ==============================

#[test]
fn build_base_args_with_format_preset_best() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_format_preset(FormatPreset::Best);
    let args = yt.build_base_args();
    assert!(args.contains(&"-f".to_string()));
    assert!(args.contains(&"bestvideo+bestaudio/best".to_string()));
}

#[test]
fn build_base_args_with_format_preset_audio_only() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_format_preset(FormatPreset::AudioOnly);
    let args = yt.build_base_args();
    assert!(args.contains(&"-f".to_string()));
    assert!(args.contains(&"bestaudio/best".to_string()));
}

#[test]
fn build_base_args_with_format_preset_medium() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_format_preset(FormatPreset::Medium);
    let args = yt.build_base_args();
    assert!(args.contains(&"-f".to_string()));
    let format_arg = args.iter().find(|a| a.contains("720")).unwrap();
    assert!(format_arg.contains("bestvideo[height<=720]"));
}

#[test]
fn build_base_args_with_format_preset_custom() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_format_preset(FormatPreset::Custom("best[height<=360]".to_string()));
    let args = yt.build_base_args();
    assert!(args.contains(&"-f".to_string()));
    assert!(args.contains(&"best[height<=360]".to_string()));
}

#[test]
fn build_base_args_with_format_preset_modern_codecs() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_format_preset(FormatPreset::ModernCodecs);
    let args = yt.build_base_args();
    let selector = args.iter().find(|a| a.contains("vp9")).unwrap();
    assert!(selector.contains("opus"));
}

#[test]
fn build_base_args_with_format_preset_legacy_compatible() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_format_preset(FormatPreset::LegacyCompatible);
    let args = yt.build_base_args();
    let selector = args.iter().find(|a| a.contains("mp4")).unwrap();
    assert!(!selector.is_empty());
}

#[test]
fn build_base_args_includes_custom_args() {
    let mut yt = Youtube::new(PathBuf::from("yt-dlp"));
    yt.with_arg("--no-check-certificate".to_string());
    yt.with_arg("--verbose".to_string());
    let args = yt.build_base_args();
    assert!(args.contains(&"--no-check-certificate".to_string()));
    assert!(args.contains(&"--verbose".to_string()));
}
