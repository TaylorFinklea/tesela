//! Fuzzy search implementation for note titles and content

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::collections::HashMap;

/// Fuzzy search engine for notes
pub struct FuzzySearch {
    matcher: SkimMatcherV2,
}

impl FuzzySearch {
    /// Create a new fuzzy search instance
    pub fn new() -> Self {
        Self {
            matcher: SkimMatcherV2::default(),
        }
    }

    /// Search note titles with fuzzy matching
    pub fn search_titles(
        &self,
        query: &str,
        titles: &[(String, String)],
    ) -> Vec<FuzzySearchResult> {
        if query.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();

        for (path, title) in titles {
            if let Some(score) = self.matcher.fuzzy_match(title, query) {
                let indices = self
                    .matcher
                    .fuzzy_indices(title, query)
                    .map(|(_, indices)| indices)
                    .unwrap_or_default();

                results.push(FuzzySearchResult {
                    path: path.clone(),
                    title: title.clone(),
                    score,
                    match_indices: indices,
                });
            }
        }

        // Sort by score (highest first)
        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    /// Search with multiple patterns (useful for space-separated queries)
    pub fn search_multi(&self, query: &str, titles: &[(String, String)]) -> Vec<FuzzySearchResult> {
        if query.is_empty() {
            return Vec::new();
        }

        // Split query into words
        let patterns: Vec<&str> = query.split_whitespace().collect();
        if patterns.is_empty() {
            return Vec::new();
        }

        let mut combined_scores: HashMap<String, (String, i64, Vec<usize>)> = HashMap::new();

        for (path, title) in titles {
            let mut total_score = 0i64;
            let mut all_indices = Vec::new();
            let mut all_matched = true;

            // Check if all patterns match
            for pattern in &patterns {
                if let Some(score) = self.matcher.fuzzy_match(title, pattern) {
                    total_score += score;

                    if let Some((_, indices)) = self.matcher.fuzzy_indices(title, pattern) {
                        all_indices.extend(indices);
                    }
                } else {
                    all_matched = false;
                    break;
                }
            }

            // Only include results where all patterns matched
            if all_matched && total_score > 0 {
                // Deduplicate and sort indices
                all_indices.sort_unstable();
                all_indices.dedup();

                combined_scores.insert(path.clone(), (title.clone(), total_score, all_indices));
            }
        }

        let mut results: Vec<FuzzySearchResult> = combined_scores
            .into_iter()
            .map(|(path, (title, score, indices))| FuzzySearchResult {
                path,
                title,
                score,
                match_indices: indices,
            })
            .collect();

        // Sort by score (highest first)
        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    /// Get suggestions for partial input (autocomplete)
    pub fn suggest(&self, prefix: &str, titles: &[(String, String)], limit: usize) -> Vec<String> {
        if prefix.is_empty() {
            return Vec::new();
        }

        let mut suggestions: Vec<(String, i64)> = Vec::new();

        for (_, title) in titles {
            // Check if title starts with prefix (case insensitive)
            if title.to_lowercase().starts_with(&prefix.to_lowercase()) {
                suggestions.push((title.clone(), 1000)); // High score for prefix matches
            } else if let Some(score) = self.matcher.fuzzy_match(title, prefix) {
                suggestions.push((title.clone(), score));
            }
        }

        // Sort by score and take top N
        suggestions.sort_by(|a, b| b.1.cmp(&a.1));
        suggestions
            .into_iter()
            .take(limit)
            .map(|(title, _)| title)
            .collect()
    }
}

impl Default for FuzzySearch {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a fuzzy search
#[derive(Debug, Clone)]
pub struct FuzzySearchResult {
    pub path: String,
    pub title: String,
    pub score: i64,
    pub match_indices: Vec<usize>,
}

impl FuzzySearchResult {
    /// Convert match indices to character ranges for highlighting
    pub fn get_highlight_ranges(&self) -> Vec<(usize, usize)> {
        if self.match_indices.is_empty() {
            return Vec::new();
        }

        let mut ranges = Vec::new();
        let mut start = self.match_indices[0];
        let mut end = start;

        for &idx in &self.match_indices[1..] {
            if idx == end + 1 {
                // Continuous range
                end = idx;
            } else {
                // Gap found, save current range and start new one
                ranges.push((start, end + 1));
                start = idx;
                end = idx;
            }
        }

        // Add the last range
        ranges.push((start, end + 1));
        ranges
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_search() {
        let fuzzy = FuzzySearch::new();
        let titles = vec![
            ("path1".to_string(), "Rust Programming Guide".to_string()),
            ("path2".to_string(), "Python Tutorial".to_string()),
            ("path3".to_string(), "JavaScript Basics".to_string()),
            ("path4".to_string(), "Rust Async Programming".to_string()),
        ];

        // Test single pattern search
        let results = fuzzy.search_titles("rust", &titles);
        assert_eq!(results.len(), 2);
        assert!(results[0].title.contains("Rust"));

        // Test multi-pattern search
        let results = fuzzy.search_multi("rust prog", &titles);
        assert_eq!(results.len(), 2);

        // Test suggestions
        let suggestions = fuzzy.suggest("rus", &titles, 3);
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_highlight_ranges() {
        let result = FuzzySearchResult {
            path: "test".to_string(),
            title: "test".to_string(),
            score: 100,
            match_indices: vec![0, 1, 2, 5, 6, 10],
        };

        let ranges = result.get_highlight_ranges();
        assert_eq!(ranges, vec![(0, 3), (5, 7), (10, 11)]);
    }
}
