//! Tag-system Phase 13 — corpus rewrite helpers.
//!
//! Pure functions used by the server's `:rename-slug` and `:delete-tag`
//! verbs to walk markdown bodies and rewrite references. Each helper
//! returns `(new_text, count)` so the caller can surface a preview ("N
//! references rewritten across M notes") without committing.
//!
//! Locked behavior per the tag-system spec product decisions
//! (2026-05-17):
//! - Word-boundary match for `#tag` tokens (alphabet `[A-Za-z0-9_/-]`).
//! - Case-insensitive read, canonical-lowercase write.
//! - Wiki-link rewrite preserves pipe-alias display text:
//!   `[[OldSlug|the bird]]` → `[[new-slug|the bird]]`.
//! - Fenced code blocks (lines between ` ``` ` markers) are SKIPPED so
//!   code samples containing `#cardinal` as a literal aren't molested.
//! - Inline code spans (`` `text` ``) are NOT skipped — they're typically
//!   too short to harbor real references, and skipping them would require
//!   per-character state tracking. If this turns into a real footgun we
//!   add it later.

/// Replace every occurrence of `#<old>` (word-boundary, case-insensitive)
/// with `#<new>` (lowercase). Returns the new body plus the count of
/// substitutions.
///
/// Word-boundary rule: the character at `match.end` must not be a tag-name
/// character (`[A-Za-z0-9_/-]`), and the character at `match.start - 1`
/// is always `#` so the start side is implicit.
///
/// Skips fenced code blocks.
pub fn rewrite_inline_tag(body: &str, old_slug: &str, new_slug: &str) -> (String, usize) {
    rewrite_inline_internal(body, old_slug, Some(new_slug))
}

/// Strip every occurrence of `#<old>` token from the body. Returns the
/// new body plus the count of substitutions. Adjacent whitespace is
/// collapsed: ` #foo ` becomes a single space.
///
/// Used by the delete-tag cleanup path.
pub fn strip_inline_tag(body: &str, old_slug: &str) -> (String, usize) {
    rewrite_inline_internal(body, old_slug, None)
}

fn rewrite_inline_internal(body: &str, old_slug: &str, new_slug: Option<&str>) -> (String, usize) {
    let old_lower = old_slug.to_ascii_lowercase();
    let mut out = String::with_capacity(body.len());
    let mut count = 0usize;

    for segment in iter_code_fence_segments(body) {
        if segment.is_code {
            out.push_str(segment.text);
            continue;
        }
        let (rewritten, n) = rewrite_inline_outside_fence(segment.text, &old_lower, new_slug);
        out.push_str(&rewritten);
        count += n;
    }

    (out, count)
}

fn rewrite_inline_outside_fence(
    text: &str,
    old_lower: &str,
    new_slug: Option<&str>,
) -> (String, usize) {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut count = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] != b'#' {
            // Copy through to the next `#` (or end of segment) as a str
            // slice so multi-byte UTF-8 chars survive intact. `#` is
            // ASCII, so any byte equal to b'#' is a char boundary.
            let run_start = i;
            while i < bytes.len() && bytes[i] != b'#' {
                i += 1;
            }
            out.push_str(&text[run_start..i]);
            continue;
        }

        // Found a `#`. Read the tag-name characters that follow.
        let name_start = i + 1;
        let mut name_end = name_start;
        while name_end < bytes.len() {
            let c = bytes[name_end] as char;
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '/' {
                name_end += 1;
            } else {
                break;
            }
        }

        if name_end == name_start {
            // Just a bare `#` (e.g., the start of a heading). Copy through.
            out.push('#');
            i += 1;
            continue;
        }

        let name = &text[name_start..name_end];
        if !name.eq_ignore_ascii_case(old_lower) {
            // Different tag. Copy through verbatim.
            out.push_str(&text[i..name_end]);
            i = name_end;
            continue;
        }

        // Match. Rewrite or strip.
        match new_slug {
            Some(slug) => {
                out.push('#');
                out.push_str(&slug.to_ascii_lowercase());
            }
            None => {
                // Strip the token + leading whitespace immediately before it
                // (to avoid leaving doubled spaces). Look at what we just
                // wrote: if the last char is whitespace, peek ahead to see
                // if there's also whitespace after, and skip one space
                // through.
                let last = out.chars().last();
                let has_left_ws = matches!(last, Some(' ') | Some('\t'));
                let next_is_ws = name_end < bytes.len()
                    && matches!(bytes[name_end] as char, ' ' | '\t');
                if has_left_ws && next_is_ws {
                    // Drop one of the doubled spaces.
                    i = name_end + 1;
                    count += 1;
                    continue;
                }
            }
        }
        count += 1;
        i = name_end;
    }

    (out, count)
}

