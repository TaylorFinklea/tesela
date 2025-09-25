//! Power Search module - unified search and create interface
//!
//! Provides a Logseq-style power search that intelligently combines:
//! - Note creation (only when note doesn't exist)
//! - Note search by title
//! - Content search (tiles/blocks)
//! - Recent notes tracking
//! - Filter support

use chrono::{DateTime, Local};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Instant;

use crate::tui::async_runtime::AsyncSearchResult;
use crate::tui::search_filters::SearchFilters;

/// Maximum number of recent notes to track
const MAX_RECENTS: usize = 10;

/// Power Search mode state
#[derive(Debug, Clone, PartialEq)]
pub struct PowerSearchMode {
    /// The current search query
    pub query: String,
    /// Cursor position in the query
    pub cursor_position: usize,
    /// All search results organized by section
    pub sections: Vec<SearchSection>,
    /// Currently selected section index
    pub selected_section: usize,
    /// Selected item within the current section
    pub selected_item: usize,
    /// Search filters (activated with '/')
    pub filters: SearchFilters,
    /// Whether filter mode is active
    pub filter_mode: bool,
    /// Recently accessed notes
    pub recents: RecentNotes,
    /// Last query time for debouncing
    pub last_query_time: Instant,
    /// Pending query for debounced search
    pub pending_query: Option<String>,
    /// Whether a search is in progress
    pub is_searching: bool,
}

/// A section of search results
#[derive(Debug, Clone, PartialEq)]
pub struct SearchSection {
    /// Section type
    pub section_type: SectionType,
    /// Section title for display
    pub title: String,
    /// Items in this section
    pub items: Vec<SearchItem>,
    /// Whether this section is expanded
    pub is_expanded: bool,
}

/// Type of search section
#[derive(Debug, Clone, PartialEq)]
pub enum SectionType {
    Create,  // Create new note option
    Notes,   // Existing notes matching title
    Tiles,   // Content search results (blocks/tiles)
    Recents, // Recently accessed notes
}

/// A single search result item
#[derive(Debug, Clone, PartialEq)]
pub struct SearchItem {
    /// Display title
    pub title: String,
    /// File path or identifier
    pub path: String,
    /// Additional metadata (e.g., tags, date)
    pub metadata: String,
    /// Preview snippet (for content matches)
    pub snippet: Option<String>,
    /// Match score/rank
    pub score: f32,
    /// Item-specific action
    pub action: ItemAction,
}

/// Action to take when item is selected
#[derive(Debug, Clone, PartialEq)]
pub enum ItemAction {
    CreateNote(String),         // Create new note with given title
    OpenNote(String),           // Open existing note
    JumpToBlock(String, usize), // Jump to specific block in note
}

/// Tracks recently accessed notes
#[derive(Debug, Clone, PartialEq)]
pub struct RecentNotes {
    /// Queue of recent note paths with access times
    notes: VecDeque<(PathBuf, DateTime<Local>)>,
}

impl Default for PowerSearchMode {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerSearchMode {
    /// Create a new power search mode
    pub fn new() -> Self {
        Self {
            query: String::new(),
            cursor_position: 0,
            sections: vec![],
            selected_section: 0,
            selected_item: 0,
            filters: SearchFilters::new(),
            filter_mode: false,
            recents: RecentNotes::new(),
            last_query_time: Instant::now(),
            pending_query: None,
            is_searching: false,
        }
    }

