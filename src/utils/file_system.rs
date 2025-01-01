//! Tools for working with the file system.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use tar::Archive;
use tokio::fs::{File, OpenOptions};
use xz2::read::XzDecoder;
use zip::ZipArchive;

/// Returns the name of the given path.
pub fn try_name(path: impl AsRef<Path>) -> Result<String> {
    let name = path
        .as_ref()
        .file_name()
        .ok_or(Error::Path("Failed to get name".to_string()))?;
    let name = name
        .to_str()
        .ok_or(Error::Path("Failed to convert name".to_string()))?;

    Ok(name.to_string())
}

/// Returns the name of the given path without the extension.
pub fn try_without_extension(path: impl AsRef<Path>) -> Result<String> {
    let name = try_name(path)?;
    let name = name
        .split('.')
        .next()
        .ok_or(Error::Path("Failed to get name".to_string()))?;

    Ok(name.to_string())
}

/// Returns the parent directory of the given path.
pub fn try_parent(path: impl AsRef<Path>) -> Result<PathBuf> {
    let parent = path
        .as_ref()
        .parent()
        .ok_or(Error::Path("Failed to get parent".to_string()))?;

    Ok(parent.to_path_buf())
}

/// Creates a new file at the given destination.
///
/// # Arguments
///
/// * `destination` - The path to create the file at.
pub async fn create_file(destination: impl AsRef<Path>) -> Result<File> {
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
pub fn create_dir(destination: impl AsRef<Path>) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    Ok(())
}

/// Creates the parent directory of the given destination.
/// If the parent directory already exists, nothing is done.
///
/// # Arguments
///
/// * `destination` - The path to create the parent directory for.
pub fn create_parent_dir(destination: impl AsRef<Path>) -> Result<()> {
    if let Some(parent) = destination.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }

    Ok(())
}

/// Extracts a zip file to the given destination.
///
/// # Arguments
///
/// * `zip_path` - The path to the zip file.
/// * `destination` - The path to extract the zip file to.
#[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
pub async fn extract_zip(
    zip_path: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<()> {
    #[cfg(feature = "tracing")]
    tracing::debug!(
        "Extracting zip file: {:?} to {:?}",
        zip_path.as_ref(),
        destination.as_ref()
    );

    let file = File::open(zip_path).await?;
    let file = file.into_std().await;

    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let destination = destination.as_ref().join(
            file.enclosed_name()
                .ok_or(Error::Unknown("Failed to get file name".to_string()))?,
        );

        match file.is_file() {
            true => {
                create_parent_dir(&destination)?;

                let dest_file = create_file(&destination).await?;
                let mut dest_file = dest_file.into_std().await;

                std::io::copy(&mut file, &mut dest_file)?;
            }
            false => {
                create_file(&destination).await?;
            }
        }
    }

    Ok(())
}

/// Extracts a tar.xz file to the given destination.
///
/// # Arguments
///
/// * `tar_path` - The path to the tar.xz file.
/// * `destination` - The path to extract the tar.xz file to.
#[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
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

    let tar_gz = File::open(tar_path).await?;
    let tar_gz = tar_gz.into_std().await;

    let decompressor = XzDecoder::new(tar_gz);
    let mut archive = Archive::new(decompressor);

    archive.unpack(destination)?;

    Ok(())
}

/// Sets the executable bit on the given file.
///
/// # Arguments
///
/// * `executable` - The path to the executable file.
#[cfg(not(target_os = "windows"))]
pub fn set_executable(executable: impl AsRef<Path>) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(executable.as_ref())?.permissions();

    perms.set_mode(0o755);
    std::fs::set_permissions(executable, perms)?;

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn set_executable(executable: impl AsRef<Path>) -> Result<()> {
    Ok(())
}
