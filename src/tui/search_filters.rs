//! Search filters module for advanced search functionality
//!
//! Provides UI components and logic for filtering search results by:
//! - Tags
//! - Date ranges
//! - Note types (daily vs regular notes)
//! - Combined filters

use anyhow::Result;
use chrono::{DateTime, Local, NaiveDate, Utc};
use std::collections::HashSet;

/// Search filter state
#[derive(Debug, Clone, PartialEq)]
pub struct SearchFilters {
    /// Active tag filters
    pub tags: HashSet<String>,
    /// Date range filter - from date
    pub from_date: Option<DateTime<Utc>>,
    /// Date range filter - to date
    pub to_date: Option<DateTime<Utc>>,
    /// Filter by note type
    pub note_type: NoteTypeFilter,
    /// Raw filter string for display
    pub filter_string: String,
    /// Whether filters are currently active
    pub is_active: bool,
}

/// Note type filter options
#[derive(Debug, Clone, PartialEq)]
pub enum NoteTypeFilter {
    All,
    RegularNotes,
    DailyNotes,
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            tags: HashSet::new(),
            from_date: None,
            to_date: None,
            note_type: NoteTypeFilter::All,
            filter_string: String::new(),
            is_active: false,
        }
    }
}

impl SearchFilters {
    /// Create a new search filters instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a filter string into search filters
    /// Supported syntax:
    /// - tag:rust tag:programming - filter by tags
    /// - after:2024-01-01 - notes modified after date
    /// - before:2024-12-31 - notes modified before date
    /// - since:7d - notes from last N days
    /// - type:daily - filter by note type
    pub fn parse(filter_string: &str) -> Result<Self> {
        let mut filters = Self::new();
        filters.filter_string = filter_string.to_string();

        let parts: Vec<&str> = filter_string.split_whitespace().collect();

        for part in parts {
            if let Some((key, value)) = part.split_once(':') {
                match key.to_lowercase().as_str() {
                    "tag" => {
                        filters.tags.insert(value.to_string());
                        filters.is_active = true;
                    }
                    "after" | "since" => {
                        if let Some(date) = Self::parse_date_value(value, true) {
                            filters.from_date = Some(date);
                            filters.is_active = true;
                        }
                    }
                    "before" | "until" => {
                        if let Some(date) = Self::parse_date_value(value, false) {
                            filters.to_date = Some(date);
                            filters.is_active = true;
                        }
                    }
                    "type" => {
                        filters.note_type = match value.to_lowercase().as_str() {
                            "daily" | "dailies" => NoteTypeFilter::DailyNotes,
                            "note" | "notes" | "regular" => NoteTypeFilter::RegularNotes,
                            _ => NoteTypeFilter::All,
                        };
                        if filters.note_type != NoteTypeFilter::All {
                            filters.is_active = true;
                        }
                    }
                    _ => {} // Ignore unknown filter types
                }
            }
        }

        Ok(filters)
    }

    /// Parse a date value from various formats
    fn parse_date_value(value: &str, is_from: bool) -> Option<DateTime<Utc>> {
        // Check for relative dates (e.g., "7d", "1w", "1m")
        if let Some(relative_date) = Self::parse_relative_date(value, is_from) {
            return Some(relative_date);
        }

        // Try parsing as YYYY-MM-DD
        if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
            let time = if is_from {
                date.and_hms_opt(0, 0, 0)?
            } else {
                date.and_hms_opt(23, 59, 59)?
            };
            return Some(DateTime::from_naive_utc_and_offset(time, Utc));
        }

        // Try parsing as MM/DD/YYYY
        if let Ok(date) = NaiveDate::parse_from_str(value, "%m/%d/%Y") {
            let time = if is_from {
                date.and_hms_opt(0, 0, 0)?
            } else {
                date.and_hms_opt(23, 59, 59)?
            };
            return Some(DateTime::from_naive_utc_and_offset(time, Utc));
        }

