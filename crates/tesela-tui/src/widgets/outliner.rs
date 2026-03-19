use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::theme::DEFAULT as T;

/// A parsed block from a note's outliner content.
#[derive(Debug, Clone)]
pub struct OutlinerBlock {
    /// Indentation level (0 = top-level, 1 = one indent, etc.)
    pub indent: usize,
    /// Content of the block after stripping the leading `- `
    pub content: String,
}

/// Parse the body of a note into outliner blocks.
///
/// Lines starting with optional whitespace followed by `- ` are treated as blocks.
/// Indentation level is derived from leading whitespace (2 spaces per level).
/// Non-block lines are included as plain text at indent level 0.
pub fn parse_blocks(body: &str) -> Vec<OutlinerBlock> {
    body.lines()
        .map(|line| {
            let leading = line.len() - line.trim_start().len();
            let trimmed = line.trim_start();
            if let Some(content) = trimmed.strip_prefix("- ") {
                OutlinerBlock {
                    indent: leading / 2,
                    content: content.to_string(),
                }
            } else if trimmed == "-" {
                OutlinerBlock {
                    indent: leading / 2,
                    content: String::new(),
                }
            } else {
                // Non-block content (headings, blank lines, prose)
                OutlinerBlock {
                    indent: 0,
                    content: line.to_string(),
                }
            }
        })
        .collect()
}

/// Format blocks into styled display lines with tree-drawing characters.
pub fn render_lines(blocks: &[OutlinerBlock]) -> Vec<Line<'static>> {
    let tree_style = Style::default().fg(T.tree);
    let tag_style = Style::default().fg(T.tag);

    blocks
        .iter()
        .enumerate()
        .map(|(i, block)| {
            if block.indent == 0 {
                colorize_tags(&block.content, tag_style)
            } else {
                let indent_str = "  ".repeat(block.indent.saturating_sub(1));
                let is_last = blocks
                    .get(i + 1)
                    .map(|next| next.indent < block.indent)
                    .unwrap_or(true);
                let tree_char = if is_last { "└─ " } else { "├─ " };

                let mut spans = vec![
                    Span::raw(indent_str),
                    Span::styled(tree_char.to_string(), tree_style),
                ];

                let content_line = colorize_tags(&block.content, tag_style);
                spans.extend(content_line.spans);

                Line::from(spans)
            }
        })
        .collect()
}

/// Colorize #tags within a line of text.
fn colorize_tags(text: &str, tag_style: Style) -> Line<'static> {
    let parts: Vec<&str> = text.split('#').collect();
    if parts.len() == 1 {
        return Line::from(text.to_string());
    }
    let mut spans = Vec::new();
    if !parts[0].is_empty() {
        spans.push(Span::raw(parts[0].to_string()));
    }
    for part in &parts[1..] {
        let (tag_word, rest) = if let Some(pos) = part.find(|c: char| c.is_whitespace()) {
            (&part[..pos], &part[pos..])
        } else {
            (*part, "")
        };
        spans.push(Span::styled(format!("#{tag_word}"), tag_style));
        if !rest.is_empty() {
            spans.push(Span::raw(rest.to_string()));
        }
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_top_level_blocks() {
        let body = "- First block\n- Second block";
        let blocks = parse_blocks(body);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].indent, 0);
        assert_eq!(blocks[0].content, "First block");
        assert_eq!(blocks[1].indent, 0);
        assert_eq!(blocks[1].content, "Second block");
    }

    #[test]
    fn test_parse_nested_blocks() {
        let body = "- Parent\n  - Child\n    - Grandchild";
        let blocks = parse_blocks(body);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].indent, 0);
        assert_eq!(blocks[1].indent, 1);
        assert_eq!(blocks[2].indent, 2);
    }

    #[test]
    fn test_render_lines_tree_chars() {
        let blocks = vec![
            OutlinerBlock {
                indent: 0,
                content: "Top".to_string(),
            },
            OutlinerBlock {
                indent: 1,
                content: "Child 1".to_string(),
            },
            OutlinerBlock {
                indent: 1,
                content: "Child 2".to_string(),
            },
        ];
        let lines = render_lines(&blocks);
        let line1_str: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        let line2_str: String = lines[2].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(line1_str.contains("├─"), "expected ├─ in: {}", line1_str);
        assert!(line2_str.contains("└─"), "expected └─ in: {}", line2_str);
    }

    #[test]
    fn test_empty_body() {
        let blocks = parse_blocks("");
        assert!(blocks.is_empty());
    }
}
