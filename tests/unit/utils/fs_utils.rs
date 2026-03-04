use std::path::Path;

use yt_dlp::utils::fs::{
    create_temp_path, determine_mime_type, try_extension, try_name, try_parent, try_path_str, try_without_extension,
};

// ============================== try_path_str ==============================

#[test]
fn try_path_str_valid() {
    let path = Path::new("/tmp/video.mp4");
    assert_eq!(try_path_str(path).unwrap(), "/tmp/video.mp4");
}

#[test]
fn try_path_str_relative() {
    let path = Path::new("video.mp4");
    assert_eq!(try_path_str(path).unwrap(), "video.mp4");
}

#[test]
fn try_path_str_unicode() {
    let path = Path::new("/tmp/vidéo.mp4");
    assert_eq!(try_path_str(path).unwrap(), "/tmp/vidéo.mp4");
}

// ============================== try_extension ==============================

#[test]
fn try_extension_mp4() {
    let path = Path::new("/tmp/video.MP4");
    assert_eq!(try_extension(path).unwrap(), "mp4"); // lowercased
}

#[test]
fn try_extension_webm() {
    let path = Path::new("video.webm");
    assert_eq!(try_extension(path).unwrap(), "webm");
}

#[test]
fn try_extension_no_ext() {
    let path = Path::new("/tmp/video");
    assert!(try_extension(path).is_err());
}

#[test]
fn try_extension_hidden_file() {
    // .gitignore is treated as a file with no extension on Unix
    let path = Path::new(".gitignore");
    assert!(try_extension(path).is_err());
}

// ============================== create_temp_path ==============================

#[test]
fn create_temp_path_preserves_parent() {
    let path = Path::new("/tmp/downloads/video.mp4");
    let temp = create_temp_path(path, "mp4");
    assert_eq!(temp.parent().unwrap(), Path::new("/tmp/downloads"));
}

#[test]
fn create_temp_path_uses_extension() {
    let path = Path::new("/tmp/video.mp4");
    let temp = create_temp_path(path, "mkv");
    assert_eq!(temp.extension().unwrap(), "mkv");
}

#[test]
fn create_temp_path_contains_stem() {
    let path = Path::new("/tmp/my_video.mp4");
    let temp = create_temp_path(path, "mp4");
    let name = temp.file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with("my_video_"));
    assert!(name.contains("_temp."));
}

#[test]
fn create_temp_path_unique() {
    let path = Path::new("/tmp/video.mp4");
    let temp1 = create_temp_path(path, "mp4");
    let temp2 = create_temp_path(path, "mp4");
    assert_ne!(temp1, temp2); // UUID-based, always different
}

#[test]
fn create_temp_path_no_parent() {
    let path = Path::new("video.mp4");
    let temp = create_temp_path(path, "mp4");
    let name = temp.file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with("video_"));
}

// ============================== determine_mime_type ==============================

#[test]
fn mime_type_mp4() {
    assert_eq!(determine_mime_type(Path::new("video.mp4")), "video/mp4");
}

#[test]
fn mime_type_webm() {
    assert_eq!(determine_mime_type(Path::new("video.webm")), "video/webm");
}

#[test]
fn mime_type_mp3() {
    assert_eq!(determine_mime_type(Path::new("audio.mp3")), "audio/mpeg");
}

#[test]
fn mime_type_m4a() {
    assert_eq!(determine_mime_type(Path::new("audio.m4a")), "audio/mp4");
}

#[test]
fn mime_type_jpg() {
    assert_eq!(determine_mime_type(Path::new("thumb.jpg")), "image/jpeg");
}

#[test]
fn mime_type_jpeg() {
    assert_eq!(determine_mime_type(Path::new("thumb.jpeg")), "image/jpeg");
}

#[test]
fn mime_type_png() {
    assert_eq!(determine_mime_type(Path::new("thumb.png")), "image/png");
}

#[test]
fn mime_type_vtt() {
    assert_eq!(determine_mime_type(Path::new("subs.vtt")), "text/vtt");
}

#[test]
fn mime_type_srt() {
    assert_eq!(determine_mime_type(Path::new("subs.srt")), "application/x-subrip");
}

#[test]
fn mime_type_ass() {
    assert_eq!(determine_mime_type(Path::new("subs.ass")), "text/x-ssa");
}

#[test]
fn mime_type_ssa() {
    assert_eq!(determine_mime_type(Path::new("subs.ssa")), "text/x-ssa");
}

#[test]
fn mime_type_unknown() {
    assert_eq!(determine_mime_type(Path::new("file.xyz")), "application/octet-stream");
}

#[test]
fn mime_type_no_extension() {
    assert_eq!(determine_mime_type(Path::new("noext")), "application/octet-stream");
}

#[test]
fn mime_type_case_insensitive() {
    assert_eq!(determine_mime_type(Path::new("video.MP4")), "video/mp4");
}

// ============================== try_name ==============================

#[test]
fn try_name_simple() {
    assert_eq!(try_name("/tmp/video.mp4").unwrap(), "video.mp4");
}

#[test]
fn try_name_no_parent() {
    assert_eq!(try_name("video.mp4").unwrap(), "video.mp4");
}

// ============================== try_without_extension ==============================

#[test]
fn try_without_extension_simple() {
    assert_eq!(try_without_extension("/tmp/video.mp4").unwrap(), "video");
}

#[test]
fn try_without_extension_dotted() {
    assert_eq!(try_without_extension("/tmp/my.video.mp4").unwrap(), "my.video");
}

// ============================== try_parent ==============================

#[test]
fn try_parent_simple() {
    let parent = try_parent("/tmp/downloads/video.mp4").unwrap();
    assert_eq!(parent, Path::new("/tmp/downloads"));
}

#[test]
fn try_parent_nested() {
    let parent = try_parent("/a/b/c/d.txt").unwrap();
    assert_eq!(parent, Path::new("/a/b/c"));
}