        None
    }

    /// Parse relative date strings like "7d", "1w", "2m"
    fn parse_relative_date(value: &str, _is_from: bool) -> Option<DateTime<Utc>> {
        let value_lower = value.to_lowercase();

        // Extract number and unit
        let (num_str, unit) = if value_lower.ends_with("d")
            || value_lower.ends_with("day")
            || value_lower.ends_with("days")
        {
            let num_str = value_lower.trim_end_matches(char::is_alphabetic);
            (num_str, "days")
        } else if value_lower.ends_with("w")
            || value_lower.ends_with("week")
            || value_lower.ends_with("weeks")
        {
            let num_str = value_lower.trim_end_matches(char::is_alphabetic);
            (num_str, "weeks")
        } else if value_lower.ends_with("m")
            || value_lower.ends_with("month")
            || value_lower.ends_with("months")
        {
            let num_str = value_lower.trim_end_matches(char::is_alphabetic);
            (num_str, "months")
        } else if value_lower == "today" {
            return Some(Local::now().date_naive().and_hms_opt(0, 0, 0)?.and_utc());
        } else if value_lower == "yesterday" {
            let yesterday = Local::now().date_naive() - chrono::Duration::days(1);
            return Some(yesterday.and_hms_opt(0, 0, 0)?.and_utc());
        } else if value_lower == "week" || value_lower == "lastweek" {
            return Some(Utc::now() - chrono::Duration::weeks(1));
        } else {
            return None;
        };

        // Parse the number
        if let Ok(num) = num_str.parse::<i64>() {
            let duration = match unit {
                "days" => chrono::Duration::days(num),
                "weeks" => chrono::Duration::weeks(num),
                "months" => chrono::Duration::days(num * 30), // Approximate
                _ => return None,
            };

            Some(Utc::now() - duration)
        } else {
            None
        }
    }

    /// Add a tag filter
    pub fn add_tag(&mut self, tag: String) {
        self.tags.insert(tag);
        self.is_active = true;
        self.update_filter_string();
    }

    /// Remove a tag filter
    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.remove(tag);
        self.update_active_state();
        self.update_filter_string();
    }

    /// Clear all tag filters
    pub fn clear_tags(&mut self) {
        self.tags.clear();
        self.update_active_state();
        self.update_filter_string();
    }

    /// Set date range
    pub fn set_date_range(&mut self, from: Option<DateTime<Utc>>, to: Option<DateTime<Utc>>) {
        self.from_date = from;
        self.to_date = to;
        self.update_active_state();
        self.update_filter_string();
    }

    /// Clear date filters
    pub fn clear_dates(&mut self) {
        self.from_date = None;
        self.to_date = None;
        self.update_active_state();
        self.update_filter_string();
    }

    /// Set note type filter
    pub fn set_note_type(&mut self, note_type: NoteTypeFilter) {
        self.note_type = note_type;
        self.update_active_state();
        self.update_filter_string();
    }

    /// Clear all filters
    pub fn clear_all(&mut self) {
        *self = Self::default();
    }

    /// Update the active state based on current filters
    fn update_active_state(&mut self) {
        self.is_active = !self.tags.is_empty()
            || self.from_date.is_some()
            || self.to_date.is_some()
            || self.note_type != NoteTypeFilter::All;
    }

    /// Update the filter string representation
    fn update_filter_string(&mut self) {
        let mut parts = Vec::new();

        // Add tag filters
        for tag in &self.tags {
            parts.push(format!("tag:{}", tag));
        }

        // Add date filters
        if let Some(from) = self.from_date {
            parts.push(format!("after:{}", from.format("%Y-%m-%d")));
        }
        if let Some(to) = self.to_date {
            parts.push(format!("before:{}", to.format("%Y-%m-%d")));
        }

        // Add note type filter
        match self.note_type {
            NoteTypeFilter::DailyNotes => parts.push("type:daily".to_string()),
            NoteTypeFilter::RegularNotes => parts.push("type:notes".to_string()),
            NoteTypeFilter::All => {}
        }

        self.filter_string = parts.join(" ");
    }

    /// Get a human-readable description of active filters
    pub fn description(&self) -> String {
        if !self.is_active {
            return "No filters".to_string();
        }

        let mut parts = Vec::new();

        if !self.tags.is_empty() {
            let tags: Vec<_> = self.tags.iter().cloned().collect();
            parts.push(format!("tags: {}", tags.join(", ")));
        }

        if let Some(from) = self.from_date {
            parts.push(format!("after {}", from.format("%b %d, %Y")));
        }

        if let Some(to) = self.to_date {
            parts.push(format!("before {}", to.format("%b %d, %Y")));
        }

        match self.note_type {
            NoteTypeFilter::DailyNotes => parts.push("daily notes".to_string()),
            NoteTypeFilter::RegularNotes => parts.push("regular notes".to_string()),
            NoteTypeFilter::All => {}
        }

        parts.join(" | ")
    }

    /// Check if a path matches the note type filter
    pub fn matches_path(&self, path: &str) -> bool {
        match self.note_type {
            NoteTypeFilter::All => true,
            NoteTypeFilter::DailyNotes => path.starts_with("dailies/"),
            NoteTypeFilter::RegularNotes => !path.starts_with("dailies/"),
        }
    }
}

