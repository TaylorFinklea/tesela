//! Block-lifecycle side effects, extracted from `tesela-server`'s
//! `routes/notes.rs` (tesela-ows.1, step 1). Pure functions operating on
//! parsed markdown content — no I/O, no `AppState`. Today only the server's
//! HTTP handlers call these (behavior-neutral extraction); step 2 wires
//! them into the engine-side apply path so every writer (WS, relay, FFI)
//! inherits them (see `docs/ai` / bead tesela-ows.1 for the step-2 hook-point
//! proposal — not implemented here).
//!
//! Covers three side effects that previously lived inline in the HTTP PUT
//! handler:
//!   - recurrence bump (`try_bump_block` / `try_skip_block`)
//!   - same-note dependency-cycle unblock (`apply_dependency_cycles`)
//!   - tag auto-create's pure tag-collection + template pieces
//!     (`collect_note_tags` / `tag_page_content`) — the store I/O
//!     (slug resolution, note creation) stays server-side.

use crate::block::{parse_blocks, ParsedBlock};
use crate::note::Note;
use crate::recurrence::{self, Recurrence};
use crate::storage::markdown::parse_frontmatter;

/// Pure helper. Returns `Some((new_content, next_deadline_iso))` if `block_id`
/// resolves to a block in `content` with `status:: done` + valid `recurring::`
/// + valid anchor date (`deadline::` or `scheduled::`).
///
/// Behaviour (Task 6 semantics):
/// - Reads `recurrence_done::` (default 0) and calls `recurrence::advance` to
///   check whether the series still has occurrences.
/// - **Series active** (`advance` returns `Some`): advance every date field
///   (`deadline::`, `scheduled::`) by one step each from their own current
///   values; stamp `recurrence_done:: <done+1>`; reset `status:: todo`;
///   stamp `last_completed::`.
/// - **Series spent** (`advance` returns `None`): leave `status:: done`;
///   leave date fields unchanged; set `recurrence_done:: <done+1>`.
///   The `recurring::` property is NOT removed.
///
/// Returns `None` for any reason a bump cannot apply (idempotent, caller
/// just leaves content unchanged).
pub fn try_bump_block(content: &str, block_id: &str) -> Option<(String, String)> {
    let (note_id_str, line_str) = block_id.rsplit_once(':')?;
    let line_num: usize = line_str.parse().ok()?;
    let (_meta, body) = parse_frontmatter(content).ok()?;
    let blocks = parse_blocks(note_id_str, &body);
    let block = blocks.iter().find(|b| b.id == block_id)?;

    if block.properties.get("status").map(|s| s.as_str()) != Some("done") {
        return None;
    }

    let step = compute_recurrence_step(block)?;
    let last_completed_str = format!("[[{}]]", step.anchor_date.format("%Y-%m-%d"));

    match step.active {
        Some(ActiveStep {
            new_deadline,
            new_scheduled,
            next_iso,
        }) => {
            // Series still active — advance every date field from its own value.
            let new_body = rewrite_block_for_complete(
                &body,
                line_num,
                new_deadline.as_deref(),
                new_scheduled.as_deref(),
                &last_completed_str,
                step.new_done,
            )?;
            let new_content = reassemble_content(content, &body, &new_body);
            Some((new_content, next_iso))
        }
        None => {
            // Series spent — leave dates, leave status done, only bump counter.
            let new_body = rewrite_block_for_spent(&body, line_num, step.new_done)?;
            let new_content = reassemble_content(content, &body, &new_body);
            // Return a sentinel ISO so the endpoint can report *something*;
            // the `bumped: true` flag is still meaningful (counter updated).
            let iso = step.anchor_date.format("%Y-%m-%d").to_string();
            Some((new_content, iso))
        }
    }
}

/// Like `try_bump_block` but for `mode: skip`. Advances date fields and
/// increments `recurrence_done::` without touching `status::` or stamping
/// `last_completed::`. Requires `recurring::` to be present and parseable
/// but does NOT require `status:: done` — the block may be in any state.
pub fn try_skip_block(content: &str, block_id: &str) -> Option<(String, String)> {
    let (note_id_str, line_str) = block_id.rsplit_once(':')?;
    let line_num: usize = line_str.parse().ok()?;
    let (_meta, body) = parse_frontmatter(content).ok()?;
    let blocks = parse_blocks(note_id_str, &body);
    let block = blocks.iter().find(|b| b.id == block_id)?;

    let step = compute_recurrence_step(block)?;

    match step.active {
        Some(ActiveStep {
            new_deadline,
            new_scheduled,
            next_iso,
        }) => {
            let new_body = rewrite_block_for_skip(
                &body,
                line_num,
                new_deadline.as_deref(),
                new_scheduled.as_deref(),
                step.new_done,
            )?;
            let new_content = reassemble_content(content, &body, &new_body);
            Some((new_content, next_iso))
        }
        None => {
            // Series spent — only bump the counter, leave everything else.
            let new_body = rewrite_block_for_spent(&body, line_num, step.new_done)?;
            let new_content = reassemble_content(content, &body, &new_body);
            let iso = step.anchor_date.format("%Y-%m-%d").to_string();
            Some((new_content, iso))
        }
    }
}

