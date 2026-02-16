//! Tools for working with the file system.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use tar::Archive;
use tokio::fs::{File, OpenOptions};
use uuid::Uuid;
use xz2::read::XzDecoder;
use zip::ZipArchive;

/// Returns the name of the given path.
pub fn try_name(path: impl AsRef<Path>) -> Result<String> {
    let name = path
        .as_ref()
        .file_name()
        .ok_or(Error::Unknown("Failed to get name".to_string()))?;
    let name = name
        .to_str()
        .ok_or(Error::Unknown("Failed to convert name".to_string()))?;

    Ok(name.to_string())
}

/// Returns the name of the given path without the extension.
pub fn try_without_extension(path: impl AsRef<Path>) -> Result<String> {
    let name = path
        .as_ref()
        .file_stem()
        .ok_or(Error::Unknown("Failed to get file stem".to_string()))?;
    let name = name.to_str().ok_or(Error::Unknown(
        "Failed to convert file stem to string".to_string(),
    ))?;

    Ok(name.to_string())
}

/// Returns the parent directory of the given path.
pub fn try_parent(path: impl AsRef<Path>) -> Result<PathBuf> {
    let parent = path
        .as_ref()
        .parent()
        .ok_or(Error::Unknown("Failed to get parent".to_string()))?;

    Ok(parent.to_path_buf())
}

/// Creates a new file at the given destination.
///
/// # Arguments
///
/// * `destination` - The path to create the file at.
pub async fn create_file(destination: impl AsRef<Path> + Send + Sync) -> Result<File> {
    let mut open_options = OpenOptions::new();
    open_options.read(true);
    open_options.write(true);
    open_options.create(true);

    #[cfg(not(target_os = "windows"))]
    {
        open_options.mode(0o755);
    }

    let file = open_options.open(destination).await?;
    Ok(file)
}

/// Creates a new directory at the given destination.
/// If the directory already exists, nothing is done.
///
/// # Arguments
///
/// * `destination` - The path to create the directory at.
pub async fn create_dir(destination: impl AsRef<Path> + Send + Sync) -> Result<()> {
    tokio::fs::create_dir_all(destination).await?;
    Ok(())
}

/// Creates the parent directory of the given destination.
/// If the parent directory already exists, nothing is done.
///
/// # Arguments
///
/// * `destination` - The path to create the parent directory for.
pub async fn create_parent_dir(destination: impl AsRef<Path> + Send + Sync) -> Result<()> {
    if let Some(parent) = destination.as_ref().parent() {
        tokio::fs::create_dir_all(parent).await?;
    } else {
        tokio::fs::create_dir_all(destination.as_ref()).await?;
    }

    Ok(())
}

/// Extracts a zip file to the given destination.
///
/// # Arguments
///
/// * `zip_path` - The path to the zip file.
/// * `destination` - The path to extract the zip file to.
pub async fn extract_zip(
    zip_path: impl AsRef<Path> + std::fmt::Debug,
    destination: impl AsRef<Path> + std::fmt::Debug,
) -> Result<()> {
    #[cfg(feature = "tracing")]
    tracing::debug!(
        "Extracting zip file: {:?} to {:?}",
        zip_path.as_ref(),
        destination.as_ref()
    );

    let zip_path = zip_path.as_ref().to_path_buf();
    let destination = destination.as_ref().to_path_buf();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&zip_path)
            .map_err(|e| Error::io_with_path("open zip file", &zip_path, e))?;

        let mut archive = ZipArchive::new(file)
            .map_err(|e| Error::Unknown(format!("Failed to read zip archive: {}", e)))?;

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| Error::Unknown(format!("Failed to read zip entry {}: {}", i, e)))?;

            let file_name = file
                .enclosed_name()
                .ok_or(Error::Unknown(
                    "Failed to get file name from zip entry".to_string(),
                ))?
                .to_path_buf();

            let dest_path = destination.join(file_name);

            if file.is_dir() {
                std::fs::create_dir_all(&dest_path)
                    .map_err(|e| Error::io_with_path("create directory from zip", &dest_path, e))?;
            } else {
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        Error::io_with_path("create parent directory from zip", parent, e)
                    })?;
                }

                let mut outfile = std::fs::File::create(&dest_path)
                    .map_err(|e| Error::io_with_path("create file from zip", &dest_path, e))?;

                std::io::copy(&mut file, &mut outfile).map_err(|e| {
                    Error::io_with_path("copy file content from zip", &dest_path, e)
                })?;
            }

            // Get and set permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(mode))
                        .map_err(|e| {
                            Error::io_with_path("set permissions from zip", &dest_path, e)
                        })?;
                }
            }
        }

        Ok::<_, Error>(())
    })
    .await
    .map_err(|e| Error::Unknown(e.to_string()))??;

    Ok(())
}

/// Extracts a tar.xz file to the given destination.
///
/// # Arguments
///
/// * `tar_path` - The path to the tar.xz file.
/// * `destination` - The path to extract the tar.xz file to.
pub async fn extract_tar_xz(
    tar_path: impl AsRef<Path> + std::fmt::Debug,
    destination: impl AsRef<Path> + std::fmt::Debug,
) -> Result<()> {
    #[cfg(feature = "tracing")]
    tracing::debug!(
        "Extracting tar.xz file: {:?} to {:?}",
        tar_path.as_ref(),
        destination.as_ref()
    );

    let tar_path = tar_path.as_ref().to_path_buf();
    let destination = destination.as_ref().to_path_buf();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&tar_path)
            .map_err(|e| Error::io_with_path("open tar.xz file", &tar_path, e))?;

        let decompressor = XzDecoder::new(file);
        let mut archive = Archive::new(decompressor);

        archive
            .unpack(&destination)
            .map_err(|e| Error::io_with_path("unpack tar.xz archive", &destination, e))?;

        Ok::<_, Error>(())
    })
    .await
    .map_err(|e| Error::Unknown(e.to_string()))??;

    Ok(())
}

/// Sets the executable bit on the given file.
///
/// # Arguments
///
/// * `executable` - The path to the executable file.
#[cfg(not(target_os = "windows"))]
pub async fn set_executable(executable: impl AsRef<Path> + Send + Sync) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = tokio::fs::metadata(executable.as_ref())
        .await?
        .permissions();

    perms.set_mode(0o755);
    tokio::fs::set_permissions(executable, perms).await?;

    Ok(())
}

/// No-op implementation for Windows, as Windows doesn't use executable bits.
///
/// # Arguments
///
/// * `executable` - The path to the executable file.
#[cfg(target_os = "windows")]
pub async fn set_executable(_executable: impl AsRef<Path> + Send + Sync) -> Result<()> {
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

use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref VIDEO_ID_REGEX_1: Regex =
        Regex::new(r"(?:video|audio)-([a-zA-Z0-9_-]{11})").expect("Invalid regex");
    static ref VIDEO_ID_REGEX_2: Regex =
        Regex::new(r"([a-zA-Z0-9_-]{11})\.[a-zA-Z0-9]+$").expect("Invalid regex");
    static ref VIDEO_ID_REGEX_3: Regex = Regex::new(r"[a-zA-Z0-9_-]{11}").expect("Invalid regex");
}

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
pub async fn remove_temp_file(file_path: impl AsRef<Path> + std::fmt::Debug + Send + Sync) -> bool {
    let result = tokio::fs::remove_file(&file_path).await;

    #[cfg(feature = "tracing")]
    if let Err(ref e) = result {
        tracing::warn!("Failed to remove temporary file {:?}: {}", file_path, e);
    }

    result.is_ok()
}