/// UI component for rendering search filters
pub struct SearchFilterUI;

impl SearchFilterUI {
    /// Generate help text for search filters
    pub fn help_text() -> Vec<String> {
        vec![
            "Search Filter Syntax:".to_string(),
            "".to_string(),
            "Tags:        tag:rust tag:programming".to_string(),
            "Date After:  after:2024-01-01 or since:7d".to_string(),
            "Date Before: before:2024-12-31 or until:1w".to_string(),
            "Note Type:   type:daily or type:notes".to_string(),
            "".to_string(),
            "Relative Dates: 7d (days), 2w (weeks), 1m (months)".to_string(),
            "Special: today, yesterday, lastweek".to_string(),
            "".to_string(),
            "Example: tag:project after:7d type:notes".to_string(),
        ]
    }

    /// Format filter chips for display
    pub fn format_chips(filters: &SearchFilters) -> Vec<(String, String)> {
        let mut chips = Vec::new();

        // Tag chips
        for tag in &filters.tags {
            chips.push(("tag".to_string(), tag.clone()));
        }

        // Date chips
        if let Some(from) = filters.from_date {
            chips.push(("after".to_string(), from.format("%Y-%m-%d").to_string()));
        }
        if let Some(to) = filters.to_date {
            chips.push(("before".to_string(), to.format("%Y-%m-%d").to_string()));
        }

        // Type chip
        match filters.note_type {
            NoteTypeFilter::DailyNotes => chips.push(("type".to_string(), "daily".to_string())),
            NoteTypeFilter::RegularNotes => chips.push(("type".to_string(), "notes".to_string())),
            NoteTypeFilter::All => {}
        }

        chips
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_filters() {
        let filters = SearchFilters::parse("tag:rust tag:async after:2024-01-01").unwrap();
        assert_eq!(filters.tags.len(), 2);
        assert!(filters.tags.contains("rust"));
        assert!(filters.tags.contains("async"));
        assert!(filters.from_date.is_some());
    }

    #[test]
    fn test_relative_dates() {
        let filters = SearchFilters::parse("since:7d").unwrap();
        assert!(filters.from_date.is_some());

        let filters = SearchFilters::parse("since:today").unwrap();
        assert!(filters.from_date.is_some());
    }

    #[test]
    fn test_note_type_filter() {
        let filters = SearchFilters::parse("type:daily").unwrap();
        assert_eq!(filters.note_type, NoteTypeFilter::DailyNotes);

        let filters = SearchFilters::parse("type:notes").unwrap();
        assert_eq!(filters.note_type, NoteTypeFilter::RegularNotes);
    }

    #[test]
    fn test_filter_description() {
        let mut filters = SearchFilters::new();
        filters.add_tag("rust".to_string());
        filters.add_tag("async".to_string());

        let desc = filters.description();
        assert!(desc.contains("tags:"));
        assert!(desc.contains("rust"));
    }
}
