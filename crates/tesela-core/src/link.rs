//! Link types and wiki-link parsing for Tesela

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Extracted link from markdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    /// Type of link (internal, external, attachment)
    pub link_type: LinkType,
    /// Target of the link
    pub target: String,
    /// Link text
    pub text: String,
    /// Position in the document (byte offset)
    pub position: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LinkType {
    Internal,
    External,
    Attachment,
}

/// Lightweight source→target pair for graph rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
}

static WIKI_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").unwrap());

/// Parse [[wiki-links]] from markdown content
pub fn extract_wiki_links(content: &str) -> Vec<Link> {
    WIKI_LINK_RE
        .captures_iter(content)
        .map(|cap| {
            let whole_match = cap.get(0).unwrap();
            let target = cap[1].trim().to_string();
            let pos = whole_match.start();
            // Extract the full line containing the link for context
            let line_start = content[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let line_end = content[pos..].find('\n').map(|i| pos + i).unwrap_or(content.len());
            let full_line = content[line_start..line_end].trim().to_string();
            Link {
                link_type: LinkType::Internal,
                target,
                text: full_line,
                position: pos,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_wiki_link() {
        let links = extract_wiki_links("See [[my-note]] for details.");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "my-note");
        assert_eq!(links[0].text, "See [[my-note]] for details.");
        assert_eq!(links[0].link_type, LinkType::Internal);
        assert_eq!(links[0].position, 4);
    }

    #[test]
    fn test_extract_wiki_link_with_alias() {
        let links = extract_wiki_links("Check [[target-note|display text]] here.");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "target-note");
        assert_eq!(links[0].text, "Check [[target-note|display text]] here.");
    }

    #[test]
    fn test_extract_multiple_wiki_links() {
        let content = "Link to [[note-a]] and [[note-b|Note B]] end.";
        let links = extract_wiki_links(content);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, "note-a");
        assert_eq!(links[0].text, "Link to [[note-a]] and [[note-b|Note B]] end.");
        assert_eq!(links[1].target, "note-b");
        assert_eq!(links[1].text, "Link to [[note-a]] and [[note-b|Note B]] end.");
    }

    #[test]
    fn test_extract_no_wiki_links() {
        let links = extract_wiki_links("No links here, just [markdown](link).");
        assert!(links.is_empty());
    }

    #[test]
    fn test_wiki_link_position() {
        let content = "ABC [[target]] XYZ";
        let links = extract_wiki_links(content);
        assert_eq!(links[0].position, 4);
    }
}
