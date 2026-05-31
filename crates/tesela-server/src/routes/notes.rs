use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use tesela_core::{
    block::parse_blocks,
    daily::DailyNoteConfig,
    link::{GraphEdge, Link, LinkType},
    note::NoteId,
    note_tree::{parse_note, serialize_note},
    recurrence::{self, Recurrence},
    storage::markdown::parse_frontmatter,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
    Note,
};
use tesela_sync::OpPayload;

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
pub struct RenameTagReq {
    pub from_slug: String,
    pub to_slug: String,
    /// When `false`, walk the corpus and return the rewrite count without
    /// touching any file. When `true`, apply the rewrite + the file move.
    /// Defaults to `false` so accidental calls don't mutate.
    #[serde(default)]
    pub commit: bool,
}

#[derive(Deserialize)]
pub struct ResolveTagReq {
    /// Path-form (`nature/birds/cardinal`) or bare (`cardinal`).
    pub path: String,
}

#[derive(Deserialize)]
pub struct CleanupTagReq {
    /// Same two-phase contract as `RenameTagReq.commit`.
    #[serde(default)]
    pub commit: bool,
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

#[derive(serde::Serialize)]
pub struct LoroIndexEntry {
    pub note_id: String,
    pub title: String,
    pub slug: String,
    pub tags: Vec<String>,
    pub links: Vec<String>,
}

/// `GET /api/loro/index` — dump the Loro index doc (the hybrid-model
/// spine): every note's `{note_id, title, slug, tags, links}`. Debug
/// surface for Phase 2.
pub async fn get_loro_index(
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Vec<LoroIndexEntry>>> {
    let entries = s
        .sync_engine
        .index_entries()
        .await
        .into_iter()
        .map(|e| LoroIndexEntry {
            note_id: e.note_id,
            title: e.title,
            slug: e.slug,
            tags: e.tags,
            links: e.links,
        })
        .collect();
    Ok(Json(entries))
}

/// `GET /api/loro/notes/{id}/snapshot` — the full Loro snapshot for a
/// single note's doc, as raw bytes. A fresh device imports this as a
/// **shared base** before it authors locally, so its `BlockUpsert`s
/// resolve to the server's existing tree nodes instead of minting rival
/// TreeIDs (multi-device convergence — Part D). 404 when the doc isn't
/// resident.
pub async fn get_loro_snapshot(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<impl IntoResponse> {
    let note_id = stable_uuid_from_slug(&id);
    let bytes = s
        .sync_engine
        .export_doc_update(note_id, None)
        .await
        .ok_or_else(|| AppError::NotFound(format!("Loro doc not found: {}", id)))?;
    Ok((
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
        bytes,
    ))
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
    // Probe first so we can detect the lazy-creation branch and emit a
    // NoteUpsert into the sync oplog only on the first-ever create. Without
    // this, peers receive BlockUpserts for the daily but can't resolve a
    // slug for the note_id and silently drop them.
    let resolved_date = date.unwrap_or_else(|| chrono::Local::now().date_naive());
    let slug = tesela_core::daily::daily_note_title(resolved_date, &config);
    let existed = s.store.get(&NoteId::new(&slug)).await?;
    let note = s.store.daily_note(date, &config).await?;
    if existed.is_none() {
        s.index.reindex(&note).await?;
        record_sync_create(&s, &note).await;
        let _ = s.ws_tx.send(WsEvent::NoteCreated { note: note.clone() });
    }
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
    // Phase 2.2 (2026-05-27): no longer auto-prune blank blocks.
    // Daisy reported web preserves blanks but iOS stripped them,
    // creating an asymmetry. Both clients now preserve blanks; the
    // user can delete blocks explicitly when they're genuinely
    // abandoned. Re-introducing prune as an opt-in "tidy" action is
    // a future-friendly option.
    let stamped = stamp_block_ids(&req.content);
    let note = s.store.create(&req.title, &stamped, &tags).await?;
    s.index.reindex(&note).await?;
    {
        use tesela_core::link::extract_wiki_links;
        use tesela_core::traits::link_graph::LinkGraph;
        let links = extract_wiki_links(&note.content);
        if let Err(e) = s.index.update_links(&note.id, &links).await {
            tracing::warn!("Failed to update links on create for {:?}: {}", note.id, e);
        }
    }
    ensure_tag_pages(&s, &note).await;
    record_sync_create(&s, &note).await;
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
    // Phase 2.2 (2026-05-27): no longer auto-prune blank blocks here
    // either. Both clients preserve blanks consistently.
    let stamped_new = stamp_block_ids(&new_content);
    note.content = stamped_new;
    // Instant-multidevice (Phase A): capture this note's Loro version
    // vector BEFORE the edit so we can export the cursor-free delta for
    // just-this-change afterward and push it to live WS clients. Cursor-
    // free (`doc_version` + `export_doc_update(.., Some(pre_vv))`) so the
    // live path never contends with the relay's broadcast cursor (spec
    // finding #3). `None` when the doc isn't yet resident (first write) —
    // we then export the full state below.
    let delta_note_id = stable_uuid_from_slug(note.id.as_str());
    let pre_vv = s.sync_engine.doc_version(delta_note_id).await;
    // Phase 1 (sync redesign 2026-05-26): single write path through
    // the engine. Previously this called s.store.update(&note) to
    // write the file via FsNoteStore AND then record_sync_update
    // below to log the op + materialize again — two write paths to
    // the same file, with race-on-mtime semantics when concurrent
    // peer ops arrived between the two writes. Now record_sync_update
    // is the sole writer: it emits BlockUpsert/Move/Delete ops (or a
    // NoteUpsert fallback for frontmatter-only changes), each of
    // which materializes the file via the engine's apply_block_*
    // functions. The HTTP handler becomes a thin op-submission
    // wrapper around the engine. See
    // `.docs/ai/phases/2026-05-26-sync-architecture-redesign.md`.
    record_sync_update(&s, &prev_content, &note).await;
    // Re-read to get fresh parsed metadata and checksum from the
    // file the engine just wrote. The engine's serialization is the
    // canonical form; downstream indexing should index THAT, not the
    // pre-canonicalization `note.content` we passed in.
    let updated = s
        .store
        .get(&note_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note not found after update: {}", id)))?;
    s.index.reindex(&updated).await?;
    // v5 polish: refresh the link graph for this note. Without this, the
    // wiki-link extractor only runs via the fs-watcher path, leaving the
    // `links` table empty when notes round-trip through PUT only. The
    // backlinks API + fullscreen graph both depend on the `links` table.
    {
        use tesela_core::link::extract_wiki_links;
        use tesela_core::traits::link_graph::LinkGraph;
        let links = extract_wiki_links(&updated.content);
        if let Err(e) = s.index.update_links(&note_id, &links).await {
            tracing::warn!("Failed to update links on PUT for {:?}: {}", note_id, e);
        }
    }
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
    // `record_sync_update` already ran above before the re-read — it
    // is what wrote the file. No second invocation here.
    let _ = s.ws_tx.send(WsEvent::NoteUpdated {
        note: updated.clone(),
    });
    // Instant-multidevice (Phase A): export the cursor-free delta for the
    // change `record_sync_update` just applied and push it to live WS
    // clients as a binary frame, so peer devices converge in <1s without
    // waiting on the relay poll. `origin: None` — an HTTP edit fans out to
    // every connected socket. Best-effort: if the doc isn't resident or
    // export fails we skip the push (the slower relay/poll path still
    // carries it). Does NOT touch the relay's broadcast cursor.
    if let Some(delta) = s
        .sync_engine
        .export_doc_update(delta_note_id, pre_vv.as_deref())
        .await
    {
        match tesela_sync::encode_loro_relay_payload(&[tesela_sync::LoroDocUpdate {
            doc: delta_note_id,
            update_bytes: delta,
        }]) {
            Ok(frame) => {
                let _ = s.ws_delta_tx.send(crate::state::WsDelta {
                    origin: None,
                    frame,
                });
            }
            Err(e) => tracing::warn!("ws: encode live delta for {} failed: {}", id, e),
        }
    }
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
        tracing::debug!(
            "dependency cycles: unblocked {} block(s) in note {}",
            unblocked.len(),
            id
        );
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
    record_sync_delete(&s, &note_id).await;
    let _ = s.ws_tx.send(WsEvent::NoteDeleted { id });
    Ok(StatusCode::NO_CONTENT)
}

/// Explicit per-block deletion. Phase 2.2 (sync redesign 2026-05-27):
/// the server-side PUT diff no longer infers `BlockDelete` from
/// "absent in PUT body" because clients with stale views were
/// stomping peer-added blocks they hadn't fetched yet. Genuine
/// user-intent deletes now route through this endpoint instead.
///
/// `bid_param` accepts EITHER:
///   1. A canonical 36-char dashed UUID — the bid stamped into the
///      on-disk `<!-- bid:UUID -->` marker. iOS native passes this.
///   2. The web client's composite id form `<note_id>:<line_number>`.
///      Web's `ParsedBlock.id` is line-based (the server's block-parser
///      shape) and doesn't currently surface the bid as a separate
///      field, so we accept that form and resolve to the bid by
///      reading the current file's line-N bid marker.
///
/// Either way the handler ends up recording a `BlockDelete(bid)` op
/// via the sync engine, which materializes by removing the block from
/// `<mosaic>/notes/<id>.md`; the file watcher + WS broadcast then
/// propagate to other clients.
pub async fn delete_block(
    Path((id, bid_param)): Path<(String, String)>,
    State(s): State<Arc<AppState>>,
) -> AppResult<StatusCode> {
    let block_uuid = if let Ok(u) = uuid::Uuid::parse_str(&bid_param) {
        u
    } else {
        // Try the composite `<note>:<line>` form web sends. Split on
        // the last `:` to tolerate note ids that themselves contain
        // colons (rare but possible for tag pages etc.).
        let (_, line_str) = bid_param
            .rsplit_once(':')
            .ok_or_else(|| AppError::Validation(format!("Invalid block id: {bid_param}")))?;
        let line: usize = line_str
            .parse()
            .map_err(|_| AppError::Validation(format!("Invalid block id: {bid_param}")))?;
        let note_id = NoteId::new(&id);
        let note = match s.store.get(&note_id).await? {
            Some(n) => n,
            None => return Ok(StatusCode::NO_CONTENT),
        };
        // Web's `ParsedBlock.id = <note>:<line_num>` numbers lines
        // relative to the parsed BODY (post-frontmatter), not the
        // whole file — see `tesela_core::block::parse_blocks`. Match
        // that addressing here. Without this conversion, line 1 of
        // the full content is `title: 2026-05-27`, no bid marker,
        // delete is a silent no-op and the block survives.
        let lines: Vec<&str> = note.body.lines().collect();
        let raw_line = lines
            .get(line)
            .ok_or_else(|| AppError::Validation(format!("Line {line} out of range")))?;
        // Pull the bid out of the `<!-- bid:UUID -->` marker. If the
        // line doesn't have a bid, the block was created locally on web
        // and hasn't round-tripped through the server's stamp pass yet
        // — there's nothing to delete on the server side, the client
        // can drop it locally.
        let re = regex::Regex::new(r"<!--\s*bid:([0-9a-fA-F-]+)\s*-->").unwrap();
        match re.captures(raw_line).and_then(|c| c.get(1)) {
            Some(m) => uuid::Uuid::parse_str(m.as_str())
                .map_err(|_| AppError::Validation("Malformed bid in file".into()))?,
            None => return Ok(StatusCode::NO_CONTENT),
        }
    };
    let payload = OpPayload::BlockDelete {
        block_id: *block_uuid.as_bytes(),
    };
    if let Err(e) = s.sync_engine.record_local(payload).await {
        tracing::warn!("sync: record_local BlockDelete failed for {bid_param}: {e}");
        return Err(AppError::Internal(anyhow::anyhow!(
            "Failed to record BlockDelete: {e}"
        )));
    }
    let note_id = NoteId::new(&id);
    let updated = match s.store.get(&note_id).await {
        Ok(Some(n)) => n,
        Ok(None) => return Ok(StatusCode::NO_CONTENT),
        Err(e) => return Err(e.into()),
    };
    s.index.reindex(&updated).await?;
    let _ = s.ws_tx.send(WsEvent::NoteUpdated { note: updated });
    Ok(StatusCode::NO_CONTENT)
}

/// Tag usage counts — what would be affected by deleting this tag. Surfaced
/// by `:delete-tag` so the user can decide how to handle refs / children.
///
/// `references` counts inline `#tag` occurrences across all note bodies.
/// `page_instances` counts pages with the tag in their `tags:` frontmatter.
/// `block_instances` counts blocks whose parsed `tags` include this tag.
/// `children` counts tag pages whose `parent:` frontmatter is this tag.
pub async fn get_tag_usage(
    Path(slug): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<serde_json::Value>> {
    let slug_lc = slug.to_lowercase();
    let all = s.store.list(None, 10_000, 0).await?;

    let mut references = 0usize;
    let mut page_instances = 0usize;
    let mut block_instances = 0usize;
    let mut children = 0usize;

    let needle_inline = format!("#{}", slug_lc);
    let needle_wiki = format!("[[{}]]", slug_lc);

    for note in &all {
        // Self skip — don't count the tag's own page.
        if note.id.as_str().eq_ignore_ascii_case(&slug_lc) {
            continue;
        }

        // Page-level instance.
        if note
            .metadata
            .tags
            .iter()
            .any(|t| t.eq_ignore_ascii_case(&slug_lc))
        {
            page_instances += 1;
        }

        // Block-level instances + references. We do a body scan per note —
        // good enough for typical corpora.
        let lower = note.body.to_lowercase();
        for line in lower.lines() {
            // Count `#tag` occurrences on this line.
            for (idx, _) in line.match_indices(&needle_inline) {
                // Word-boundary check: char before `#` must not be a tag-name
                // char (already true since `#` is the delimiter); char after
                // the slug must not extend the token.
                let after_idx = idx + needle_inline.len();
                let after = line.as_bytes().get(after_idx).copied().unwrap_or(b' ');
                let extends = (after as char).is_ascii_alphanumeric()
                    || after == b'_'
                    || after == b'-'
                    || after == b'/';
                if !extends {
                    references += 1;
                }
            }
            // Don't count `[[slug]]` toward references — those are wiki links,
            // separate concept. Tracked here just so we can mention them in
            // the prompt if needed; not surfaced in the count for now.
            let _ = needle_wiki;
        }

        // Block-level instance: any block whose parsed tags include the slug.
        // Cheap re-parse using crate::block::parse_blocks.
        let blocks = parse_blocks(note.id.as_str(), &note.body);
        if blocks
            .iter()
            .any(|b| b.tags.iter().any(|t| t.eq_ignore_ascii_case(&slug_lc)))
        {
            block_instances += 1;
        }

        // Children: a tag page whose `parent:` frontmatter equals this slug.
        let is_tag = note
            .metadata
            .note_type
            .as_deref()
            .map(|t| t.eq_ignore_ascii_case("tag"))
            .unwrap_or(false);
        if is_tag {
            let parent = note
                .metadata
                .custom
                .get("parent")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            if parent == slug_lc {
                children += 1;
            }
        }
    }

    Ok(Json(serde_json::json!({
        "slug": slug_lc,
        "references": references,
        "page_instances": page_instances,
        "block_instances": block_instances,
        "children": children,
    })))
}

/// Resolve a path-form tag reference (`nature/birds/cardinal`) into a concrete
/// slug, cascade-creating missing ancestors top-down.
///
/// Algorithm:
///   1. Split the path by `/` into segments.
///   2. Walk segments top-to-bottom. For each segment, look for an existing
///      tag whose leaf name matches the segment AND whose `parent` matches
///      the previous segment's resolved slug (or empty for top-level).
///   3. If a matching tag exists, use it as the resolved slug for this
///      segment. Otherwise, create a new tag with `parent` set to the
///      previous segment's slug.
///   4. After walking, the last segment's slug is the resolved target.
///
/// Returns the final resolved slug plus an audit trail of any new tags
/// that were cascade-created. The frontend uses that audit trail to
/// inform the user ("created 2 ancestor tags: nature, nature/birds").
pub async fn resolve_tag(
    State(s): State<Arc<AppState>>,
    Json(req): Json<ResolveTagReq>,
) -> AppResult<Json<serde_json::Value>> {
    let path = req.path.trim().trim_matches('/');
    if path.is_empty() {
        return Err(AppError::Validation("path is empty".into()));
    }
    let segments: Vec<String> = path
        .split('/')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if segments.is_empty() {
        return Err(AppError::Validation("path has no segments".into()));
    }

    let mut cascade_created: Vec<String> = Vec::new();
    let mut parent_slug: String = String::new();

    for (i, segment) in segments.iter().enumerate() {
        let is_last = i + 1 == segments.len();
        let resolved = resolve_one_segment(&s, segment, &parent_slug).await?;
        parent_slug = match resolved {
            SegmentResolution::Existing(slug) => slug,
            SegmentResolution::Created(slug) => {
                cascade_created.push(slug.clone());
                slug
            }
        };
        let _ = is_last; // currently only the final segment differs in usage; reserved for future hooks
    }

    Ok(Json(serde_json::json!({
        "slug": parent_slug,
        "cascade_created": cascade_created,
    })))
}

enum SegmentResolution {
    Existing(String),
    Created(String),
}

/// Resolve one path segment against the existing tag corpus. Returns the
/// segment's resolved slug, creating a new tag page if no match exists.
///
/// Match rule: an existing tag matches if its leaf name (lowercased) equals
/// the segment AND its `parent` frontmatter (case-insensitive, empty for
/// top-level) equals `parent_slug`.
async fn resolve_one_segment(
    s: &Arc<AppState>,
    segment: &str,
    parent_slug: &str,
) -> AppResult<SegmentResolution> {
    // List all tag pages. The corpus is small relative to per-segment cost,
    // so a single list-and-filter is acceptable. If this becomes a hot path,
    // add a name-indexed tag map in AppState.
    let all = s.store.list(None, 10_000, 0).await?;
    for note in &all {
        let is_tag = note
            .metadata
            .note_type
            .as_deref()
            .map(|t| t.eq_ignore_ascii_case("tag"))
            .unwrap_or(false);
        if !is_tag {
            continue;
        }
        let title_lc = note
            .metadata
            .title
            .clone()
            .unwrap_or_else(|| note.id.as_str().to_string())
            .to_lowercase();
        if title_lc != segment {
            continue;
        }
        let parent = note
            .metadata
            .custom
            .get("parent")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        if parent == parent_slug.to_lowercase() {
            return Ok(SegmentResolution::Existing(note.id.as_str().to_string()));
        }
    }

    // No match — create a new tag at this segment with the resolved parent.
    let slug_base = segment.to_string();
    let resolved_slug = match resolve_free_tag_slug(s, &slug_base).await {
        Ok(Some(slug)) => slug,
        Ok(None) => slug_base.clone(), // shouldn't happen given our filter above
        Err(e) => return Err(AppError::Internal(anyhow::anyhow!(e))),
    };

    let content = format!(
        "---\ntitle: \"{}\"\ntype: tag\nextends: \"Root Tag\"\ntag_properties: []\nparent: \"{}\"\ntags: []\n---\n- Tag properties are inherited by all nodes using the tag.\n",
        segment, parent_slug
    );
    let created = s.store.create(&resolved_slug, &content, &[]).await?;
    s.index.reindex(&created).await?;
    record_sync_create(s, &created).await;
    let _ = s.ws_tx.send(WsEvent::NoteCreated { note: created });
    Ok(SegmentResolution::Created(resolved_slug))
}

/// Rename a tag page's slug and rewrite references across the corpus.
///
/// Two-phase contract: when `req.commit == false` the handler returns the
/// rewrite counts (refs touched, notes affected) without mutating anything,
/// so the frontend can show a confirm dialog. When `req.commit == true` the
/// rewrite is applied for real.
///
/// Rewrite scope (per the 2026-05-17 product decisions):
/// - `#<oldslug>` tokens in note bodies → `#<newslug>`
/// - `[[<oldslug>]]` wiki links → `[[<newslug>]]` (alias preserved)
/// - Children's `parent: <oldslug>` frontmatter → `parent: <newslug>`
/// - Source tag's own file moves from `<oldslug>.md` to `<newslug>.md`
///
/// NOT touched:
/// - Page-level `tags: [oldslug, ...]` frontmatter arrays — by explicit
///   product decision (those are page-level, the rename targets the
///   tag-entity slug only).
/// - References inside fenced code blocks (` ``` ... ``` `).
pub async fn rename_tag(
    State(s): State<Arc<AppState>>,
    Json(req): Json<RenameTagReq>,
) -> AppResult<Json<serde_json::Value>> {
    let from_id = NoteId::new(&req.from_slug);
    let to_id = NoteId::new(&req.to_slug);
    let from_slug_lc = req.from_slug.to_lowercase();
    let to_slug_lc = req.to_slug.to_lowercase();

    if from_slug_lc == to_slug_lc {
        return Err(AppError::Validation(
            "from_slug and to_slug are identical".into(),
        ));
    }

    let source = s
        .store
        .get(&from_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("tag '{}'", req.from_slug)))?;

    let is_tag = source
        .metadata
        .note_type
        .as_deref()
        .map(|t| t.eq_ignore_ascii_case("tag"))
        .unwrap_or(false);
    if !is_tag {
        return Err(AppError::Validation(format!(
            "page '{}' is not a tag (type: {:?})",
            req.from_slug, source.metadata.note_type
        )));
    }

    if s.store.get(&to_id).await?.is_some() {
        return Err(AppError::Validation(format!(
            "slug '{}' is already taken",
            req.to_slug
        )));
    }

    // Walk the corpus and compute rewrites. For each note we record:
    //   - the new content (after rewrite)
    //   - count of refs rewritten on this note
    // The source tag's own file is excluded — it'll be deleted in the
    // file-move step regardless.
    use tesela_core::tag_rewrite::{
        rewrite_inline_tag, rewrite_parent_frontmatter, rewrite_wiki_link,
    };
    let all = s.store.list(None, 100_000, 0).await?;
    let mut plan: Vec<(Note, String, usize)> = Vec::new();
    let mut total_refs = 0usize;
    for note in all {
        if note.id.as_str().eq_ignore_ascii_case(&from_slug_lc) {
            continue;
        }
        let (body_after_inline, n_inline) =
            rewrite_inline_tag(&note.body, &from_slug_lc, &to_slug_lc);
        let (body_after_wiki, n_wiki) =
            rewrite_wiki_link(&body_after_inline, &from_slug_lc, &to_slug_lc);
        // Frontmatter parent rewrite runs on the full content so the
        // frontmatter block is correctly delimited.
        let (full_content_for_parent, _) =
            rewrite_parent_frontmatter(&note.content, &from_slug_lc, &to_slug_lc);
        // We want to assemble: frontmatter (potentially rewritten) + body
        // (rewritten). Easiest: rebuild the full content as frontmatter +
        // body. The store's `update` writes the whole content blob to disk.
        let n_total = n_inline + n_wiki;
        // If neither the body nor the frontmatter changed, skip.
        let body_changed = n_total > 0;
        let parent_changed = full_content_for_parent != note.content;
        if !body_changed && !parent_changed {
            continue;
        }
        // Take the parent-rewritten frontmatter (if any) and splice on the
        // body-rewritten body.
        let new_content = splice_body_into_content(&full_content_for_parent, &body_after_wiki);
        total_refs += n_total + if parent_changed { 1 } else { 0 };
        plan.push((note, new_content, n_total));
    }

    let notes_affected = plan.len();

    if !req.commit {
        return Ok(Json(serde_json::json!({
            "commit": false,
            "from_slug": req.from_slug,
            "to_slug": req.to_slug,
            "refs": total_refs,
            "notes": notes_affected,
        })));
    }

    // Commit phase. Apply each plan entry through the store's update path,
    // reindex, and emit sync ops. Errors abort the rest of the rewrite —
    // partial state is acceptable since each note is independently valid.
    for (note, new_content, _n) in plan {
        let updated_note = Note {
            content: new_content,
            ..note.clone()
        };
        s.store.update(&updated_note).await?;
        s.index.reindex(&updated_note).await?;
        record_sync_update(&s, &note.content, &updated_note).await;
        let _ = s.ws_tx.send(WsEvent::NoteUpdated {
            note: updated_note,
        });
    }

    // Now move the source tag's own file.
    let renamed = s
        .store
        .create(&req.to_slug, &source.content, &[])
        .await?;
    s.index.reindex(&renamed).await?;
    record_sync_create(&s, &renamed).await;
    let _ = s.ws_tx.send(WsEvent::NoteCreated {
        note: renamed.clone(),
    });

    s.store.delete(&from_id).await?;
    s.index.remove(&from_id).await?;
    record_sync_delete(&s, &from_id).await;
    let _ = s.ws_tx.send(WsEvent::NoteDeleted {
        id: req.from_slug.clone(),
    });

    Ok(Json(serde_json::json!({
        "commit": true,
        "from_slug": req.from_slug,
        "to_slug": req.to_slug,
        "refs": total_refs,
        "notes": notes_affected,
    })))
}

/// `:delete-tag` cleanup path — strip every `#<slug>` and `[[<slug>]]`
/// reference from the corpus, and clear children's `parent: <slug>`
/// frontmatter so they orphan cleanly.
///
/// Same two-phase contract as `rename_tag`: `commit=false` previews,
/// `commit=true` applies. Returns counts.
///
/// This is intentionally a separate verb from the delete: the frontend
/// calls cleanup THEN delete-note. The tag's own file is NOT deleted by
/// this handler.
pub async fn cleanup_tag_references(
    Path(slug): Path<String>,
    State(s): State<Arc<AppState>>,
    Json(req): Json<CleanupTagReq>,
) -> AppResult<Json<serde_json::Value>> {
    let slug_lc = slug.to_lowercase();

    use tesela_core::tag_rewrite::{
        clear_parent_frontmatter, strip_inline_tag, strip_wiki_link,
    };

    let all = s.store.list(None, 100_000, 0).await?;
    let mut plan: Vec<(Note, String, usize)> = Vec::new();
    let mut total_refs = 0usize;

    for note in all {
        // Skip the tag's own file — the caller will delete it next.
        if note.id.as_str().eq_ignore_ascii_case(&slug_lc) {
            continue;
        }
        let (body_after_inline, n_inline) = strip_inline_tag(&note.body, &slug_lc);
        let (body_after_wiki, n_wiki) = strip_wiki_link(&body_after_inline, &slug_lc);
        let (content_after_parent, parent_cleared) =
            clear_parent_frontmatter(&note.content, &slug_lc);
        let n_total = n_inline + n_wiki;
        let body_changed = n_total > 0;
        if !body_changed && !parent_cleared {
            continue;
        }
        let new_content = splice_body_into_content(&content_after_parent, &body_after_wiki);
        total_refs += n_total + if parent_cleared { 1 } else { 0 };
        plan.push((note, new_content, n_total));
    }

    let notes_affected = plan.len();

    if !req.commit {
        return Ok(Json(serde_json::json!({
            "commit": false,
            "slug": slug_lc,
            "refs": total_refs,
            "notes": notes_affected,
        })));
    }

    for (note, new_content, _) in plan {
        let updated_note = Note {
            content: new_content,
            ..note.clone()
        };
        s.store.update(&updated_note).await?;
        s.index.reindex(&updated_note).await?;
        record_sync_update(&s, &note.content, &updated_note).await;
        let _ = s.ws_tx.send(WsEvent::NoteUpdated {
            note: updated_note,
        });
    }

    Ok(Json(serde_json::json!({
        "commit": true,
        "slug": slug_lc,
        "refs": total_refs,
        "notes": notes_affected,
    })))
}

/// Helper for the rename / cleanup handlers: given a full note `content`
/// (frontmatter + body) and a new body, produce a content string that uses
/// the original frontmatter plus the new body.
///
/// If `content` has no frontmatter block, the new body becomes the whole
/// content.
fn splice_body_into_content(content: &str, new_body: &str) -> String {
    // Find the closing `---` of the frontmatter block. Same logic as the
    // frontend's splitContent.
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return new_body.to_string();
    }
    let after_first_newline = match content.find('\n') {
        Some(idx) => idx + 1,
        None => return new_body.to_string(),
    };
    let rest = &content[after_first_newline..];
    let close = rest.find("\n---\n").or_else(|| rest.find("\n---\r\n"));
    let close = match close {
        Some(off) => after_first_newline + off + 1, // position of `---`
        None => return new_body.to_string(),
    };
    // close points at the `---`; the line ends after `\n`.
    let line_end = match content[close..].find('\n') {
        Some(n) => close + n + 1,
        None => content.len(),
    };
    let frontmatter = &content[..line_end];
    format!("{}{}", frontmatter, new_body)
}

/// Producer path for note creation. Emits one NoteUpsert that carries
/// the slug, title, and full stamped content so any peer (including one
/// that has never seen this note before) can materialize it correctly.
async fn record_sync_create(s: &Arc<AppState>, note: &Note) {
    let payload = OpPayload::NoteUpsert {
        note_id: stable_uuid_from_slug(note.id.as_str()),
        display_alias: Some(note.id.as_str().to_string()),
        title: note.title.clone(),
        content: note.content.clone(),
        created_at_millis: note.created_at.timestamp_millis(),
    };
    if let Err(e) = s.sync_engine.record_local(payload).await {
        tracing::warn!(
            "sync: record_local NoteUpsert failed for {}: {}",
            note.id,
            e
        );
    }
}

/// Producer path for note updates. Diffs the prior on-disk content
/// against the new content and emits BlockUpsert / BlockMove /
/// BlockDelete ops. Avoids emitting a NoteUpsert: when two peers edit
/// different blocks of the same note concurrently, NoteUpsert's
/// last-writer-wins on the whole blob would stomp the loser's edit,
/// whereas block-level ops converge correctly per [[plan/block-level-sync.md]].
///
/// Falls back to emitting a NoteUpsert blob when the diff is empty but
/// the content actually changed (e.g. frontmatter-only edits like a
/// title change, which the block parser does not currently surface).
/// The fallback is data-lossy under concurrent edits to the same note,
/// matching Phase 1.5 behavior; it is recorded as a known limitation
/// in the block-level-sync plan.
async fn record_sync_update(s: &Arc<AppState>, prev_content: &str, note: &Note) {
    let note_id = stable_uuid_from_slug(note.id.as_str());
    let old_tree = parse_note(prev_content);
    let new_tree = parse_note(&note.content);
    // Phase 2.2 (sync redesign 2026-05-27): suppress inferred
    // `BlockDelete` emission. The server diffs Mac's authoritative
    // file against the client's PUT body, but the client may have a
    // stale view (e.g. typed locally while a peer's edit landed via
    // WS but hasn't yet been merged into the client's local state).
    // Treating "absent from PUT body" as "user deleted this block"
    // then stomps the peer's edit on the receiver — exactly the data-
    // loss class Daisy hit ("iOS's fella was cleared in favor of web's
    // dude"). User-intent deletes now go through the explicit
    // `DELETE /notes/<id>/blocks/<bid>` endpoint instead.
    let ops = tesela_sync::diff::diff_note_trees_with_options(
        note_id,
        &old_tree,
        &new_tree,
        tesela_sync::diff::DiffOptions { emit_deletes: false },
    );

    if ops.is_empty() {
        if prev_content == note.content {
            return;
        }
        // Body parses identical (or both empty) but raw content
        // differs: frontmatter or non-bullet content changed. Fall
        // back to NoteUpsert.
        let payload = OpPayload::NoteUpsert {
            note_id,
            display_alias: Some(note.id.as_str().to_string()),
            title: note.title.clone(),
            content: note.content.clone(),
            created_at_millis: note.created_at.timestamp_millis(),
        };
        if let Err(e) = s.sync_engine.record_local(payload).await {
            tracing::warn!(
                "sync: record_local NoteUpsert fallback failed for {}: {}",
                note.id,
                e
            );
        }
        return;
    }

    for op in ops {
        if let Err(e) = s.sync_engine.record_local(op).await {
            tracing::warn!(
                "sync: record_local Block op failed for {}: {}",
                note.id,
                e
            );
        }
    }
}

/// Parse `content`, stamp persistent block ids onto any unstamped
/// bullets, and return the canonical serialized form. Returns
/// `content` unchanged if every bullet already has a bid.
fn stamp_block_ids(content: &str) -> String {
    let tree = parse_note(content);
    if !tree.stamped_any {
        return content.to_string();
    }
    serialize_note(&tree)
}

async fn record_sync_delete(s: &Arc<AppState>, note_id: &NoteId) {
    let slug = note_id.as_str();
    let payload = OpPayload::NoteDelete {
        note_id: stable_uuid_from_slug(slug),
        display_alias: Some(slug.to_string()),
    };
    if let Err(e) = s.sync_engine.record_local(payload).await {
        tracing::warn!("sync: record_local delete failed for {}: {}", note_id, e);
    }
}

/// Phase 1.5 stable note_id derivation: blake3(slug) truncated to 16
/// bytes. Two devices independently creating the same slug produce the
/// same note_id, so it looks like an update rather than a primary-key
/// collision. Real UUID-v7 identity arrives with the Mutation API
/// refactor (Phase 2 data model).
fn stable_uuid_from_slug(slug: &str) -> [u8; 16] {
    let hash = blake3::hash(slug.as_bytes());
    let bytes = hash.as_bytes();
    let mut out = [0u8; 16];
    out.copy_from_slice(&bytes[..16]);
    out
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

/// GET `/notes/:id/unlinked` — pages that mention this page's title in
/// plain text without `[[...]]` wrapping. Logseq-style. Useful for
/// discovering implicit references the user hasn't yet promoted to a
/// real wiki link.
///
/// Returns `Link[]` where `target` is the SOURCE note's id (matching the
/// `get_backlinks` shape so the frontend can reuse its row renderer),
/// `text` is the full line of context, and `position` is the byte offset
/// of the match within the source note.
pub async fn get_unlinked(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Vec<Link>>> {
    let note_id = NoteId::new(&id);
    let focused = s
        .store
        .get(&note_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note not found: {}", id)))?;
    let title = focused
        .metadata
        .title
        .clone()
        .unwrap_or_else(|| focused.id.as_str().to_string());
    if title.trim().is_empty() {
        return Ok(Json(Vec::new()));
    }

    // Pull every note in the store (cap at a generous limit — same as the
    // notes list). We linear-scan because the link index doesn't track
    // unlinked mentions; a real index lives behind a TODO.
    let all = s.store.list(None, 5000, 0).await?;
    let needle = title.to_lowercase();
    let mut out: Vec<Link> = Vec::new();
    for n in &all {
        if n.id.as_str() == note_id.as_str() {
            continue; // skip the page itself
        }
        let body = n.content.to_lowercase();
        // Build a small set of byte offsets where the title appears.
        let mut search_from = 0usize;
        while let Some(found) = body[search_from..].find(&needle) {
            let pos = search_from + found;
            search_from = pos + needle.len();
            // Word boundary: char before+after must NOT be ascii-alphanumeric
            // (covers most real cases without dragging in a regex crate).
            let before_ok = pos == 0
                || !body
                    .as_bytes()
                    .get(pos - 1)
                    .map(|b| b.is_ascii_alphanumeric() || *b == b'_')
                    .unwrap_or(false);
            let after = pos + needle.len();
            let after_ok = after >= body.len()
                || !body
                    .as_bytes()
                    .get(after)
                    .map(|b| b.is_ascii_alphanumeric() || *b == b'_')
                    .unwrap_or(false);
            if !before_ok || !after_ok {
                continue;
            }
            // Extract the line containing the match.
            let line_start = n.content[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let line_end = n.content[pos..]
                .find('\n')
                .map(|i| pos + i)
                .unwrap_or(n.content.len());
            let line = &n.content[line_start..line_end];
            // Skip if the line already has a [[title]] wiki link to the
            // focused note — that's a regular backlink, not unlinked.
            let line_lc = line.to_lowercase();
            let wiki_marker = format!("[[{}]]", needle);
            if line_lc.contains(&wiki_marker) {
                continue;
            }
            out.push(Link {
                link_type: LinkType::Internal,
                target: n.id.as_str().to_string(),
                text: line.trim().to_string(),
                position: pos,
            });
            // Only one row per source note + position; loop continues to
            // find additional matches in the SAME source note on different
            // lines, which is what we want.
        }
    }
    Ok(Json(out))
}

pub async fn get_all_edges(State(s): State<Arc<AppState>>) -> AppResult<Json<Vec<GraphEdge>>> {
    let edges = s.index.get_all_edges().await?;
    Ok(Json(edges))
}

#[derive(Deserialize, Default, PartialEq, Eq, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum RecurBumpMode {
    #[default]
    Complete,
    Skip,
}

#[derive(Deserialize)]
pub struct RecurBumpReq {
    /// Block id in `<note_id>:<line>` format (matches `ParsedBlock.id`).
    pub block_id: String,
    /// `"complete"` (default): mark done + advance dates + stamp `last_completed::`.
    /// `"skip"`: advance dates only — do not touch `status::` or `last_completed::`.
    #[serde(default)]
    pub mode: RecurBumpMode,
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
        None => {
            return Ok(Json(RecurBumpResp {
                bumped: false,
                next_deadline: None,
            }))
        }
    };

    let note_id = NoteId::new(&note_id_str);
    let note = s
        .store
        .get(&note_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note not found: {}", note_id_str)))?;

    let bump_result = match req.mode {
        RecurBumpMode::Complete => try_bump_block(&note.content, &req.block_id),
        RecurBumpMode::Skip => try_skip_block(&note.content, &req.block_id),
    };
    let Some((new_content, next_iso)) = bump_result else {
        return Ok(Json(RecurBumpResp {
            bumped: false,
            next_deadline: None,
        }));
    };

    let prev_content = note.content.clone();
    let mut updated_note = note.clone();
    updated_note.content = new_content;
    s.store.update(&updated_note).await?;
    let updated = s.store.get(&note_id).await?.ok_or_else(|| {
        AppError::NotFound(format!("Note not found after recur bump: {}", note_id_str))
    })?;
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

// ---------------------------------------------------------------------------
// POST /blocks/set-property — generic single-block property upsert
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct SetBlockPropertyReq {
    /// Block id in `<note_id>:<line>` format (matches `ParsedBlock.id`).
    pub block_id: String,
    /// Property key (e.g. `"status"`, `"scheduled"`, `"recurring"`).
    pub key: String,
    /// New property value (e.g. `"done"`, `"[[2026-06-01]]"`).
    pub value: String,
}

/// Upsert a single `key:: value` property on a block and persist, triggering
/// the same `apply_post_save_bumps` path that a full note PUT does.  This
/// means:
///   - marking a task `status:: done` on a recurring block → the server
///     auto-bumps its deadline to the next occurrence (same as full PUT).
///   - marking a task `status:: done` on a non-recurring block → the block
///     stays done (no bump, nothing to advance).
///   - writing `scheduled:: [[YYYY-MM-DD]]` / `recurring:: <rrule>` works
///     identically to the client side's `upsertBlockProperty`.
///
/// The block_id encodes the note id (`note_id_str:line_num`), so no separate
/// note-id path parameter is needed.
pub async fn set_block_property(
    State(s): State<Arc<AppState>>,
    Json(req): Json<SetBlockPropertyReq>,
) -> AppResult<Json<serde_json::Value>> {
    let (note_id_str, line_str) = match req.block_id.rsplit_once(':') {
        Some(pair) => pair,
        None => {
            return Err(AppError::Validation(format!(
                "invalid block_id '{}': expected '<note_id>:<line>'",
                req.block_id
            )))
        }
    };
    let line_num: usize = line_str.parse().map_err(|_| {
        AppError::Validation(format!(
            "invalid block_id '{}': line suffix is not a number",
            req.block_id
        ))
    })?;

    let key = req.key.trim().to_lowercase();
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(AppError::Validation(format!(
            "invalid property key '{}'",
            req.key
        )));
    }

    let note_id = NoteId::new(note_id_str);
    let note = s
        .store
        .get(&note_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note not found: {}", note_id_str)))?;

    let prev_content = note.content.clone();

    // Locate the block in the body and upsert the property.
    let new_content =
        upsert_block_property_in_note(&prev_content, note_id_str, line_num, &key, &req.value)
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "block '{}' not found in note '{}'",
                    req.block_id, note_id_str
                ))
            })?;

    // Run post-save bumps (handles recurring blocks marked done).
    let (new_content, bumps) = apply_post_save_bumps_with_info(&prev_content, &new_content, note_id_str);
    let (new_content, _unblocked) = apply_dependency_cycles(&prev_content, &new_content, note_id_str);

    let stamped = stamp_block_ids(&new_content);
    let mut updated_note = note.clone();
    updated_note.content = stamped;
    s.store.update(&updated_note).await?;

    let updated = s
        .store
        .get(&note_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note not found after set-property: {}", note_id_str)))?;

    s.index.reindex(&updated).await?;
    {
        use tesela_core::link::extract_wiki_links;
        use tesela_core::traits::link_graph::LinkGraph;
        let links = extract_wiki_links(&updated.content);
        if let Err(e) = s.index.update_links(&note_id, &links).await {
            tracing::warn!("Failed to update links on set-property for {:?}: {}", note_id, e);
        }
    }
    if updated.content != prev_content {
        if let Err(e) = s
            .index
            .record_version(&note_id, Some(&prev_content), &updated.content, 200)
            .await
        {
            tracing::warn!("Failed to record version on set-property: {}", e);
        }
    }

    record_sync_update(&s, &prev_content, &updated).await;
    let _ = s.ws_tx.send(WsEvent::NoteUpdated { note: updated });

    for info in bumps {
        let _ = s.ws_tx.send(WsEvent::RecurringRolled {
            block_id: info.block_id,
            title: info.title,
            note_id: note_id_str.to_string(),
            next_deadline: info.next_deadline,
        });
    }

    tracing::info!("set-property: {}::{} = {}", req.block_id, key, req.value);
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Locate block `line_num` in the note's body and upsert `key:: value` on it.
/// Returns `None` if the block is not found at that line.
///
/// Mirrors the client-side `upsertBlockProperty` logic:
/// - walks continuation lines below the bullet header;
/// - if a matching `key::` line is found, replaces it in place;
/// - otherwise appends a new continuation line at the end of the block.
pub fn upsert_block_property_in_note(
    content: &str,
    note_id_str: &str,
    line_num: usize,
    key: &str,
    value: &str,
) -> Option<String> {
    let (_meta, body) = parse_frontmatter(content).ok()?;
    let blocks = parse_blocks(note_id_str, &body);
    // Confirm the block exists at this line.
    let block_id = format!("{}:{}", note_id_str, line_num);
    let _block = blocks.iter().find(|b| b.id == block_id)?;

    // Perform the upsert directly on the body lines.
    let new_body = upsert_property_in_body(&body, line_num, key, value)?;
    Some(reassemble_content(content, &body, &new_body))
}

/// Upsert `key:: value` on the block whose bullet header is on `bullet_line`
/// in `body`. Returns the rewritten body, or `None` if `bullet_line` is out
/// of bounds or not a bullet.
fn upsert_property_in_body(body: &str, bullet_line: usize, key: &str, value: &str) -> Option<String> {
    let trailing_newline = body.ends_with('\n');
    let (mut lines, end, cont_indent) = block_range(body, bullet_line)?;

    // Walk continuation lines looking for an existing `key::` entry.
    let mut found_idx: Option<usize> = None;
    for i in (bullet_line + 1)..end {
        if let Some((k, _)) = property_kv(&lines[i]) {
            if k == key {
                found_idx = Some(i);
                break;
            }
        }
    }

    if let Some(idx) = found_idx {
        lines[idx] = format!("{}{}:: {}", cont_indent, key, value);
    } else {
        lines.insert(end, format!("{}{}:: {}", cont_indent, key, value));
    }

    Some(join_lines(lines, trailing_newline))
}

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
        Some(ActiveStep { new_deadline, new_scheduled, next_iso }) => {
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
        Some(ActiveStep { new_deadline, new_scheduled, next_iso }) => {
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
fn compute_recurrence_step(block: &tesela_core::block::ParsedBlock) -> Option<RecurrenceStep> {
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
                    Some(recurrence::next_after(&rec, d).format("%Y-%m-%d").to_string())
                })
                .or_else(|| {
                    block.properties.get("scheduled").and_then(|v| {
                        let (d, _) = parse_deadline_value(v)?;
                        Some(recurrence::next_after(&rec, d).format("%Y-%m-%d").to_string())
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
fn block_range(
    body: &str,
    block_line_num: usize,
) -> Option<(Vec<String>, usize, String)> {
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
        additions.push(format!("{}last_completed:: {}", cont_indent, last_completed));
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
fn rewrite_block_for_spent(
    body: &str,
    block_line_num: usize,
    new_done: u32,
) -> Option<String> {
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
        lines.insert(end, format!("{}recurrence_done:: {}", cont_indent, new_done));
    }

    Some(join_lines(lines, trailing_newline))
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

        // Slug resolution per the tag-system spec: if a page already exists
        // at the bare slug, two cases:
        //   (a) it's itself a tag page → reuse, nothing to create.
        //   (b) it's a different kind of page (note, etc.) → auto-number a
        //       disambiguating slug (`fella-2.md`, `fella-3.md`, …) and
        //       create the tag there. The display name is still `fella`.
        let slug_base = tag.to_lowercase();
        let resolved_slug = match resolve_free_tag_slug(s, &slug_base).await {
            Ok(Some(slug)) => slug,
            Ok(None) => continue, // existing tag at this slug — nothing to do
            Err(e) => {
                tracing::warn!("Failed to resolve tag slug '{}': {}", tag, e);
                continue;
            }
        };

        // Auto-create tag page. `type: tag` (lowercase, bare) is the
        // canonical form per the tag-system spec.
        let content = format!(
            "---\ntitle: \"{}\"\ntype: tag\nextends: \"Root Tag\"\ntag_properties: []\nparent: \"\"\ntags: []\n---\n- Tag properties are inherited by all nodes using the tag.\n",
            tag
        );
        match s.store.create(&resolved_slug, &content, &[]).await {
            Ok(tag_note) => {
                let _ = s.index.reindex(&tag_note).await;
                // Sync visibility: peers need a NoteUpsert in the
                // oplog so subsequent BlockUpserts against this
                // page can resolve its slug.
                record_sync_create(s, &tag_note).await;
                let _ = s.ws_tx.send(WsEvent::NoteCreated { note: tag_note });
                tracing::info!("Auto-created tag page at slug '{}' (display name: '{}')", resolved_slug, tag);
            }
            Err(e) => {
                tracing::warn!("Failed to auto-create tag page '{}' at slug '{}': {}", tag, resolved_slug, e);
            }
        }
    }
}

/// Pick the slug to use for a tag page being auto-created.
///
/// Returns:
///   - `Ok(Some(slug))` when a new file should be created at this slug
///     (either the bare slug is free, or we picked `slug-N` after a
///     collision with a non-tag page).
///   - `Ok(None)` when the bare slug already holds a tag page; the caller
///     should reuse it and skip creation.
///   - `Err` on store errors.
async fn resolve_free_tag_slug(s: &Arc<AppState>, slug_base: &str) -> Result<Option<String>, String> {
    let bare = slug_base.to_string();
    let bare_id = NoteId::new(bare.clone());
    match s.store.get(&bare_id).await {
        Ok(Some(existing)) => {
            // If the existing page is itself a tag, reuse.
            let is_tag = existing
                .metadata
                .note_type
                .as_deref()
                .map(|t| t.eq_ignore_ascii_case("tag"))
                .unwrap_or(false);
            if is_tag {
                return Ok(None);
            }
            // Collision with a non-tag page. Walk `slug-2`, `slug-3`, … until
            // we find a free slug. Bounded loop (1000) to avoid wedging on a
            // pathological mosaic; in practice this terminates after one or
            // two attempts.
            for n in 2..1000 {
                let candidate = format!("{}-{}", bare, n);
                match s.store.get(&NoteId::new(candidate.clone())).await {
                    Ok(Some(other)) => {
                        // If this auto-numbered slug already holds a tag for
                        // the same display name, reuse it instead of creating
                        // yet another disambiguator.
                        let is_tag = other
                            .metadata
                            .note_type
                            .as_deref()
                            .map(|t| t.eq_ignore_ascii_case("tag"))
                            .unwrap_or(false);
                        let display_match = other
                            .metadata
                            .title
                            .as_deref()
                            .map(|t| t.eq_ignore_ascii_case(slug_base))
                            .unwrap_or(false);
                        if is_tag && display_match {
                            return Ok(None);
                        }
                    }
                    Ok(None) => return Ok(Some(candidate)),
                    Err(e) => return Err(format!("store.get: {}", e)),
                }
            }
            Err(format!("exhausted slug suffixes for '{}'", slug_base))
        }
        Ok(None) => Ok(Some(bare)),
        Err(e) => Err(format!("store.get: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// Unit tests for the recurrence bump logic (pure functions, no I/O)
// ---------------------------------------------------------------------------

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
        let content_with_done2 =
            content_after_first.replace("status:: todo", "status:: done");

        // Second complete — series is now spent (count 2, done_so_far=1 → advance returns None).
        let (bumped2, _iso) =
            try_bump_block(&content_with_done2, BLOCK_ID).expect("bump returns Some even when spent");

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

        let (skipped, next_iso) =
            try_skip_block(&content, BLOCK_ID).expect("skip should succeed");

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

    // -----------------------------------------------------------------------
    // Tests for upsert_block_property_in_note (set-property endpoint)
    // -----------------------------------------------------------------------

    #[test]
    fn set_property_updates_existing_key_in_place() {
        let content = make_note(&[]);
        // Update the existing `status:: todo` to `status:: done`.
        let new_content =
            upsert_block_property_in_note(&content, "note", 0, "status", "done")
                .expect("should return Some");
        let status = get_prop(&new_content, BLOCK_ID, "status");
        assert_eq!(status.as_deref(), Some("done"));
        // Other properties must survive unchanged.
        assert_eq!(
            get_prop(&new_content, BLOCK_ID, "deadline").as_deref(),
            Some("[[2026-05-07]]")
        );
    }

    #[test]
    fn set_property_appends_new_key_when_absent() {
        let content = make_note(&[]);
        // Append a `priority:: high` property that doesn't exist yet.
        let new_content =
            upsert_block_property_in_note(&content, "note", 0, "priority", "high")
                .expect("should return Some");
        let priority = get_prop(&new_content, BLOCK_ID, "priority");
        assert_eq!(priority.as_deref(), Some("high"));
        // Existing properties must still be intact.
        assert_eq!(
            get_prop(&new_content, BLOCK_ID, "status").as_deref(),
            Some("todo")
        );
    }

    #[test]
    fn set_property_scheduled_updates_correctly() {
        let content = make_note(&[]);
        let new_content =
            upsert_block_property_in_note(&content, "note", 0, "scheduled", "[[2026-06-01]]")
                .expect("should return Some");
        assert_eq!(
            get_prop(&new_content, BLOCK_ID, "scheduled").as_deref(),
            Some("[[2026-06-01]]")
        );
    }

    #[test]
    fn set_property_returns_none_for_invalid_line() {
        let content = make_note(&[]);
        // Line 999 does not exist → should return None.
        let result = upsert_block_property_in_note(&content, "note", 999, "status", "done");
        assert!(result.is_none());
    }
}