/// Detect any blocks whose `status::` flipped from non-done in `prev` to
/// `done` in `next`, and apply recurrence bumps to all of them. Returns
/// the (possibly identical) content to persist.
///
/// Done in a loop: each bump re-parses, so subsequent bumps in the same
/// PUT see fresh line numbers. Bumps the same block at most once per call
/// (after a bump, that block's status is `todo`, so it no longer matches).
/// Detect any block whose status flipped to `done` in this PUT and bump
/// its deadline before saving. Returns (rewritten_content, bumps) so the
/// caller can fire `WsEvent::RecurringRolled` for each. `note_id` is used
/// to rewrite block ids in the returned `BumpInfo`s to `<note_id>:<line>`.
pub fn apply_post_save_bumps_with_info(
    prev: &str,
    next: &str,
    note_id: &str,
) -> (String, Vec<BumpInfo>) {
    let flipped = detect_status_flips_to_done(prev, next);
    let mut content = next.to_string();
    let mut bumps = Vec::new();
    for block_id in flipped {
        // try_bump_block uses the note-id prefix from `block_id` and parses
        // body blocks against that prefix. Our block_id here came from a
        // `__diff__` parse, so try_bump_block will still find a match
        // because it re-parses with the same prefix.
        if let Some((bumped, next_iso)) = try_bump_block(&content, &block_id) {
            // Resolve the bumped block's title from the freshly-parsed
            // content. Re-parse to get the title — the line number may
            // have changed if `last_completed::` was inserted.
            let title = title_for_block(&bumped, &block_id).unwrap_or_default();
            content = bumped;
            // Rewrite the block id from `__diff__:N` to `<note_id>:N`
            // so the WS event carries a useful pointer.
            let line = block_id.rsplit_once(':').map(|(_, l)| l).unwrap_or("0");
            let real_block_id = format!("{}:{}", note_id, line);
            bumps.push(BumpInfo {
                block_id: real_block_id,
                title,
                next_deadline: next_iso,
            });
        }
    }
    (content, bumps)
}

#[derive(Debug, Clone)]
pub struct BumpInfo {
    pub block_id: String,
    pub title: String,
    pub next_deadline: String,
}

