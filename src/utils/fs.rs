//! Tools for working with the file system.

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tar::Archive;
use tokio::fs::{File, OpenOptions};
use uuid::Uuid;
use xz2::read::XzDecoder;
use zip::ZipArchive;

use crate::error::{Error, Result};

/// Converts a path to a UTF-8 string reference.
///
/// # Arguments
///
/// * `path` - The path to convert
///
/// # Returns
///
/// The path as a UTF-8 string slice
///
/// # Errors
///
/// Returns `Error::PathValidation` if the path contains invalid UTF-8
pub fn try_path_str(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| Error::path_validation(path, "Path contains invalid UTF-8"))
}

/// Gets the file extension from a path, lowercased.
///
/// # Arguments
///
/// * `path` - The path to extract the extension from
///
/// # Returns
///
/// Lowercase file extension string
///
/// # Errors
///
/// Returns `Error::PathValidation` if the file has no extension or contains invalid characters
pub fn try_extension(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .ok_or_else(|| Error::path_validation(path, "File has no extension"))?
        .to_str()
        .ok_or_else(|| Error::path_validation(path, "Invalid characters in file extension"))?
        .to_lowercase();

    Ok(ext)
}

/// Creates a temporary output path for file processing.
///
/// # Arguments
///
/// * `file_path` - Original file path
/// * `file_format` - File extension for the temporary file
///
/// # Returns
///
/// `PathBuf` to a unique temporary file in the same directory
pub fn create_temp_path(file_path: &Path, file_format: &str) -> PathBuf {
    let parent_dir = file_path.parent().unwrap_or_else(|| Path::new(""));
    let uuid = Uuid::new_v4();

    if let Some(file_stem) = file_path.file_stem().and_then(|s| s.to_str()) {
        parent_dir.join(format!("{}_{}_temp.{}", file_stem, uuid, file_format))
    } else {
        parent_dir.join(format!("output_{}_temp.{}", uuid, file_format))
    }
}

/// Determines the MIME type of a file based on its extension.
///
/// # Arguments
///
/// * `path` - Path to the file
///
/// # Returns
///
/// The MIME type as a string
pub fn determine_mime_type(path: &Path) -> String {
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

    match extension.to_lowercase().as_str() {
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "vtt" => "text/vtt",
        "srt" => "application/x-subrip",
        "ass" | "ssa" => "text/x-ssa",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Returns the name of the given path.
///
/// # Arguments
///
/// * `path` - The path to extract the name from
///
/// # Returns
///
/// The file name as a string
///
/// # Errors
///
/// Returns an error if the path has no file name or contains invalid UTF-8
pub fn try_name(path: impl Into<PathBuf>) -> Result<String> {
    let path: PathBuf = path.into();

    let name = path
        .file_name()
        .ok_or_else(|| Error::path_validation(&path, "Path has no file name"))?;
    let name = name
        .to_str()
        .ok_or_else(|| Error::path_validation(&path, "File name contains invalid UTF-8"))?;

    Ok(name.to_string())
}

/// Returns the name of the given path without the extension.
///
/// # Arguments
///
/// * `path` - The path to extract the name from
///
/// # Returns
///
/// The file name without extension
///
/// # Errors
///
/// Returns an error if the path has no file stem or contains invalid UTF-8
pub fn try_without_extension(path: impl Into<PathBuf>) -> Result<String> {
    let path: PathBuf = path.into();

    let name = path
        .file_stem()
        .ok_or_else(|| Error::path_validation(&path, "Path has no file stem"))?;
    let name = name
        .to_str()
        .ok_or_else(|| Error::path_validation(&path, "File stem contains invalid UTF-8"))?;

    Ok(name.to_string())
}

/// Returns the parent directory of the given path.
///
/// # Arguments
///
/// * `path` - The path to extract the parent from
///
/// # Returns
///
/// The parent directory path
///
/// # Errors
///
/// Returns an error if the path has no parent
pub fn try_parent(path: impl Into<PathBuf>) -> Result<PathBuf> {
    let path: PathBuf = path.into();

    let parent = path
        .parent()
        .ok_or_else(|| Error::path_validation(&path, "Path has no parent directory"))?;

    Ok(parent.to_path_buf())
}

/// Creates a new file at the given destination.
///
/// # Arguments
///
/// * `destination` - The path to create the file at
///
/// # Returns
///
/// An opened file handle
///
/// # Errors
///
/// Returns an error if the file cannot be created
pub async fn create_file(destination: impl Into<PathBuf>) -> Result<File> {
    let destination: PathBuf = destination.into();

    tracing::debug!(
        destination = ?destination,
        "⚙️ Creating new file"
    );

    let mut open_options = OpenOptions::new();
    open_options.read(true);
    open_options.write(true);
    open_options.create(true);
    open_options.truncate(true);

    #[cfg(unix)]
    {
        open_options.mode(0o644);
    }

    let file = open_options.open(&destination).await?;

    tracing::debug!(
        destination = ?destination,
        "✅ File created successfully"
    );

    Ok(file)
}

/// Creates a new directory at the given destination.
/// If the directory already exists, nothing is done.
///
/// # Arguments
///
/// * `destination` - The path to create the directory at
///
/// # Returns
///
/// Ok(()) if the directory was created or already exists
///
/// # Errors
///
/// Returns an error if the directory cannot be created
pub async fn create_dir(destination: impl Into<PathBuf>) -> Result<()> {
    let destination: PathBuf = destination.into();

    tracing::debug!(
        destination = ?destination,
        "⚙️ Creating directory"
    );

    tokio::fs::create_dir_all(&destination).await?;

    tracing::debug!(
        destination = ?destination,
        "✅ Directory created successfully"
    );

    Ok(())
}

/// Creates the parent directory of the given destination.
/// If the parent directory already exists, nothing is done.
///
/// # Arguments
///
/// * `destination` - The path to create the parent directory for
///
/// # Returns
///
/// Ok(()) if the parent directory was created or already exists
///
/// # Errors
///
/// Returns an error if the parent directory cannot be created
pub async fn create_parent_dir(destination: impl Into<PathBuf>) -> Result<()> {
    let destination: PathBuf = destination.into();

    tracing::debug!(
        destination = ?destination,
        "⚙️ Creating parent directory"
    );

    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent).await?;
    } else {
        tokio::fs::create_dir_all(&destination).await?;
    }

    tracing::debug!(
        destination = ?destination,
        "✅ Parent directory created successfully"
    );

    Ok(())
}

