//! Chapter-related models.

use std::fmt;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

/// Represents a chapter in a YouTube video.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    /// The start time of the chapter in seconds.
    pub start_time: f64,
    /// The end time of the chapter in seconds.
    pub end_time: f64,
    /// The title of the chapter.
    pub title: Option<String>,
}

impl Chapter {
    /// Returns the duration of the chapter in seconds.
    ///
    /// # Returns
    ///
    /// The duration in seconds (end_time - start_time)
    pub fn duration(&self) -> f64 {
        self.end_time - self.start_time
    }

    /// Returns the duration in minutes.
    ///
    /// # Returns
    ///
    /// The duration in minutes
    pub fn duration_minutes(&self) -> f64 {
        self.duration() / 60.0
    }

    /// Checks if a given timestamp (in seconds) is within this chapter.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The timestamp in seconds to check
    ///
    /// # Returns
    ///
    /// `true` if the timestamp falls within this chapter's time range, `false` otherwise
    pub fn contains_timestamp(&self, timestamp: f64) -> bool {
        timestamp >= self.start_time && timestamp < self.end_time
    }

    /// Checks if the chapter has a title.
    ///
    /// # Returns
    ///
    /// `true` if the chapter has a title, `false` otherwise
    pub fn has_title(&self) -> bool {
        self.title.is_some()
    }

    /// Gets the chapter title or a default value.
    ///
    /// # Arguments
    ///
    /// * `default` - The default value to return if the chapter has no title
    ///
    /// # Returns
    ///
    /// The chapter title, or the provided default if no title is set
    pub fn title_or<'a>(&'a self, default: &'a str) -> &'a str {
        self.title.as_deref().unwrap_or(default)
    }

    /// Checks if the chapter title contains the given string (case-insensitive).
    ///
    /// # Arguments
    ///
    /// * `query` - The string to search for in the title
    ///
    /// # Returns
    ///
    /// Returns `true` if the title contains the query string, `false` otherwise
    pub fn title_contains(&self, query: &str) -> bool {
        self.title
            .as_ref()
            .is_some_and(|title| title.to_lowercase().contains(&query.to_lowercase()))
    }

    /// Checks if the chapter title matches the given string exactly (case-insensitive).
    ///
    /// # Arguments
    ///
    /// * `query` - The string to match against the title
    ///
    /// # Returns
    ///
    /// Returns `true` if the title matches exactly, `false` otherwise
    pub fn title_matches(&self, query: &str) -> bool {
        self.title
            .as_ref()
            .is_some_and(|title| title.to_lowercase() == query.to_lowercase())
    }

    /// Checks if the chapter title starts with the given string (case-insensitive).
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix to check for
    ///
    /// # Returns
    ///
    /// Returns `true` if the title starts with the prefix, `false` otherwise
    pub fn title_starts_with(&self, prefix: &str) -> bool {
        self.title
            .as_ref()
            .is_some_and(|title| title.to_lowercase().starts_with(&prefix.to_lowercase()))
    }

    /// Checks if the chapter duration is within the given range (in seconds).
    ///
    /// # Arguments
    ///
    /// * `min_duration` - Minimum duration in seconds
    /// * `max_duration` - Maximum duration in seconds
    ///
    /// # Returns
    ///
    /// Returns `true` if the chapter duration is within range, `false` otherwise
    pub fn duration_in_range(&self, min_duration: f64, max_duration: f64) -> bool {
        let duration = self.duration();
        duration >= min_duration && duration <= max_duration
    }
}

/// Helper struct for working with collections of chapters.
pub struct ChapterList<'a> {
    chapters: &'a [Chapter],
}

impl<'a> ChapterList<'a> {
    /// Creates a new ChapterList from a slice of chapters.
    pub fn new(chapters: &'a [Chapter]) -> Self {
        Self { chapters }
    }

