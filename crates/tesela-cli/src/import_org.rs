//! Org-mode importer.
//!
//! Walks one or more `.org` files (or a directory of them, e.g. an
//! org-roam vault) and converts each into a Tesela markdown note.
//! Hand-rolled parser — `orgize` would cover more cases but pulls in
//! a lot of dependencies for the narrow subset Tesela needs.
//!
//! v1 scope (matches the Phase 13.E plan):
//! - Headlines (`*`, `**`, ...) → block hierarchy (one block per
//!   headline; depth = `*` count).
//! - TODO state → `status:: todo|done|canceled|...` plus a `tags:: Task`
//!   continuation line (task surfaces filter on the Task tag). Reads
//!   `#+TODO:` per-file overrides if present, else the standard `TODO`/
//!   `DONE`/`CANCELED` states.
//! - `[#A]`/`[#B]`/`[#C]` priority cookies → `priority:: high|medium|low`.
//! - `DEADLINE: <YYYY-MM-DD ...>` → `deadline:: [[YYYY-MM-DD]]`.
//! - `SCHEDULED: <YYYY-MM-DD ...>` → `scheduled:: [[YYYY-MM-DD]]`.
//! - Repeaters `+1m`, `++1m`, `.+1m` (and other units) → `recurring`
//!   property with the closest Tesela rule. The variant distinction
//!   (`+` vs `++` vs `.+`) is dropped + logged.
//! - `:PROPERTIES:` drawer → inline `key:: value` lines.
//! - `:LOGBOOK:` and other drawers → dropped with a logged warning.
//! - Headline tag list `:tag1:tag2:` → `#tag1 #tag2` inline on the
//!   block title.
//! - `[[file:foo.org][alias]]` → `[[foo|alias]]`. Bare URLs are
//!   kept verbatim.
//! - Babel `#+BEGIN_SRC <lang>` ... `#+END_SRC` → fenced code blocks.
//!   Tangling and `:RESULTS:` blocks are dropped.
//! - Idempotent via `source_org_path` + `source_org_sha` frontmatter.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE_PATH_KEY: &str = "source_org_path";
const SOURCE_SHA_KEY: &str = "source_org_sha";

// Status values must come from the seed Status property's choices
// (backlog/todo/doing/in-review/done/canceled — `status.md` seed in
// tesela-server's ensure_seed pages). Note single-L `canceled`.
const DEFAULT_TODO_STATES: &[(&str, &str)] = &[
    ("TODO", "todo"),
    ("NEXT", "doing"),
    ("WAITING", "backlog"),
    ("LATER", "backlog"),
    ("NOW", "doing"),
    ("DONE", "done"),
    ("CANCELED", "canceled"),
    ("CANCELLED", "canceled"),
];

#[derive(Debug, Default)]
struct ImportStats {
    imported: usize,
    unchanged: usize,
    conflicts: usize,
    warnings: usize,
}

pub async fn run(mosaic: &Path, source: PathBuf, dry_run: bool) -> Result<()> {
    if !source.exists() {
        anyhow::bail!("Org source not found: {}", source.display());
    }
    let notes_dir = mosaic.join("notes");
    fs::create_dir_all(&notes_dir)?;

    let mut stats = ImportStats::default();
    let mut log: Vec<String> = Vec::new();

    let files: Vec<PathBuf> = if source.is_file() {
        vec![source.clone()]
    } else {
        let mut out = Vec::new();
        for entry in walkdir::WalkDir::new(&source) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("walk org: {}", e);
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.path().extension().and_then(|e| e.to_str()) == Some("org") {
                out.push(entry.path().to_path_buf());
            }
        }
        out
    };

    for path in files {
        match import_one(&source, &path, &notes_dir, dry_run, &mut log) {
            Ok(IndexAction::Imported) => stats.imported += 1,
            Ok(IndexAction::Unchanged) => stats.unchanged += 1,
            Ok(IndexAction::Conflict) => stats.conflicts += 1,
            Err(e) => {
                tracing::warn!("Failed to import {}: {}", path.display(), e);
                log.push(format!("[error] {}: {}", path.display(), e));
                stats.warnings += 1;
            }
        }
    }

    if !log.is_empty() && !dry_run {
        let log_path = mosaic.join("_import-skipped.log");
        let mut content = format!(
            "Org import @ {}\nSource: {}\n\n",
            chrono::Local::now().to_rfc3339(),
            source.display()
        );
        content.push_str(&log.join("\n"));
        content.push('\n');
        fs::write(&log_path, content)?;
    }

    println!("Org import complete:");
    println!("  Imported: {}", stats.imported);
    println!("  Unchanged (idempotent): {}", stats.unchanged);
    println!("  Conflicts (skipped): {}", stats.conflicts);
    if stats.warnings > 0 {
        println!("  Warnings: {}", stats.warnings);
    }
    if !log.is_empty() && !dry_run {
        println!("  Log: {}", mosaic.join("_import-skipped.log").display());
    }
    if dry_run {
        println!("  (dry run — no files written)");
    }
    Ok(())
}