/// Extracts a zip file to the given destination.
///
/// # Arguments
///
/// * `zip_path` - The path to the zip file.
/// * `destination` - The path to extract the zip file to.
pub async fn extract_zip(zip_path: impl Into<PathBuf>, destination: impl Into<PathBuf>) -> Result<()> {
    let zip_path: PathBuf = zip_path.into();
    let destination: PathBuf = destination.into();

    tracing::debug!(
        zip_path = ?zip_path,
        destination = ?destination,
        "⚙️ Extracting zip file"
    );

    let zip_path_for_tracing = zip_path.clone();
    let destination_for_tracing = destination.clone();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&zip_path).map_err(|e| Error::io_with_path("open zip file", &zip_path, e))?;

        let mut archive = ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;

            let file_name = file
                .enclosed_name()
                .ok_or_else(|| {
                    Error::path_validation(
                        PathBuf::from(format!("zip entry {}", i)),
                        "Zip entry has no valid file name",
                    )
                })?
                .to_path_buf();

            let dest_path = destination.join(file_name);

            if file.is_dir() {
                std::fs::create_dir_all(&dest_path)
                    .map_err(|e| Error::io_with_path("create directory from zip", &dest_path, e))?;
            } else {
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| Error::io_with_path("create parent directory from zip", parent, e))?;
                }

                let mut outfile = std::fs::File::create(&dest_path)
                    .map_err(|e| Error::io_with_path("create file from zip", &dest_path, e))?;

                std::io::copy(&mut file, &mut outfile)
                    .map_err(|e| Error::io_with_path("copy file content from zip", &dest_path, e))?;
            }

            // Get and set permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(mode))
                        .map_err(|e| Error::io_with_path("set permissions from zip", &dest_path, e))?;
                }
            }
        }

        Ok::<_, Error>(())
    })
    .await
    .map_err(|e| Error::runtime("extract zip archive", e))??;

    tracing::debug!(
        zip_path = ?zip_path_for_tracing,
        destination = ?destination_for_tracing,
        "✅ Zip file extracted successfully"
    );

    Ok(())
}

