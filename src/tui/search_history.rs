//! Search history management for the TUI

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

const MAX_HISTORY_SIZE: usize = 100;
const HISTORY_FILE: &str = ".tesela/search_history.json";

/// Search history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub query: String,
    pub timestamp: i64,
    pub result_count: usize,
}

/// Search history manager
#[derive(Debug)]
pub struct SearchHistory {
    entries: VecDeque<HistoryEntry>,
    path: PathBuf,
}

impl SearchHistory {
    /// Create or load search history
    pub fn new() -> Self {
        let path = PathBuf::from(HISTORY_FILE);
        let entries = Self::load_from_file(&path).unwrap_or_else(|_| VecDeque::new());

        Self { entries, path }
    }

    /// Add a new search to history
    pub fn add(&mut self, query: String, result_count: usize) {
        // Don't add empty queries
        if query.trim().is_empty() {
            return;
        }

        // Remove duplicate if it exists
        self.entries.retain(|e| e.query != query);

        // Add new entry at the front
        let entry = HistoryEntry {
            query,
            timestamp: chrono::Utc::now().timestamp(),
            result_count,
        };

        self.entries.push_front(entry);

        // Limit history size
        while self.entries.len() > MAX_HISTORY_SIZE {
            self.entries.pop_back();
        }

        // Save to disk
        let _ = self.save_to_file();
    }

    /// Get recent searches
    pub fn recent(&self, limit: usize) -> Vec<HistoryEntry> {
        self.entries.iter().take(limit).cloned().collect()
    }

    /// Search history for entries matching a prefix
    pub fn search(&self, prefix: &str) -> Vec<HistoryEntry> {
        if prefix.is_empty() {
            return self.recent(10);
        }

        let prefix_lower = prefix.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.query.to_lowercase().starts_with(&prefix_lower))
            .take(10)
            .cloned()
            .collect()
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.entries.clear();
        let _ = self.save_to_file();
    }

    /// Load history from file
    fn load_from_file(path: &PathBuf) -> anyhow::Result<VecDeque<HistoryEntry>> {
        let content = fs::read_to_string(path)?;
        let entries: Vec<HistoryEntry> = serde_json::from_str(&content)?;
        Ok(entries.into_iter().collect())
    }

    /// Save history to file
    fn save_to_file(&self) -> anyhow::Result<()> {
        // Ensure directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let entries: Vec<&HistoryEntry> = self.entries.iter().collect();
        let content = serde_json::to_string_pretty(&entries)?;
        fs::write(&self.path, content)?;
        Ok(())
    }

    /// Get all unique queries for autocomplete
    pub fn unique_queries(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        self.entries
            .iter()
            .filter_map(|e| {
                if seen.insert(e.query.clone()) {
                    Some(e.query.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for SearchHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_history() {
        let mut history = SearchHistory::new();

        // Add some searches
        history.add("rust programming".to_string(), 10);
        history.add("cargo test".to_string(), 5);
        history.add("rust async".to_string(), 8);

        // Check recent
        let recent = history.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].query, "rust async");

        // Check search
        let results = history.search("rust");
        assert_eq!(results.len(), 2);

        // Check deduplication
        history.add("rust async".to_string(), 12);
        let recent = history.recent(3);
        assert_eq!(recent[0].query, "rust async");
        assert_eq!(recent[0].result_count, 12);
    }
}
