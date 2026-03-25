//! Markdown frontmatter parsing and filename sanitization

use crate::error::{Result, TeselaError};
use crate::link::{extract_wiki_links, Link};
use crate::note::NoteMetadata;
use chrono::{DateTime, Utc};
use gray_matter::engine::YAML;
use gray_matter::Matter;
use std::collections::HashMap;

/// Parse frontmatter from markdown content.
/// Returns (metadata, body) where body is the content without frontmatter.
pub fn parse_frontmatter(content: &str) -> Result<(NoteMetadata, String)> {
    let matter = Matter::<YAML>::new();
    let parsed = matter.parse(content);
    let metadata = extract_metadata(&parsed.data)?;
    Ok((metadata, parsed.content))
}

/// Extract wiki links from the body of a note
pub fn extract_links_from_body(body: &str) -> Vec<Link> {
    extract_wiki_links(body)
}

fn extract_metadata(data: &Option<gray_matter::Pod>) -> Result<NoteMetadata> {
    let mut metadata = NoteMetadata::default();

    if let Some(gray_matter::Pod::Hash(map)) = data {
        // Extract title
        if let Some(gray_matter::Pod::String(title)) = map.get("title") {
            metadata.title = Some(title.clone());
        }

        // Extract tags
        if let Some(gray_matter::Pod::Array(tags)) = map.get("tags") {
            metadata.tags = tags
                .iter()
                .filter_map(|t| {
                    if let gray_matter::Pod::String(s) = t {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect();
        }

        // Extract aliases
        if let Some(gray_matter::Pod::Array(aliases)) = map.get("aliases") {
            metadata.aliases = aliases
                .iter()
                .filter_map(|a| {
                    if let gray_matter::Pod::String(s) = a {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect();
        }

        // Extract created date
        if let Some(gray_matter::Pod::String(created)) = map.get("created") {
            metadata.created = created.parse::<DateTime<Utc>>().ok();
        }

        // Extract modified date
        if let Some(gray_matter::Pod::String(modified)) = map.get("modified") {
            metadata.modified = modified.parse::<DateTime<Utc>>().ok();
        }

        // Extract page type
        if let Some(gray_matter::Pod::String(note_type)) = map.get("type") {
            metadata.note_type = Some(note_type.clone());
        }

        // Collect remaining keys as custom
        let known_keys = ["title", "tags", "aliases", "created", "modified", "type"];
        for (key, value) in map {
            if !known_keys.contains(&key.as_str()) {
                if let Ok(json_val) = pod_to_json(value) {
                    metadata.custom.insert(key.clone(), json_val);
                }
            }
        }
    }

    Ok(metadata)
}

fn pod_to_json(pod: &gray_matter::Pod) -> std::result::Result<serde_json::Value, TeselaError> {
    match pod {
        gray_matter::Pod::String(s) => Ok(serde_json::Value::String(s.clone())),
        gray_matter::Pod::Integer(i) => Ok(serde_json::json!(*i)),
        gray_matter::Pod::Float(f) => Ok(serde_json::json!(*f)),
        gray_matter::Pod::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
        gray_matter::Pod::Array(arr) => {
            let items: std::result::Result<Vec<_>, _> = arr.iter().map(pod_to_json).collect();
            Ok(serde_json::Value::Array(items?))
        }
        gray_matter::Pod::Hash(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                obj.insert(k.clone(), pod_to_json(v)?);
            }
            Ok(serde_json::Value::Object(obj))
        }
        gray_matter::Pod::Null => Ok(serde_json::Value::Null),
    }
}

/// Sanitize a string for use as a filename
pub fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();

    sanitized.trim_matches('-').to_lowercase().replace(' ', "-")
}

/// Generate frontmatter string from metadata
pub fn generate_frontmatter(
    title: &str,
    tags: &[&str],
    created: DateTime<Utc>,
    extra: &HashMap<String, serde_json::Value>,
) -> String {
    let mut fm = format!("---\ntitle: \"{}\"\n", title);

    if !tags.is_empty() {
        let tag_list: Vec<String> = tags.iter().map(|t| format!("\"{}\"", t)).collect();
        fm.push_str(&format!("tags: [{}]\n", tag_list.join(", ")));
    }

    fm.push_str(&format!(
        "created: {}\n",
        created.format("%Y-%m-%dT%H:%M:%SZ")
    ));

    for (key, value) in extra {
        fm.push_str(&format!("{}: {}\n", key, value));
    }

    fm.push_str("---\n");
    fm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_with_metadata() {
        let content = r#"---
title: Test Note
tags: [test, example]
aliases: [test-note, sample]
---

# Test Note

This is a test note."#;

        let (metadata, body) = parse_frontmatter(content).unwrap();
        assert_eq!(metadata.title, Some("Test Note".to_string()));
        assert_eq!(metadata.tags, vec!["test", "example"]);
        assert_eq!(metadata.aliases, vec!["test-note", "sample"]);
        assert!(body.contains("This is a test note"));
    }

    #[test]
    fn test_parse_frontmatter_without_metadata() {
        let content = "# Just a heading\n\nSome content.";
        let (metadata, body) = parse_frontmatter(content).unwrap();
        assert!(metadata.title.is_none());
        assert!(metadata.tags.is_empty());
        assert!(body.contains("Just a heading"));
    }

    #[test]
    fn test_sanitize_filename_spaces() {
        assert_eq!(sanitize_filename("My Great Note"), "my-great-note");
    }

    #[test]
    fn test_sanitize_filename_special_chars() {
        assert_eq!(
            sanitize_filename("file:name/with*bad?chars"),
            "file-name-with-bad-chars"
        );
    }

    #[test]
    fn test_sanitize_filename_leading_trailing_dashes() {
        assert_eq!(sanitize_filename("/leading/"), "leading");
    }

    #[test]
    fn test_sanitize_filename_unicode() {
        // Unicode letters are preserved, lowercased
        let result = sanitize_filename("Cafe Resume");
        assert_eq!(result, "cafe-resume");
    }

    #[test]
    fn test_sanitize_filename_empty_after_strip() {
        assert_eq!(sanitize_filename("///"), "");
    }

    #[test]
    fn test_frontmatter_roundtrip() {
        let original_content = r#"---
title: "Round Trip"
tags: ["alpha", "beta"]
created: 2026-03-18T00:00:00Z
---

Body content here."#;

        let (metadata, body) = parse_frontmatter(original_content).unwrap();
        assert_eq!(metadata.title, Some("Round Trip".to_string()));
        assert_eq!(metadata.tags, vec!["alpha", "beta"]);
        assert!(body.contains("Body content here"));

        // Reconstruct
        let tags: Vec<&str> = metadata.tags.iter().map(|s| s.as_str()).collect();
        let created = metadata.created.unwrap();
        let fm = generate_frontmatter(
            metadata.title.as_deref().unwrap_or(""),
            &tags,
            created,
            &Default::default(),
        );
        assert!(fm.contains("title: \"Round Trip\""));
        assert!(fm.contains("tags:"));
        assert!(fm.contains("alpha"));
    }

    #[test]
    fn test_extract_links_from_body() {
        let body = "See [[other-note]] and [[link|display]].";
        let links = extract_links_from_body(body);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, "other-note");
    }
}
