use pretty_assertions::assert_eq;
use yt_dlp::model::format::Extension;

use crate::common::fixtures;

/// Generic extractor fixtures carry exactly one direct HTTP format.
const GENERIC_FIXTURE_FORMAT_COUNT: usize = 1;
/// `age_limit` is absent in generic output and must default to "no restriction".
const GENERIC_FIXTURE_AGE_LIMIT: i64 = 0;

#[test]
fn parse_generic_video_fixture_succeeds() {
    let video = fixtures::load_generic_video_fixture();

    assert_eq!(video.id, "small");
    assert_eq!(video.title, "small");
    assert_eq!(video.extractor_info.extractor, "generic");
    assert_eq!(video.extractor_info.extractor_key, "Generic");
    assert_eq!(video.version.version, "2026.10.26");
}

#[test]
fn parse_generic_video_fixture_defaults_age_limit() {
    let video = fixtures::load_generic_video_fixture();

    assert_eq!(video.age_limit, GENERIC_FIXTURE_AGE_LIMIT);
}

#[test]
fn parse_generic_video_fixture_leaves_live_fields_unset() {
    let video = fixtures::load_generic_video_fixture();

    assert_eq!(video.live_status, None);
    assert_eq!(video.playable_in_embed, None);
}

#[test]
fn parse_generic_video_fixture_keeps_direct_format() {
    let video = fixtures::load_generic_video_fixture();

    assert_eq!(video.formats.len(), GENERIC_FIXTURE_FORMAT_COUNT);
    assert_eq!(video.formats[0].format_id, "mp4");
    assert_eq!(video.formats[0].download_info.ext, Extension::Mp4);
}
