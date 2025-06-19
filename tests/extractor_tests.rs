//! Tests for the multi-platform extractor functionality.

use yt_dlp::extractor::{Extractor, ExtractorDetector};

#[test]
fn test_youtube_detection() {
    let detector = ExtractorDetector::new();

    let youtube_urls = vec![
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        "https://youtu.be/dQw4w9WgXcQ",
        "https://m.youtube.com/watch?v=dQw4w9WgXcQ",
        "https://youtube.com/watch?v=dQw4w9WgXcQ",
    ];

    for url in youtube_urls {
        assert_eq!(
            detector.detect(url),
            Extractor::YouTube,
            "Failed to detect YouTube for URL: {}",
            url
        );
        assert!(
            detector.is_supported(url),
            "YouTube URL should be supported: {}",
            url
        );
    }
}

#[test]
fn test_vimeo_detection() {
    let detector = ExtractorDetector::new();

    let vimeo_urls = vec![
        "https://vimeo.com/1084537",
        "https://www.vimeo.com/1084537",
        "https://vimeo.com/channels/staffpicks/123456",
    ];

    for url in vimeo_urls {
        assert_eq!(
            detector.detect(url),
            Extractor::Vimeo,
            "Failed to detect Vimeo for URL: {}",
            url
        );
        assert!(
            detector.is_supported(url),
            "Vimeo URL should be supported: {}",
            url
        );
    }
}

#[test]
fn test_twitch_detection() {
    let detector = ExtractorDetector::new();

    let twitch_urls = vec![
        "https://www.twitch.tv/videos/123456789",
        "https://twitch.tv/streamer",
        "https://clips.twitch.tv/clip-id",
    ];

    for url in twitch_urls {
        assert_eq!(
            detector.detect(url),
            Extractor::Twitch,
            "Failed to detect Twitch for URL: {}",
            url
        );
        assert!(
            detector.is_supported(url),
            "Twitch URL should be supported: {}",
            url
        );
    }
}

#[test]
fn test_tiktok_detection() {
    let detector = ExtractorDetector::new();

    let tiktok_urls = vec![
        "https://www.tiktok.com/@user/video/123456789",
        "https://tiktok.com/@user/video/123456789",
        "https://vm.tiktok.com/shortlink",
    ];

    for url in tiktok_urls {
        assert_eq!(
            detector.detect(url),
            Extractor::TikTok,
            "Failed to detect TikTok for URL: {}",
            url
        );
        assert!(
            detector.is_supported(url),
            "TikTok URL should be supported: {}",
            url
        );
    }
}

#[test]
fn test_instagram_detection() {
    let detector = ExtractorDetector::new();

    let instagram_urls = vec![
        "https://www.instagram.com/p/ABC123/",
        "https://instagram.com/p/ABC123/",
        "https://www.instagram.com/reel/ABC123/",
    ];

    for url in instagram_urls {
        assert_eq!(
            detector.detect(url),
            Extractor::Instagram,
            "Failed to detect Instagram for URL: {}",
            url
        );
        assert!(
            detector.is_supported(url),
            "Instagram URL should be supported: {}",
            url
        );
    }
}

#[test]
fn test_twitter_detection() {
    let detector = ExtractorDetector::new();

    let twitter_urls = vec![
        "https://twitter.com/user/status/123456789",
        "https://www.twitter.com/user/status/123456789",
        "https://x.com/user/status/123456789",
        "https://www.x.com/user/status/123456789",
    ];

    for url in twitter_urls {
        assert_eq!(
            detector.detect(url),
            Extractor::Twitter,
            "Failed to detect Twitter for URL: {}",
            url
        );
        assert!(
            detector.is_supported(url),
            "Twitter URL should be supported: {}",
            url
        );
    }
}

#[test]
fn test_facebook_detection() {
    let detector = ExtractorDetector::new();

    let facebook_urls = vec![
        "https://www.facebook.com/video/123456789",
        "https://facebook.com/video/123456789",
        "https://fb.watch/shortlink",
    ];

    for url in facebook_urls {
        assert_eq!(
            detector.detect(url),
            Extractor::Facebook,
            "Failed to detect Facebook for URL: {}",
            url
        );
        assert!(
            detector.is_supported(url),
            "Facebook URL should be supported: {}",
            url
        );
    }
}

#[test]
fn test_generic_detection() {
    let detector = ExtractorDetector::new();

    let unsupported_urls = vec![
        "https://example.com/video",
        "https://unknown-site.com/content",
        "https://not-supported.org/media",
    ];

    for url in unsupported_urls {
        assert_eq!(
            detector.detect(url),
            Extractor::Generic,
            "Should detect as Generic for URL: {}",
            url
        );
        assert!(
            !detector.is_supported(url),
            "Unsupported URL should return false: {}",
            url
        );
    }
}

#[test]
fn test_supported_extractors_list() {
    let detector = ExtractorDetector::new();
    let supported = detector.supported_extractors();

    // Should contain all major platforms
    assert!(supported.contains(&Extractor::YouTube));
    assert!(supported.contains(&Extractor::Vimeo));
    assert!(supported.contains(&Extractor::Twitch));
    assert!(supported.contains(&Extractor::TikTok));
    assert!(supported.contains(&Extractor::Instagram));
    assert!(supported.contains(&Extractor::Twitter));
    assert!(supported.contains(&Extractor::Facebook));

    // Should have exactly 7 supported extractors (excluding Generic)
    assert_eq!(supported.len(), 7);
}

#[test]
fn test_extractor_display() {
    assert_eq!(format!("{}", Extractor::YouTube), "YouTube");
    assert_eq!(format!("{}", Extractor::Vimeo), "Vimeo");
    assert_eq!(format!("{}", Extractor::Twitch), "Twitch");
    assert_eq!(format!("{}", Extractor::TikTok), "TikTok");
    assert_eq!(format!("{}", Extractor::Instagram), "Instagram");
    assert_eq!(format!("{}", Extractor::Twitter), "Twitter/X");
    assert_eq!(format!("{}", Extractor::Facebook), "Facebook");
    assert_eq!(format!("{}", Extractor::Generic), "Generic");
}
