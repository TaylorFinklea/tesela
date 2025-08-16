//! Search module for Tesela
//!
//! This module provides advanced search capabilities including full-text search,
//! tag filtering, date range queries, and semantic search functionality.

use crate::core::database::Database;
use crate::core::error::{Result, TeselaError};
use crate::core::storage::Note;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use std::sync::Arc;
use tracing::debug;

/// Search query types
#[derive(Debug, Clone, PartialEq)]
pub enum QueryType {
    /// Full-text search across note content
    FullText(String),
    /// Tag-based search
    Tag(String),
    /// Search by note title
    Title(String),
    /// Date range search
    DateRange {
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    },
    /// Combined search with multiple criteria
    Combined(Vec<SearchCriteria>),
}

/// Individual search criteria
#[derive(Debug, Clone, PartialEq)]
pub struct SearchCriteria {
    pub query_type: QueryType,
    pub weight: f32,
    pub required: bool,
}

/// Search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Maximum number of search results
    pub max_results: usize,
    /// Number of context lines to show around matches
    pub context_lines: usize,
    /// Enable fuzzy matching
    pub fuzzy_search: bool,
    /// Fuzzy search threshold (0.0 to 1.0)
    pub fuzzy_threshold: f32,
    /// Enable search result highlighting
    pub highlight_matches: bool,
    /// Boost factor for title matches
    pub title_boost: f32,
    /// Boost factor for recent notes
    pub recency_boost: f32,
    /// Enable search suggestions
    pub enable_suggestions: bool,
    /// Maximum number of suggestions
    pub max_suggestions: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_results: 50,
            context_lines: 2,
            fuzzy_search: true,
            fuzzy_threshold: 0.6,
            highlight_matches: true,
            title_boost: 2.0,
            recency_boost: 0.1,
            enable_suggestions: true,
            max_suggestions: 5,
        }
    }
}

/// Search result with metadata
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub note: Note,
    pub score: f32,
    pub matches: Vec<SearchMatch>,
    pub highlighted_content: Option<String>,
}

/// Individual match within a note
#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub match_type: MatchType,
    pub content: String,
    pub line_number: usize,
    pub start_offset: usize,
    pub end_offset: usize,
    pub context_before: String,
    pub context_after: String,
}

/// Type of match found
#[derive(Debug, Clone, PartialEq)]
pub enum MatchType {
    Title,
    Body,
    Tag,
    Metadata,
}

/// Search suggestion
#[derive(Debug, Clone)]
pub struct SearchSuggestion {
    pub suggestion: String,
    pub suggestion_type: SuggestionType,
    pub confidence: f32,
}

/// Type of search suggestion
#[derive(Debug, Clone, PartialEq)]
pub enum SuggestionType {
    Query,
    Tag,
    Title,
    Correction,
}

/// Main search engine
pub struct SearchEngine {
    database: Arc<Database>,
    config: SearchConfig,
    query_history: Vec<String>,
}

impl SearchEngine {
    /// Create a new search engine
    pub fn new(database: Arc<Database>, config: SearchConfig) -> Self {
        Self {
            database,
            config,
            query_history: Vec::new(),
        }
    }

    /// Perform a search with the given query
    pub async fn search(&mut self, query: &str) -> Result<Vec<SearchResult>> {
        debug!("Performing search: {}", query);

        // Add to query history
        self.query_history.push(query.to_string());
        if self.query_history.len() > 100 {
            self.query_history.remove(0);
        }

        // Parse the query
        let parsed_query = self.parse_query(query)?;

        // Execute the search
        let results = self.execute_search(parsed_query).await?;

        debug!("Search completed: {} results", results.len());
        Ok(results)
    }

