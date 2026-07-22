//! Link types and wiki-link parsing for Tesela

use crate::regex_cache::WIKI_LINK_RE;
use serde::{Deserialize, Serialize};

#[cfg(test)]
use ts_rs::TS;

/// Extracted link from markdown
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
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
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub enum LinkType {
    Internal,
    External,
    Attachment,
}

/// Lightweight source→target pair for graph rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
}

/// Rebuildable typed relation projection. It is intentionally separate from
/// [`Link`] so wiki-link storage and consumers remain unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct RelationEdge {
    pub source_page_id: crate::PageId,
    pub source_note_id: String,
    pub source_block_id: Option<String>,
    pub property_key: String,
    pub target_page_id: crate::PageId,
}

/// Relation backlink plus source display context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct RelationBacklink {
    pub edge: RelationEdge,
    pub source_slug: String,
    pub source_title: String,
}

/// Parse [[wiki-links]] from markdown content
pub fn extract_wiki_links(content: &str) -> Vec<Link> {
    let fenced = crate::note_tree::markdown_fence_mask(content);
    extract_wiki_links_with_mask(content, &fenced)
}

/// Parse wiki links from an already-extracted note body or block fragment.
/// Leading `---` thematic rules remain body content rather than being treated
/// as YAML frontmatter delimiters.
pub fn extract_wiki_links_from_body(body: &str) -> Vec<Link> {
    let fenced = crate::note_tree::markdown_body_fence_mask(body);
    extract_wiki_links_with_mask(body, &fenced)
}

fn extract_wiki_links_with_mask(
    content: &str,
    fenced: &crate::note_tree::MarkdownFenceMask,
) -> Vec<Link> {
    WIKI_LINK_RE
        .captures_iter(content)
        .filter(|cap| {
            let whole = cap.get(0).expect("wiki-link regex has whole match");
            !fenced.overlaps(whole.start()..whole.end())
        })
        .map(|cap| {
            let whole_match = cap.get(0).unwrap();
            let target = cap[1].trim().to_string();
            let pos = whole_match.start();
            // Extract the full line containing the link for context
            let line_start = content[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let line_end = content[pos..]
                .find('\n')
                .map(|i| pos + i)
                .unwrap_or(content.len());
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
        assert_eq!(
            links[0].text,
            "Link to [[note-a]] and [[note-b|Note B]] end."
        );
        assert_eq!(links[1].target, "note-b");
        assert_eq!(
            links[1].text,
            "Link to [[note-a]] and [[note-b|Note B]] end."
        );
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

    #[test]
    fn fenced_wiki_links_are_inert_and_outside_positions_stay_original() {
        let content = "before [[visible-a]]\n```text\n[[hidden]]\n```\nafter [[visible-b]]";
        let links = extract_wiki_links(content);
        assert_eq!(
            links
                .iter()
                .map(|link| link.target.as_str())
                .collect::<Vec<_>>(),
            vec!["visible-a", "visible-b"]
        );
        assert_eq!(
            links[1].position,
            content.find("[[visible-b]]").unwrap(),
            "filtering fenced links must not shift source byte offsets"
        );
    }

    #[test]
    fn nested_and_same_line_fenced_links_are_inert() {
        let content = concat!(
            "before [[visible-a]]\n",
            "- Parent <!-- bid:11111111-1111-1111-1111-111111111111 -->\n",
            "  - Child <!-- bid:22222222-2222-2222-2222-222222222222 -->\n",
            "    ```text\n    [[nested-hidden]]\n    ```\n",
            "  - ```text\n    [[same-line-hidden]]\n    ```\n",
            "after [[visible-b]]",
        );
        let links = extract_wiki_links(content);
        assert_eq!(
            links
                .iter()
                .map(|link| link.target.as_str())
                .collect::<Vec<_>>(),
            vec!["visible-a", "visible-b"]
        );
        assert_eq!(links[1].position, content.find("[[visible-b]]").unwrap());
    }

    #[test]
    fn wiki_link_crossing_a_fence_is_rejected() {
        let content = "[[broken\n```text\nhidden\n```\n]] then [[visible]]";
        let links = extract_wiki_links(content);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "visible");
        assert_eq!(links[0].position, content.find("[[visible]]").unwrap());
    }
}