    /// Update search results based on current query
    pub fn update_results(
        &mut self,
        query: &str,
        existing_notes: Vec<(String, String)>, // (path, title) pairs
        content_results: Vec<AsyncSearchResult>,
    ) {
        // Clear previous results
        self.sections.clear();

        if query.is_empty() {
            // Show only recents when query is empty
            self.add_recents_section();
            return;
        }

        // Check if a note with this exact name exists
        let query_lower = query.to_lowercase();
        let safe_filename = self.make_safe_filename(query);
        let note_exists = existing_notes.iter().any(|(path, _)| {
            path.to_lowercase().contains(&safe_filename.to_lowercase())
                || path.to_lowercase().contains(&query_lower)
        });

        // Section 1: Create (only if note doesn't exist)
        if !note_exists && !query.is_empty() {
            let create_item = SearchItem {
                title: format!("Create page called '{}'", query),
                path: format!("notes/{}.md", safe_filename),
                metadata: "New note".to_string(),
                snippet: None,
                score: 100.0,
                action: ItemAction::CreateNote(query.to_string()),
            };

            self.sections.push(SearchSection {
                section_type: SectionType::Create,
                title: "Create".to_string(),
                items: vec![create_item],
                is_expanded: true,
            });
        }

        // Section 2: Notes/Pages (existing notes matching query)
        let mut note_matches: Vec<SearchItem> = existing_notes
            .iter()
            .filter(|(path, title)| {
                let title_lower = title.to_lowercase();
                let path_lower = path.to_lowercase();
                title_lower.contains(&query_lower) || path_lower.contains(&query_lower)
            })
            .map(|(path, title)| {
                let score = self.calculate_title_match_score(title, query);
                SearchItem {
                    title: title.clone(),
                    path: path.clone(),
                    metadata: self.format_note_metadata(path),
                    snippet: None,
                    score,
                    action: ItemAction::OpenNote(path.clone()),
                }
            })
            .collect();

        // Sort by score
        note_matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if !note_matches.is_empty() {
            self.sections.push(SearchSection {
                section_type: SectionType::Notes,
                title: format!("Pages {}", note_matches.len()),
                items: note_matches,
                is_expanded: true,
            });
        }

        // Section 3: Tiles/Blocks (content search results)
        if !content_results.is_empty() {
            let mut tiles: Vec<SearchItem> = content_results
                .into_iter()
                .map(|result| {
                    let snippet = result.snippet.as_ref().cloned().unwrap_or_else(|| {
                        result
                            .content
                            .lines()
                            .find(|line| line.to_lowercase().contains(&query_lower))
                            .unwrap_or("")
                            .to_string()
                    });

                    SearchItem {
                        title: result.title.clone(),
                        path: result.path.clone(),
                        metadata: self.format_tile_metadata(&result),
                        snippet: Some(snippet),
                        score: result.rank,
                        action: ItemAction::OpenNote(result.path),
                    }
                })
                .collect();

            tiles.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            self.sections.push(SearchSection {
                section_type: SectionType::Tiles,
                title: format!("Tiles {}", tiles.len()),
                items: tiles,
                is_expanded: true,
            });
        }

        // Section 4: Recents (always at the bottom if there are recent notes)
        self.add_recents_section();

        // Reset selection to first item
        self.selected_section = 0;
        self.selected_item = 0;
    }

    /// Add recents section if there are recent notes
    fn add_recents_section(&mut self) {
        let recent_items: Vec<SearchItem> = self
            .recents
            .get_recent(5)
            .into_iter()
            .map(|(path, time)| {
                let title = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                SearchItem {
                    title,
                    path: path.to_string_lossy().to_string(),
                    metadata: format!("Opened {}", self.format_time_ago(time)),
                    snippet: None,
                    score: 0.0,
                    action: ItemAction::OpenNote(path.to_string_lossy().to_string()),
                }
            })
            .collect();

        if !recent_items.is_empty() {
            self.sections.push(SearchSection {
                section_type: SectionType::Recents,
                title: format!("Recents {}", recent_items.len()),
                items: recent_items,
                is_expanded: true,
            });
        }
    }

    /// Calculate match score for title matching
    fn calculate_title_match_score(&self, title: &str, query: &str) -> f32 {
        let title_lower = title.to_lowercase();
        let query_lower = query.to_lowercase();

        if title_lower == query_lower {
            100.0 // Exact match
        } else if title_lower.starts_with(&query_lower) {
            80.0 // Prefix match
        } else if title_lower.contains(&query_lower) {
            60.0 // Contains match
        } else {
            // Fuzzy match scoring
            let mut score = 0.0;
            let mut query_chars = query_lower.chars().peekable();

            for ch in title_lower.chars() {
                if Some(&ch) == query_chars.peek() {
                    score += 10.0;
                    query_chars.next();
                }
            }

            score
        }
    }

    /// Format metadata for a note
    fn format_note_metadata(&self, path: &str) -> String {
        if path.starts_with("dailies/") {
            "📅 Daily Note".to_string()
        } else {
            "📄 Note".to_string()
        }
    }

    /// Format metadata for a tile/block result
    fn format_tile_metadata(&self, result: &AsyncSearchResult) -> String {
        let mut parts = vec![];

        if !result.tags.is_empty() {
            parts.push(format!("🏷️ {}", result.tags.join(", ")));
        }

        parts.push(format!("Score: {:.0}", result.rank * 100.0));

        parts.join(" • ")
    }