/// Extracts a tar.xz file to the given destination.
///
/// # Arguments
///
/// * `tar_path` - The path to the tar.xz file.
/// * `destination` - The path to extract the tar.xz file to.
pub async fn extract_tar_xz(tar_path: impl Into<PathBuf>, destination: impl Into<PathBuf>) -> Result<()> {
    let tar_path: PathBuf = tar_path.into();
    let destination: PathBuf = destination.into();

    tracing::debug!(
        tar_path = ?tar_path,
        destination = ?destination,
        "⚙️ Extracting tar.xz file"
    );

    let tar_path_for_tracing = tar_path.clone();
    let destination_for_tracing = destination.clone();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&tar_path).map_err(|e| Error::io_with_path("open tar.xz file", &tar_path, e))?;

        let decompressor = XzDecoder::new(file);
        let mut archive = Archive::new(decompressor);

        archive
            .unpack(&destination)
            .map_err(|e| Error::io_with_path("unpack tar.xz archive", &destination, e))?;

        Ok::<_, Error>(())
    })
    .await
    .map_err(|e| Error::runtime("extract tar.xz archive", e))??;

    tracing::debug!(
        tar_path = ?tar_path_for_tracing,
        destination = ?destination_for_tracing,
        "✅ Tar.xz file extracted successfully"
    );

    Ok(())
}

/// Sets the executable bit on the given file.
///
/// # Arguments
///
/// * `executable` - The path to the executable file.
#[cfg(unix)]
pub async fn set_executable(executable: impl Into<PathBuf>) -> Result<()> {
    let executable: PathBuf = executable.into();

    tracing::debug!(path = ?executable, "⚙️ Setting executable permissions");

    let mut perms = tokio::fs::metadata(&executable).await?.permissions();

    perms.set_mode(0o755);
    tokio::fs::set_permissions(executable, perms).await?;

    Ok(())
}

/// No-op implementation for Windows, as Windows doesn't use executable bits.
///
/// # Arguments
///
/// * `executable` - The path to the executable file.
#[cfg(not(unix))]
pub async fn set_executable(_executable: impl Into<PathBuf>) -> Result<()> {
    // Windows doesn't use executable bits, so this is a no-op
    Ok(())
}

/// Generates a random filename with the specified length.
///
/// # Arguments
///
/// * `length` - The length of the random string to generate.
///
/// # Returns
///
/// A random string of the specified length.
pub fn random_filename(length: usize) -> String {
    let uuid = Uuid::new_v4().to_string().replace('-', "");

    uuid.chars().take(length).collect()
}

use std::sync::LazyLock;

use regex::Regex;

static VIDEO_ID_REGEX_1: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:video|audio)-([a-zA-Z0-9_-]{11})").expect("Invalid regex"));
static VIDEO_ID_REGEX_2: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([a-zA-Z0-9_-]{11})\.[a-zA-Z0-9]+$").expect("Invalid regex"));
static VIDEO_ID_REGEX_3: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[a-zA-Z0-9_-]{11}").expect("Invalid regex"));

/// Extracts a potential video ID from a filename.
pub fn extract_video_id(filename: &str) -> Option<String> {
    // Pattern 1: filename contains "video-[ID]" or "audio-[ID]"
    if let Some(captures) = VIDEO_ID_REGEX_1.captures(filename)
        && let Some(id) = captures.get(1)
    {
        return Some(id.as_str().to_string());
    }

    // Pattern 2: filename contains "[ID].mp4" or "[ID].mp3", etc.
    if let Some(captures) = VIDEO_ID_REGEX_2.captures(filename)
        && let Some(id) = captures.get(1)
    {
        return Some(id.as_str().to_string());
    }

    // Pattern 3: if the name directly contains a YouTube ID (11 characters)
    if let Some(captures) = VIDEO_ID_REGEX_3.captures(filename)
        && let Some(id) = captures.get(0)
    {
        let id_str = id.as_str();
        if id_str.len() == 11 {
            return Some(id_str.to_string());
        }
    }

    None
}

/// Removes a temporary file and logs any errors.
/// Does not propagate errors to avoid interrupting the execution flow.
///
/// # Arguments
///
/// * `file_path` - The path of the file to delete
///
/// # Returns
///
/// `true` if the file was successfully deleted, `false` otherwise
pub async fn remove_temp_file(file_path: impl Into<PathBuf>) -> bool {
    let file_path: PathBuf = file_path.into();
    let result = tokio::fs::remove_file(&file_path).await;

    if let Err(ref e) = result {
        tracing::warn!(path = ?file_path, error = %e, "Failed to remove temporary file");
    }

    result.is_ok()
}
