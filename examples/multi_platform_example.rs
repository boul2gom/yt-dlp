//! Example demonstrating multi-platform video downloading with the new MediaDownloader.

use std::path::PathBuf;
use yt_dlp::fetcher::deps::Libraries;
use yt_dlp::{MediaDownloader, extractor::ExtractorConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup directories
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    // Setup yt-dlp and ffmpeg paths
    let yt_dlp = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    // Create libraries configuration
    let libraries = Libraries::new(yt_dlp, ffmpeg);

    // Create the media downloader
    let mut downloader = MediaDownloader::new(libraries, output_dir)?;

    // Configure extractor-specific options
    let mut config = ExtractorConfig::default();
    config.youtube.skip_unavailable = true;
    config.vimeo.include_password_protected = false;
    config.tiktok.include_watermark = false;

    downloader.set_extractor_config(config);

    // Example URLs for different platforms
    let test_urls = vec![
        ("https://www.youtube.com/watch?v=dQw4w9WgXcQ", "YouTube"),
        ("https://vimeo.com/1084537", "Vimeo"),
        ("https://www.twitch.tv/videos/123456789", "Twitch"),
        ("https://www.tiktok.com/@user/video/123456789", "TikTok"),
        ("https://www.instagram.com/p/ABC123/", "Instagram"),
        ("https://twitter.com/user/status/123456789", "Twitter"),
        ("https://www.facebook.com/video/123456789", "Facebook"),
    ];

    println!("🚀 Multi-Platform Video Downloader Example");
    println!("==========================================");

    for (url, platform) in test_urls {
        println!("\n📺 Testing {} URL: {}", platform, url);

        // Detect the extractor
        let extractor = downloader.detect_extractor(url);
        println!("   🔍 Detected extractor: {}", extractor);

        // Check if URL is supported
        let is_supported = downloader.is_url_supported(url);
        println!("   ✅ URL supported: {}", is_supported);

        // For demonstration, we'll only fetch video info (not download)
        // In a real scenario, you would uncomment the download line below
        match downloader.fetch_video_infos(url).await {
            Ok(video) => {
                println!("   📋 Title: {}", video.title);
                println!("   👤 Channel: {}", video.channel);
                println!("   📺 Channel ID: {}", video.channel_id);
                println!("   👀 Views: {}", video.view_count);
                println!("   🎥 Formats available: {}", video.formats.len());

                // Uncomment to actually download the video:
                // let output_filename = format!("{}-video.mp4", platform.to_lowercase());
                // let video_path = downloader.download_video_from_url(url, &output_filename).await?;
                // println!("   💾 Downloaded to: {:?}", video_path);
            }
            Err(e) => {
                println!("   ❌ Error fetching video info: {}", e);
            }
        }
    }

    // Show all supported extractors
    println!("\n🌐 All Supported Extractors:");
    let supported = downloader.supported_extractors();
    for extractor in supported {
        println!("   • {}", extractor);
    }

    println!("\n✨ Multi-platform support demonstration complete!");
    println!(
        "   The library now supports {} different extractors through yt-dlp",
        1868
    );

    Ok(())
}
