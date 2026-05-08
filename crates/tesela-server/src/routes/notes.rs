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
    note.content = apply_post_save_bumps(&prev_content, &req.content);
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
pub fn apply_post_save_bumps(prev: &str, next: &str) -> String {
    let flipped = detect_status_flips_to_done(prev, next);
    let mut content = next.to_string();
    for block_id in flipped {
        if let Some((bumped, _)) = try_bump_block(&content, &block_id) {
            content = bumped;
        }
    }
    content
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