enum IndexAction {
    Imported,
    Unchanged,
    Conflict,
}

fn import_one(
    source_root: &Path,
    org_path: &Path,
    notes_dir: &Path,
    dry_run: bool,
    log: &mut Vec<String>,
) -> Result<IndexAction> {
    let raw =
        fs::read_to_string(org_path).with_context(|| format!("read {}", org_path.display()))?;
    let sha = sha256_hex(&raw);

    let rel_str = if source_root.is_file() {
        org_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled.org")
            .to_string()
    } else {
        org_path
            .strip_prefix(source_root)
            .unwrap_or(org_path)
            .to_string_lossy()
            .replace('\\', "/")
    };

    let stem = org_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    let note_id = slugify(stem);
    let target_path = notes_dir.join(format!("{}.md", note_id));

    if target_path.exists() {
        let existing = fs::read_to_string(&target_path).unwrap_or_default();
        if let Some(prev_sha) = extract_frontmatter_value(&existing, SOURCE_SHA_KEY) {
            if prev_sha == sha {
                return Ok(IndexAction::Unchanged);
            }
            log.push(format!(
                "[conflict] {} → {} target exists with different SHA. Skipped.",
                rel_str, note_id
            ));
            return Ok(IndexAction::Conflict);
        }
        log.push(format!(
            "[conflict] {} → {} target exists and was not produced by the org importer. Skipped.",
            rel_str, note_id
        ));
        return Ok(IndexAction::Conflict);
    }

    let converted = convert_org_to_markdown(&raw, log, &rel_str);
    let title = stem.replace('_', " ");
    let mut full = String::new();
    full.push_str("---\n");
    full.push_str(&format!("title: \"{}\"\n", title));
    full.push_str("tags: []\n");
    full.push_str(&format!("{}: \"{}\"\n", SOURCE_PATH_KEY, rel_str));
    full.push_str(&format!("{}: \"{}\"\n", SOURCE_SHA_KEY, sha));
    full.push_str("---\n");
    full.push_str(&converted);

    if !dry_run {
        fs::write(&target_path, &full)
            .with_context(|| format!("write {}", target_path.display()))?;
    }
    Ok(IndexAction::Imported)
}

fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("untitled");
    }
    out
}