/// Rewrite `[[old]]` and `[[old|alias]]` wiki links to `[[new]]` /
/// `[[new|alias]]`. Case-insensitive read on the link target; alias text
/// is preserved verbatim. The new target is always lowercase.
///
/// Skips fenced code blocks.
pub fn rewrite_wiki_link(body: &str, old_slug: &str, new_slug: &str) -> (String, usize) {
    rewrite_wiki_internal(body, old_slug, Some(new_slug))
}

/// Unwrap `[[old]]` and `[[old|alias]]` to plain text. `[[old]]` becomes
/// `old`; `[[old|alias]]` becomes `alias`. Used by the delete-tag cleanup
/// path.
pub fn strip_wiki_link(body: &str, old_slug: &str) -> (String, usize) {
    rewrite_wiki_internal(body, old_slug, None)
}

fn rewrite_wiki_internal(body: &str, old_slug: &str, new_slug: Option<&str>) -> (String, usize) {
    let old_lower = old_slug.to_ascii_lowercase();
    let mut out = String::with_capacity(body.len());
    let mut count = 0usize;

    for segment in iter_code_fence_segments(body) {
        if segment.is_code {
            out.push_str(segment.text);
            continue;
        }
        let (rewritten, n) = rewrite_wiki_outside_fence(segment.text, &old_lower, new_slug);
        out.push_str(&rewritten);
        count += n;
    }

    (out, count)
}

fn rewrite_wiki_outside_fence(
    text: &str,
    old_lower: &str,
    new_slug: Option<&str>,
) -> (String, usize) {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut count = 0usize;
    let mut i = 0usize;

    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            // Find the closing `]]`. If not on the same line, this isn't
            // a wiki link by our convention; copy through.
            let from = i + 2;
            let mut j = from;
            let mut closed = false;
            while j + 1 < bytes.len() {
                if bytes[j] == b']' && bytes[j + 1] == b']' {
                    closed = true;
                    break;
                }
                if bytes[j] == b'\n' {
                    break;
                }
                j += 1;
            }
            if !closed {
                out.push('[');
                i += 1;
                continue;
            }
            let inner = &text[from..j];
            let (target, alias) = match inner.find('|') {
                Some(pipe) => (&inner[..pipe], Some(&inner[pipe + 1..])),
                None => (inner, None),
            };
            if !target.eq_ignore_ascii_case(old_lower) {
                out.push_str(&text[i..j + 2]);
                i = j + 2;
                continue;
            }
            match new_slug {
                Some(slug) => {
                    out.push_str("[[");
                    out.push_str(&slug.to_ascii_lowercase());
                    if let Some(a) = alias {
                        out.push('|');
                        out.push_str(a);
                    }
                    out.push_str("]]");
                }
                None => {
                    // Unwrap: prefer the alias text if present, else the
                    // original target.
                    out.push_str(alias.unwrap_or(target));
                }
            }
            count += 1;
            i = j + 2;
            continue;
        }
        // Copy through to the next `[` (or end of segment) as a str
        // slice so multi-byte UTF-8 chars survive intact. `[` is ASCII,
        // so the scan can only stop on a char boundary; if it stops past
        // the second-to-last byte, fall through to the tail copy below.
        let run_start = i;
        i += 1;
        while i < bytes.len() && bytes[i] != b'[' {
            i += 1;
        }
        if i + 1 < bytes.len() {
            out.push_str(&text[run_start..i]);
        } else {
            out.push_str(&text[run_start..]);
            i = bytes.len();
        }
    }
    if i < bytes.len() {
        out.push_str(&text[i..]);
    }

    (out, count)
}