    /// Finds all chapters with titles containing the given query string.
    ///
    /// # Arguments
    ///
    /// * `query` - The string to search for
    ///
    /// # Returns
    ///
    /// Returns a vector of references to matching chapters
    pub fn search_by_title(&self, query: &str) -> Vec<&'a Chapter> {
        self.chapters
            .iter()
            .filter(|chapter| chapter.title_contains(query))
            .collect()
    }

    /// Finds the first chapter with a title matching the query exactly.
    ///
    /// # Arguments
    ///
    /// * `title` - The exact title to search for
    ///
    /// # Returns
    ///
    /// Returns an Option containing a reference to the matching chapter
    pub fn find_by_exact_title(&self, title: &str) -> Option<&'a Chapter> {
        self.chapters.iter().find(|chapter| chapter.title_matches(title))
    }

    /// Finds all chapters with titles starting with the given prefix.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix to search for
    ///
    /// # Returns
    ///
    /// Returns a vector of references to matching chapters
    pub fn find_by_title_prefix(&self, prefix: &str) -> Vec<&'a Chapter> {
        self.chapters
            .iter()
            .filter(|chapter| chapter.title_starts_with(prefix))
            .collect()
    }

    /// Finds the chapter containing the given timestamp.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The timestamp in seconds
    ///
    /// # Returns
    ///
    /// Returns an Option containing a reference to the chapter
    pub fn find_by_timestamp(&self, timestamp: f64) -> Option<&'a Chapter> {
        self.chapters
            .iter()
            .find(|chapter| chapter.contains_timestamp(timestamp))
    }

    /// Filters chapters by duration range.
    ///
    /// # Arguments
    ///
    /// * `min_duration` - Minimum duration in seconds
    /// * `max_duration` - Maximum duration in seconds
    ///
    /// # Returns
    ///
    /// Returns a vector of references to matching chapters
    pub fn filter_by_duration(&self, min_duration: f64, max_duration: f64) -> Vec<&'a Chapter> {
        self.chapters
            .iter()
            .filter(|chapter| chapter.duration_in_range(min_duration, max_duration))
            .collect()
    }

    /// Gets all chapters that have titles.
    ///
    /// # Returns
    ///
    /// Returns a vector of references to chapters with titles
    pub fn with_titles(&self) -> Vec<&'a Chapter> {
        self.chapters.iter().filter(|chapter| chapter.has_title()).collect()
    }

    /// Gets the total number of chapters.
    pub fn count(&self) -> usize {
        self.chapters.len()
    }

    /// Gets the total duration of all chapters in seconds.
    pub fn total_duration(&self) -> f64 {
        self.chapters.iter().map(|c| c.duration()).sum()
    }

    /// Validates the chapters for consistency and correctness.
    ///
    /// # Returns
    ///
    /// Returns a `ChapterValidation` result containing validation status and any issues found
    pub fn validate(&self) -> ChapterValidation {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if self.chapters.is_empty() {
            return ChapterValidation {
                is_valid: true,
                errors,
                warnings,
            };
        }

        Self::validate_individual_chapters(self.chapters, &mut errors, &mut warnings);
        Self::validate_chapter_ordering(self.chapters, &mut errors, &mut warnings);

        ChapterValidation {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        }
    }

    fn validate_individual_chapters(chapters: &[Chapter], errors: &mut Vec<String>, warnings: &mut Vec<String>) {
        for (i, chapter) in chapters.iter().enumerate() {
            if chapter.start_time < 0.0 {
                errors.push(format!(
                    "Chapter {} has negative start time: {:.2}s",
                    i + 1,
                    chapter.start_time
                ));
            }

            if chapter.end_time < 0.0 {
                errors.push(format!(
                    "Chapter {} has negative end time: {:.2}s",
                    i + 1,
                    chapter.end_time
                ));
            }

            if chapter.start_time >= chapter.end_time {
                errors.push(format!(
                    "Chapter {} has invalid time range: start ({:.2}s) >= end ({:.2}s)",
                    i + 1,
                    chapter.start_time,
                    chapter.end_time
                ));
            }

            if !chapter.has_title() {
                warnings.push(format!("Chapter {} has no title", i + 1));
            }

            if chapter.duration() < 1.0 {
                warnings.push(format!("Chapter {} is very short ({:.2}s)", i + 1, chapter.duration()));
            }
        }
    }

    fn validate_chapter_ordering(chapters: &[Chapter], errors: &mut Vec<String>, warnings: &mut Vec<String>) {
        for i in 0..chapters.len().saturating_sub(1) {
            let current = &chapters[i];
            let next = &chapters[i + 1];

            if current.start_time > next.start_time {
                errors.push(format!(
                    "Chapters {} and {} are out of order (current starts at {:.2}s, next starts at {:.2}s)",
                    i + 1,
                    i + 2,
                    current.start_time,
                    next.start_time
                ));
            }

            if current.end_time > next.start_time {
                errors.push(format!(
                    "Chapters {} and {} overlap (current ends at {:.2}s, next starts at {:.2}s)",
                    i + 1,
                    i + 2,
                    current.end_time,
                    next.start_time
                ));
            }

            if current.end_time < next.start_time {
                let gap = next.start_time - current.end_time;
                if gap > 0.1 {
                    warnings.push(format!(
                        "Gap of {:.2}s between chapters {} and {} ({:.2}s to {:.2}s)",
                        gap,
                        i + 1,
                        i + 2,
                        current.end_time,
                        next.start_time
                    ));
                }
            }
        }
    }

    /// Checks if the chapters are in chronological order.
    pub fn is_sorted(&self) -> bool {
        self.chapters
            .windows(2)
            .all(|pair| pair[0].start_time <= pair[1].start_time)
    }

    /// Checks if any chapters overlap.
    pub fn has_overlaps(&self) -> bool {
        self.chapters
            .windows(2)
            .any(|pair| pair[0].end_time > pair[1].start_time)
    }
}

/// Result of chapter validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterValidation {
    /// Whether the chapters are valid (no errors)
    pub is_valid: bool,
    /// List of validation errors found
    pub errors: Vec<String>,
    /// List of validation warnings (non-critical issues)
    pub warnings: Vec<String>,
}

impl ChapterValidation {
    /// Creates a validation result indicating success.
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Creates a validation result indicating failure.
    pub fn invalid(errors: Vec<String>) -> Self {
        Self {
            is_valid: false,
            errors,
            warnings: Vec::new(),
        }
    }

    /// Adds a warning to the validation result.
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Adds multiple warnings to the validation result.
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings.extend(warnings);
        self
    }

    /// Returns whether there are any errors or warnings.
    pub fn has_issues(&self) -> bool {
        !self.errors.is_empty() || !self.warnings.is_empty()
    }
}

// Implementation of the Display trait for Chapter
impl fmt::Display for Chapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Chapter(start={:.2}s, end={:.2}s, title={:?})",
            self.start_time,
            self.end_time,
            self.title.as_deref().unwrap_or("untitled")
        )
    }
}

impl fmt::Display for ChapterValidation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ChapterValidation(valid={}, errors={}, warnings={})",
            self.is_valid,
            self.errors.len(),
            self.warnings.len()
        )
    }
}

impl PartialEq for Chapter {
    fn eq(&self, other: &Self) -> bool {
        self.start_time.to_bits() == other.start_time.to_bits()
            && self.end_time.to_bits() == other.end_time.to_bits()
            && self.title == other.title
    }
}

// Implementation of Hash for Chapter
impl Hash for Chapter {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Use ordered float for hashing
        self.start_time.to_bits().hash(state);
        self.end_time.to_bits().hash(state);
        self.title.hash(state);
    }
}
