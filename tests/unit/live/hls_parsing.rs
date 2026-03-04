use yt_dlp::live::hls::{HlsPlaylist, HlsSegment, HlsVariant, select_variant};

// ============================== HlsSegment ==============================

#[test]
fn hls_segment_display() {
    let segment = HlsSegment {
        url: "https://example.com/seg0.ts".to_string(),
        duration: 6.0,
        sequence: 42,
    };
    let display = format!("{}", segment);
    assert!(display.contains("seq=42"));
    assert!(display.contains("6.00s"));
}

#[test]
fn hls_segment_clone() {
    let segment = HlsSegment {
        url: "https://example.com/seg0.ts".to_string(),
        duration: 6.0,
        sequence: 0,
    };
    let cloned = segment.clone();
    assert_eq!(segment.url, cloned.url);
    assert!((segment.duration - cloned.duration).abs() < f64::EPSILON);
    assert_eq!(segment.sequence, cloned.sequence);
}

// ============================== HlsPlaylist ==============================

#[test]
fn hls_playlist_display() {
    let playlist = HlsPlaylist {
        target_duration: 6.0,
        media_sequence: 100,
        segments: vec![
            HlsSegment {
                url: "https://example.com/seg100.ts".to_string(),
                duration: 6.0,
                sequence: 100,
            },
            HlsSegment {
                url: "https://example.com/seg101.ts".to_string(),
                duration: 6.0,
                sequence: 101,
            },
        ],
        is_endlist: false,
    };
    let display = format!("{}", playlist);
    assert!(display.contains("segments=2"));
    assert!(display.contains("endlist=false"));
    assert!(display.contains("media_sequence=100"));
}

#[test]
fn hls_playlist_endlist() {
    let playlist = HlsPlaylist {
        target_duration: 10.0,
        media_sequence: 0,
        segments: vec![],
        is_endlist: true,
    };
    assert!(playlist.is_endlist);
}

// ============================== HlsVariant ==============================

#[test]
fn hls_variant_display() {
    let variant = HlsVariant {
        url: "https://example.com/720p.m3u8".to_string(),
        bandwidth: 3_000_000,
        resolution: Some("1280x720".to_string()),
        codecs: Some("avc1.4d001f,mp4a.40.2".to_string()),
    };
    let display = format!("{}", variant);
    assert!(display.contains("bandwidth=3000000"));
    assert!(display.contains("1280x720"));
}

#[test]
fn hls_variant_display_no_resolution() {
    let variant = HlsVariant {
        url: "https://example.com/audio.m3u8".to_string(),
        bandwidth: 128_000,
        resolution: None,
        codecs: None,
    };
    let display = format!("{}", variant);
    assert!(display.contains("unknown"));
}

// ============================== select_variant ==============================

#[test]
fn select_variant_empty_list() {
    let variants: Vec<HlsVariant> = vec![];
    assert!(select_variant(&variants, None).is_none());
}

#[test]
fn select_variant_best_no_target() {
    let variants = vec![
        HlsVariant {
            url: "https://example.com/360p.m3u8".to_string(),
            bandwidth: 500_000,
            resolution: Some("640x360".to_string()),
            codecs: None,
        },
        HlsVariant {
            url: "https://example.com/720p.m3u8".to_string(),
            bandwidth: 3_000_000,
            resolution: Some("1280x720".to_string()),
            codecs: None,
        },
        HlsVariant {
            url: "https://example.com/1080p.m3u8".to_string(),
            bandwidth: 6_000_000,
            resolution: Some("1920x1080".to_string()),
            codecs: None,
        },
    ];
    let best = select_variant(&variants, None).unwrap();
    assert_eq!(best.bandwidth, 6_000_000);
    assert_eq!(best.resolution.as_deref(), Some("1920x1080"));
}

#[test]
fn select_variant_with_target_bandwidth() {
    let variants = vec![
        HlsVariant {
            url: "https://example.com/360p.m3u8".to_string(),
            bandwidth: 500_000,
            resolution: Some("640x360".to_string()),
            codecs: None,
        },
        HlsVariant {
            url: "https://example.com/720p.m3u8".to_string(),
            bandwidth: 3_000_000,
            resolution: Some("1280x720".to_string()),
            codecs: None,
        },
        HlsVariant {
            url: "https://example.com/1080p.m3u8".to_string(),
            bandwidth: 6_000_000,
            resolution: Some("1920x1080".to_string()),
            codecs: None,
        },
    ];
    // Target 4Mbps should select 720p (3Mbps, highest under target)
    let selected = select_variant(&variants, Some(4_000_000)).unwrap();
    assert_eq!(selected.bandwidth, 3_000_000);
}

#[test]
fn select_variant_target_below_all() {
    let variants = vec![
        HlsVariant {
            url: "https://example.com/720p.m3u8".to_string(),
            bandwidth: 3_000_000,
            resolution: Some("1280x720".to_string()),
            codecs: None,
        },
        HlsVariant {
            url: "https://example.com/1080p.m3u8".to_string(),
            bandwidth: 6_000_000,
            resolution: Some("1920x1080".to_string()),
            codecs: None,
        },
    ];
    // Target 100Kbps is below all variants -> falls back to lowest
    let selected = select_variant(&variants, Some(100_000)).unwrap();
    assert_eq!(selected.bandwidth, 3_000_000);
}

#[test]
fn select_variant_exact_match() {
    let variants = vec![
        HlsVariant {
            url: "https://example.com/360p.m3u8".to_string(),
            bandwidth: 500_000,
            resolution: None,
            codecs: None,
        },
        HlsVariant {
            url: "https://example.com/720p.m3u8".to_string(),
            bandwidth: 3_000_000,
            resolution: None,
            codecs: None,
        },
    ];
    let selected = select_variant(&variants, Some(3_000_000)).unwrap();
    assert_eq!(selected.bandwidth, 3_000_000);
}

#[test]
fn select_variant_single() {
    let variants = vec![HlsVariant {
        url: "https://example.com/only.m3u8".to_string(),
        bandwidth: 1_000_000,
        resolution: None,
        codecs: None,
    }];
    let selected = select_variant(&variants, None).unwrap();
    assert_eq!(selected.bandwidth, 1_000_000);
}