/// Rewrite a tag page's `parent: <oldslug>` frontmatter entry to a new
/// slug. Matches a YAML scalar line of the form `parent: "<value>"` or
/// `parent: <value>` (with or without quotes). Returns `(new_content,
/// changed)`.
///
/// Only the first `---`-delimited frontmatter block is scanned.
pub fn rewrite_parent_frontmatter(
    content: &str,
    old_slug: &str,
    new_slug: &str,
) -> (String, bool) {
    let Some(fm_end) = find_frontmatter_end(content) else {
        return (content.to_string(), false);
    };
    let head = &content[..fm_end];
    let tail = &content[fm_end..];

    let mut changed = false;
    let new_head = head
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("parent:") {
                return line.to_string();
            }
            let value = trimmed[7..].trim();
            let unquoted = value.trim_matches('"').trim_matches('\'');
            if !unquoted.eq_ignore_ascii_case(old_slug) {
                return line.to_string();
            }
            changed = true;
            let indent_len = line.len() - trimmed.len();
            let indent = &line[..indent_len];
            format!("{}parent: \"{}\"", indent, new_slug.to_ascii_lowercase())
        })
        .collect::<Vec<_>>()
        .join("\n");
    let new_content = if head.ends_with('\n') {
        format!("{}\n{}", new_head, tail)
    } else {
        format!("{}{}", new_head, tail)
    };
    (new_content, changed)
}

/// Clear a tag page's `parent:` frontmatter to an empty string. Used by
/// the delete-tag cleanup path to orphan children. Returns
/// `(new_content, changed)`.
pub fn clear_parent_frontmatter(content: &str, old_slug: &str) -> (String, bool) {
    rewrite_parent_frontmatter(content, old_slug, "")
}

fn find_frontmatter_end(content: &str) -> Option<usize> {
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return None;
    }
    let after_open = content.find('\n')? + 1;
    let rest = &content[after_open..];
    let close_offset = rest.find("\n---\n").or_else(|| rest.find("\n---\r\n"))?;
    Some(after_open + close_offset)
}

// ── code-fence iteration ─────────────────────────────────────────────────

struct CodeFenceSegment<'a> {
    text: &'a str,
    is_code: bool,
}