    /// Search by tag
    pub async fn search_by_tag(&self, tag: &str) -> Result<Vec<SearchResult>> {
        debug!("Searching by tag: {}", tag);

        let notes = self.database.get_notes_by_tag(tag).await?;
        let mut results = Vec::new();

        for note in notes {
            let search_result = SearchResult {
                score: 1.0, // Tag searches get perfect score
                matches: vec![SearchMatch {
                    match_type: MatchType::Tag,
                    content: tag.to_string(),
                    line_number: 0,
                    start_offset: 0,
                    end_offset: tag.len(),
                    context_before: String::new(),
                    context_after: String::new(),
                }],
                highlighted_content: None,
                note,
            };
            results.push(search_result);
        }

        Ok(results)
    }

    /// Get search suggestions for a partial query
    pub async fn get_suggestions(&self, partial_query: &str) -> Result<Vec<SearchSuggestion>> {
        if !self.config.enable_suggestions || partial_query.len() < 2 {
            return Ok(Vec::new());
        }

        debug!("Getting suggestions for: {}", partial_query);

        let mut suggestions = Vec::new();

        // Get tag suggestions
        let tags = self.database.get_tags_with_counts().await?;
        for (tag, _count) in tags {
            if tag
                .to_lowercase()
                .starts_with(&partial_query.to_lowercase())
            {
                let confidence = if tag.len() == partial_query.len() {
                    1.0
                } else {
                    0.8 - (tag.len() - partial_query.len()) as f32 * 0.1
                };

                suggestions.push(SearchSuggestion {
                    suggestion: format!("tag:{}", tag),
                    suggestion_type: SuggestionType::Tag,
                    confidence: confidence.max(0.1),
                });
            }
        }

        // Get query suggestions from history
        for query in &self.query_history {
            if query
                .to_lowercase()
                .starts_with(&partial_query.to_lowercase())
                && query != partial_query
            {
                suggestions.push(SearchSuggestion {
                    suggestion: query.clone(),
                    suggestion_type: SuggestionType::Query,
                    confidence: 0.7,
                });
            }
        }

        // Sort by confidence and limit
        suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        suggestions.truncate(self.config.max_suggestions);

        Ok(suggestions)
    }

    /// Parse a search query into structured criteria
    fn parse_query(&self, query: &str) -> Result<Vec<SearchCriteria>> {
        let mut criteria = Vec::new();
        let trimmed = query.trim();

        if trimmed.is_empty() {
            return Err(TeselaError::search("Empty search query"));
        }

        // Check for special syntax
        if trimmed.starts_with("tag:") {
            let tag = trimmed.strip_prefix("tag:").unwrap().trim();
            criteria.push(SearchCriteria {
                query_type: QueryType::Tag(tag.to_string()),
                weight: 1.0,
                required: true,
            });
        } else if trimmed.starts_with("title:") {
            let title = trimmed.strip_prefix("title:").unwrap().trim();
            criteria.push(SearchCriteria {
                query_type: QueryType::Title(title.to_string()),
                weight: self.config.title_boost,
                required: true,
            });
        } else {
            // Default to full-text search
            criteria.push(SearchCriteria {
                query_type: QueryType::FullText(trimmed.to_string()),
                weight: 1.0,
                required: true,
            });
        }

        Ok(criteria)
    }

    /// Execute a parsed search query
    async fn execute_search(&self, criteria: Vec<SearchCriteria>) -> Result<Vec<SearchResult>> {
        let mut all_results = Vec::new();

        for criterion in criteria {
            let mut results = match criterion.query_type {
                QueryType::FullText(ref query) => self.perform_fulltext_search(query).await?,
                QueryType::Tag(ref tag) => self.search_by_tag(tag).await?,
                QueryType::Title(ref title) => self.search_by_title(title).await?,
                QueryType::DateRange { from, to } => self.search_by_date_range(from, to).await?,
                QueryType::Combined(_) => {
                    // Recursive handling for combined queries
                    continue;
                }
            };

            // Apply weight to scores
            for result in &mut results {
                result.score *= criterion.weight;
            }

            all_results.extend(results);
        }

        // Merge and rank results
        self.merge_and_rank_results(all_results).await
    }

