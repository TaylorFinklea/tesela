use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use tesela_core::{
    block::parse_blocks,
    daily::DailyNoteConfig,
    link::{GraphEdge, Link},
    note::NoteId,
    recurrence::{self, Recurrence},
    storage::markdown::parse_frontmatter,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
    Note,
};

use crate::{
    error::{AppError, AppResult},
    state::{AppState, WsEvent},
};

#[derive(Deserialize)]
pub struct ListQuery {
    pub tag: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Deserialize)]
pub struct CreateNoteReq {
    pub title: String,
    pub content: String,
    pub tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct UpdateNoteReq {
    /// Full note content (including frontmatter). The store writes this directly to disk.
    pub content: String,
}

pub async fn list_notes(
    Query(q): Query<ListQuery>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Vec<Note>>> {
    let limit = q.limit.unwrap_or(100);
    let offset = q.offset.unwrap_or(0);
    let notes = s.store.list(q.tag.as_deref(), limit, offset).await?;
    Ok(Json(notes))
}

pub async fn get_note(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Note>> {
    let note_id = NoteId::new(&id);
    let note = s
        .store
        .get(&note_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note not found: {}", id)))?;
    Ok(Json(note))
}

#[derive(Deserialize)]
pub struct DailyQuery {
    pub date: Option<String>, // optional ISO date "2026-03-30"
}

pub async fn get_daily_note(
    Query(q): Query<DailyQuery>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Note>> {
    let config = DailyNoteConfig::default();
    let date = q.date.and_then(|d| {
        // Parse "YYYY-MM-DD" without pulling in chrono directly
        let parts: Vec<&str> = d.split('-').collect();
        if parts.len() == 3 {
            if let (Ok(y), Ok(m), Ok(d)) = (
                parts[0].parse::<i32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<u32>(),
            ) {
                return chrono::NaiveDate::from_ymd_opt(y, m, d);
            }
        }
        None
    });
    let note = s.store.daily_note(date, &config).await?;
    Ok(Json(note))
}

pub async fn create_note(
    State(s): State<Arc<AppState>>,
    Json(req): Json<CreateNoteReq>,
) -> AppResult<Json<Note>> {
    let tags: Vec<&str> = req
        .tags
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(String::as_str)
        .collect();
    let note = s.store.create(&req.title, &req.content, &tags).await?;
    s.index.reindex(&note).await?;
    ensure_tag_pages(&s, &note).await;
    let _ = s.ws_tx.send(WsEvent::NoteCreated { note: note.clone() });
    Ok(Json(note))
}

pub async fn update_note(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
    Json(req): Json<UpdateNoteReq>,
) -> AppResult<Json<Note>> {
    let note_id = NoteId::new(&id);
    let mut note = s
        .store
        .get(&note_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note not found: {}", id)))?;
    let prev_content = note.content.clone();
    // Phase 12.2 — server-side recurrence: detect any block whose status
    // flipped to `done` in this PUT and bump its deadline before saving.
    // Single-source-of-truth so all three web write paths (KanbanBoard,
    // BottomDrawer, BlockOutliner status cycle) trigger consistently.
    let (new_content, bumps) = apply_post_save_bumps_with_info(&prev_content, &req.content, &id);
    // Phase 12.4 — same-note dependency unblock: if a block's blocker just
    // flipped to done and the block is currently `backlog`, advance it to
    // `todo`. Cross-note dependencies are out of v1 scope; users can
    // manually unblock or wait for the dependent's own save to re-evaluate.
    let (new_content, unblocked) = apply_dependency_cycles(&prev_content, &new_content, &id);
    note.content = new_content;
    s.store.update(&note).await?;
    // Re-read to get fresh parsed metadata and checksum
    let updated = s
        .store
        .get(&note_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note not found after update: {}", id)))?;
    s.index.reindex(&updated).await?;
    // Phase 9.3: append a version row. Best-effort — a versioning failure
    // shouldn't fail the PUT. Cap each note at 200 historical versions.
    if updated.content != prev_content {
        if let Err(e) = s
            .index
            .record_version(&note_id, Some(&prev_content), &updated.content, 200)
            .await
        {
            tracing::warn!("Failed to record note version: {}", e);
        }
    }
    ensure_tag_pages(&s, &updated).await;
    let _ = s.ws_tx.send(WsEvent::NoteUpdated {
        note: updated.clone(),
    });
    // Phase 12.3 — fire RecurringRolled per bumped block so the client
    // can surface "rolled to next month" notifications.
    for info in bumps {
        let _ = s.ws_tx.send(WsEvent::RecurringRolled {
            block_id: info.block_id,
            title: info.title,
            note_id: id.clone(),
            next_deadline: info.next_deadline,
        });
    }
    if !unblocked.is_empty() {
        tracing::debug!("dependency cycles: unblocked {} block(s) in note {}", unblocked.len(), id);
    }
    Ok(Json(updated))
}

pub async fn delete_note(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<StatusCode> {
    let note_id = NoteId::new(&id);
    s.store.delete(&note_id).await?;
    s.index.remove(&note_id).await?;
    let _ = s.ws_tx.send(WsEvent::NoteDeleted { id });
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_backlinks(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Vec<Link>>> {
    let note_id = NoteId::new(&id);
    let links = s.index.get_backlinks(&note_id).await?;
    Ok(Json(links))
}

pub async fn get_forward_links(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Vec<Link>>> {
    let note_id = NoteId::new(&id);
    let links = s.index.get_forward_links(&note_id).await?;
    Ok(Json(links))
}

pub async fn get_all_edges(State(s): State<Arc<AppState>>) -> AppResult<Json<Vec<GraphEdge>>> {
    let edges = s.index.get_all_edges().await?;
    Ok(Json(edges))
}

#[derive(Deserialize)]
pub struct RecurBumpReq {
    /// Block id in `<note_id>:<line>` format (matches `ParsedBlock.id`).
    pub block_id: String,
}

#[derive(serde::Serialize)]
pub struct RecurBumpResp {
    pub bumped: bool,
    pub next_deadline: Option<String>,
}

/// When a recurring task flips to `status:: done`, advance its `deadline::`
/// to the next occurrence and stamp `last_completed::`. Idempotent and
/// no-ops on any of: status != done, missing/unparseable `recurring::`,
/// missing/unparseable `deadline::`. Reuses `update_note`'s persistence
/// path so the same WS broadcast + version row + reindex behavior applies.
///
/// In normal usage `update_note` auto-detects flips via
/// `apply_post_save_bumps`, so this endpoint mainly exists as an explicit
/// trigger for debugging / future CLI use.
pub async fn recur_bump(
    State(s): State<Arc<AppState>>,
    Json(req): Json<RecurBumpReq>,
) -> AppResult<Json<RecurBumpResp>> {
    let (note_id_str, _) = match req.block_id.rsplit_once(':') {
        Some((nid, _)) => (nid.to_string(), ()),
        None => return Ok(Json(RecurBumpResp { bumped: false, next_deadline: None })),
    };

    let note_id = NoteId::new(&note_id_str);
    let note = s
        .store
        .get(&note_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note not found: {}", note_id_str)))?;

    let Some((new_content, next_iso)) = try_bump_block(&note.content, &req.block_id) else {
        return Ok(Json(RecurBumpResp { bumped: false, next_deadline: None }));
    };

    let prev_content = note.content.clone();
    let mut updated_note = note.clone();
    updated_note.content = new_content;
    s.store.update(&updated_note).await?;
    let updated = s
        .store
        .get(&note_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note not found after recur bump: {}", note_id_str)))?;
    s.index.reindex(&updated).await?;
    if updated.content != prev_content {
        if let Err(e) = s
            .index
            .record_version(&note_id, Some(&prev_content), &updated.content, 200)
            .await
        {
            tracing::warn!("Failed to record note version on recur bump: {}", e);
        }
    }
    let _ = s.ws_tx.send(WsEvent::NoteUpdated { note: updated });

    tracing::info!("recur-bump: {} -> {}", req.block_id, next_iso);
    Ok(Json(RecurBumpResp {
        bumped: true,
        next_deadline: Some(next_iso),
    }))
}

/// Pure helper. Returns `Some((new_content, next_deadline_iso))` if `block_id`
/// resolves to a block in `content` with `status:: done` + valid `recurring::`
/// + valid `deadline::`. Returns `None` for any reason a bump cannot apply
/// (idempotent caller-side: just don't replace `content`).
///
/// The same routine drives both the explicit `recur_bump` endpoint and the
/// post-save detection inside `update_note`, so semantics stay identical
/// across both paths.
pub fn try_bump_block(content: &str, block_id: &str) -> Option<(String, String)> {
    let (note_id_str, line_str) = block_id.rsplit_once(':')?;
    let line_num: usize = line_str.parse().ok()?;
    let (_meta, body) = parse_frontmatter(content).ok()?;
    let blocks = parse_blocks(note_id_str, &body);
    let block = blocks.iter().find(|b| b.id == block_id)?;

    if block.properties.get("status").map(|s| s.as_str()) != Some("done") {
        return None;
    }
    let recurring_str = block.properties.get("recurring")?;
    let rec: Recurrence = recurrence::parse(recurring_str)?;
    let deadline_raw = block.properties.get("deadline")?;
    let (anchor_date, time_suffix) = parse_deadline_value(deadline_raw)?;

    let next_date = recurrence::next_after(&rec, anchor_date);
    let new_deadline = format_deadline(next_date, time_suffix.as_deref());
    let last_completed = format!("[[{}]]", anchor_date.format("%Y-%m-%d"));

    let new_body = rewrite_block_for_bump(&body, line_num, &new_deadline, &last_completed)?;
    let new_content = reassemble_content(content, &body, &new_body);
    Some((new_content, next_date.format("%Y-%m-%d").to_string()))
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
    fn parse_body_blocks(content: &str) -> Vec<tesela_core::block::ParsedBlock> {
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
pub fn apply_dependency_cycles(
    prev: &str,
    next: &str,
    note_id: &str,
) -> (String, Vec<String>) {
    let flipped_to_done = detect_status_flips_to_done(prev, next);
    if flipped_to_done.is_empty() {
        return (next.to_string(), Vec::new());
    }

    // Map __diff__ ids → real note_id ids so the dependency check can match
    // `<note_id>:<line>` references inside `blocked_by::` values verbatim.
    let just_done: std::collections::HashSet<String> = flipped_to_done
        .iter()
        .filter_map(|id| id.rsplit_once(':').map(|(_, l)| format!("{}:{}", note_id, l)))
        .collect();

    let (_meta, body) = match parse_frontmatter(next) {
        Ok(b) => b,
        Err(_) => return (next.to_string(), Vec::new()),
    };
    let blocks = parse_blocks(note_id, &body);
    let block_index: std::collections::HashMap<&str, &tesela_core::block::ParsedBlock> =
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
            .map(|s| s.trim().trim_start_matches("[[").trim_end_matches("]]").to_string())
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
            let line = block.id.rsplit_once(':').and_then(|(_, l)| l.parse().ok()).unwrap_or(0);
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
        if trim.is_empty() { continue; }
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

/// Walk `body` lines and rewrite the block beginning at `block_line_num`:
/// `status::` → `todo`, `deadline::` → `new_deadline`, `last_completed::`
/// updated or appended. Continuation lines belong to the block until we
/// hit the next line whose indent is `<= block_indent` and starts a new
/// `- ` bullet (matches the parser's notion of block boundaries).
///
/// Returns `None` if the block can't be located (its line is missing, or
/// not a bullet) — caller treats that as a no-op.
fn rewrite_block_for_bump(
    body: &str,
    block_line_num: usize,
    new_deadline: &str,
    last_completed: &str,
) -> Option<String> {
    let mut lines: Vec<String> = body.lines().map(String::from).collect();
    if block_line_num >= lines.len() {
        return None;
    }
    let block_line = &lines[block_line_num];
    let trim_start = block_line.trim_start();
    if !(trim_start.starts_with("- ") || trim_start.trim_end() == "-") {
        return None;
    }
    let block_indent_spaces = block_line.len() - trim_start.len();

    // Walk forward from line+1 to find the block's continuation range.
    let mut end = lines.len();
    for i in (block_line_num + 1)..lines.len() {
        let l = &lines[i];
        let t = l.trim_start();
        if t.is_empty() {
            continue;
        }
        let l_indent = l.len() - t.len();
        let is_bullet = t.starts_with("- ") || t.trim_end() == "-";
        // Same-or-shallower indented bullet ends the block.
        if is_bullet && l_indent <= block_indent_spaces {
            end = i;
            break;
        }
    }

    // Continuation lines indent: block indent + 2 spaces (matches parser convention).
    let cont_indent = " ".repeat(block_indent_spaces + 2);

    let mut updated_status = false;
    let mut updated_deadline = false;
    let mut updated_last_completed = false;
    for line in lines.iter_mut().take(end).skip(block_line_num + 1) {
        if let Some((key, _)) = property_kv(line) {
            match key.as_str() {
                "status" => {
                    *line = format!("{}status:: todo", cont_indent);
                    updated_status = true;
                }
                "deadline" => {
                    *line = format!("{}deadline:: {}", cont_indent, new_deadline);
                    updated_deadline = true;
                }
                "last_completed" => {
                    *line = format!("{}last_completed:: {}", cont_indent, last_completed);
                    updated_last_completed = true;
                }
                _ => {}
            }
        }
    }
    // If anything was missing, append it at the block's tail (just before `end`).
    let mut additions: Vec<String> = Vec::new();
    if !updated_status {
        additions.push(format!("{}status:: todo", cont_indent));
    }
    if !updated_deadline {
        additions.push(format!("{}deadline:: {}", cont_indent, new_deadline));
    }
    if !updated_last_completed {
        additions.push(format!("{}last_completed:: {}", cont_indent, last_completed));
    }
    if !additions.is_empty() {
        for (offset, add) in additions.into_iter().enumerate() {
            lines.insert(end + offset, add);
        }
    }

    let trailing_newline = body.ends_with('\n');
    let mut out = lines.join("\n");
    if trailing_newline {
        out.push('\n');
    }
    Some(out)
}

/// Match an indented `key:: value` line. Returns `(key, value)` lowercased
/// key plus raw value (trimmed). Only call on continuation lines.
fn property_kv(line: &str) -> Option<(String, String)> {
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

/// Auto-create tag pages for any tags in the note that don't have a corresponding page.
/// Scans both frontmatter tags AND inline #tags in the body.
async fn ensure_tag_pages(s: &Arc<AppState>, note: &Note) {
    // Collect tags from frontmatter AND inline body text
    let mut all_tags: Vec<String> = note.metadata.tags.clone();
    // Extract inline #tags from body
    let tag_re = regex::Regex::new(r"#([A-Za-z][A-Za-z0-9_-]*)").unwrap();
    for cap in tag_re.captures_iter(&note.body) {
        let tag = cap[1].to_string();
        if !all_tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
            all_tags.push(tag);
        }
    }

    for tag in &all_tags {
        if tag == "daily" {
            continue;
        }

        let tag_id = NoteId::new(tag.to_lowercase());
        match s.store.get(&tag_id).await {
            Ok(Some(_)) => {} // Page already exists
            Ok(None) => {
                // Auto-create tag page
                let content = format!(
                    "---\ntitle: \"{}\"\ntype: \"Tag\"\nextends: \"Root Tag\"\ntag_properties: []\ntags: []\n---\n- Tag properties are inherited by all nodes using the tag.\n",
                    tag
                );
                match s.store.create(tag, &content, &[]).await {
                    Ok(tag_note) => {
                        let _ = s.index.reindex(&tag_note).await;
                        let _ = s.ws_tx.send(WsEvent::NoteCreated { note: tag_note });
                        tracing::info!("Auto-created tag page: {}", tag);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to auto-create tag page '{}': {}", tag, e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to check tag page '{}': {}", tag, e);
            }
        }
    }
}
