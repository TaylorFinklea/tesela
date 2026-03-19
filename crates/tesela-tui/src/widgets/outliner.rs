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

/// Format blocks into display lines with tree-drawing characters.
pub fn render_lines(blocks: &[OutlinerBlock]) -> Vec<String> {
    blocks
        .iter()
        .enumerate()
        .map(|(i, block)| {
            if block.indent == 0 {
                block.content.clone()
            } else {
                let indent_str = "  ".repeat(block.indent.saturating_sub(1));
                let is_last = blocks
                    .get(i + 1)
                    .map(|next| next.indent < block.indent)
                    .unwrap_or(true);
                let tree_char = if is_last { "└─ " } else { "├─ " };
                format!("{}{}{}", indent_str, tree_char, block.content)
            }
        })
        .collect()
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
        assert_eq!(lines[0], "Top");
        assert!(lines[1].contains("├─"), "expected ├─ in: {}", lines[1]);
        assert!(lines[2].contains("└─"), "expected └─ in: {}", lines[2]);
    }

    #[test]
    fn test_empty_body() {
        let blocks = parse_blocks("");
        // Empty string produces no lines
        assert!(blocks.is_empty());
    }
}
