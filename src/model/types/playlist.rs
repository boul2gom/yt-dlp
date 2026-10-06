//! Playlist-related models.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Represents a YouTube playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    /// The unique identifier of the playlist.
    pub id: String,
    /// The title of the playlist.
    pub title: String,
    /// The description of the playlist.
    pub description: Option<String>,

    /// The uploader name (channel name).
    pub uploader: Option<String>,
    /// The uploader ID (channel ID).
    pub uploader_id: Option<String>,
    /// The uploader URL (channel URL).
    pub uploader_url: Option<String>,

    /// The list of video entries in the playlist.
    #[serde(default)]
    pub entries: Vec<PlaylistEntry>,
    /// The total number of videos in the playlist.
    /// This may differ from entries.len() if the playlist was partially fetched.
    #[serde(rename = "playlist_count")]
    pub video_count: Option<usize>,

    /// The webpage URL of the playlist.
    #[serde(rename = "webpage_url")]
    pub url: Option<String>,
}

impl Playlist {
    /// Returns the number of videos currently in the entries list.
    ///
    /// # Returns
    ///
    /// The number of entries currently loaded
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Checks if all videos in the playlist have been fetched.
    ///
    /// # Returns
    ///
    /// `true` if all entries have been fetched, `false` otherwise
    pub fn is_complete(&self) -> bool {
        if let Some(total) = self.video_count {
            self.entries.len() >= total
        } else {
            true
        }
    }

    /// Gets a playlist entry by its index (0-based).
    ///
    /// # Arguments
    ///
    /// * `index` - The 0-based index of the entry
    ///
    /// # Returns
    ///
    /// A reference to the entry at the given index, or `None` if out of bounds
    pub fn get_entry_by_index(&self, index: usize) -> Option<&PlaylistEntry> {
        self.entries.get(index)
    }

    /// Gets all entries within a range (inclusive).
    ///
    /// # Arguments
    ///
    /// * `start` - The start index (0-based)
    /// * `end` - The end index (0-based, inclusive)
    ///
    /// # Returns
    ///
    /// A slice of entries within the specified range
    pub fn get_entries_in_range(&self, start: usize, end: usize) -> &[PlaylistEntry] {
        let end = end.min(self.entries.len().saturating_sub(1));
        if start >= self.entries.len() || start > end {
            &[]
        } else {
            &self.entries[start..=end]
        }
    }

    /// Filters entries by their availability.
    ///
    /// # Returns
    ///
    /// A vector of references to available entries
    pub fn available_entries(&self) -> Vec<&PlaylistEntry> {
        self.entries.iter().filter(|entry| entry.is_available()).collect()
    }

    /// Filters entries by title search (case-insensitive).
    ///
    /// # Arguments
    ///
    /// * `query` - The search query
    ///
    /// # Returns
    ///
    /// Returns a vector of matching entries
    pub fn search_entries_by_title(&self, query: &str) -> Vec<&PlaylistEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|entry| entry.title.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Filters entries by duration range.
    ///
    /// # Arguments
    ///
    /// * `min_duration` - Minimum duration in seconds (None for no minimum)
    /// * `max_duration` - Maximum duration in seconds (None for no maximum)
    ///
    /// # Returns
    ///
    /// Returns a vector of entries within the duration range
    pub fn filter_by_duration(&self, min_duration: Option<f64>, max_duration: Option<f64>) -> Vec<&PlaylistEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                if let Some(duration) = entry.duration {
                    let meets_min = min_duration.is_none_or(|min| duration >= min);
                    let meets_max = max_duration.is_none_or(|max| duration <= max);
                    meets_min && meets_max
                } else {
                    false
                }
            })
            .collect()
    }

    /// Filters entries by uploader/channel.
    ///
    /// # Arguments
    ///
    /// * `uploader` - The uploader name to filter by (case-insensitive)
    ///
    /// # Returns
    ///
    /// Returns a vector of entries from the specified uploader
    pub fn filter_by_uploader(&self, uploader: &str) -> Vec<&PlaylistEntry> {
        let uploader_lower = uploader.to_lowercase();
        self.entries
            .iter()
            .filter(|entry| {
                entry
                    .uploader
                    .as_ref()
                    .is_some_and(|u| u.to_lowercase() == uploader_lower)
            })
            .collect()
    }

    /// Filters entries by channel ID.
    ///
    /// # Arguments
    ///
    /// * `channel_id` - The channel ID to filter by
    ///
    /// # Returns
    ///
    /// Returns a vector of entries from the specified channel
    pub fn filter_by_channel(&self, channel_id: &str) -> Vec<&PlaylistEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.channel_id.as_ref().is_some_and(|id| id == channel_id))
            .collect()
    }

    /// Gets all entries with thumbnails.
    ///
    /// # Returns
    ///
    /// Returns a vector of entries that have thumbnail URLs
    pub fn entries_with_thumbnails(&self) -> Vec<&PlaylistEntry> {
        self.entries.iter().filter(|entry| entry.thumbnail.is_some()).collect()
    }

    /// Gets entries within a specific index range (inclusive, 1-based).
    ///
    /// # Arguments
    ///
    /// * `start_index` - Starting playlist index (1-based)
    /// * `end_index` - Ending playlist index (1-based)
    ///
    /// # Returns
    ///
    /// Returns a vector of entries within the index range
    pub fn filter_by_index_range(&self, start_index: usize, end_index: usize) -> Vec<&PlaylistEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.index.is_some_and(|idx| idx >= start_index && idx <= end_index))
            .collect()
    }
}