/// Walk `body`, yielding alternating non-code and fenced-code segments.
/// A code fence is a line that starts with ``` (three backticks) at the
/// start of the line (ignoring leading whitespace).
fn iter_code_fence_segments(body: &str) -> Vec<CodeFenceSegment<'_>> {
    let mut segments = Vec::new();
    let bytes = body.as_bytes();
    let mut i = 0usize;
    let mut segment_start = 0usize;
    let mut in_code = false;

    while i < bytes.len() {
        // Look for `\n   ```` or start-of-doc `\`\`\``.
        let line_start = i;
        // Advance to end of line.
        while i < bytes.len() && bytes[i] != b'\n' {
            i += 1;
        }
        let line_end = i;
        let line = &body[line_start..line_end];
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            // Toggle. Include the fence line on the side we're leaving so
            // both segments contain their delimiter (so the fence char
            // sequence isn't lost when stitching back together).
            let next_seg_start = if i < bytes.len() { i + 1 } else { i };
            let seg = if in_code {
                // Code segment ends after this line's newline.
                CodeFenceSegment {
                    text: &body[segment_start..next_seg_start],
                    is_code: true,
                }
            } else {
                // Non-code segment ends just before this line; the fence
                // line belongs with the code segment.
                CodeFenceSegment {
                    text: &body[segment_start..line_start],
                    is_code: false,
                }
            };
            segments.push(seg);
            segment_start = if in_code {
                next_seg_start
            } else {
                line_start
            };
            in_code = !in_code;
        }
        if i < bytes.len() {
            i += 1; // consume newline
        }
    }
    if segment_start < bytes.len() {
        segments.push(CodeFenceSegment {
            text: &body[segment_start..],
            is_code: in_code,
        });
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_simple_inline_tag() {
        let (out, n) = rewrite_inline_tag("- hello #cardinal world", "cardinal", "cardinal-religion");
        assert_eq!(out, "- hello #cardinal-religion world");
        assert_eq!(n, 1);
    }

    #[test]
    fn rewrite_inline_tag_case_insensitive_read_lowercase_write() {
        let (out, n) = rewrite_inline_tag("a #Cardinal token", "cardinal", "Cardinal-Religion");
        assert_eq!(out, "a #cardinal-religion token");
        assert_eq!(n, 1);
    }

    #[test]
    fn rewrite_inline_tag_word_boundary_no_partial_match() {
        // `#cardinals` should NOT match `cardinal`.
        let (out, n) = rewrite_inline_tag("a #cardinals plural", "cardinal", "x");
        assert_eq!(out, "a #cardinals plural");
        assert_eq!(n, 0);
    }

    #[test]
    fn rewrite_inline_tag_skips_code_fences() {
        let body = "before #cardinal\n```\ncode #cardinal here\n```\nafter #cardinal";
        let (out, n) = rewrite_inline_tag(body, "cardinal", "bird");
        assert_eq!(
            out,
            "before #bird\n```\ncode #cardinal here\n```\nafter #bird"
        );
        assert_eq!(n, 2);
    }

    #[test]
    fn strip_inline_tag_removes_token() {
        let (out, n) = strip_inline_tag("- task #cardinal #other", "cardinal");
        assert_eq!(out, "- task #other");
        assert_eq!(n, 1);
    }

    #[test]
    fn rewrite_wiki_link_with_alias_preserves_alias() {
        let (out, n) = rewrite_wiki_link("see [[Cardinal|the bird]] flying", "cardinal", "Cardinal-Religion");
        assert_eq!(out, "see [[cardinal-religion|the bird]] flying");
        assert_eq!(n, 1);
    }

    #[test]
    fn rewrite_wiki_link_without_alias() {
        let (out, n) = rewrite_wiki_link("see [[cardinal]] flying", "cardinal", "bird");
        assert_eq!(out, "see [[bird]] flying");
        assert_eq!(n, 1);
    }

    #[test]
    fn rewrite_wiki_link_skips_non_matching() {
        let (out, n) = rewrite_wiki_link("see [[other]] flying", "cardinal", "bird");
        assert_eq!(out, "see [[other]] flying");
        assert_eq!(n, 0);
    }

    #[test]
    fn strip_wiki_link_unwraps_to_alias_then_target() {
        let (out, _) = strip_wiki_link("see [[cardinal|the bird]] and [[cardinal]]", "cardinal");
        assert_eq!(out, "see the bird and cardinal");
    }

    #[test]
    fn rewrite_parent_frontmatter_simple() {
        let content = "---\ntype: tag\nparent: \"cardinal\"\ntags: []\n---\n- body";
        let (out, changed) = rewrite_parent_frontmatter(content, "cardinal", "cardinal-religion");
        assert!(changed);
        assert!(out.contains("parent: \"cardinal-religion\""));
        assert!(out.contains("- body"));
    }

    #[test]
    fn rewrite_parent_frontmatter_unquoted_value() {
        let content = "---\nparent: cardinal\n---\n";
        let (out, changed) = rewrite_parent_frontmatter(content, "cardinal", "religion");
        assert!(changed);
        assert!(out.contains("parent: \"religion\""));
    }

    #[test]
    fn rewrite_parent_frontmatter_no_match() {
        let content = "---\nparent: other\n---\n";
        let (_, changed) = rewrite_parent_frontmatter(content, "cardinal", "religion");
        assert!(!changed);
    }

    #[test]
    fn clear_parent_frontmatter_empties_value() {
        let content = "---\nparent: cardinal\n---\n";
        let (out, changed) = clear_parent_frontmatter(content, "cardinal");
        assert!(changed);
        assert!(out.contains("parent: \"\""));
    }

    #[test]
    fn rewrite_inline_tag_multiple_occurrences() {
        let (out, n) = rewrite_inline_tag(
            "#cardinal and #cardinal again, also #Cardinal",
            "cardinal",
            "bird",
        );
        assert_eq!(out, "#bird and #bird again, also #bird");
        assert_eq!(n, 3);
    }

    #[test]
    fn rewrite_inline_tag_preserves_non_ascii_text() {
        let body = "café ☕ “naïve” — résumé #cardinal fin 🚀";
        let (out, n) = rewrite_inline_tag(body, "cardinal", "bird");
        assert_eq!(out, "café ☕ “naïve” — résumé #bird fin 🚀");
        assert_eq!(n, 1);
    }

    #[test]
    fn rename_double_pass_preserves_non_ascii_text() {
        // The server's :rename-slug runs the wiki pass over the inline
        // pass's output; non-tag text must survive both passes
        // byte-identical.
        let body = "héllo — “quotes” #cardinal et [[cardinal|l’oiseau]] 🦤";
        let (after_inline, n1) = rewrite_inline_tag(body, "cardinal", "bird");
        let (after_wiki, n2) = rewrite_wiki_link(&after_inline, "cardinal", "bird");
        assert_eq!(after_wiki, "héllo — “quotes” #bird et [[bird|l’oiseau]] 🦤");
        assert_eq!(n1, 1);
        assert_eq!(n2, 1);
    }

    #[test]
    fn delete_path_preserves_non_ascii_text() {
        // The server's :delete-tag runs strip_inline_tag then
        // strip_wiki_link over its output.
        let body = "déjà vu #cardinal — voir [[cardinal|l’alias]] 😀";
        let (after_strip, n1) = strip_inline_tag(body, "cardinal");
        let (after_wiki, n2) = strip_wiki_link(&after_strip, "cardinal");
        assert_eq!(after_wiki, "déjà vu — voir l’alias 😀");
        assert_eq!(n1, 1);
        assert_eq!(n2, 1);
    }

    #[test]
    fn non_matching_body_passes_through_byte_identical() {
        // The server splices the rewritten body back into the note even
        // when the tag never matched (n_total == 0, parent-only change),
        // so pure pass-through must be byte-identical for all four
        // helpers.
        let body = "naïve — “smart quotes” ☕ #other et [[autre|l’été]] 🌟";
        let (out, n) = rewrite_inline_tag(body, "cardinal", "bird");
        assert_eq!(n, 0);
        assert_eq!(out.as_bytes(), body.as_bytes());
        let (out, n) = rewrite_wiki_link(body, "cardinal", "bird");
        assert_eq!(n, 0);
        assert_eq!(out.as_bytes(), body.as_bytes());
        let (out, n) = strip_inline_tag(body, "cardinal");
        assert_eq!(n, 0);
        assert_eq!(out.as_bytes(), body.as_bytes());
        let (out, n) = strip_wiki_link(body, "cardinal");
        assert_eq!(n, 0);
        assert_eq!(out.as_bytes(), body.as_bytes());
    }

    #[test]
    fn rewrite_inline_tag_idempotent_after_apply() {
        let (after_first, n1) = rewrite_inline_tag("- #cardinal", "cardinal", "bird");
        let (after_second, n2) = rewrite_inline_tag(&after_first, "cardinal", "bird");
        assert_eq!(after_first, "- #bird");
        assert_eq!(after_second, "- #bird");
        assert_eq!(n1, 1);
        assert_eq!(n2, 0);
    }
}