    /// Perform full-text search using the database
    async fn perform_fulltext_search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let notes = self
            .database
            .search_notes(query, self.config.max_results as i32, 0)
            .await?;

        let mut results = Vec::new();

        for note in notes {
            let matches = self.find_matches_in_note(&note, query);
            let highlighted_content = if self.config.highlight_matches {
                Some(self.highlight_matches(&note.content, query))
            } else {
                None
            };

            let mut score = 1.0;

            // Apply recency boost
            let days_old = (Utc::now() - note.modified_at).num_days() as f32;
            score += self.config.recency_boost * (1.0 / (days_old + 1.0));

            // Apply title boost if query matches title
            if note.title.to_lowercase().contains(&query.to_lowercase()) {
                score *= self.config.title_boost;
            }

            results.push(SearchResult {
                note,
                score,
                matches,
                highlighted_content,
            });
        }

        Ok(results)
    }

    /// Search by note title
    async fn search_by_title(&self, title: &str) -> Result<Vec<SearchResult>> {
        // Use FTS to search in titles, but this is a simplified implementation
        self.perform_fulltext_search(title).await
    }

    /// Search by date range
    async fn search_by_date_range(
        &self,
        _from: Option<DateTime<Utc>>,
        _to: Option<DateTime<Utc>>,
    ) -> Result<Vec<SearchResult>> {
        // This would need additional database queries by date
        // For now, return empty results
        Ok(Vec::new())
    }

    /// Find matches within a note
    fn find_matches_in_note(&self, note: &Note, query: &str) -> Vec<SearchMatch> {
        let mut matches = Vec::new();
        let query_lower = query.to_lowercase();

        // Search in title
        if note.title.to_lowercase().contains(&query_lower) {
            matches.push(SearchMatch {
                match_type: MatchType::Title,
                content: note.title.clone(),
                line_number: 0,
                start_offset: 0,
                end_offset: note.title.len(),
                context_before: String::new(),
                context_after: String::new(),
            });
        }

        // Search in body content
        let lines: Vec<&str> = note.body.lines().collect();
        for (line_number, line) in lines.iter().enumerate() {
            if line.to_lowercase().contains(&query_lower) {
                let start_offset = line.to_lowercase().find(&query_lower).unwrap_or(0);
                let end_offset = start_offset + query.len();

                let context_before = if line_number > 0 {
                    lines.get(line_number - 1).unwrap_or(&"").to_string()
                } else {
                    String::new()
                };

                let context_after = if line_number < lines.len() - 1 {
                    lines.get(line_number + 1).unwrap_or(&"").to_string()
                } else {
                    String::new()
                };

                matches.push(SearchMatch {
                    match_type: MatchType::Body,
                    content: line.to_string(),
                    line_number: line_number + 1,
                    start_offset,
                    end_offset,
                    context_before,
                    context_after,
                });
            }
        }

        // Search in tags
        for tag in &note.metadata.tags {
            if tag.to_lowercase().contains(&query_lower) {
                matches.push(SearchMatch {
                    match_type: MatchType::Tag,
                    content: tag.clone(),
                    line_number: 0,
                    start_offset: 0,
                    end_offset: tag.len(),
                    context_before: String::new(),
                    context_after: String::new(),
                });
            }
        }

        matches
    }

    /// Highlight matches in content
    fn highlight_matches(&self, content: &str, query: &str) -> String {
        let query_lower = query.to_lowercase();
        let content_lower = content.to_lowercase();

        if let Some(start) = content_lower.find(&query_lower) {
            let end = start + query.len();
            let before = &content[..start];
            let matched = &content[start..end];
            let after = &content[end..];

            format!("{}**{}**{}", before, matched, after)
        } else {
            content.to_string()
        }
    }

    /// Merge and rank search results
    async fn merge_and_rank_results(
        &self,
        mut results: Vec<SearchResult>,
    ) -> Result<Vec<SearchResult>> {
        // Remove duplicates based on note ID
        let mut seen_ids = std::collections::HashSet::new();
        results.retain(|result| seen_ids.insert(result.note.id.clone()));

        // Sort by score (descending)
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Limit results
        results.truncate(self.config.max_results);

        Ok(results)
    }

    /// Get search statistics
    pub fn get_search_stats(&self) -> SearchStats {
        SearchStats {
            total_queries: self.query_history.len(),
            recent_queries: self.query_history.iter().rev().take(10).cloned().collect(),
            config: self.config.clone(),
        }
    }
}