fn extract_frontmatter_value(content: &str, key: &str) -> Option<String> {
    let after = if content.starts_with("---\n") {
        4
    } else if content.starts_with("---\r\n") {
        5
    } else {
        return None;
    };
    for line in content[after..].lines() {
        if line.trim_end_matches('\r') == "---" {
            return None;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.trim() == key {
                return Some(v.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

/// Walk the org file, converting each headline + its drawer + body
/// into Tesela's `- ` block syntax with `key:: value` properties.
fn convert_org_to_markdown(raw: &str, log: &mut Vec<String>, rel_str: &str) -> String {
    let todo_states = parse_todo_keywords(raw);
    let mut out = String::new();
    let lines: Vec<&str> = raw.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(level) = headline_level(line) {
            // Convert headline → block.
            let parsed = parse_headline(line, level, &todo_states);
            let indent = "  ".repeat(level.saturating_sub(1));
            let prop_indent = format!("{}  ", indent);

            let title_with_tags = if parsed.tags.is_empty() {
                parsed.title.clone()
            } else {
                let tag_str = parsed
                    .tags
                    .iter()
                    .map(|t| format!("#{}", t))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("{} {}", parsed.title, tag_str)
            };
            out.push_str(&format!("{}- {}\n", indent, title_with_tags));
            if let Some(status) = &parsed.status {
                out.push_str(&format!("{}status:: {}\n", prop_indent, status));
            }
            if let Some(priority) = &parsed.priority {
                out.push_str(&format!("{}priority:: {}\n", prop_indent, priority));
            }
            // A TODO-state headline is a real Tesela task — it needs a
            // `tags:: Task` continuation line (the materialized form the
            // engine emits; task surfaces filter on the Task tag, not
            // bare `status::`). Held pending so a `tags` key in the
            // :PROPERTIES: drawer unions instead of duplicating; flushed
            // when the block's property region ends. Skipped when the
            // headline's own org tags already include `task` (rendered
            // as an inline #task above).
            let mut task_tag_pending = parsed.status.is_some()
                && !parsed.tags.iter().any(|t| t.eq_ignore_ascii_case("task"));

            i += 1;
            // Now consume planning lines + drawers + body until the
            // next headline (or EOF).
            while i < lines.len() && headline_level(lines[i]).is_none() {
                let l = lines[i];
                let trimmed = l.trim_start();
                if trimmed.is_empty() {
                    i += 1;
                    continue;
                }
                if let Some((kind, date, repeater)) = parse_planning(trimmed) {
                    let key = match kind {
                        PlanningKind::Deadline => "deadline",
                        PlanningKind::Scheduled => "scheduled",
                        PlanningKind::Closed => "closed",
                    };
                    out.push_str(&format!("{}{}:: [[{}]]\n", prop_indent, key, date));
                    if let Some(rec) = repeater {
                        out.push_str(&format!("{}recurring:: {}\n", prop_indent, rec));
                    }
                    i += 1;
                    continue;
                }
                if trimmed.eq_ignore_ascii_case(":PROPERTIES:") {
                    i += 1;
                    while i < lines.len() {
                        let pl = lines[i].trim_start();
                        if pl.eq_ignore_ascii_case(":END:") {
                            i += 1;
                            break;
                        }
                        if let Some((k, v)) = parse_property_drawer_line(pl) {
                            // Org prefixes property keys with `:`, the
                            // value is space-separated. Lowercase the
                            // key so it matches Tesela conventions.
                            let key = k.to_lowercase();
                            // Skip `id` (org-roam) since Tesela has no
                            // place for it; log so the user knows.
                            if key == "id" {
                                log.push(format!(
                                    "[note] {} :ID: dropped (Tesela has no node-id property yet)",
                                    rel_str
                                ));
                                i += 1;
                                continue;
                            }
                            if key == "tags" && task_tag_pending {
                                task_tag_pending = false;
                                if !v.split(',').any(|t| t.trim().eq_ignore_ascii_case("task")) {
                                    out.push_str(&format!(
                                        "{}tags:: {}, Task\n",
                                        prop_indent, v
                                    ));
                                    i += 1;
                                    continue;
                                }
                            }
                            out.push_str(&format!("{}{}:: {}\n", prop_indent, key, v));
                        }
                        i += 1;
                    }
                    continue;
                }
                if trimmed.starts_with(':') && trimmed.ends_with(':') {
                    // Some other drawer (LOGBOOK, RESULTS, etc.) —
                    // skip until :END:.
                    let drawer_name = trimmed.trim_matches(':');
                    log.push(format!(
                        "[skip] {} :{}: drawer dropped",
                        rel_str, drawer_name
                    ));
                    i += 1;
                    while i < lines.len() {
                        if lines[i].trim().eq_ignore_ascii_case(":END:") {
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
                    continue;
                }
                if trimmed.starts_with("#+BEGIN_SRC") {
                    if task_tag_pending {
                        out.push_str(&format!("{}tags:: Task\n", prop_indent));
                        task_tag_pending = false;
                    }
                    let lang = trimmed
                        .split(char::is_whitespace)
                        .nth(1)
                        .unwrap_or("")
                        .to_string();
                    out.push_str(&format!("{}```{}\n", prop_indent, lang));
                    i += 1;
                    while i < lines.len() {
                        let bl = lines[i].trim_start();
                        if bl.eq_ignore_ascii_case("#+END_SRC") {
                            i += 1;
                            break;
                        }
                        out.push_str(&format!("{}{}\n", prop_indent, lines[i]));
                        i += 1;
                    }
                    out.push_str(&format!("{}```\n", prop_indent));
                    continue;
                }
                if trimmed.starts_with("#+") {
                    // Other org meta-lines (#+TITLE, #+OPTIONS, etc.) — drop.
                    i += 1;
                    continue;
                }
                // Plain body line — render under the block.
                if task_tag_pending {
                    out.push_str(&format!("{}tags:: Task\n", prop_indent));
                    task_tag_pending = false;
                }
                let rewritten = rewrite_org_links(l);
                out.push_str(&format!("{}{}\n", prop_indent, rewritten.trim_end()));
                i += 1;
            }
            if task_tag_pending {
                out.push_str(&format!("{}tags:: Task\n", prop_indent));
            }
            continue;
        }

        // Non-headline content before any headline (file preamble).
        // Drop `#+TITLE:` etc.; pass through the rest.
        let trimmed = line.trim_start();
        if trimmed.starts_with("#+") {
            i += 1;
            continue;
        }
        if !trimmed.is_empty() {
            out.push_str(&rewrite_org_links(line));
            out.push('\n');
        }
        i += 1;
    }
    out
}

fn headline_level(line: &str) -> Option<usize> {
    let mut count = 0;
    for c in line.chars() {
        if c == '*' {
            count += 1;
        } else if c == ' ' && count > 0 {
            return Some(count);
        } else {
            return None;
        }
    }
    None
}

#[derive(Debug)]
struct ParsedHeadline {
    title: String,
    status: Option<String>,
    priority: Option<String>,
    tags: Vec<String>,
}

fn parse_headline(line: &str, level: usize, todo_states: &[(String, String)]) -> ParsedHeadline {
    let after_stars = &line[level..].trim_start();
    let mut rest = after_stars.to_string();

    // Tags `:tag1:tag2:` at end of line.
    let tags = if let Some(start) = rest.rfind(":  ").map(|p| p + 1).or_else(|| {
        rest.find(' ').and_then(|first_space| {
            // Tags must be the last token, separated by whitespace,
            // and bracketed by `:`.
            let last_token_start = rest[first_space..]
                .rfind(' ')
                .map(|p| first_space + p + 1)
                .unwrap_or(first_space);
            let last = &rest[last_token_start..];
            if last.starts_with(':') && last.ends_with(':') && last.len() > 2 {
                Some(last_token_start)
            } else {
                None
            }
        })
    }) {
        let tag_str = rest[start..].trim().trim_matches(':');
        let parsed: Vec<String> = tag_str
            .split(':')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        rest = rest[..start].trim().to_string();
        parsed
    } else {
        Vec::new()
    };

    // TODO state.
    let mut status = None;
    for (kw, mapped) in todo_states {
        if rest.starts_with(&format!("{} ", kw)) {
            status = Some(mapped.clone());
            rest = rest[kw.len() + 1..].to_string();
            break;
        } else if rest == *kw {
            status = Some(mapped.clone());
            rest.clear();
            break;
        }
    }

    // Priority cookie [#A].
    let priority = if rest.starts_with("[#") && rest.len() >= 4 && rest.as_bytes()[3] == b']' {
        let p = rest.as_bytes()[2] as char;
        let mapped = match p {
            'A' => Some("high"),
            'B' => Some("medium"),
            'C' => Some("low"),
            _ => None,
        };
        if mapped.is_some() {
            rest = rest[4..].trim_start().to_string();
        }
        mapped.map(|s| s.to_string())
    } else {
        None
    };

    ParsedHeadline {
        title: rest,
        status,
        priority,
        tags,
    }
}

#[derive(Debug, Clone, Copy)]
enum PlanningKind {
    Deadline,
    Scheduled,
    Closed,
}

/// Parse a `DEADLINE:`/`SCHEDULED:`/`CLOSED:` line. Returns the kind,
/// the date (YYYY-MM-DD), and an optional Tesela-style recurring
/// rule extracted from the org repeater.
fn parse_planning(line: &str) -> Option<(PlanningKind, String, Option<String>)> {
    let (kind, after) = if let Some(rest) = line.strip_prefix("DEADLINE:") {
        (PlanningKind::Deadline, rest)
    } else if let Some(rest) = line.strip_prefix("SCHEDULED:") {
        (PlanningKind::Scheduled, rest)
    } else if let Some(rest) = line.strip_prefix("CLOSED:") {
        (PlanningKind::Closed, rest)
    } else {
        return None;
    };
    let after = after.trim();
    // Org timestamps come in `<...>` or `[...]` brackets.
    let inner = after
        .strip_prefix('<')
        .and_then(|s| s.split('>').next())
        .or_else(|| after.strip_prefix('[').and_then(|s| s.split(']').next()))?;
    // Date is the first 10 chars (YYYY-MM-DD).
    if inner.len() < 10 {
        return None;
    }
    let date = inner[..10].to_string();
    let recurring = inner.split_ascii_whitespace().find_map(|tok| {
        let stripped = tok.trim_start_matches(['+', '.']).trim_start_matches('+');
        if stripped == tok {
            return None;
        }
        let (n_str, unit) = stripped.split_at(stripped.len().saturating_sub(1));
        let n: u32 = n_str.parse().ok()?;
        let rule = match (n, unit) {
            (1, "d") => "daily".to_string(),
            (1, "w") => "weekly".to_string(),
            (1, "m") => "monthly".to_string(),
            (1, "y") => "yearly".to_string(),
            (n, "d") => format!("every {} days", n),
            (n, "w") => format!("every {} weeks", n),
            (n, "m") => format!("every {} months", n),
            (n, "y") => format!("every {} years", n),
            _ => return None,
        };
        Some(rule)
    });
    Some((kind, date, recurring))
}

fn parse_property_drawer_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if !line.starts_with(':') {
        return None;
    }
    let after_colon = &line[1..];
    let (key, rest) = after_colon.split_once(':')?;
    Some((key.trim().to_string(), rest.trim().to_string()))
}

fn parse_todo_keywords(raw: &str) -> Vec<(String, String)> {
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("#+TODO:") {
            // Format: `#+TODO: TODO NEXT WAITING | DONE CANCELED`
            let parts: Vec<&str> = rest.split('|').collect();
            let mut out = Vec::new();
            if let Some(active) = parts.first() {
                for kw in active.split_ascii_whitespace() {
                    out.push((
                        kw.to_string(),
                        if kw.eq_ignore_ascii_case("NEXT") || kw.eq_ignore_ascii_case("DOING") {
                            "doing".to_string()
                        } else if kw.eq_ignore_ascii_case("WAITING") {
                            "backlog".to_string()
                        } else {
                            "todo".to_string()
                        },
                    ));
                }
            }
            if let Some(done) = parts.get(1) {
                for kw in done.split_ascii_whitespace() {
                    out.push((
                        kw.to_string(),
                        if kw.to_ascii_uppercase().contains("CANCEL") {
                            "canceled".to_string()
                        } else {
                            "done".to_string()
                        },
                    ));
                }
            }
            if !out.is_empty() {
                return out;
            }
        }
    }
    DEFAULT_TODO_STATES
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Convert org-mode link syntax to Tesela's. The narrow set we
/// understand: `[[file:foo.org][alias]]` → `[[foo|alias]]`,
/// `[[id:UUID][alias]]` → keep the raw alias (UUID is dropped — it's
/// only meaningful inside the org-roam ecosystem).
fn rewrite_org_links(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < line.len() {
        if bytes[i] == b'[' && i + 1 < line.len() && bytes[i + 1] == b'[' {
            if let Some(end) = line[i + 2..].find("]]") {
                let inner = &line[i + 2..i + 2 + end];
                let mut target = inner;
                let mut alias: Option<&str> = None;
                if let Some(close_inner) = inner.find("][") {
                    target = &inner[..close_inner];
                    alias = Some(&inner[close_inner + 2..]);
                }
                if let Some(t) = target.strip_prefix("file:") {
                    let bare = t.trim_end_matches(".org");
                    out.push_str("[[");
                    out.push_str(bare);
                    if let Some(a) = alias {
                        out.push('|');
                        out.push_str(a);
                    }
                    out.push_str("]]");
                } else if target.strip_prefix("id:").is_some() {
                    if let Some(a) = alias {
                        out.push_str("[[");
                        out.push_str(a);
                        out.push_str("]]");
                    } else {
                        // Bare id link with no alias — drop the link
                        // entirely (no useful display). Caller could
                        // do better later.
                    }
                } else {
                    // Some other scheme (https://, mailto:, etc.) —
                    // emit a standard markdown link, since wiki links
                    // don't make sense for external URLs.
                    if let Some(a) = alias {
                        out.push('[');
                        out.push_str(a);
                        out.push(']');
                        out.push('(');
                        out.push_str(target);
                        out.push(')');
                    } else {
                        out.push('<');
                        out.push_str(target);
                        out.push('>');
                    }
                }
                i = i + 2 + end + 2;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_fixture(dir: &Path) {
        fs::write(
            dir.join("project.org"),
            r#"#+TITLE: Project
#+TODO: TODO NEXT WAITING | DONE CANCELED

* TODO Pay rent                                         :finance:
DEADLINE: <2026-05-15 Fri +1m>
:PROPERTIES:
:ID:       abc-uuid
:OWNER:    me
:END:

Body of pay rent task.

** DONE Get receipt                                      :finance:
SCHEDULED: <2026-05-10 Mon>

Sub-body.

* NEXT Plan vacation                                    :personal:family:
:LOGBOOK:
- State change to NEXT
:END:

Plan content.

* DONE [#A] Eat lunch
"#,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn imports_org_file_with_planning_and_drawers() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let mosaic = temp.path().join("mosaic");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(mosaic.join("notes")).unwrap();
        write_fixture(&src);

        run(&mosaic, src.clone(), false).await.unwrap();

        let body = fs::read_to_string(mosaic.join("notes/project.md")).unwrap();
        assert!(body.contains("- Pay rent #finance"));
        assert!(body.contains("status:: todo"));
        // Every TODO-state headline is tagged Task (4 in this fixture),
        // at the block's own property indent.
        assert_eq!(body.matches("tags:: Task").count(), 4, "{body}");
        assert!(body.contains("\n  tags:: Task"), "level-1 indent\n{body}");
        assert!(body.contains("\n    tags:: Task"), "level-2 indent\n{body}");
        assert!(body.contains("deadline:: [[2026-05-15]]"));
        assert!(body.contains("recurring:: monthly"));
        assert!(body.contains("owner:: me"));
        // ID drawer key was logged + dropped, not emitted.
        assert!(!body.contains("id::"));
        // Sub-headline rendered with double-indent.
        assert!(body.contains("  - Get receipt #finance"));
        assert!(body.contains("    status:: done"));
        assert!(body.contains("    scheduled:: [[2026-05-10]]"));
        // Logbook drawer dropped.
        assert!(!body.contains("LOGBOOK"));
        assert!(!body.contains("State change"));
        // Custom NEXT keyword mapped to doing.
        assert!(body.contains("- Plan vacation"));
        assert!(body.contains("status:: doing"));
        // Multi-tag headline → multiple #tags
        assert!(body.contains("#personal"));
        assert!(body.contains("#family"));
        // Priority cookie
        assert!(body.contains("- Eat lunch"));
        assert!(body.contains("priority:: high"));
        // Source tracking
        assert!(body.contains(SOURCE_PATH_KEY));
        assert!(body.contains(SOURCE_SHA_KEY));

        let log = fs::read_to_string(mosaic.join("_import-skipped.log")).unwrap();
        assert!(log.contains(":ID:"));
        assert!(log.contains("LOGBOOK"));
    }

    #[tokio::test]
    async fn re_import_is_idempotent() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let mosaic = temp.path().join("mosaic");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(mosaic.join("notes")).unwrap();
        write_fixture(&src);

        run(&mosaic, src.clone(), false).await.unwrap();
        let mtime1 = fs::metadata(mosaic.join("notes/project.md"))
            .unwrap()
            .modified()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        run(&mosaic, src.clone(), false).await.unwrap();
        let mtime2 = fs::metadata(mosaic.join("notes/project.md"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(mtime1, mtime2);
    }

    fn convert(raw: &str) -> String {
        let mut log = Vec::new();
        convert_org_to_markdown(raw, &mut log, "test.org")
    }

    // ── Task tag — converted TODO states must produce REAL Tesela tasks
    // (task surfaces filter on the Task tag, not bare `status::`). ──

    #[test]
    fn task_headline_gets_task_tag() {
        let out = convert("* TODO Buy milk\n");
        assert!(out.contains("status:: todo"), "{out}");
        assert_eq!(out.matches("tags:: Task").count(), 1, "{out}");
    }

    #[test]
    fn canceled_maps_to_seed_spelling() {
        // Canonical status choices use single-L `canceled`
        // (status.md seed in crates/tesela-server/src/main.rs).
        let out = convert("* CANCELED Drop the plan\n");
        assert!(out.contains("status:: canceled"), "{out}");
        assert!(!out.contains("cancelled"), "{out}");
        let custom = convert("#+TODO: TODO | DONE CANCELLED\n* CANCELLED Old idea\n");
        assert!(custom.contains("status:: canceled"), "{custom}");
    }

    #[test]
    fn task_tag_unions_with_drawer_tags() {
        let out = convert("* TODO Chore\n:PROPERTIES:\n:TAGS: home\n:END:\n");
        assert_eq!(out.matches("tags::").count(), 1, "{out}");
        assert!(out.contains("tags:: home, Task"), "{out}");
    }

    #[test]
    fn headline_task_tag_not_duplicated() {
        let out = convert("* TODO Foo :task:\n");
        assert!(out.contains("#task"), "{out}");
        assert!(!out.contains("tags:: Task"), "{out}");
    }

    #[test]
    fn non_task_headline_gets_no_task_tag() {
        let out = convert("* Just a heading\nBody.\n");
        assert!(!out.contains("tags:: Task"), "{out}");
    }

    #[test]
    fn repeater_parses_known_units() {
        let (_, _, rec) = parse_planning("DEADLINE: <2026-05-15 Fri +1m>").unwrap();
        assert_eq!(rec.as_deref(), Some("monthly"));
        let (_, _, rec) = parse_planning("DEADLINE: <2026-05-15 Fri +2w>").unwrap();
        assert_eq!(rec.as_deref(), Some("every 2 weeks"));
        let (_, _, rec) = parse_planning("DEADLINE: <2026-05-15 Fri ++3d>").unwrap();
        assert_eq!(rec.as_deref(), Some("every 3 days"));
        let (_, _, rec) = parse_planning("DEADLINE: <2026-05-15 Fri>").unwrap();
        assert_eq!(rec, None);
    }

    #[test]
    fn link_rewrite_handles_file_and_id() {
        assert_eq!(
            rewrite_org_links("See [[file:other.org][Other]]"),
            "See [[other|Other]]"
        );
        assert_eq!(
            rewrite_org_links("From [[id:abc-uuid][Roam Page]]"),
            "From [[Roam Page]]"
        );
        assert_eq!(
            rewrite_org_links("Visit [[https://example.com][site]]"),
            "Visit [site](https://example.com)"
        );
    }
}
