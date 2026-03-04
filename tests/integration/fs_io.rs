use std::io::Write;

// ---------------------------------------------------------------------------
// create_file — creates file and parent directories
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_file_with_parent_dirs() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let path = dir.path().join("output.txt");

    let _file = yt_dlp::utils::fs::create_file(&path).await.expect("create_file failed");

    assert!(path.exists(), "file should be created");
}

#[tokio::test]
async fn create_file_overwrites_existing() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let path = dir.path().join("existing.txt");

    // Create file first time
    let _file = yt_dlp::utils::fs::create_file(&path).await.expect("create_file failed");
    assert!(path.exists());

    // Create again — should not error
    let _file = yt_dlp::utils::fs::create_file(&path)
        .await
        .expect("create_file again failed");
    assert!(path.exists());
}

// ---------------------------------------------------------------------------
// create_dir — idempotent directory creation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_dir_idempotent() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let path = dir.path().join("new_dir");

    yt_dlp::utils::fs::create_dir(&path).await.expect("create_dir failed");
    assert!(path.is_dir());

    // Call again — should not error
    yt_dlp::utils::fs::create_dir(&path)
        .await
        .expect("create_dir again failed");
    assert!(path.is_dir());
}

#[tokio::test]
async fn create_dir_nested() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let path = dir.path().join("x/y/z");

    yt_dlp::utils::fs::create_dir(&path)
        .await
        .expect("create_dir nested failed");
    assert!(path.is_dir());
}

// ---------------------------------------------------------------------------
// create_parent_dir
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_parent_dir() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let file_path = dir.path().join("parent/child/file.txt");

    yt_dlp::utils::fs::create_parent_dir(&file_path)
        .await
        .expect("create_parent_dir failed");
    assert!(file_path.parent().unwrap().is_dir());
}

// ---------------------------------------------------------------------------
// extract_zip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn extract_zip() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let zip_path = dir.path().join("test.zip");
    let extract_dir = dir.path().join("extracted");

    // Create a small zip in memory
    {
        let file = std::fs::File::create(&zip_path).expect("create zip file");
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        zip.start_file("hello.txt", options).expect("zip start_file");
        zip.write_all(b"Hello from zip!").expect("zip write");
        zip.finish().expect("zip finish");
    }

    yt_dlp::utils::fs::extract_zip(&zip_path, &extract_dir)
        .await
        .expect("extract_zip failed");

    let extracted = extract_dir.join("hello.txt");
    assert!(extracted.exists(), "extracted file should exist");
    let content = std::fs::read_to_string(&extracted).expect("read extracted");
    assert_eq!(content, "Hello from zip!");
}

// ---------------------------------------------------------------------------
// remove_temp_file
// ---------------------------------------------------------------------------

#[tokio::test]
async fn remove_temp_file() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let path = dir.path().join("temp_file.tmp");

    std::fs::write(&path, b"temporary").expect("write temp file");
    assert!(path.exists());

    let removed = yt_dlp::utils::fs::remove_temp_file(&path).await;
    assert!(removed);
    assert!(!path.exists());
}

#[tokio::test]
async fn remove_temp_file_nonexistent() {
    let removed = yt_dlp::utils::fs::remove_temp_file("/tmp/nonexistent_file_12345.tmp").await;
    assert!(!removed);
}

// ---------------------------------------------------------------------------
// random_filename
// ---------------------------------------------------------------------------

#[test]
fn random_filename_length() {
    let name = yt_dlp::utils::fs::random_filename(12);
    assert_eq!(name.len(), 12);
}

#[test]
fn random_filename_unique() {
    let a = yt_dlp::utils::fs::random_filename(20);
    let b = yt_dlp::utils::fs::random_filename(20);
    assert_ne!(a, b);
}

// ---------------------------------------------------------------------------
// extract_video_id
// ---------------------------------------------------------------------------

#[test]
fn extract_video_id_from_filename() {
    let id = yt_dlp::utils::fs::extract_video_id("My Video [dQw4w9WgXcQ].mp4");
    assert_eq!(id.as_deref(), Some("dQw4w9WgXcQ"));
}

#[test]
fn extract_video_id_no_brackets() {
    let id = yt_dlp::utils::fs::extract_video_id("video.mp4");
    assert!(id.is_none());
}

// ---------------------------------------------------------------------------
// set_executable (Unix only)
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[tokio::test]
async fn set_executable() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let path = dir.path().join("script.sh");
    std::fs::write(&path, "#!/bin/sh\necho hello").expect("write script");

    yt_dlp::utils::fs::set_executable(&path)
        .await
        .expect("set_executable failed");

    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::metadata(&path).unwrap().permissions();
    assert!(perms.mode() & 0o111 != 0, "should have execute permission");
}