/// Search statistics
#[derive(Debug, Clone)]
pub struct SearchStats {
    pub total_queries: usize,
    pub recent_queries: Vec<String>,
    pub config: SearchConfig,
}

/// Fuzzy string matching utility
pub fn fuzzy_match(pattern: &str, text: &str, threshold: f32) -> Option<f32> {
    if pattern.is_empty() || text.is_empty() {
        return None;
    }

    let pattern_lower = pattern.to_lowercase();
    let text_lower = text.to_lowercase();

    // Simple fuzzy matching based on character overlap
    let mut matches = 0;
    let mut last_pos = 0;

    for ch in pattern_lower.chars() {
        if let Some(pos) = text_lower[last_pos..].find(ch) {
            matches += 1;
            last_pos += pos + 1;
        }
    }

    let score = matches as f32 / pattern.len() as f32;
    if score >= threshold {
        Some(score)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::DatabaseConfig;
    use crate::core::storage::StorageConfig;
    use tempfile::TempDir;

    async fn create_test_search_engine() -> SearchEngine {
        let temp_dir = TempDir::new().unwrap();
        let db_config = DatabaseConfig {
            db_path: temp_dir.path().join("test.db"),
            ..Default::default()
        };
        let database = Arc::new(Database::new(db_config).await.unwrap());
        let config = SearchConfig::default();

        SearchEngine::new(database, config)
    }

    #[tokio::test]
    async fn test_search_engine_creation() {
        let _engine = create_test_search_engine().await;
    }

    #[tokio::test]
    async fn test_parse_query() {
        let engine = create_test_search_engine().await;

        let criteria = engine.parse_query("test query").unwrap();
        assert_eq!(criteria.len(), 1);
        assert!(matches!(criteria[0].query_type, QueryType::FullText(_)));

        let criteria = engine.parse_query("tag:rust").unwrap();
        assert_eq!(criteria.len(), 1);
        assert!(matches!(criteria[0].query_type, QueryType::Tag(_)));

        let criteria = engine.parse_query("title:my note").unwrap();
        assert_eq!(criteria.len(), 1);
        assert!(matches!(criteria[0].query_type, QueryType::Title(_)));
    }

    #[test]
    fn test_fuzzy_match() {
        assert!(fuzzy_match("hello", "hello world", 0.5).is_some());
        assert!(fuzzy_match("helo", "hello", 0.7).is_some());
        assert!(fuzzy_match("xyz", "hello", 0.5).is_none());

        let score = fuzzy_match("test", "testing", 0.5).unwrap();
        assert!(score > 0.5);
    }

    // TODO: Fix this test - database table issue
    // #[tokio::test]
    // async fn test_search_suggestions() {
    //     let mut engine = create_test_search_engine().await;

    //     // Add some query history
    //     engine.query_history.push("rust programming".to_string());
    //     engine.query_history.push("rust tutorial".to_string());

    //     let suggestions = engine.get_suggestions("ru").await.unwrap();
    //     // Should return suggestions starting with "ru"
    //     assert!(!suggestions.is_empty());
    // }

    #[tokio::test]
    async fn test_highlight_matches() {
        let engine = create_test_search_engine().await;

        let content = "This is a test content";
        let highlighted = engine.highlight_matches(content, "test");
        assert!(highlighted.contains("**test**"));
    }
}
