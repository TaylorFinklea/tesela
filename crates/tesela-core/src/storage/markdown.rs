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

/// Add `tag` to the YAML frontmatter's `tags:` array. Returns `Some(updated_content)`
/// if a change was made, or `None` if the tag was already present (or the input is
/// unparseable / the tag is empty).
///
/// Preserves the body byte-for-byte and leaves every other frontmatter field
/// untouched. Only the `tags:` line (or a newly appended one) is modified.
///
/// Handles the forms produced by `generate_frontmatter` and the importers:
/// - `tags: []`                                       → `tags: ["tag"]`
/// - `tags: [a]`                                      → `tags: [a, "tag"]`
/// - `tags: [a, b]`                                   → `tags: [a, b, "tag"]`
/// - `tags: ["a", "b"]`                               → `tags: ["a", "b", "tag"]`
/// - `tags: a` / `tags: a, b` (no brackets, rare)     → `tags: a, tag` / `tags: a, b, tag`
/// - `tags:` block-style (items on subsequent lines)  → appends a new `- tag` line
/// - No `tags:` line at all                           → appends `tags: ["tag"]\n`
/// - No frontmatter at all                            → prepends `---\ntags: ["tag"]\n---\n`
pub fn add_tag_to_frontmatter(content: &str, tag: &str) -> Option<String> {
    if tag.is_empty() {
        return None;
    }

    // Use the existing parser to check for the tag — handles every quoting form
    // uniformly so idempotency is automatic.
    let (parsed_meta, _) = parse_frontmatter(content).ok()?;
    if parsed_meta.tags.iter().any(|t| t == tag) {
        return None;
    }

    // No frontmatter → prepend a minimal one.
    if !content.starts_with("---\n") {
        let mut out = String::with_capacity(content.len() + tag.len() + 16);
        out.push_str("---\ntags: [\"");
        out.push_str(tag);
        out.push_str("\"]\n---\n");
        out.push_str(content);
        return Some(out);
    }

    // Locate the frontmatter block: the opening `---\n` and the closing `---` line.
    let (header_end, body_start) = locate_frontmatter_block(content)?;

    // Modify the existing tags line, or append a new one if absent.
    let frontmatter = &content[header_end..body_start];
    let new_frontmatter = upsert_tags_line(frontmatter, tag)?;

    let mut out = String::with_capacity(content.len() + 32);
    out.push_str(&content[..header_end]);
    out.push_str(&new_frontmatter);
    out.push_str(&content[body_start..]);
    Some(out)
}

/// Returns `(header_end, body_start)` byte indices into `content` such that:
/// - `content[..header_end]` is the opening `---\n`
/// - `content[header_end..body_start]` is the frontmatter body
/// - `content[body_start..]` is the closing `---` line and the rest of the file
fn locate_frontmatter_block(content: &str) -> Option<(usize, usize)> {
    const OPEN: &str = "---\n";
    if !content.starts_with(OPEN) {
        return None;
    }
    let rest = &content[OPEN.len()..];
    let mut consumed = 0usize;
    for line in rest.split_inclusive('\n') {
        let stripped = line.trim_end_matches('\n').trim_end_matches('\r');
        if stripped == "---" {
            return Some((OPEN.len(), OPEN.len() + consumed));
        }
        consumed += line.len();
    }
    None
}

/// Walk the frontmatter, rewriting the first `tags:` line to include `tag` (or
/// appending a new `tags:` line if none is present). Preserves every other line
/// byte-for-byte, including its trailing newline.
fn upsert_tags_line(frontmatter: &str, tag: &str) -> Option<String> {
    let lines: Vec<&str> = frontmatter.split_inclusive('\n').collect();
    let mut out = String::with_capacity(frontmatter.len() + tag.len() + 16);
    let mut found = false;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if !found && is_tags_line(line) {
            if is_block_style_start(line) {
                // Block-style: re-emit the `tags:` line, walk past any
                // existing `- item` lines, then append a new item with the
                // same indent as the first existing one.
                out.push_str(line);
                i += 1;
                let item_indent = if i < lines.len() {
                    block_item_indent(lines[i])
                } else {
                    "  ".to_string()
                };
                while i < lines.len() && is_block_item_line(lines[i]) {
                    out.push_str(lines[i]);
                    i += 1;
                }
                out.push_str(&item_indent);
                out.push_str("- ");
                out.push_str(tag);
                out.push('\n');
                found = true;
            } else {
                out.push_str(&modify_tags_line(line, tag));
                i += 1;
                found = true;
            }
        } else {
            out.push_str(line);
            i += 1;
        }
    }
    if !found {
        // No tags line — append a new one at the end of the frontmatter.
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("tags: [\"");
        out.push_str(tag);
        out.push_str("\"]\n");
    }
    Some(out)
}