/// Represents an entry (video) in a playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistEntry {
    /// The video ID.
    pub id: String,
    /// The video title.
    pub title: String,
    /// The video URL.
    pub url: String,

    /// The position in the playlist (1-based, as provided by yt-dlp).
    #[serde(rename = "playlist_index")]
    pub index: Option<usize>,
    /// The duration of the video in seconds.
    pub duration: Option<f64>,
    /// The thumbnail URL.
    pub thumbnail: Option<String>,

    /// The uploader/channel name.
    pub uploader: Option<String>,
    /// The channel ID.
    pub channel_id: Option<String>,

    /// Video availability status.
    pub availability: Option<String>,
}

impl PlaylistEntry {
    /// Checks if the video is available for viewing/download.
    ///
    /// # Returns
    ///
    /// `true` if the video is public or unlisted (or has no availability info), `false` otherwise
    pub fn is_available(&self) -> bool {
        self.availability
            .as_ref()
            .map(|a| a == "public" || a == "unlisted")
            .unwrap_or(true)
    }

    /// Returns the duration in minutes.
    ///
    /// # Returns
    ///
    /// The duration in minutes, or `None` if the duration is unknown
    pub fn duration_minutes(&self) -> Option<f64> {
        self.duration.map(|d| d / 60.0)
    }
}

// Implementation of the Display trait for Playlist
impl fmt::Display for Playlist {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Playlist(id={}, title={:?}, videos={})",
            self.id,
            self.title,
            self.video_count
                .map(|c| c.to_string())
                .unwrap_or_else(|| self.entries.len().to_string())
        )
    }
}

// Implementation of the Display trait for PlaylistEntry
impl fmt::Display for PlaylistEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PlaylistEntry(id={}, title={:?}, index={})",
            self.id,
            self.title,
            self.index
                .map(|i| i.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        )
    }
}

impl PartialEq for Playlist {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.title == other.title
            && self.description == other.description
            && self.uploader == other.uploader
            && self.uploader_id == other.uploader_id
            && self.uploader_url == other.uploader_url
            && self.entries == other.entries
            && self.video_count == other.video_count
            && self.url == other.url
    }
}

impl PartialEq for PlaylistEntry {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.title == other.title
            && self.url == other.url
            && self.index == other.index
            && match (self.duration, other.duration) {
                (Some(a), Some(b)) => a.to_bits() == b.to_bits(),
                (None, None) => true,
                _ => false,
            }
            && self.thumbnail == other.thumbnail
            && self.uploader == other.uploader
            && self.channel_id == other.channel_id
            && self.availability == other.availability
    }
}

// Implementation of Hash for Playlist
impl Hash for Playlist {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.title.hash(state);
        if let Some(ref uid) = self.uploader_id {
            uid.hash(state);
        }
    }
}

// Implementation of Hash for PlaylistEntry
impl Hash for PlaylistEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.index.hash(state);
    }
}

/// Progress information for playlist downloads.
#[derive(Debug, Clone)]
pub struct PlaylistDownloadProgress {
    /// The entry that was downloaded or failed
    pub entry: PlaylistEntry,
    /// The result of the download (Ok with path, or Err)
    pub result: Result<PathBuf, String>,
    /// Number of videos completed so far (including this one)
    pub completed: usize,
    /// Total number of videos to download
    pub total: usize,
}

impl PlaylistDownloadProgress {
    /// Returns the progress as a percentage (0.0 to 100.0).
    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.completed as f64 / self.total as f64) * 100.0
        }
    }

    /// Checks if the download was successful.
    pub fn is_success(&self) -> bool {
        self.result.is_ok()
    }

    /// Checks if the download failed.
    pub fn is_failure(&self) -> bool {
        self.result.is_err()
    }
}

impl fmt::Display for PlaylistDownloadProgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PlaylistDownloadProgress(completed={}/{}, entry={})",
            self.completed, self.total, self.entry
        )
    }
}
