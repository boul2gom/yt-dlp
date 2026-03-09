use yt_dlp::download::PartialRange;
use yt_dlp::model::chapter::Chapter;

fn make_chapters() -> Vec<Chapter> {
    vec![
        Chapter {
            start_time: 0.0,
            end_time: 30.0,
            title: Some("Intro".to_string()),
        },
        Chapter {
            start_time: 30.0,
            end_time: 90.0,
            title: Some("Part 2".to_string()),
        },
        Chapter {
            start_time: 90.0,
            end_time: 180.0,
            title: Some("Part 3".to_string()),
        },
    ]
}

// ---------------------------------------------------------------------------
// time_range constructor validation
// ---------------------------------------------------------------------------

#[test]
fn time_range_valid_returns_ok() {
    let range = PartialRange::time_range(0.0, 60.0);
    assert!(range.is_ok());
    assert_eq!(range.unwrap(), PartialRange::TimeRange { start: 0.0, end: 60.0 });
}

#[test]
fn time_range_negative_start_returns_err() {
    assert!(PartialRange::time_range(-1.0, 60.0).is_err());
}

#[test]
fn time_range_start_equals_end_returns_err() {
    assert!(PartialRange::time_range(30.0, 30.0).is_err());
}

#[test]
fn time_range_start_greater_than_end_returns_err() {
    assert!(PartialRange::time_range(90.0, 30.0).is_err());
}

// ---------------------------------------------------------------------------
// chapter_range constructor validation
// ---------------------------------------------------------------------------

#[test]
fn chapter_range_valid_returns_ok() {
    let range = PartialRange::chapter_range(0, 2);
    assert!(range.is_ok());
    assert_eq!(range.unwrap(), PartialRange::ChapterRange { start: 0, end: 2 });
}

#[test]
fn chapter_range_equal_start_end_returns_ok() {
    assert!(PartialRange::chapter_range(2, 2).is_ok());
}

#[test]
fn chapter_range_start_greater_than_end_returns_err() {
    assert!(PartialRange::chapter_range(3, 1).is_err());
}

// ---------------------------------------------------------------------------
// single_chapter constructor
// ---------------------------------------------------------------------------

#[test]
fn single_chapter_returns_self() {
    let range = PartialRange::single_chapter(5);
    assert_eq!(range, PartialRange::SingleChapter { index: 5 });
}

// ---------------------------------------------------------------------------
// needs_chapter_metadata
// ---------------------------------------------------------------------------

#[test]
fn time_range_needs_no_chapter_metadata() {
    let range = PartialRange::TimeRange { start: 0.0, end: 60.0 };
    assert!(!range.needs_chapter_metadata());
}

#[test]
fn chapter_range_needs_chapter_metadata() {
    let range = PartialRange::ChapterRange { start: 0, end: 2 };
    assert!(range.needs_chapter_metadata());
}

#[test]
fn single_chapter_needs_chapter_metadata() {
    assert!(PartialRange::SingleChapter { index: 0 }.needs_chapter_metadata());
}

// ---------------------------------------------------------------------------
// get_times
// ---------------------------------------------------------------------------

#[test]
fn time_range_get_times_returns_pair() {
    let range = PartialRange::TimeRange { start: 1.5, end: 10.0 };
    assert_eq!(range.get_times(), Some((1.5, 10.0)));
}

#[test]
fn chapter_range_get_times_returns_none() {
    assert_eq!(PartialRange::ChapterRange { start: 0, end: 1 }.get_times(), None);
}

#[test]
fn single_chapter_get_times_returns_none() {
    assert_eq!(PartialRange::SingleChapter { index: 0 }.get_times(), None);
}

// ---------------------------------------------------------------------------
// to_ytdlp_arg
// ---------------------------------------------------------------------------

#[test]
fn time_range_to_ytdlp_arg_formats_correctly() {
    let range = PartialRange::TimeRange {
        start: 90.0,
        end: 300.0,
    };
    assert_eq!(range.to_ytdlp_arg(), "*00:01:30.000-00:05:00.000");
}

#[test]
fn chapter_range_to_ytdlp_arg_formats_correctly() {
    let range = PartialRange::ChapterRange { start: 2, end: 5 };
    assert_eq!(range.to_ytdlp_arg(), "chapters:2-5");
}

#[test]
fn single_chapter_to_ytdlp_arg_formats_correctly() {
    assert_eq!(PartialRange::SingleChapter { index: 3 }.to_ytdlp_arg(), "chapters:3-3");
}

// ---------------------------------------------------------------------------
// to_time_range
// ---------------------------------------------------------------------------

#[test]
fn to_time_range_from_time_range_returns_clone() {
    let range = PartialRange::TimeRange { start: 10.0, end: 50.0 };
    let result = range.to_time_range(&[]);
    assert_eq!(result, Some(PartialRange::TimeRange { start: 10.0, end: 50.0 }));
}

#[test]
fn to_time_range_from_chapter_range_resolves() {
    let chapters = make_chapters();
    let range = PartialRange::ChapterRange { start: 0, end: 1 };
    let result = range.to_time_range(&chapters);
    // chapter 0: 0.0-30.0, chapter 1: 30.0-90.0 → combined 0.0-90.0
    assert_eq!(result, Some(PartialRange::TimeRange { start: 0.0, end: 90.0 }));
}

#[test]
fn to_time_range_from_single_chapter_resolves() {
    let chapters = make_chapters();
    let range = PartialRange::SingleChapter { index: 1 };
    let result = range.to_time_range(&chapters);
    // chapter 1: 30.0-90.0
    assert_eq!(result, Some(PartialRange::TimeRange { start: 30.0, end: 90.0 }));
}

#[test]
fn to_time_range_chapter_range_out_of_bounds_returns_none() {
    let chapters = make_chapters(); // 3 chapters, indices 0-2
    let range = PartialRange::ChapterRange { start: 0, end: 5 };
    assert_eq!(range.to_time_range(&chapters), None);
}

#[test]
fn to_time_range_single_chapter_out_of_bounds_returns_none() {
    let chapters = make_chapters();
    assert_eq!(PartialRange::SingleChapter { index: 99 }.to_time_range(&chapters), None);
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

#[test]
fn display_time_range() {
    let range = PartialRange::TimeRange {
        start: 90.0,
        end: 300.0,
    };
    assert_eq!(format!("{range}"), "TimeRange(start=00:01:30.000, end=00:05:00.000)");
}

#[test]
fn display_chapter_range() {
    let range = PartialRange::ChapterRange { start: 0, end: 2 };
    assert_eq!(format!("{range}"), "ChapterRange(start=0, end=2)");
}

#[test]
fn display_single_chapter() {
    assert_eq!(
        format!("{}", PartialRange::SingleChapter { index: 5 }),
        "SingleChapter(index=5)"
    );
}