fn is_block_style_start(line: &str) -> bool {
    // `tags:` alone on a line, with items on subsequent `- ` lines.
    line.trim_start().trim_end() == "tags:"
}

fn is_block_item_line(line: &str) -> bool {
    // A list item: optional indent, then `- ` (or trailing `-`).
    let trimmed = line.trim_start();
    trimmed.starts_with("- ") || trimmed.trim_end() == "-"
}

fn block_item_indent(line: &str) -> String {
    // Re-use the indent of the first existing item so we match the user's
    // chosen list-item indent (typically 2 spaces).
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();
    line[..indent_len].to_string()
}

fn is_tags_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    // Match `tags:` but NOT `tags::` (the engine's materialized continuation
    // syntax for the `tags` block property — those are per-block, not page-level).
    trimmed.starts_with("tags:") && !trimmed.starts_with("tags::")
}

fn modify_tags_line(line: &str, tag: &str) -> String {
    let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    let trimmed = line.trim_start().trim_end();
    let trailing = &line[line.trim_end_matches('\n').len()..];
    let after_colon = trimmed.strip_prefix("tags:").unwrap_or("").trim_start();

    if after_colon.is_empty() {
        // Block-style: `tags:` alone, items follow on `- ` lines.
        let mut out = String::with_capacity(line.len() + tag.len() + 8);
        out.push_str(&indent);
        out.push_str("tags:\n");
        out.push_str(&indent);
        out.push_str("  - ");
        out.push_str(tag);
        out.push('\n');
        out.push_str(trailing);
        return out;
    }

    if after_colon.starts_with('[') {
        // Bracket form. Find the closing bracket on the same line.
        if let Some(close_idx) = after_colon.find(']') {
            let inner = after_colon[1..close_idx].trim();
            // Match the existing quoting style: if any item is quoted, quote the new one too.
            // An empty array has no style preference — emit the canonical quoted form.
            let uses_quotes = inner.is_empty() || inner.contains('"') || inner.contains('\'');
            let new_item = if uses_quotes {
                let q = if inner.contains('\'') && !inner.contains('"') {
                    '\''
                } else {
                    '"'
                };
                let mut s = String::with_capacity(tag.len() + 2);
                s.push(q);
                s.push_str(tag);
                s.push(q);
                s
            } else {
                tag.to_string()
            };
            let new_inner = if inner.is_empty() {
                new_item
            } else {
                let mut s = String::with_capacity(inner.len() + new_item.len() + 2);
                s.push_str(inner.trim_end_matches(','));
                s.push_str(", ");
                s.push_str(&new_item);
                s
            };
            let mut out = String::with_capacity(line.len() + tag.len() + 4);
            out.push_str(&indent);
            out.push_str("tags: [");
            out.push_str(&new_inner);
            out.push(']');
            out.push_str(trailing);
            return out;
        }
        // Multi-line bracket form is rare and ambiguous — return the line unchanged.
        return line.to_string();
    }

    // No brackets: `tags: a` or `tags: a, b`. Append `, tag` (no quote to match the unquoted style).
    let value = after_colon.trim();
    let mut out = String::with_capacity(line.len() + tag.len() + 2);
    out.push_str(&indent);
    out.push_str("tags: ");
    if value.is_empty() {
        out.push_str(tag);
    } else {
        out.push_str(value);
        out.push_str(", ");
        out.push_str(tag);
    }
    out.push_str(trailing);
    out
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

    // --- add_tag_to_frontmatter ---

    #[test]
    fn test_add_tag_to_frontmatter_already_present_returns_none() {
        let content = "---\ntitle: \"t\"\ntags: [\"daily\"]\n---\nbody\n";
        assert!(add_tag_to_frontmatter(content, "daily").is_none());
    }

    #[test]
    fn test_add_tag_to_frontmatter_appends_to_quoted_list() {
        let content = "---\ntitle: \"t\"\ntags: [\"a\", \"b\"]\n---\nbody\n";
        let updated = add_tag_to_frontmatter(content, "daily").unwrap();
        assert!(updated.contains("tags: [\"a\", \"b\", \"daily\"]"));
        assert!(updated.contains("body\n"));
    }

    #[test]
    fn test_add_tag_to_frontmatter_appends_to_unquoted_list() {
        let content = "---\ntitle: \"t\"\ntags: [a, b]\n---\nbody\n";
        let updated = add_tag_to_frontmatter(content, "daily").unwrap();
        // Unquoted style is preserved — no quotes on the appended tag.
        assert!(updated.contains("tags: [a, b, daily]"));
    }

    #[test]
    fn test_add_tag_to_frontmatter_empty_brackets() {
        let content = "---\ntitle: \"t\"\ntags: []\n---\nbody\n";
        let updated = add_tag_to_frontmatter(content, "daily").unwrap();
        assert!(updated.contains("tags: [\"daily\"]"));
    }

    #[test]
    fn test_add_tag_to_frontmatter_no_tags_line_appends() {
        let content = "---\ntitle: \"t\"\ncreated: 2026-06-10T00:00:00Z\n---\nbody\n";
        let updated = add_tag_to_frontmatter(content, "daily").unwrap();
        // Other fields are preserved exactly.
        assert!(updated.contains("title: \"t\""));
        assert!(updated.contains("created: 2026-06-10T00:00:00Z"));
        // The new tags line is appended at the end of the frontmatter.
        assert!(updated.contains("tags: [\"daily\"]\n---\nbody\n"));
    }

    #[test]
    fn test_add_tag_to_frontmatter_no_frontmatter_prepends() {
        let content = "just a body line\n";
        let updated = add_tag_to_frontmatter(content, "daily").unwrap();
        assert!(updated.starts_with("---\ntags: [\"daily\"]\n---\n"));
        assert!(updated.contains("just a body line"));
    }

    #[test]
    fn test_add_tag_to_frontmatter_preserves_body_and_other_fields() {
        let content = "\
---
title: \"2026-06-10\"
created: 2026-06-10T00:00:00Z
aliases: [\"journal-2026-06-10\"]
---

- visible journal block
  - child
";
        let updated = add_tag_to_frontmatter(content, "daily").unwrap();
        // Every other field is preserved exactly.
        assert!(updated.contains("title: \"2026-06-10\""));
        assert!(updated.contains("created: 2026-06-10T00:00:00Z"));
        assert!(updated.contains("aliases: [\"journal-2026-06-10\"]"));
        // Body is byte-for-byte identical to the original body.
        let body_start = updated.find("- visible journal block").unwrap();
        assert_eq!(
            &updated[body_start..],
            "- visible journal block\n  - child\n"
        );
    }

    #[test]
    fn test_add_tag_to_frontmatter_block_style_appends_dash_line() {
        let content = "---\ntitle: \"t\"\ntags:\n  - a\n  - b\n---\nbody\n";
        let updated = add_tag_to_frontmatter(content, "daily").unwrap();
        assert!(updated.contains("tags:\n  - a\n  - b\n  - daily\n"));
    }

    #[test]
    fn test_add_tag_to_frontmatter_empty_tag_returns_none() {
        let content = "---\ntitle: \"t\"\ntags: []\n---\nbody\n";
        assert!(add_tag_to_frontmatter(content, "").is_none());
    }

    #[test]
    fn test_add_tag_to_frontmatter_idempotent_via_parser() {
        // The accepted form is parsed and re-detected on a second run, so a
        // caller that always canonicalizes through this function gets
        // idempotency for free.
        let content = "---\ntitle: \"t\"\n---\nbody\n";
        let once = add_tag_to_frontmatter(content, "daily").unwrap();
        let twice = add_tag_to_frontmatter(&once, "daily");
        assert!(twice.is_none(), "second run is a no-op: got {twice:?}");
    }
}