    /// Format time ago string
    fn format_time_ago(&self, time: DateTime<Local>) -> String {
        let now = Local::now();
        let duration = now.signed_duration_since(time);

        if duration.num_seconds() < 60 {
            "just now".to_string()
        } else if duration.num_minutes() < 60 {
            format!("{}m ago", duration.num_minutes())
        } else if duration.num_hours() < 24 {
            format!("{}h ago", duration.num_hours())
        } else if duration.num_days() < 7 {
            format!("{}d ago", duration.num_days())
        } else {
            time.format("%b %d").to_string()
        }
    }

    /// Make a safe filename from a title
    fn make_safe_filename(&self, title: &str) -> String {
        title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .replace(' ', "-")
            .to_lowercase()
    }

    /// Navigate to next section
    pub fn next_section(&mut self) {
        if !self.sections.is_empty() {
            self.selected_section = (self.selected_section + 1) % self.sections.len();
            self.selected_item = 0;
        }
    }

    /// Navigate to previous section
    pub fn prev_section(&mut self) {
        if !self.sections.is_empty() {
            if self.selected_section == 0 {
                self.selected_section = self.sections.len() - 1;
            } else {
                self.selected_section -= 1;
            }
            self.selected_item = 0;
        }
    }

    /// Navigate to next item in current section
    pub fn next_item(&mut self) {
        if let Some(section) = self.sections.get(self.selected_section) {
            if !section.items.is_empty() {
                self.selected_item = (self.selected_item + 1) % section.items.len();
            }
        }
    }

    /// Navigate to previous item in current section
    pub fn prev_item(&mut self) {
        if let Some(section) = self.sections.get(self.selected_section) {
            if !section.items.is_empty() {
                if self.selected_item == 0 {
                    self.selected_item = section.items.len() - 1;
                } else {
                    self.selected_item -= 1;
                }
            }
        }
    }

    /// Get the currently selected item
    pub fn get_selected_item(&self) -> Option<&SearchItem> {
        self.sections
            .get(self.selected_section)
            .and_then(|section| section.items.get(self.selected_item))
    }

    /// Add a note to recents when accessed
    pub fn add_to_recents(&mut self, path: PathBuf) {
        self.recents.add(path);
    }
}

impl RecentNotes {
    /// Create a new recents tracker
    pub fn new() -> Self {
        Self {
            notes: VecDeque::with_capacity(MAX_RECENTS),
        }
    }

    /// Add a note to recents
    pub fn add(&mut self, path: PathBuf) {
        // Remove if already exists
        self.notes.retain(|(p, _)| p != &path);

        // Add to front
        self.notes.push_front((path, Local::now()));

        // Trim to max size
        while self.notes.len() > MAX_RECENTS {
            self.notes.pop_back();
        }
    }

    /// Get recent notes
    pub fn get_recent(&self, count: usize) -> Vec<(PathBuf, DateTime<Local>)> {
        self.notes.iter().take(count).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_search_create_section() {
        let mut mode = PowerSearchMode::new();
        mode.update_results("new_note", vec![], vec![]);

        assert_eq!(mode.sections.len(), 1);
        assert_eq!(mode.sections[0].section_type, SectionType::Create);
    }

    #[test]
    fn test_power_search_no_create_for_existing() {
        let mut mode = PowerSearchMode::new();
        let existing = vec![("notes/test.md".to_string(), "test".to_string())];
        mode.update_results("test", existing, vec![]);

        // Should have Notes section but not Create
        assert!(mode
            .sections
            .iter()
            .any(|s| s.section_type == SectionType::Notes));
        assert!(!mode
            .sections
            .iter()
            .any(|s| s.section_type == SectionType::Create));
    }

    #[test]
    fn test_recent_notes() {
        let mut recents = RecentNotes::new();
        recents.add(PathBuf::from("note1.md"));
        recents.add(PathBuf::from("note2.md"));
        recents.add(PathBuf::from("note1.md")); // Should move to front

        let recent = recents.get_recent(2);
        assert_eq!(recent[0].0, PathBuf::from("note1.md"));
        assert_eq!(recent[1].0, PathBuf::from("note2.md"));
    }
}