fn title_for_block(content: &str, block_id: &str) -> Option<String> {
    let (note_id_str, _) = block_id.rsplit_once(':')?;
    let (_meta, body) = parse_frontmatter(content).ok()?;
    let blocks = parse_blocks(note_id_str, &body);
    let block = blocks.iter().find(|b| b.id == block_id)?;
    Some(
        block
            .text
            .split_whitespace()
            .filter(|tok| !tok.starts_with('#'))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// Block ids whose `status` was missing/non-done in `prev` and is `done`
/// in `next`. Lossless against block re-numbering: line numbers can shift
/// across edits, so a block's id may differ between snapshots. We match
/// blocks by `(text, raw_text_first_line)` rather than just id.
fn detect_status_flips_to_done(prev: &str, next: &str) -> Vec<String> {
    fn parse_body_blocks(content: &str) -> Vec<ParsedBlock> {
        match parse_frontmatter(content) {
            Ok((_, body)) => {
                // The note id we pass here only forms ParsedBlock.id; the
                // bumper re-parses with the same string so consistency is
                // self-contained as long as we use the same placeholder
                // both times.
                parse_blocks("__diff__", &body)
            }
            Err(_) => Vec::new(),
        }
    }
    let prev_blocks = parse_body_blocks(prev);
    let next_blocks = parse_body_blocks(next);

    let mut flipped = Vec::new();
    for nb in &next_blocks {
        if nb.properties.get("status").map(|s| s.as_str()) != Some("done") {
            continue;
        }
        let was_done = prev_blocks
            .iter()
            .find(|pb| pb.text == nb.text && pb.raw_text == nb.raw_text)
            .and_then(|pb| pb.properties.get("status"))
            .map(|s| s.as_str())
            == Some("done");
        if !was_done {
            // Resolve the block id by parsing `next` against the real
            // note id so the bumper finds the right block.
            // Reconstruct: the placeholder __diff__ in id is specific to
            // this diff pass; the caller's `update_note` will re-parse
            // with the actual note id when calling try_bump_block.
            // Pass back the *original* next-side block.id for now —
            // try_bump_block re-parses using the prefix from the id.
            flipped.push(nb.id.clone());
        }
    }
    flipped
}

/// Phase 12.4 — same-note dependency unblock. After the bumps applied,
/// look for blocks that became unblocked because one of their blockers
/// just flipped to `done` *in this PUT*. If a block's status is `backlog`
/// and no remaining blocker is incomplete, advance it to `todo`.
///
/// Returns the rewritten content + the list of unblocked block ids so
/// the caller can log them. Cross-note dependency walking is deferred —
/// users with cross-note `blocked_by::` will see the unblock take effect
/// the next time the dependent's own note is re-saved (or they manually
/// edit it). v1.1 will add a reverse-index walk for cross-note unblock.
pub fn apply_dependency_cycles(prev: &str, next: &str, note_id: &str) -> (String, Vec<String>) {
    let flipped_to_done = detect_status_flips_to_done(prev, next);
    if flipped_to_done.is_empty() {
        return (next.to_string(), Vec::new());
    }

    // Map __diff__ ids → real note_id ids so the dependency check can match
    // `<note_id>:<line>` references inside `blocked_by::` values verbatim.
    let just_done: std::collections::HashSet<String> = flipped_to_done
        .iter()
        .filter_map(|id| {
            id.rsplit_once(':')
                .map(|(_, l)| format!("{}:{}", note_id, l))
        })
        .collect();

    let (_meta, body) = match parse_frontmatter(next) {
        Ok(b) => b,
        Err(_) => return (next.to_string(), Vec::new()),
    };
    let blocks = parse_blocks(note_id, &body);
    let block_index: std::collections::HashMap<&str, &ParsedBlock> =
        blocks.iter().map(|b| (b.id.as_str(), b)).collect();

    let mut to_unblock: Vec<(String, usize)> = Vec::new();
    for block in &blocks {
        if block.properties.get("status").map(String::as_str) != Some("backlog") {
            continue;
        }
        let Some(blocked_by_raw) = block.properties.get("blocked_by") else {
            continue;
        };
        let refs: Vec<String> = blocked_by_raw
            .split(',')
            .map(|s| {
                s.trim()
                    .trim_start_matches("[[")
                    .trim_end_matches("]]")
                    .to_string()
            })
            .filter(|s| !s.is_empty())
            .collect();
        if refs.is_empty() {
            continue;
        }
        let any_changed = refs.iter().any(|r| just_done.contains(r));
        if !any_changed {
            continue;
        }
        // Recheck: are *all* blockers now done?
        let still_blocked = refs.iter().any(|r| {
            // Same-note ref → look up; missing or non-done → still blocked.
            // External ref (different note id) → conservatively still blocked.
            let target = block_index.get(r.as_str());
            match target {
                Some(t) => t.properties.get("status").map(String::as_str) != Some("done"),
                None => true,
            }
        });
        if !still_blocked {
            let line = block
                .id
                .rsplit_once(':')
                .and_then(|(_, l)| l.parse().ok())
                .unwrap_or(0);
            to_unblock.push((block.id.clone(), line));
        }
    }

    if to_unblock.is_empty() {
        return (next.to_string(), Vec::new());
    }

    // Rewrite each unblocked block's `status:: backlog` → `status:: todo`.
    let mut new_body = body.clone();
    let mut unblocked_ids = Vec::new();
    for (block_id, line) in to_unblock {
        if let Some(rewritten) = set_status_to_todo(&new_body, line) {
            new_body = rewritten;
            unblocked_ids.push(block_id);
        }
    }

    let new_content = reassemble_content(next, &body, &new_body);
    (new_content, unblocked_ids)
}

/// Find the `status::` continuation line under the bullet at `bullet_line`
/// and rewrite it to `status:: todo`. Idempotent on already-todo. Returns
/// `None` when no `status::` line is found within the block's continuation
/// range, which signals the caller to skip rather than silently mis-edit.
fn set_status_to_todo(body: &str, bullet_line: usize) -> Option<String> {
    let lines: Vec<&str> = body.lines().collect();
    if bullet_line >= lines.len() {
        return None;
    }
    let bullet = lines[bullet_line];
    let bullet_indent = bullet.len() - bullet.trim_start().len();
    let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();

    for (i, line) in lines.iter().enumerate().skip(bullet_line + 1) {
        let trim = line.trim_start();
        if trim.is_empty() {
            continue;
        }
        let indent = line.len() - trim.len();
        // End of block: indent <= bullet's, AND the line starts a new bullet.
        if indent <= bullet_indent && (trim.starts_with("- ") || trim == "-") {
            break;
        }
        if let Some(_rest) = trim.strip_prefix("status::") {
            let prefix: String = " ".repeat(indent);
            new_lines[i] = format!("{}status:: todo", prefix);
            // Preserve trailing newline behavior — `lines()` strips them,
            // and `join("\n")` rebuilds.
            return Some(new_lines.join("\n") + if body.ends_with('\n') { "\n" } else { "" });
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Shared recurrence date-step helper
// ---------------------------------------------------------------------------

/// Outcome of stepping a recurring block forward by one occurrence.
///
/// `new_deadline` / `new_scheduled` are `None` when the block had no
/// corresponding date field.  The `next_iso` is the stepped date from
/// whichever field was preferred (deadline > scheduled).
///
/// When the series is exhausted `advance` returns `None`; callers that only
/// need the `spent` path can check `is_active` or match on
/// `RecurrenceStep::active_fields()`.
struct RecurrenceStep {
    /// Parsed recurrence rule (needed by neither caller after this point, but
    /// returned for completeness / future use).
    #[allow(dead_code)]
    rec: Recurrence,
    /// Anchor date used for `recurrence::advance` (deadline, else scheduled).
    anchor_date: chrono::NaiveDate,
    /// `recurrence_done` counter value *before* this occurrence.
    #[allow(dead_code)]
    done_so_far: u32,
    /// `done_so_far + 1` — the value to write back.
    new_done: u32,
    /// `Some(...)` when the series is still active after this step.
    /// Contains the new formatted deadline / scheduled strings and the
    /// ISO date string for the response.
    active: Option<ActiveStep>,
}

struct ActiveStep {
    /// New `deadline::` string (formatted), or `None` if the block had none.
    new_deadline: Option<String>,
    /// New `scheduled::` string (formatted), or `None` if the block had none.
    new_scheduled: Option<String>,
    /// ISO `YYYY-MM-DD` of the stepped date (deadline preferred, else scheduled).
    next_iso: String,
}

/// Compute the shared recurrence step from a parsed block.
///
/// Returns `None` if the block has no parseable `recurring::` property or
/// no parseable anchor date (deadline / scheduled).
fn compute_recurrence_step(block: &ParsedBlock) -> Option<RecurrenceStep> {
    let recurring_str = block.properties.get("recurring")?;
    let rec: Recurrence = recurrence::parse(recurring_str)?;

    // Anchor: prefer deadline::, fall back to scheduled::.
    let anchor_date = {
        let from_deadline = block
            .properties
            .get("deadline")
            .and_then(|v| parse_deadline_value(v))
            .map(|(d, _)| d);
        let from_scheduled = block
            .properties
            .get("scheduled")
            .and_then(|v| parse_deadline_value(v))
            .map(|(d, _)| d);
        from_deadline.or(from_scheduled)?
    };

    let done_so_far: u32 = block
        .properties
        .get("recurrence_done")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);

    let new_done = done_so_far + 1;

    let active = match recurrence::advance(&rec, anchor_date, done_so_far) {
        None => None,
        Some(_) => {
            // Step each date field from its own current value.
            let new_deadline = block.properties.get("deadline").and_then(|v| {
                let (d, t) = parse_deadline_value(v)?;
                let nd = recurrence::next_after(&rec, d);
                Some(format_deadline(nd, t.as_deref()))
            });
            let new_scheduled = block.properties.get("scheduled").and_then(|v| {
                let (d, t) = parse_deadline_value(v)?;
                let nd = recurrence::next_after(&rec, d);
                Some(format_deadline(nd, t.as_deref()))
            });

            // Derive next_iso directly from the stepped NaiveDate that
            // parse_deadline_value already returned — no string round-trip.
            let next_iso = block
                .properties
                .get("deadline")
                .and_then(|v| {
                    let (d, _) = parse_deadline_value(v)?;
                    Some(
                        recurrence::next_after(&rec, d)
                            .format("%Y-%m-%d")
                            .to_string(),
                    )
                })
                .or_else(|| {
                    block.properties.get("scheduled").and_then(|v| {
                        let (d, _) = parse_deadline_value(v)?;
                        Some(
                            recurrence::next_after(&rec, d)
                                .format("%Y-%m-%d")
                                .to_string(),
                        )
                    })
                })?;

            Some(ActiveStep {
                new_deadline,
                new_scheduled,
                next_iso,
            })
        }
    };

    Some(RecurrenceStep {
        rec,
        anchor_date,
        done_so_far,
        new_done,
        active,
    })
}

/// Parse a `deadline::` value into `(date, optional_time_suffix)`. Accepts
/// `[[YYYY-MM-DD]]`, `YYYY-MM-DD`, with an optional trailing `HH:mm` time.
/// The time suffix (e.g. ` 10:30`) is preserved verbatim so the bumped
/// deadline carries the same time-of-day forward.
fn parse_deadline_value(v: &str) -> Option<(chrono::NaiveDate, Option<String>)> {
    let trimmed = v.trim();
    let (date_part, time_part) = match trimmed.find(' ') {
        Some(idx) => (trimmed[..idx].trim(), Some(trimmed[idx..].to_string())),
        None => (trimmed, None),
    };
    let bare = date_part
        .strip_prefix("[[")
        .and_then(|s| s.strip_suffix("]]"))
        .unwrap_or(date_part);
    let mut parts = bare.split('-');
    let y: i32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    let date = chrono::NaiveDate::from_ymd_opt(y, m, d)?;
    Some((date, time_part))
}

/// Build a `[[YYYY-MM-DD]]` value with the same trailing time the original
/// had, so a deadline like `[[2026-05-01]] 10:00` stays timed after the bump.
fn format_deadline(date: chrono::NaiveDate, time_suffix: Option<&str>) -> String {
    let base = format!("[[{}]]", date.format("%Y-%m-%d"));
    match time_suffix {
        Some(t) => format!("{}{}", base, t),
        None => base,
    }
}

// ---------------------------------------------------------------------------
// Block-rewrite helpers (shared by complete / skip / spent paths)
// ---------------------------------------------------------------------------

/// Shared block-boundary scanner. Returns `(lines, end_index, cont_indent)`
/// where `end_index` is the first line after the block's continuation range
/// (exclusive upper bound for in-place mutation).
fn block_range(body: &str, block_line_num: usize) -> Option<(Vec<String>, usize, String)> {
    let lines: Vec<String> = body.lines().map(String::from).collect();
    if block_line_num >= lines.len() {
        return None;
    }
    let block_line = &lines[block_line_num];
    let trim_start = block_line.trim_start();
    if !(trim_start.starts_with("- ") || trim_start.trim_end() == "-") {
        return None;
    }
    let block_indent_spaces = block_line.len() - trim_start.len();

    let mut end = lines.len();
    for (i, l) in lines.iter().enumerate().skip(block_line_num + 1) {
        let t = l.trim_start();
        if t.is_empty() {
            continue;
        }
        let l_indent = l.len() - t.len();
        let is_bullet = t.starts_with("- ") || t.trim_end() == "-";
        if is_bullet && l_indent <= block_indent_spaces {
            end = i;
            break;
        }
    }

    let cont_indent = " ".repeat(block_indent_spaces + 2);
    Some((lines, end, cont_indent))
}

/// Finish a `block_range` mutation: join lines, restore trailing newline.
fn join_lines(lines: Vec<String>, trailing_newline: bool) -> String {
    let mut out = lines.join("\n");
    if trailing_newline {
        out.push('\n');
    }
    out
}

/// `complete` mode: reset `status:: todo`, advance `deadline::` and/or
/// `scheduled::`, stamp `last_completed::`, update/insert `recurrence_done::`.
fn rewrite_block_for_complete(
    body: &str,
    block_line_num: usize,
    new_deadline: Option<&str>,
    new_scheduled: Option<&str>,
    last_completed: &str,
    new_done: u32,
) -> Option<String> {
    let trailing_newline = body.ends_with('\n');
    let (mut lines, end, cont_indent) = block_range(body, block_line_num)?;

    let mut updated_status = false;
    let mut updated_deadline = false;
    let mut updated_scheduled = false;
    let mut updated_last_completed = false;
    let mut updated_recurrence_done = false;

    for line in lines.iter_mut().take(end).skip(block_line_num + 1) {
        if let Some((key, _)) = property_kv(line) {
            match key.as_str() {
                "status" => {
                    *line = format!("{}status:: todo", cont_indent);
                    updated_status = true;
                }
                "deadline" => {
                    if let Some(nd) = new_deadline {
                        *line = format!("{}deadline:: {}", cont_indent, nd);
                        updated_deadline = true;
                    }
                }
                "scheduled" => {
                    if let Some(ns) = new_scheduled {
                        *line = format!("{}scheduled:: {}", cont_indent, ns);
                        updated_scheduled = true;
                    }
                }
                "last_completed" => {
                    *line = format!("{}last_completed:: {}", cont_indent, last_completed);
                    updated_last_completed = true;
                }
                "recurrence_done" => {
                    *line = format!("{}recurrence_done:: {}", cont_indent, new_done);
                    updated_recurrence_done = true;
                }
                _ => {}
            }
        }
    }

    let mut additions: Vec<String> = Vec::new();
    if !updated_status {
        additions.push(format!("{}status:: todo", cont_indent));
    }
    if !updated_deadline {
        if let Some(nd) = new_deadline {
            additions.push(format!("{}deadline:: {}", cont_indent, nd));
        }
    }
    if !updated_scheduled {
        if let Some(ns) = new_scheduled {
            additions.push(format!("{}scheduled:: {}", cont_indent, ns));
        }
    }
    if !updated_last_completed {
        additions.push(format!(
            "{}last_completed:: {}",
            cont_indent, last_completed
        ));
    }
    if !updated_recurrence_done {
        additions.push(format!("{}recurrence_done:: {}", cont_indent, new_done));
    }
    for (offset, add) in additions.into_iter().enumerate() {
        lines.insert(end + offset, add);
    }

    Some(join_lines(lines, trailing_newline))
}

/// `skip` mode: advance `deadline::` and/or `scheduled::`, increment
/// `recurrence_done::`. Does NOT touch `status::` or `last_completed::`.
fn rewrite_block_for_skip(
    body: &str,
    block_line_num: usize,
    new_deadline: Option<&str>,
    new_scheduled: Option<&str>,
    new_done: u32,
) -> Option<String> {
    let trailing_newline = body.ends_with('\n');
    let (mut lines, end, cont_indent) = block_range(body, block_line_num)?;

    let mut updated_deadline = false;
    let mut updated_scheduled = false;
    let mut updated_recurrence_done = false;

    for line in lines.iter_mut().take(end).skip(block_line_num + 1) {
        if let Some((key, _)) = property_kv(line) {
            match key.as_str() {
                "deadline" => {
                    if let Some(nd) = new_deadline {
                        *line = format!("{}deadline:: {}", cont_indent, nd);
                        updated_deadline = true;
                    }
                }
                "scheduled" => {
                    if let Some(ns) = new_scheduled {
                        *line = format!("{}scheduled:: {}", cont_indent, ns);
                        updated_scheduled = true;
                    }
                }
                "recurrence_done" => {
                    *line = format!("{}recurrence_done:: {}", cont_indent, new_done);
                    updated_recurrence_done = true;
                }
                _ => {}
            }
        }
    }

    let mut additions: Vec<String> = Vec::new();
    if !updated_deadline {
        if let Some(nd) = new_deadline {
            additions.push(format!("{}deadline:: {}", cont_indent, nd));
        }
    }
    if !updated_scheduled {
        if let Some(ns) = new_scheduled {
            additions.push(format!("{}scheduled:: {}", cont_indent, ns));
        }
    }
    if !updated_recurrence_done {
        additions.push(format!("{}recurrence_done:: {}", cont_indent, new_done));
    }
    for (offset, add) in additions.into_iter().enumerate() {
        lines.insert(end + offset, add);
    }

    Some(join_lines(lines, trailing_newline))
}

/// `spent` mode: series exhausted — only update `recurrence_done::`. Does not
/// touch dates, `status::`, or `last_completed::`.
fn rewrite_block_for_spent(body: &str, block_line_num: usize, new_done: u32) -> Option<String> {
    let trailing_newline = body.ends_with('\n');
    let (mut lines, end, cont_indent) = block_range(body, block_line_num)?;

    let mut updated_recurrence_done = false;
    for line in lines.iter_mut().take(end).skip(block_line_num + 1) {
        if let Some((key, _)) = property_kv(line) {
            if key == "recurrence_done" {
                *line = format!("{}recurrence_done:: {}", cont_indent, new_done);
                updated_recurrence_done = true;
            }
        }
    }
    if !updated_recurrence_done {
        lines.insert(
            end,
            format!("{}recurrence_done:: {}", cont_indent, new_done),
        );
    }

    Some(join_lines(lines, trailing_newline))
}

/// Match an indented `key:: value` line. Returns `(key, value)` lowercased
/// key plus raw value (trimmed). Only call on continuation lines.
///
/// `pub` (unlike the other block-rewrite internals in this module):
/// `tesela-server`'s `strip_block_intext_prop` also needs this exact parse
/// to strip an in-text property line, so it's a shared leaf helper rather
/// than lifecycle-module-private.
pub fn property_kv(line: &str) -> Option<(String, String)> {
    let trim = line.trim_start();
    let (k, v) = trim.split_once("::")?;
    let key = k.trim();
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let value = v.trim_start_matches([' ', '\t']).trim_end().to_string();
    Some((key.to_lowercase(), value))
}

/// Replace the body portion of `original_content` with `new_body`.
/// Frontmatter (everything before the body in the source content) is
/// preserved verbatim.
fn reassemble_content(original_content: &str, original_body: &str, new_body: &str) -> String {
    if original_content.ends_with(original_body) {
        let prefix_len = original_content.len() - original_body.len();
        let mut out = String::with_capacity(prefix_len + new_body.len());
        out.push_str(&original_content[..prefix_len]);
        out.push_str(new_body);
        return out;
    }
    // Fallback: substring replace. Safe because `body` is unique in
    // `content` for any well-formed note (frontmatter delimiters never
    // appear inside the body).
    original_content.replacen(original_body, new_body, 1)
}

// ---------------------------------------------------------------------------
// Tag auto-create: pure tag-collection + template pieces
// ---------------------------------------------------------------------------

/// Collect every tag a note should auto-create a page for: frontmatter
/// `tags:`, inline `#tag` text in the body, and block-level `tags:: a, b`
/// continuation lines (the chip-tag gesture writes only these — no
/// frontmatter tag, no inline `#tag` — so without scanning them a
/// committed chip tag would never materialize a Tag page). Case-insensitive
/// de-dup against tags already collected.
///
/// Pure: no I/O, no store lookups. The caller (`ensure_tag_pages` in
/// `tesela-server`) does the slug resolution + page creation, and still
/// owns skipping the `daily` tag.
pub fn collect_note_tags(note: &Note) -> Vec<String> {
    let mut all_tags: Vec<String> = note.metadata.tags.clone();

    let tag_re = regex::Regex::new(r"#([A-Za-z][A-Za-z0-9_-]*)").unwrap();
    for cap in tag_re.captures_iter(&note.body) {
        let tag = cap[1].to_string();
        if !all_tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
            all_tags.push(tag);
        }
    }

    let block_tags_re = regex::Regex::new(r"(?m)^\s*tags::\s*(.+)$").unwrap();
    for cap in block_tags_re.captures_iter(&note.body) {
        for raw in cap[1].split(',') {
            let tag = raw.trim();
            if !tag.is_empty() && !all_tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                all_tags.push(tag.to_string());
            }
        }
    }

    all_tags
}

/// Markdown content for a newly auto-created tag page. `type: tag`
/// (lowercase, bare) is the canonical form per the tag-system spec.
pub fn tag_page_content(tag: &str) -> String {
    format!(
        "---\ntitle: \"{}\"\ntype: tag\nextends: \"Root Tag\"\ntag_properties: []\nparent: \"\"\ntags: []\n---\n- Tag properties are inherited by all nodes using the tag.\n",
        tag
    )
}
#[cfg(test)]
mod recurrence_tests {
    use super::*;

    /// Helper: extract a named property from a block identified by block_id
    /// in the given content string.
    fn get_prop(content: &str, block_id: &str, key: &str) -> Option<String> {
        let (note_id_str, _) = block_id.rsplit_once(':')?;
        let (_meta, body) = parse_frontmatter(content).ok()?;
        let blocks = parse_blocks(note_id_str, &body);
        let block = blocks.iter().find(|b| b.id == block_id)?;
        block.properties.get(key).cloned()
    }

    /// Build a synthetic note content string where the task block is on
    /// body line 0 (so block_id is `"note:0"`).
    fn make_note(body_extra_props: &[(&str, &str)]) -> String {
        let mut lines = vec![
            "---".to_string(),
            "title: \"Test\"".to_string(),
            "tags: []".to_string(),
            "---".to_string(),
            "- task".to_string(),
            "  recurring:: daily count 2".to_string(),
            "  deadline:: [[2026-05-07]]".to_string(),
            "  scheduled:: [[2026-05-06]]".to_string(),
            "  status:: todo".to_string(),
        ];
        for (k, v) in body_extra_props {
            lines.push(format!("  {}:: {}", k, v));
        }
        lines.join("\n") + "\n"
    }

    /// Block id: note body starts after the frontmatter (4 header lines),
    /// but `parse_blocks` operates on the *body* slice and assigns
    /// line numbers relative to the body. The bullet `- task` is on body
    /// line 0, so block_id is `"note:0"`.
    const BLOCK_ID: &str = "note:0";

    // -----------------------------------------------------------------------
    // Task 6 core test: multi-field anchor + recurrence_done + spent series
    // -----------------------------------------------------------------------

    #[test]
    fn recurrence_first_done_advances_both_dates_and_stamps_counter() {
        // Start: todo, daily count 2, deadline 2026-05-07, scheduled 2026-05-06
        let content = make_note(&[]);

        // Flip to done (simulate what the client would PUT).
        let content_with_done = content.replace("status:: todo", "status:: done");

        // First complete.
        let (bumped1, next_iso1) =
            try_bump_block(&content_with_done, BLOCK_ID).expect("bump should succeed");

        assert_eq!(next_iso1, "2026-05-08", "deadline next date");
        // deadline advanced from 2026-05-07 → 2026-05-08
        assert_eq!(
            get_prop(&bumped1, BLOCK_ID, "deadline").as_deref(),
            Some("[[2026-05-08]]"),
            "deadline advanced"
        );
        // scheduled advanced from 2026-05-06 → 2026-05-07
        assert_eq!(
            get_prop(&bumped1, BLOCK_ID, "scheduled").as_deref(),
            Some("[[2026-05-07]]"),
            "scheduled advanced"
        );
        // recurrence_done stamped to 1
        assert_eq!(
            get_prop(&bumped1, BLOCK_ID, "recurrence_done").as_deref(),
            Some("1"),
            "recurrence_done = 1"
        );
        // status reset to todo
        assert_eq!(
            get_prop(&bumped1, BLOCK_ID, "status").as_deref(),
            Some("todo"),
            "status reset to todo"
        );
        // last_completed stamped with the prior anchor
        assert_eq!(
            get_prop(&bumped1, BLOCK_ID, "last_completed").as_deref(),
            Some("[[2026-05-07]]"),
            "last_completed = prior anchor"
        );
    }

    #[test]
    fn recurrence_second_done_exhausts_series() {
        // Build content as if the first bump already happened:
        // deadline 2026-05-08, scheduled 2026-05-07, recurrence_done 1, status todo.
        let content_after_first = {
            let base = make_note(&[("recurrence_done", "1")]);
            base.replace("deadline:: [[2026-05-07]]", "deadline:: [[2026-05-08]]")
                .replace("scheduled:: [[2026-05-06]]", "scheduled:: [[2026-05-07]]")
        };
        // Flip to done again.
        let content_with_done2 = content_after_first.replace("status:: todo", "status:: done");

        // Second complete — series is now spent (count 2, done_so_far=1 → advance returns None).
        let (bumped2, _iso) = try_bump_block(&content_with_done2, BLOCK_ID)
            .expect("bump returns Some even when spent");

        // status stays done
        assert_eq!(
            get_prop(&bumped2, BLOCK_ID, "status").as_deref(),
            Some("done"),
            "status stays done when series is spent"
        );
        // deadline unchanged
        assert_eq!(
            get_prop(&bumped2, BLOCK_ID, "deadline").as_deref(),
            Some("[[2026-05-08]]"),
            "deadline unchanged after spent"
        );
        // scheduled unchanged
        assert_eq!(
            get_prop(&bumped2, BLOCK_ID, "scheduled").as_deref(),
            Some("[[2026-05-07]]"),
            "scheduled unchanged after spent"
        );
        // recurrence_done bumped to 2
        assert_eq!(
            get_prop(&bumped2, BLOCK_ID, "recurrence_done").as_deref(),
            Some("2"),
            "recurrence_done = 2 after series is spent"
        );
        // recurring:: property preserved (not stripped)
        assert_eq!(
            get_prop(&bumped2, BLOCK_ID, "recurring").as_deref(),
            Some("daily count 2"),
            "recurring:: property preserved"
        );
    }

    // -----------------------------------------------------------------------
    // skip mode test
    // -----------------------------------------------------------------------

    #[test]
    fn skip_mode_advances_dates_without_touching_status_or_last_completed() {
        // Start in todo state (skip does not require done).
        let content = make_note(&[]);

        let (skipped, next_iso) = try_skip_block(&content, BLOCK_ID).expect("skip should succeed");

        assert_eq!(next_iso, "2026-05-08");
        // dates advanced
        assert_eq!(
            get_prop(&skipped, BLOCK_ID, "deadline").as_deref(),
            Some("[[2026-05-08]]"),
        );
        assert_eq!(
            get_prop(&skipped, BLOCK_ID, "scheduled").as_deref(),
            Some("[[2026-05-07]]"),
        );
        // recurrence_done incremented
        assert_eq!(
            get_prop(&skipped, BLOCK_ID, "recurrence_done").as_deref(),
            Some("1"),
        );
        // status NOT changed — remains todo
        assert_eq!(
            get_prop(&skipped, BLOCK_ID, "status").as_deref(),
            Some("todo"),
            "status must not be modified by skip"
        );
        // last_completed NOT stamped
        assert_eq!(
            get_prop(&skipped, BLOCK_ID, "last_completed"),
            None,
            "last_completed must not be stamped by skip"
        );
    }

    // -----------------------------------------------------------------------
    // Regression: unbounded series never exhausts
    // -----------------------------------------------------------------------

    #[test]
    fn unbounded_series_always_advances() {
        let content = {
            let lines = vec![
                "---\ntitle: \"T\"\ntags: []\n---",
                "- task",
                "  recurring:: daily",
                "  deadline:: [[2026-05-07]]",
                "  status:: done",
            ];
            lines.join("\n") + "\n"
        };

        let (bumped, iso) = try_bump_block(&content, BLOCK_ID).expect("should bump");
        assert_eq!(iso, "2026-05-08");
        assert_eq!(
            get_prop(&bumped, BLOCK_ID, "status").as_deref(),
            Some("todo")
        );
        assert_eq!(
            get_prop(&bumped, BLOCK_ID, "recurrence_done").as_deref(),
            Some("1")
        );
    }
}

#[cfg(test)]
mod dependency_cycle_tests {
    use super::*;

    fn note_content(bodies: &[&str]) -> String {
        let mut lines = vec![
            "---".to_string(),
            "title: \"Test\"".to_string(),
            "tags: []".to_string(),
            "---".to_string(),
        ];
        lines.extend(bodies.iter().map(|s| s.to_string()));
        lines.join("\n") + "\n"
    }

    #[test]
    fn blocker_flip_unblocks_dependent_backlog_block() {
        let prev = note_content(&[
            "- blocker",
            "  status:: todo",
            "- dependent",
            "  status:: backlog",
            "  blocked_by:: [[note:0]]",
        ]);
        let next = note_content(&[
            "- blocker",
            "  status:: done",
            "- dependent",
            "  status:: backlog",
            "  blocked_by:: [[note:0]]",
        ]);

        let (rewritten, unblocked) = apply_dependency_cycles(&prev, &next, "note");
        assert_eq!(unblocked.len(), 1, "exactly one block unblocked");
        assert!(
            rewritten.contains("status:: todo\n  blocked_by:: [[note:0]]")
                || rewritten.contains("status:: todo")
        );
        // The dependent's status line must now read `todo`, not `backlog`.
        let (_meta, body) = parse_frontmatter(&rewritten).unwrap();
        let blocks = parse_blocks("note", &body);
        let dependent = blocks
            .iter()
            .find(|b| b.text.trim() == "dependent")
            .expect("dependent block present");
        assert_eq!(
            dependent.properties.get("status").map(String::as_str),
            Some("todo")
        );
    }

    #[test]
    fn still_blocked_dependent_is_left_alone() {
        let prev = note_content(&[
            "- blocker a",
            "  status:: todo",
            "- blocker b",
            "  status:: todo",
            "- dependent",
            "  status:: backlog",
            "  blocked_by:: [[note:0]], [[note:2]]",
        ]);
        let next = note_content(&[
            "- blocker a",
            "  status:: done",
            "- blocker b",
            "  status:: todo",
            "- dependent",
            "  status:: backlog",
            "  blocked_by:: [[note:0]], [[note:2]]",
        ]);

        let (rewritten, unblocked) = apply_dependency_cycles(&prev, &next, "note");
        assert!(
            unblocked.is_empty(),
            "still-blocked dependent must not unblock"
        );
        assert_eq!(rewritten, next, "content unchanged when nothing unblocks");
    }

    #[test]
    fn no_status_flip_is_a_no_op() {
        let prev = note_content(&["- solo", "  status:: todo"]);
        let next = prev.clone();
        let (rewritten, unblocked) = apply_dependency_cycles(&prev, &next, "note");
        assert!(unblocked.is_empty());
        assert_eq!(rewritten, next);
    }
}

#[cfg(test)]
mod tag_autocreate_pure_tests {
    use super::*;
    use crate::note::{Note, NoteId, NoteMetadata};
    use std::path::PathBuf;

    fn note_with(tags: Vec<String>, body: &str) -> Note {
        Note {
            id: NoteId::new("test"),
            title: "Test".to_string(),
            content: format!("---\ntitle: \"Test\"\n---\n{}", body),
            body: body.to_string(),
            metadata: NoteMetadata {
                tags,
                ..Default::default()
            },
            path: PathBuf::from("test.md"),
            checksum: String::new(),
            created_at: chrono::Utc::now(),
            modified_at: chrono::Utc::now(),
            attachments: Vec::new(),
        }
    }

    #[test]
    fn collects_frontmatter_inline_and_block_tags_deduped() {
        let note = note_with(
            vec!["alpha".to_string()],
            "- a task #beta\n  tags:: gamma, Alpha\n",
        );
        let tags = collect_note_tags(&note);
        // alpha (frontmatter), beta (inline), gamma (block tags:: line);
        // the block-level "Alpha" is a case-insensitive dup of frontmatter
        // "alpha" and must not be added again.
        assert_eq!(tags.len(), 3, "tags: {:?}", tags);
        assert!(tags.iter().any(|t| t.eq_ignore_ascii_case("alpha")));
        assert!(tags.iter().any(|t| t.eq_ignore_ascii_case("beta")));
        assert!(tags.iter().any(|t| t.eq_ignore_ascii_case("gamma")));
    }

    #[test]
    fn no_tags_anywhere_returns_empty() {
        let note = note_with(vec![], "- plain task, nothing special\n");
        assert!(collect_note_tags(&note).is_empty());
    }

    #[test]
    fn tag_page_content_is_well_formed_frontmatter() {
        let content = tag_page_content("fella");
        assert!(content.starts_with("---\ntitle: \"fella\"\n"));
        assert!(content.contains("type: tag"));
        assert!(content.contains("extends: \"Root Tag\""));
    }
}
