use yt_dlp::utils::validation::{sanitize_filename, sanitize_path, validate_youtube_url};

// ============================== validate_youtube_url ==============================

#[test]
fn validate_youtube_url_standard() {
    assert!(validate_youtube_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ").is_ok());
}

#[test]
fn validate_youtube_url_short() {
    assert!(validate_youtube_url("https://youtu.be/dQw4w9WgXcQ").is_ok());
}

#[test]
fn validate_youtube_url_nocookie() {
    assert!(validate_youtube_url("https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ").is_ok());
}

#[test]
fn validate_youtube_url_invalid_domain() {
    assert!(validate_youtube_url("https://evil.com/watch?v=dQw4w9WgXcQ").is_err());
}

#[test]
fn validate_youtube_url_unsafe_scheme() {
    assert!(validate_youtube_url("file:///etc/passwd").is_err());
}

#[test]
fn validate_youtube_url_invalid_format() {
    assert!(validate_youtube_url("not a url").is_err());
}

#[test]
fn validate_youtube_url_http() {
    assert!(validate_youtube_url("http://www.youtube.com/watch?v=test").is_ok());
}

// ============================== sanitize_path ==============================

#[test]
fn sanitize_path_simple() {
    let result = sanitize_path("video.mp4").unwrap();
    assert_eq!(result.to_str().unwrap(), "video.mp4");
}

#[test]
fn sanitize_path_nested() {
    let result = sanitize_path("downloads/video.mp4").unwrap();
    assert_eq!(result.to_str().unwrap(), "downloads/video.mp4");
}

#[test]
fn sanitize_path_traversal() {
    assert!(sanitize_path("../../../etc/passwd").is_err());
}

#[test]
fn sanitize_path_absolute() {
    assert!(sanitize_path("/etc/passwd").is_err());
}

#[test]
fn sanitize_path_dotdot_in_component() {
    assert!(sanitize_path("foo..bar/test").is_err());
}

#[test]
fn sanitize_path_current_dir_stripped() {
    // ./video.mp4 -> video.mp4
    let result = sanitize_path("./video.mp4").unwrap();
    assert_eq!(result.to_str().unwrap(), "video.mp4");
}

// ============================== sanitize_filename ==============================

#[test]
fn sanitize_filename_clean() {
    assert_eq!(sanitize_filename("video.mp4"), "video.mp4");
}

#[test]
fn sanitize_filename_removes_slashes() {
    assert_eq!(sanitize_filename("my/video\\file.mp4"), "myvideofile.mp4");
}

#[test]
fn sanitize_filename_removes_colons() {
    assert_eq!(sanitize_filename("file:name.mp4"), "filename.mp4");
}

#[test]
fn sanitize_filename_removes_special_chars() {
    assert_eq!(sanitize_filename("a*b?c<d>e|f.mp4"), "abcdef.mp4");
}

#[test]
fn sanitize_filename_removes_double_dots() {
    let result = sanitize_filename("file..name.mp4");
    assert!(!result.contains(".."));
}

#[test]
fn sanitize_filename_empty_becomes_download() {
    assert_eq!(sanitize_filename(""), "download");
}

#[test]
fn sanitize_filename_only_special_becomes_download() {
    assert_eq!(sanitize_filename("///"), "download");
}

// ============================== sanitize_filename: control characters ==============================

#[test]
fn sanitize_filename_removes_null_bytes() {
    let result = sanitize_filename("file\x00name.mp4");
    assert!(!result.contains('\x00'));
    assert_eq!(result, "filename.mp4");
}

#[test]
fn sanitize_filename_removes_control_chars() {
    let result = sanitize_filename("file\x01\x02\x03name.mp4");
    assert_eq!(result, "filename.mp4");
}

#[test]
fn sanitize_filename_removes_tab_and_newline() {
    let result = sanitize_filename("file\tname\n.mp4");
    assert!(!result.contains('\t'));
    assert!(!result.contains('\n'));
}

// ============================== sanitize_filename: edge cases ==============================

#[test]
fn sanitize_filename_whitespace_only_becomes_download() {
    assert_eq!(sanitize_filename("   "), "download");
}

#[test]
fn sanitize_filename_preserves_unicode() {
    let result = sanitize_filename("日本語ファイル.mp4");
    assert_eq!(result, "日本語ファイル.mp4");
}

#[test]
fn sanitize_filename_removes_quotes() {
    let result = sanitize_filename("file\"name\".mp4");
    assert!(!result.contains('"'));
}

#[test]
fn sanitize_filename_long_name() {
    let name = "a".repeat(300) + ".mp4";
    let result = sanitize_filename(&name);
    assert!(!result.is_empty());
}

// ============================== validate_youtube_url edge cases ==============================

#[test]
fn validate_youtube_url_ftp_scheme() {
    assert!(validate_youtube_url("ftp://www.youtube.com/watch?v=test").is_err());
}

#[test]
fn validate_youtube_url_javascript_scheme() {
    assert!(validate_youtube_url("javascript:alert(1)").is_err());
}

#[test]
fn validate_youtube_url_music_subdomain() {
    assert!(validate_youtube_url("https://music.youtube.com/watch?v=test").is_ok());
}

#[test]
fn validate_youtube_url_mobile_subdomain() {
    assert!(validate_youtube_url("https://m.youtube.com/watch?v=test").is_ok());
}

// ============================== sanitize_path edge cases ==============================

#[test]
fn sanitize_path_empty_after_filter() {
    assert!(sanitize_path("..").is_err());
}

#[test]
fn sanitize_path_multiple_traversals() {
    assert!(sanitize_path("a/../../b").is_err());
}

#[test]
fn sanitize_path_deeply_nested_valid() {
    let result = sanitize_path("a/b/c/d/e/file.mp4").unwrap();
    assert_eq!(result.to_str().unwrap(), "a/b/c/d/e/file.mp4");
}
