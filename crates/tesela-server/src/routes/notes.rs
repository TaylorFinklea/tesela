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
    lifecycle::{
        apply_dependency_cycles, apply_post_save_bumps_with_info, compute_lifecycle_container_sets,
        property_kv, try_bump_block, try_skip_block, BumpInfo,
    },
    link::{GraphEdge, Link, LinkType},
    note::NoteId,
    note_tree::{parse_note, serialize_note},
    property::{parse_scalar, ValueType},
    storage::markdown::parse_frontmatter,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
    Note,
};
use tesela_sync::{OpPayload, PropOp, PropScalar};

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
    /// The full note body the client last loaded/sent — its edit BASE
    /// (the version it started THIS edit from). Optional for backward
    /// compatibility: older clients omit it. When present, the server
    /// diffs `base_content → content` (the author's REAL changes) instead
    /// of `server_file → content`, so a block the author never touched is
    /// identical base→new = NO op = a concurrent peer edit to that block
    /// survives. When absent, the server falls back to the historical
    /// `server_file → content` diff (no regression). See the base-diff
    /// spec (2026-06-02) and `concurrent_whole_body_clobber.rs`.
    #[serde(default)]
    pub base_content: Option<String>,
}

/// A single block-granular mutation for `POST /notes/{id}/blocks`.
///
/// Block-granular writes (2026-06-02 spec) let a client submit ONLY the
/// block ops it actually changed instead of the whole note body. The
/// whole-body `PUT /notes/{id}` path manufactures stale `BlockUpsert`s
/// from a server-vs-client diff, re-asserting blocks the client never
/// touched and clobbering concurrent peer edits
/// (`concurrent_whole_body_clobber.rs`). Submitting one op per edited
/// block makes that clobber structurally impossible: no op for a block
/// means no re-assertion of its text.
///
/// Each variant maps 1:1 onto an `OpPayload` block op. `bid` is the
/// canonical dashed-UUID block id stamped into the on-disk
/// `<!-- bid:UUID -->` marker (web's `ParsedBlock.bid` IS this value).
/// `parent_bid` maps to `OpPayload`'s `parent_block_id`; `None` = top-level.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockOp {
    /// Create-or-update a block's text + indent. The engine resolves the
    /// bid to an existing node (updating its text/indent in place, never
    /// moving it) or creates a new node. When `after_bid` is present, a
    /// NEW node is inserted immediately AFTER that predecessor block so a
    /// mid-note split's new half lands adjacent to its sibling; absent (or
    /// a predecessor the engine hasn't seen), the new node appends at
    /// document end (the historical behavior). `order_key` is still
    /// ignored for placement.
    Upsert {
        bid: String,
        text: String,
        #[serde(default)]
        parent_bid: Option<String>,
        indent_level: u16,
        /// Predecessor block id: insert a NEW block immediately after this
        /// one. `None` = append at document end (backward compatible).
        #[serde(default)]
        after_bid: Option<String>,
    },
    /// Recompute a block's parent/indent. `BlockMove` only recomputes
    /// indent/parent today (never reorders rows); indent/outdent is safe,
    /// true row-reorder is a deferred follow-up.
    Move {
        bid: String,
        #[serde(default)]
        parent_bid: Option<String>,
        /// Accepted on the wire per the spec, but the engine recomputes a
        /// moved block's indent from its new parent's indent (see the
        /// `BlockMove` apply); `parent_bid` carries the structure. Kept so
        /// the client request shape is stable if the engine later honors it.
        #[allow(dead_code)]
        indent_level: u16,
    },
    /// Delete a block by id.
    Delete { bid: String },
}

#[derive(Deserialize)]
pub struct UpsertBlocksReq {
    pub ops: Vec<BlockOp>,
}

/// Defensive upper bound on `?limit=` — prevents an accidental/malicious
/// huge request from building an unbounded response `Vec`. Well above any
/// realistic mosaic size today.
const MAX_LIST_LIMIT: usize = 10_000;

/// `GET /notes` — paginated note listing. Callers that omit `limit` still
/// get the historical default of 100, but every response now carries an
/// `X-Total-Count` header with the full count of notes matching `tag`
/// (before pagination), so a truncated page is always detectable instead of
/// silently dropping notes (tesela-sclr.1: the palette's `limit: 500` was
/// silently unfindable past note #500 with no signal anywhere).
pub async fn list_notes(
    Query(q): Query<ListQuery>,
    State(s): State<Arc<AppState>>,
) -> AppResult<impl IntoResponse> {
    let limit = q.limit.unwrap_or(100).min(MAX_LIST_LIMIT);
    let offset = q.offset.unwrap_or(0);
    // Fetch the full matching set once (the store already walks the whole
    // corpus regardless of `limit` — see `FsNoteStore::list`), so paginating
    // here is free and gives us an exact total for the header.
    let matching = s.store.list(q.tag.as_deref(), usize::MAX, 0).await?;
    let total = matching.len();
    let notes: Vec<Note> = matching.into_iter().skip(offset).take(limit).collect();
    Ok((
        [("x-total-count", total.to_string())],
        Json(notes),
    ))
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
        // Bootstrap-before-author (daily-garble fix, 2026-06-29): on the FIRST
        // create of this daily, if the engine does NOT yet hold its doc, pull
        // the relay's authoritative snapshot as a shared base BEFORE
        // `record_sync_create` authors the NoteUpsert below — so a daily that
        // already exists on the relay (authored from another device) is adopted
        // onto the server's lineage instead of forked into a disjoint twin that
        // later clobbers. Best-effort + deadlock-safe; no-op once resident.
        // Mirrors iOS `bootstrapNoteIfNeeded`.
        bootstrap_note_if_needed(&s, note.id.as_str()).await;
        record_sync_create(&s, &note).await?;
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
    // Bootstrap-before-author (convergence fix M1, 2026-06-29): the CREATE path
    // authors via `record_sync_create`, which does NOT bootstrap internally — it
    // record_local → doc_for_note_mut → mints a fresh empty doc with THIS
    // device's peer. So a slug that already exists on the relay (created on
    // another device) but isn't resident here would fork a DISJOINT Loro lineage
    // → same-bid twins / garble on sync. Adopt the relay's authoritative
    // snapshot as a shared base FIRST. No-op once resident / absent on the
    // relay. Mirrors iOS `bootstrapNoteIfNeeded`.
    bootstrap_note_if_needed(&s, note.id.as_str()).await;
    record_sync_create(&s, &note).await?;
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
    // `_bumps` here is the UNGUARDED bump info (this markdown rewrite has no
    // idempotence guard). It is intentionally discarded for WS notification —
    // the `RecurringRolled` event is fired below from the GUARDED
    // `persist_lifecycle_rolls` result so the event matches the roll actually
    // persisted to the container (a guard-tripped re-completion authors nothing
    // and must emit nothing). `new_content` — the rolled markdown — is still
    // used: it persists via `record_sync_update` and renders identically to the
    // container roll for a not-yet-container-resident block.
    let (new_content, _bumps) = apply_post_save_bumps_with_info(&prev_content, &req.content, &id);
    // Phase 12.4 — same-note dependency unblock: if a block's blocker just
    // flipped to done and the block is currently `backlog`, advance it to
    // `todo`. Cross-note dependencies are out of v1 scope; users can
    // manually unblock or wait for the dependent's own save to re-evaluate.
    let (new_content, unblocked) = apply_dependency_cycles(&prev_content, &new_content, &id);
    // Phase 2.2 (2026-05-27): no longer auto-prune blank blocks here
    // either. Both clients preserve blanks consistently.
    let stamped_new = stamp_block_ids(&new_content);
    note.content = stamped_new;
    // Bootstrap-before-author (daily-garble fix, 2026-06-29): if the engine
    // does NOT yet hold this note's doc, pull the relay's authoritative
    // snapshot as a shared base BEFORE `record_sync_update` authors below, so
    // the diff ops resolve onto the server's existing tree nodes instead of
    // forking a disjoint lineage that later clobbers. Best-effort + deadlock-
    // safe; no-op once the doc is resident. Runs before the `pre_vv` capture so
    // the live-WS delta exports only the author's edit over the bootstrapped
    // base. Mirrors iOS `bootstrapNoteIfNeeded`.
    bootstrap_note_if_needed(&s, note.id.as_str()).await;
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
    // Base-diff (2026-06-02): when the client sends its edit BASE (the
    // body it started this edit from), diff base→new so we emit ops ONLY
    // for blocks the AUTHOR actually changed. A block untouched by the
    // author is identical base→new = no op = a concurrent peer edit to it
    // survives (the engine already holds the peer's edit; Loro merges).
    // Stamp the base the same way `content` is stamped so block ids line
    // up across the diff. `None` (older client / true whole-note rewrite
    // like create) falls back to the historical server-file→new diff.
    let stamped_base = req.base_content.as_deref().map(stamp_block_ids);
    record_sync_update(&s, &prev_content, stamped_base.as_deref(), &note).await?;
    // Persist the lifecycle roll (recurrence bump + same-note dependency
    // unblock) as CONTAINER property sets so it lands authoritatively even when
    // the block's lifecycle keys are already container-resident — the engine
    // hook makes them so on any relay/WS-delivered completion (tesela-ows.1
    // step 2, Lead constraint (a); parity with `set_block_property` + the
    // engine hook). Without this, `record_sync_update` above persists the roll
    // ONLY as in-text markdown, which render-time dedup shadows under the stale
    // container values — the completion is a silent no-op on a container-
    // resident block. The rolled-markdown persistence above stays: it renders
    // identically for a not-yet-container-resident block and agrees with these
    // sets, which are the single authoritative writer of lifecycle state. The
    // roll is computed against `req.content` (the pre-roll client body), the
    // same input the WS `bumps` above were derived from. `delta_note_id` is the
    // note's doc id (== `stable_uuid_from_slug`).
    let rolled_bumps =
        persist_lifecycle_rolls(&s, delta_note_id, &id, &prev_content, &req.content).await;
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
    // Phase 12.3 — fire RecurringRolled per bumped block so the client can
    // surface "rolled to next month" notifications. Fired from the GUARDED
    // `persist_lifecycle_rolls` result (round 3, t5): a guard-tripped
    // re-completion authors nothing to the container, so it must also emit no
    // WS event — the event set now matches the persisted roll exactly (parity
    // with `set_block_property`, which already fires from this same guarded set).
    for info in rolled_bumps {
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

/// `POST /notes/{id}/blocks` — block-granular write. The client submits
/// ONLY the block ops it actually changed (`UpsertBlocksReq.ops`), each
/// of which maps 1:1 onto an engine `OpPayload` block op recorded via
/// `record_local`. This is the structural fix for the concurrent-edit
/// CLOBBER (`concurrent_whole_body_clobber.rs`): the whole-body
/// `PUT /notes/{id}` path diffs the server-authoritative file against the
/// client's (possibly stale) full body and emits a `BlockUpsert`
/// re-asserting the stale text of blocks the client never touched,
/// clobbering concurrent peer edits. Here, a block with no submitted op
/// is simply never re-asserted, so a concurrent peer edit to it survives.
///
/// Mirrors `update_note`'s post-write tail verbatim (re-read → reindex →
/// update_links → record_version → ensure_tag_pages → `WsEvent::NoteUpdated`
/// → cursor-free WS delta) so peers converge identically to the PUT path.
/// The note must already exist (404 otherwise); brand-new notes are
/// created via the existing `POST /notes` create path first — see the
/// spec's brand-new-note risk. No `NoteUpsert` is seeded here.
pub async fn upsert_blocks(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
    Json(req): Json<UpsertBlocksReq>,
) -> AppResult<Json<Note>> {
    let note_id = NoteId::new(&id);
    // Require the note to exist (mirror update_note). Brand-new notes
    // must be materialized via the create path first; a block-granular
    // op against an absent doc would not materialize a `<slug>.md`.
    let note = s
        .store
        .get(&note_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note not found: {}", id)))?;
    let prev_content = note.content.clone();

    // Bootstrap-before-author (daily-garble fix, 2026-06-29): if the engine
    // does NOT yet hold this note's doc, pull the relay's authoritative
    // snapshot as a shared base BEFORE the first `record_local` below, so the
    // block ops resolve onto the server's existing tree nodes instead of
    // forking a disjoint lineage that later clobbers. Best-effort + deadlock-
    // safe; no-op once the doc is resident. Mirrors iOS `bootstrapNoteIfNeeded`.
    bootstrap_note_if_needed(&s, note.id.as_str()).await;

    // Address the note's Loro doc exactly as the PUT path + the
    // producer paths (`record_sync_create`/`record_sync_update`) do:
    // blake3-truncate the slug. This is the note-id space every
    // `OpPayload` block op carries, so the ops land on the same doc the
    // file materializes from.
    let delta_note_id = stable_uuid_from_slug(note.id.as_str());
    // Instant-multidevice (Phase A): capture this note's Loro version
    // BEFORE applying the ops so we can export the cursor-free delta for
    // just-these-changes afterward. `None` when the doc isn't resident.
    let pre_vv = s.sync_engine.doc_version(delta_note_id).await;

    // Map each request op to an `OpPayload` and record it locally. Each
    // `record_local` materializes the file via the engine's apply path
    // (the engine ignores `order_key` for placement — it appends new
    // blocks at document end and updates existing ones in place). The
    // batch is NOT transactional: a mid-batch failure leaves a partial
    // apply already materialized + broadcast (acceptable v1).
    for op in req.ops {
        let payload = match op {
            BlockOp::Upsert {
                bid,
                text,
                parent_bid,
                indent_level,
                after_bid,
            } => OpPayload::BlockUpsert {
                block_id: parse_bid(&bid)?,
                note_id: delta_note_id,
                parent_block_id: parse_opt_bid(parent_bid.as_deref())?,
                // The engine ignores `order_key` for placement; pass the
                // same benign zero key the diff path emits for a first
                // sibling. Positioning of a NEW block is carried by
                // `after_block_id` (the predecessor hint); existing blocks
                // update in place.
                order_key: "00000000".to_string(),
                indent_level,
                text,
                // Positional-insert hint: place a new block immediately
                // after this predecessor. `None` → append (backward compat).
                after_block_id: parse_opt_bid(after_bid.as_deref())?,
            },
            BlockOp::Move {
                bid,
                parent_bid,
                // `BlockMove` carries no indent field — the engine
                // recomputes indent from the new parent's indent
                // (parent.indent + 1, or 0 for top-level). The request's
                // `indent_level` is the client's intent; `parent_bid` is
                // what actually carries the structure.
                indent_level: _,
            } => OpPayload::BlockMove {
                block_id: parse_bid(&bid)?,
                new_parent: parse_opt_bid(parent_bid.as_deref())?,
                new_order_key: "00000000".to_string(),
            },
            BlockOp::Delete { bid } => OpPayload::BlockDelete {
                block_id: parse_bid(&bid)?,
            },
        };
        if let Err(e) = s.sync_engine.record_local(payload).await {
            tracing::warn!("sync: record_local block op failed for {id}: {e}");
            return Err(AppError::Internal(anyhow::anyhow!(
                "Failed to record block op: {e}"
            )));
        }
    }

    // Re-read to get fresh parsed metadata + checksum from the file the
    // engine just wrote (the engine's serialization is canonical).
    let updated =
        s.store.get(&note_id).await?.ok_or_else(|| {
            AppError::NotFound(format!("Note not found after block write: {}", id))
        })?;
    s.index.reindex(&updated).await?;
    // Refresh the link graph for this note (same as the PUT path).
    {
        use tesela_core::link::extract_wiki_links;
        use tesela_core::traits::link_graph::LinkGraph;
        let links = extract_wiki_links(&updated.content);
        if let Err(e) = s.index.update_links(&note_id, &links).await {
            tracing::warn!(
                "Failed to update links on block write for {:?}: {}",
                note_id,
                e
            );
        }
    }
    // Append a version row. Best-effort; cap at 200.
    if updated.content != prev_content {
        if let Err(e) = s
            .index
            .record_version(&note_id, Some(&prev_content), &updated.content, 200)
            .await
        {
            tracing::warn!("Failed to record note version on block write: {}", e);
        }
    }
    // Parity with the PUT path: new `#tags` still spawn tag pages.
    ensure_tag_pages(&s, &updated).await;
    let _ = s.ws_tx.send(WsEvent::NoteUpdated {
        note: updated.clone(),
    });
    // Instant-multidevice (Phase A): export the cursor-free delta for the
    // ops just applied and push it to live WS clients as a binary frame,
    // so peer devices converge in <1s. `origin: None` — an HTTP edit fans
    // out to every connected socket. Best-effort.
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
    Ok(Json(updated))
}

/// Parse a dashed-UUID block id string into the 16-byte form `OpPayload`
/// block ops carry. Mirrors `delete_block`'s `uuid::Uuid::parse_str`.
fn parse_bid(bid: &str) -> AppResult<[u8; 16]> {
    uuid::Uuid::parse_str(bid)
        .map(|u| *u.as_bytes())
        .map_err(|_| AppError::Validation(format!("Invalid block id: {bid}")))
}

fn parse_opt_bid(bid: Option<&str>) -> AppResult<Option<[u8; 16]>> {
    match bid {
        Some(b) => Ok(Some(parse_bid(b)?)),
        None => Ok(None),
    }
}

pub async fn delete_note(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<StatusCode> {
    let note_id = NoteId::new(&id);
    s.store.delete(&note_id).await?;
    s.index.remove(&note_id).await?;
    record_sync_delete(&s, &note_id).await?;
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
    // Author the delete on the relay's authoritative lineage if this note
    // isn't resident yet — otherwise the tombstone lands on a fresh disjoint
    // doc and clobbers/diverges on the next relay merge (same class as the
    // text garble). Best-effort + resident-gated. Mirrors iOS bootstrapNoteIfNeeded.
    bootstrap_note_if_needed(&s, &id).await;
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
    // Bootstrap-before-author (convergence fix M1, 2026-06-29): this tag-page
    // slug can already exist on the relay (authored on another device); adopt
    // its lineage as a shared base before `record_sync_create` mints a fresh
    // disjoint doc, else they later union into same-bid twins. No-op once
    // resident / absent on the relay.
    bootstrap_note_if_needed(s, created.id.as_str()).await;
    record_sync_create(s, &created).await?;
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
        // Bootstrap-before-author (tesela-1fe, 2026-07-02): a note this bulk
        // rewrite touches may be non-resident (evicted, or a `.md`-without-
        // `.bin` import) — a fresh `record_sync_update` would then fork a
        // disjoint Loro lineage instead of diffing onto the note's shared
        // base. Adopt the relay's authoritative snapshot first, as the
        // create paths above do. No-op once resident / absent on the relay.
        bootstrap_note_if_needed(&s, note.id.as_str()).await;
        // Server-internal rewrite: `note.content` IS the base (no stale
        // client view), so the historical prev→new diff is correct. Pass
        // `None` to keep that path exactly.
        record_sync_update(&s, &note.content, None, &updated_note).await?;
        let _ = s.ws_tx.send(WsEvent::NoteUpdated { note: updated_note });
    }

    // Now move the source tag's own file.
    let renamed = s.store.create(&req.to_slug, &source.content, &[]).await?;
    s.index.reindex(&renamed).await?;
    // Bootstrap-before-author (convergence fix M1, 2026-06-29): the rename
    // target slug was verified absent LOCALLY (above), but it may already exist
    // on the relay (created on another device); adopt that lineage as a shared
    // base before `record_sync_create` mints a fresh disjoint doc, else they
    // later union into same-bid twins. No-op once resident / absent on the relay.
    bootstrap_note_if_needed(&s, renamed.id.as_str()).await;
    record_sync_create(&s, &renamed).await?;
    let _ = s.ws_tx.send(WsEvent::NoteCreated {
        note: renamed.clone(),
    });

    s.store.delete(&from_id).await?;
    s.index.remove(&from_id).await?;
    record_sync_delete(&s, &from_id).await?;
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

    use tesela_core::tag_rewrite::{clear_parent_frontmatter, strip_inline_tag, strip_wiki_link};

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
        // Bootstrap-before-author (tesela-1fe, 2026-07-02): mirrors the
        // `rename_tag` rewrite loop above — a touched note may be
        // non-resident, so adopt the relay's authoritative snapshot as a
        // shared base before diffing, else this admin op forks a disjoint
        // lineage. No-op once resident / absent on the relay.
        bootstrap_note_if_needed(&s, note.id.as_str()).await;
        // Server-internal rewrite: `note.content` IS the base. Pass `None`
        // to keep the historical prev→new diff (see the tag-rename twin).
        record_sync_update(&s, &note.content, None, &updated_note).await?;
        let _ = s.ws_tx.send(WsEvent::NoteUpdated { note: updated_note });
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
///
/// Audit A9a (2026-06-09): a `record_local` failure here used to be
/// warn-and-swallowed, so the handler returned 2xx while the sync op was
/// silently dropped — peers never saw the note and the engine (the
/// authoritative materializing writer) could later revert the file.
/// Callers must propagate this error as a 5xx. The file write is NOT
/// rolled back; the error detail says so.
async fn record_sync_create(s: &Arc<AppState>, note: &Note) -> anyhow::Result<()> {
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
        anyhow::bail!(
            "note '{}' was written to disk, but its sync op could not be \
             recorded ({e}); the note will not sync to peers and may be \
             reverted by the engine — retry the save",
            note.id
        );
    }
    Ok(())
}

/// Producer path for note updates. Emits BlockUpsert / BlockMove /
/// BlockDelete ops describing what the author actually changed. Avoids
/// emitting a NoteUpsert when blocks changed: when two peers edit
/// different blocks of the same note concurrently, NoteUpsert's
/// last-writer-wins on the whole blob would stomp the loser's edit,
/// whereas block-level ops converge correctly per [[plan/block-level-sync.md]].
///
/// ## Base-diff (2026-06-02) — the diff baseline
/// When `base_content` is `Some`, it is the AUTHOR's edit base (the body
/// the client started this edit from). The diff is `base → new`, so the
/// ops are ONLY the blocks the author truly changed. A block the author
/// never touched is identical base→new = no op = a concurrent peer edit
/// to it is never re-asserted = it survives. This closes the last
/// concurrent-edit data-loss vector (`concurrent_whole_body_clobber.rs`):
/// the historical `server_file → new` diff would see an untouched block's
/// text differ from a peer's NEWER server text and emit a stale
/// re-assertion. When `base_content` is `None` (older client, or a true
/// server-internal rewrite where `prev_content` IS the base), the diff
/// falls back to `prev_content → new` exactly as before — no regression.
///
/// ## Frontmatter-only fallback (the subtle clobber)
/// When the block diff is empty but the raw content changed (a
/// frontmatter / page-property / title-only edit), we fall back to a
/// NoteUpsert. Historically the engine's NoteUpsert apply destructively
/// RESEEDED the block tree from `content` whenever the body drifted from
/// the live tree, so a STALE frontmatter-only PUT carried the author's
/// stale body and reseeded the tree OVER a peer's concurrent block edit —
/// a whole-body clobber in disguise (spec invariant 2). Since 2026-06-10
/// the engine's NoteUpsert apply is a NON-destructive per-bid reconcile
/// (`loro_engine::reconcile_tree_to_blocks`: in-place text heals, no
/// removal of absent blocks, deleted-wins on tombstoned bids), so the
/// reseed clobber is closed engine-side. The BODY-PRESERVING NoteUpsert
/// below (author's NEW frontmatter + the SERVER's CURRENT blocks) is kept
/// as defense-in-depth: it also avoids needless in-place text rewrites of
/// blocks the author didn't touch. Without a base (legacy client) we keep
/// the historical full-content NoteUpsert.
///
/// ## Bundled frontmatter + block change (block ops are NON-empty)
/// A single PUT can change BOTH a block AND the frontmatter/page-properties.
/// The block diff only touches block-tree nodes; frontmatter reaches the doc
/// ONLY via a NoteUpsert apply. So after applying the block ops we ALSO
/// detect whether the frontmatter or page-properties changed (independent of
/// the block ops) and, if so, emit a body-preserving NoteUpsert — applied
/// AFTER the block ops so it carries the author's NEW frontmatter over the
/// server's POST-block-op blocks (no reseed). Both edits survive. Guarded on
/// the change check so a pure block edit never emits a redundant NoteUpsert.
///
/// ## Error propagation (audit A9a, 2026-06-09)
/// `record_sync_update` is the SOLE writer on the PUT path — each
/// `record_local` both appends the op and materializes the file. A
/// failure used to be warn-and-swallowed, so the client got a 200 while
/// its edit was silently dropped (never applied, never synced). Failures
/// now propagate so the handler answers 5xx and the client retries. The
/// op loop fails fast on the first error; a mid-loop failure leaves a
/// partial apply already materialized (same non-transactional contract
/// as `upsert_blocks`) — the error detail says so.
async fn record_sync_update(
    s: &Arc<AppState>,
    prev_content: &str,
    base_content: Option<&str>,
    note: &Note,
) -> anyhow::Result<()> {
    let note_id = stable_uuid_from_slug(note.id.as_str());
    // Base-diff: when the author sent its edit base, diff base→new (the
    // author's real changes). Otherwise diff prev_content→new (today's
    // behavior; `prev_content` IS the base for server-internal rewrites).
    let diff_base = base_content.unwrap_or(prev_content);
    let old_tree = parse_note(diff_base);
    let new_tree = parse_note(&note.content);
    // Phase 2.2 (sync redesign 2026-05-27): suppress inferred
    // `BlockDelete` emission. With a trustworthy author base, "present in
    // base, absent in new" IS a genuine delete — but per the spec we keep
    // `emit_deletes:false` for v1 and route user-intent deletes through
    // the explicit `DELETE /notes/<id>/blocks/<bid>` endpoint, staying
    // consistent with the block-ops delete path. The original rationale:
    // the server diffs Mac's authoritative file against the client's PUT
    // body, but a stale client view (typed locally while a peer's edit
    // landed via WS but hasn't merged into local state) would make
    // "absent from PUT body" look like a delete and stomp the peer's edit.
    let ops = tesela_sync::diff::diff_note_trees_with_options(
        note_id,
        &old_tree,
        &new_tree,
        tesela_sync::diff::DiffOptions {
            emit_deletes: false,
        },
    );

    if ops.is_empty() {
        if prev_content == note.content {
            return Ok(());
        }
        // Body parses identical (or both empty) but raw content differs:
        // frontmatter / page-property / non-bullet content changed. Fall
        // back to NoteUpsert. With a base present, make it body-preserving
        // so a stale frontmatter-only edit can't reseed the block tree
        // over a peer's concurrent block edit (spec invariant 2). Without
        // a base, keep the historical full-content NoteUpsert.
        let content = match base_content {
            Some(_) => body_preserving_noteupsert_content(s, note_id, &note.content).await,
            None => note.content.clone(),
        };
        let payload = OpPayload::NoteUpsert {
            note_id,
            display_alias: Some(note.id.as_str().to_string()),
            title: note.title.clone(),
            content,
            created_at_millis: note.created_at.timestamp_millis(),
        };
        if let Err(e) = s.sync_engine.record_local(payload).await {
            tracing::warn!(
                "sync: record_local NoteUpsert fallback failed for {}: {}",
                note.id,
                e
            );
            anyhow::bail!(
                "the frontmatter/property edit to note '{}' could not be \
                 recorded by the sync engine ({e}); the edit was NOT applied \
                 — retry the save",
                note.id
            );
        }
        return Ok(());
    }

    for op in ops {
        if let Err(e) = s.sync_engine.record_local(op).await {
            tracing::warn!("sync: record_local Block op failed for {}: {}", note.id, e);
            anyhow::bail!(
                "a block edit to note '{}' could not be recorded by the sync \
                 engine ({e}); the save may be partially applied and will not \
                 sync to peers — retry the save",
                note.id
            );
        }
    }

    // A single PUT can change BOTH a block AND the frontmatter /
    // page-properties. The block diff above only touches block-tree nodes;
    // frontmatter + page-properties reach the doc ONLY via a NoteUpsert
    // apply. Historically the non-empty-ops branch returned here, silently
    // DROPPING any frontmatter/page-property change bundled in the same PUT.
    // Detect that change (independent of the block ops) and, if present,
    // emit a body-preserving NoteUpsert so the frontmatter lands WITHOUT
    // reseeding the blocks we just applied. Order matters: the block ops are
    // applied FIRST (above), so the engine's current body already includes
    // them; `body_preserving_noteupsert_content` then reads that updated
    // body via `render_note`, so `tree_matches_blocks` stays true and the
    // NoteUpsert carries the author's NEW frontmatter over the SERVER's
    // post-block-op blocks — no reseed, both edits survive. Guard on the
    // change check so a pure block edit never emits a redundant NoteUpsert.
    if old_tree.frontmatter != new_tree.frontmatter
        || old_tree.page_properties != new_tree.page_properties
    {
        let content = body_preserving_noteupsert_content(s, note_id, &note.content).await;
        let payload = OpPayload::NoteUpsert {
            note_id,
            display_alias: Some(note.id.as_str().to_string()),
            title: note.title.clone(),
            content,
            created_at_millis: note.created_at.timestamp_millis(),
        };
        if let Err(e) = s.sync_engine.record_local(payload).await {
            tracing::warn!(
                "sync: record_local NoteUpsert (bundled frontmatter) failed for {}: {}",
                note.id,
                e
            );
            anyhow::bail!(
                "the bundled frontmatter change to note '{}' could not be \
                 recorded by the sync engine ({e}); the block edits applied \
                 but the frontmatter change did not — retry the save",
                note.id
            );
        }
    }
    Ok(())
}

/// Build a NoteUpsert `content` that carries the author's NEW frontmatter
/// and page-properties but the SERVER's CURRENT block body, so the engine's
/// NoteUpsert apply does NOT reseed the block tree (its
/// `tree_matches_blocks` fast path stays true) and a concurrent peer block
/// edit survives. Used on the base-aware frontmatter-only path.
///
/// Falls back to the author's `new_content` verbatim when the engine has
/// no resident body for this note (e.g. a note not yet seeded), which is
/// the safe default — there is no peer body to preserve.
async fn body_preserving_noteupsert_content(
    s: &Arc<AppState>,
    note_id: [u8; 16],
    new_content: &str,
) -> String {
    let new_tree = parse_note(new_content);
    match s.sync_engine.render_note(note_id).await {
        Some(server_body) => {
            // `render_note` is body-only (blocks, no frontmatter); its
            // blocks are the engine's CURRENT tree (peer edits included).
            let server_tree = parse_note(&server_body);
            let merged = tesela_core::note_tree::NoteTree {
                frontmatter: new_tree.frontmatter,
                page_properties: new_tree.page_properties,
                blocks: server_tree.blocks,
                stamped_any: false,
            };
            serialize_note(&merged)
        }
        None => new_content.to_string(),
    }
}

/// Persist a lifecycle roll — the idempotence-GUARDED recurrence bump plus the
/// same-note dependency unblock a `done` flip implies — as CONTAINER
/// `BlockPropertySet` ops, the SAME single mechanism the engine hook
/// ([`tesela_sync`]'s `apply_block_lifecycle`) uses (tesela-ows.1 step 2, Lead
/// constraint (a)). Both HTTP writers (`update_note` PUT, `set_block_property`)
/// route their roll through here so all THREE writers persist lifecycle state
/// through ONE path: the typed props container, where disjoint-twin heal's
/// per-key union protects it and render-time dedup makes it win.
///
/// Never rewrites markdown, never CLEARS the container. The predecessor code
/// cleared only the single key it was invoked with and wrote the rest of the
/// roll as in-text markdown; once the engine hook had made all lifecycle keys
/// container-resident, that left the sibling keys
/// (`deadline`/`recurrence_done`/`last_completed`) rendering their STALE
/// container values while only `status` updated (and, on the PUT path, the
/// whole completion silently shadowed). Routing every rolled key through a
/// container set makes the roll authoritative regardless of prior residency.
///
/// `prev_md` is the note's markdown BEFORE the completion; `next_md` is the
/// post-completion markdown the roll is computed against (the re-materialized
/// view for `set_block_property`, the client's PUT body for `update_note`).
/// Returns one [`BumpInfo`] per recurrence roll for the caller's
/// `WsEvent::RecurringRolled`; a pure dependency unblock carries no
/// `next_deadline` and emits no event.
async fn persist_lifecycle_rolls(
    s: &Arc<AppState>,
    doc_note_id: [u8; 16],
    note_id_str: &str,
    prev_md: &str,
    next_md: &str,
) -> Vec<BumpInfo> {
    let rolls = compute_lifecycle_container_sets(prev_md, next_md, note_id_str);
    let mut bumps = Vec::new();
    for roll in rolls {
        // Address the container node by the block's canonical bid (parity with
        // the engine hook). An unstamped block can't be addressed by bid — skip
        // it; a resident note's blocks are always stamped.
        let Some(block_id) = roll
            .bid
            .as_deref()
            .and_then(|b| uuid::Uuid::parse_str(b).ok())
            .map(|u| *u.as_bytes())
        else {
            continue;
        };
        for (key, value) in &roll.props {
            // Representation alignment (tesela-ows.1 step 2, round 3): author the
            // rolled key through the SAME registry-aware chooser `set_block_property`
            // uses, so a completion's route write and its recurrence roll agree on
            // the key's representation (free-text → `SetText`, else `SetScalar`).
            // The predecessor hard-coded `SetScalar`, which flipped a Text-typed
            // key (e.g. an unregistered `status`, which `set_block_property` writes
            // as free text) scalar<->text on every completion — the collision that
            // 500'd a second completion. The engine's write layer still tolerates
            // any residual mix in a live doc.
            //
            // A lifecycle roll only SETS/advances state, never removes it
            // (`BlockLifecycleRoll`: "never a removal"), and the Lead constraint
            // forbids a `PropOp::Clear` of a lifecycle key (a Clear would evict it
            // from the container and forfeit twin-heal protection). `Clear` only
            // appears in the multi-value branch, which a single-scalar lifecycle
            // key resolves to only under a pathological registration; skip it so
            // the key stays container-resident.
            for op in prop_ops_for_set(s, key, value).await {
                if matches!(op, PropOp::Clear) {
                    continue;
                }
                let payload = OpPayload::BlockPropertySet {
                    note_id: doc_note_id,
                    block_id,
                    key: key.clone(),
                    value: op,
                };
                if let Err(e) = s.sync_engine.record_local(payload).await {
                    tracing::warn!(
                        "sync: record_local lifecycle roll {}::{} failed for {}: {e}",
                        note_id_str,
                        key,
                        roll.block_id
                    );
                }
            }
        }
        if let Some(next_deadline) = roll.next_deadline {
            bumps.push(BumpInfo {
                block_id: roll.block_id,
                title: roll.title,
                next_deadline,
            });
        }
    }
    bumps
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

/// Producer path for note deletion. See `record_sync_create` for the
/// A9a error-propagation contract: a swallowed failure here meant the
/// delete never reached peers (the note resurrects) while the client
/// got a 204.
async fn record_sync_delete(s: &Arc<AppState>, note_id: &NoteId) -> anyhow::Result<()> {
    let slug = note_id.as_str();
    let payload = OpPayload::NoteDelete {
        note_id: stable_uuid_from_slug(slug),
        display_alias: Some(slug.to_string()),
    };
    if let Err(e) = s.sync_engine.record_local(payload).await {
        tracing::warn!("sync: record_local delete failed for {}: {}", note_id, e);
        anyhow::bail!(
            "note '{}' was deleted on disk, but the delete sync op could not \
             be recorded ({e}); peers will not see the deletion",
            note_id
        );
    }
    Ok(())
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

/// Desktop bootstrap-before-author — the piece that CLOSES the daily garble
/// (2026-06-29). Mirror of iOS `RelayTicker.bootstrapNoteIfNeeded`.
///
/// Before the server authors the FIRST local op for a note its engine does
/// NOT yet hold (non-resident), best-effort pull the relay's AUTHORITATIVE
/// snapshot for that note and import it as a SHARED BASE. Without this, a
/// fresh server `record_local` mints a brand-new DISJOINT Loro lineage; when
/// the relay's authoritative version of the same bids later arrives the two
/// lineages union into same-bid twins / clobber (the garbled daily). With the
/// base resident first, the subsequent ops resolve onto the server's existing
/// tree nodes and concurrent edits MERGE.
///
/// HARD CONSTRAINTS (all enforced here):
/// 1. **Deadlock-safe.** The relay `tick` holds `handle.state.write()` while
///    it runs (sync_relay.rs). This request-path bootstrap touches ONLY the
///    engine + the `RelayClient` (`fetch_snapshots`); it NEVER acquires
///    `handle.state.write()`, so it can never re-enter / deadlock against an
///    in-flight tick.
/// 2. **Best-effort.** No relay configured / relay offline / note absent on
///    the relay / fetch fails → silent return; the caller proceeds to author
///    fresh. That's correct for a true first-create — two devices minting the
///    same slug get distinct random bids that stay separate.
/// 3. **Resident-gate.** Only bootstraps a NON-resident doc
///    (`doc_version == None`). An already-resident note already holds its
///    base; re-importing would be wasted work (and is skipped).
/// 4. **Non-destructive.** `import_authoritative_snapshot` is a server-wins
///    re-base that MERGES; any local un-broadcast edits survive (a
///    non-resident note has none yet anyway).
async fn bootstrap_note_if_needed(s: &Arc<AppState>, slug: &str) {
    let Some(relay) = s.relay.as_ref() else {
        return; // LAN-only / no relay configured — nothing to bootstrap from.
    };
    let note_id = stable_uuid_from_slug(slug);
    // Resident-gate: an already-held doc already carries its shared base.
    if s.sync_engine.doc_version(note_id).await.is_some() {
        return;
    }
    // Best-effort fetch of the relay's deposited snapshots. Reads through the
    // `RelayClient` only — NOT `handle.state` — so it can never deadlock
    // against the relay tick's `handle.state.write()` (constraint 1).
    let snaps = match relay.client.fetch_snapshots().await {
        Ok((_watermark, snaps)) => snaps,
        Err(e) => {
            tracing::debug!(
                "bootstrap: fetch_snapshots for {slug} failed ({e}); authoring fresh"
            );
            return;
        }
    };
    // Import THIS note's authoritative snapshot (stream_id == note_id) as the
    // shared base before the caller authors. Absent on the relay → fall
    // through (true first-create).
    for (stream_id, _seq, plaintext) in snaps {
        if stream_id.as_slice() != note_id.as_slice() {
            continue;
        }
        if let Err(e) = s
            .sync_engine
            .import_authoritative_snapshot(note_id, &plaintext)
            .await
        {
            tracing::warn!(
                "bootstrap: import authoritative snapshot for {slug} failed ({e}); \
                 authoring fresh"
            );
        }
        return;
    }
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

/// Minimum needle length (title/alias) considered for unlinked-mention
/// scanning — guards against matching short common words (e.g. "a", "on").
const UNLINKED_MIN_NEEDLE_LEN: usize = 4;

/// Pure scan of a single source note's `content` for plain-text mentions of
/// any of `needles` (already lowercased). Skips matches inside fenced code
/// blocks (` ``` `) and lines that already carry a `[[wiki-link]]` to one of
/// the needles.
///
/// Extracted from `get_unlinked` so the matching logic is unit-testable
/// without spinning up an `AppState`.
fn find_unlinked_mentions(content: &str, source_id: &str, needles: &[String]) -> Vec<Link> {
    let mut out: Vec<Link> = Vec::new();
    let body = content.to_lowercase();
    let fence_ranges = code_fence_ranges(content);

    for needle in needles {
        if needle.len() < UNLINKED_MIN_NEEDLE_LEN {
            continue;
        }
        let mut search_from = 0usize;
        while let Some(found) = body[search_from..].find(needle.as_str()) {
            let pos = search_from + found;
            search_from = pos + needle.len();

            if fence_ranges.iter().any(|r| r.contains(&pos)) {
                continue; // inside a fenced code block — skip
            }

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
            let line_start = content[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let line_end = content[pos..]
                .find('\n')
                .map(|i| pos + i)
                .unwrap_or(content.len());
            let line = &content[line_start..line_end];
            // Skip if the line already has a [[needle]] wiki link to the
            // focused note (any title/alias) — that's a regular backlink,
            // not unlinked.
            let line_lc = line.to_lowercase();
            let wiki_marker = format!("[[{}]]", needle);
            if line_lc.contains(&wiki_marker) {
                continue;
            }
            // Dedup against an already-recorded match at the same position
            // (a title and an alias could both match overlapping text).
            if out
                .iter()
                .any(|l| l.target == source_id && l.position == pos)
            {
                continue;
            }
            out.push(Link {
                link_type: LinkType::Internal,
                target: source_id.to_string(),
                text: line.trim().to_string(),
                position: pos,
            });
            // Loop continues to find additional matches in the SAME source
            // note on different lines, which is what we want.
        }
    }
    out.sort_by_key(|l| l.position);
    out
}

/// Byte ranges (in `content`) covered by fenced code blocks (` ``` `...` ``` `).
/// A dangling opening fence with no closer runs to the end of the content.
fn code_fence_ranges(content: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut fence_start: Option<usize> = None;
    let mut offset = 0usize;
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            match fence_start {
                None => fence_start = Some(offset),
                Some(start) => {
                    ranges.push(start..offset + line.len());
                    fence_start = None;
                }
            }
        }
        offset += line.len();
    }
    if let Some(start) = fence_start {
        ranges.push(start..content.len());
    }
    ranges
}

/// GET `/notes/:id/unlinked` — pages that mention this page's title or any
/// of its aliases in plain text without `[[...]]` wrapping. Logseq-style.
/// Useful for discovering implicit references the user hasn't yet promoted
/// to a real wiki link.
///
/// Matching is case-insensitive, requires needles of at least
/// `UNLINKED_MIN_NEEDLE_LEN` chars, skips the focused page itself, skips
/// matches inside fenced code blocks, and skips lines that already carry a
/// `[[wiki-link]]` to the focused page.
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

    // Build the needle set: title + aliases, lowercased, deduped, filtered
    // by the minimum-length guard.
    let mut needles: Vec<String> = Vec::new();
    for candidate in
        std::iter::once(title.as_str()).chain(focused.metadata.aliases.iter().map(|a| a.as_str()))
    {
        let lc = candidate.trim().to_lowercase();
        if lc.len() >= UNLINKED_MIN_NEEDLE_LEN && !needles.contains(&lc) {
            needles.push(lc);
        }
    }
    if needles.is_empty() {
        return Ok(Json(Vec::new()));
    }

    // Pull every note in the store (cap at a generous limit — same as the
    // notes list). We linear-scan because the link index doesn't track
    // unlinked mentions; a real index lives behind a TODO.
    let all = s.store.list(None, 5000, 0).await?;
    let mut out: Vec<Link> = Vec::new();
    for n in &all {
        if n.id.as_str() == note_id.as_str() {
            continue; // skip the page itself
        }
        out.extend(find_unlinked_mentions(&n.content, n.id.as_str(), &needles));
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

#[derive(Deserialize)]
pub struct ClearBlockPropertyReq {
    /// Block id in `<note_id>:<line>` format (matches `ParsedBlock.id`).
    pub block_id: String,
    /// Property key to remove (e.g. `"status"`, `"priority"`).
    pub key: String,
}

/// Upsert a single property on a block and persist, triggering the same
/// `apply_post_save_bumps` path that a full note PUT does. This means:
///   - marking a task `status:: done` on a recurring block → the server
///     auto-bumps its deadline to the next occurrence (same as full PUT).
///   - marking a task `status:: done` on a non-recurring block → the block
///     stays done (no bump, nothing to advance).
///   - writing `scheduled:: [[YYYY-MM-DD]]` / `recurring:: <rrule>` works
///     identically to the client side's `upsertBlockProperty`.
///
/// The block_id encodes the note id (`note_id_str:line_num`), so no separate
/// note-id path parameter is needed.
///
/// ## P1.10 — write through the engine's typed container
/// The property is written via `OpPayload::BlockPropertySet` through the sync
/// engine, NOT the legacy whole-note markdown rewrite
/// (`upsert_block_property_in_note`) + whole-note re-diff. The property lands
/// in the block node's `props`/`prop_keys` containers, merging INDEPENDENTLY
/// of the block's prose `text_seq` — so a concurrent prose edit and a property
/// set no longer clobber each other (the old text-splice was the clobber
/// path). The `PropOp` is chosen from the property's registry `value_type`
/// (`SetText` for free-text, `AddToList` per item for multi-value, otherwise
/// `SetScalar` via `parse_scalar`); an unknown property degrades to a `Text`
/// scalar (coerce-and-keep).
///
/// The engine materializes the property as a `key:: value` continuation line
/// in the `<slug>.md` view. The post-save recurring-roll + dependency-unblock
/// logic then reads block properties from THAT re-materialized view (the
/// engine container's markdown projection), not a stale separate re-parse, so
/// recurring tasks + dependencies still fire. Any rewrites they produce are
/// persisted through the engine via `record_sync_update`.
pub async fn set_block_property(
    State(s): State<Arc<AppState>>,
    Json(req): Json<SetBlockPropertyReq>,
) -> AppResult<Json<serde_json::Value>> {
    let (note_id_str, id_suffix) = match req.block_id.rsplit_once(':') {
        Some(pair) => pair,
        None => {
            return Err(AppError::Validation(format!(
                "invalid block_id '{}': expected '<note_id>:<line>' or '<note_id>:<bid>'",
                req.block_id
            )))
        }
    };

    let key = req.key.trim().to_lowercase();
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
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

    // The id suffix is EITHER a line number (legacy `<note_id>:<line>`, resolved
    // against the materialized body) OR a stable bid (`<note_id>:<bid>`). The
    // editor seam (P1.13) addresses by bid because a block's LINE index goes
    // stale the moment the note reflows/prunes server-side, while its bid never
    // moves. Resolve the canonical bid (the `<!-- bid:UUID -->` marker) either
    // way so the property op targets the engine node directly.
    let block_bid =
        block_bid_from_suffix(&prev_content, note_id_str, id_suffix).ok_or_else(|| {
            AppError::NotFound(format!(
                "block '{}' not found in note '{}'",
                req.block_id, note_id_str
            ))
        })?;
    let block_id = parse_bid(&block_bid)?;
    let doc_note_id = stable_uuid_from_slug(note_id_str);

    // Author the property op on the relay's authoritative lineage if this note
    // isn't resident yet — otherwise a task toggle / scheduled set on a synced
    // daily forks a disjoint doc and clobbers/twins on the next relay merge
    // (same class as the text garble). Best-effort + resident-gated.
    bootstrap_note_if_needed(&s, note_id_str).await;

    // Choose the PropOp from the property's registry value_type, then emit it
    // through the engine. Multi-value clears then re-adds each item so a
    // route-driven set replaces the list deterministically.
    let prop_ops = prop_ops_for_set(&s, &key, &req.value).await;
    for value in prop_ops {
        let payload = OpPayload::BlockPropertySet {
            note_id: doc_note_id,
            block_id,
            key: key.clone(),
            value,
        };
        if let Err(e) = s.sync_engine.record_local(payload).await {
            tracing::warn!(
                "sync: record_local BlockPropertySet failed for {}: {e}",
                req.block_id
            );
            return Err(AppError::Internal(anyhow::anyhow!(
                "Failed to record BlockPropertySet: {e}"
            )));
        }
    }

    // Migrate-on-write for THIS key only: if the block still carries `key` as
    // a legacy in-text continuation line (the common case — notes are seeded
    // from markdown, so block props start life in `text_seq`), strip that one
    // line from the block's prose after the container write succeeds.
    // Otherwise the container value and the stale in-text line would BOTH
    // materialize, duplicating the property. Scoped to the single key being
    // written so OTHER in-text props (still readable by old peers) are
    // untouched — not the fleet-wide P1.6 migrate.
    if let Some(stripped) = strip_block_intext_prop(&prev_content, &block_bid, &key) {
        let payload = OpPayload::BlockUpsert {
            block_id,
            note_id: doc_note_id,
            // Preserve the block's real parent (mirrors the seed path at
            // loro_engine.rs `seed_tree_from_flatblocks`) so the prose-strip
            // never resets a nested block's `parent` meta to top-level.
            parent_block_id: stripped.parent.map(|p| *p.as_bytes()),
            order_key: "00000000".to_string(),
            indent_level: stripped.indent,
            text: stripped.text,
            after_block_id: None,
        };
        if let Err(e) = s.sync_engine.record_local(payload).await {
            tracing::warn!(
                "sync: record_local prose-strip BlockUpsert failed for {}: {e}",
                req.block_id
            );
            return Err(AppError::Internal(anyhow::anyhow!(
                "Failed to strip in-text property: {e}"
            )));
        }
    }

    // Re-read the engine-materialized note: the property now renders as a
    // `key:: value` line in the `<slug>.md` view (the container's markdown
    // projection). This is the post-property-set re-materialized view the
    // recurring-roll + dependency-unblock logic reads from.
    let after_prop = s.store.get(&note_id).await?.ok_or_else(|| {
        AppError::NotFound(format!(
            "Note not found after set-property: {}",
            note_id_str
        ))
    })?;
    let after_prop_content = after_prop.content.clone();

    // Run post-save bumps + dependency cycles against the re-materialized
    // view (so they see the just-set property) and persist the roll as
    // CONTAINER property sets — the SAME single mechanism the engine hook and
    // the PUT path use (tesela-ows.1 step 2, Lead constraint (a)). The
    // predecessor cleared ONLY the just-set `key` from the container and wrote
    // the rest of the roll as in-text markdown; once the engine hook had made
    // every lifecycle key container-resident, a second completion via this
    // endpoint left deadline/recurrence_done/last_completed rendering their
    // STALE container values (only `status` updated). Routing every rolled key
    // through a container set makes the roll authoritative regardless of prior
    // residency. A non-recurring / already-completed flip produces no roll and
    // authors nothing.
    let bumps =
        persist_lifecycle_rolls(&s, doc_note_id, note_id_str, &prev_content, &after_prop_content)
            .await;

    // Re-read the final materialized note for indexing + the response echo.
    let updated = s.store.get(&note_id).await?.ok_or_else(|| {
        AppError::NotFound(format!(
            "Note not found after set-property: {}",
            note_id_str
        ))
    })?;

    s.index.reindex(&updated).await?;
    {
        use tesela_core::link::extract_wiki_links;
        use tesela_core::traits::link_graph::LinkGraph;
        let links = extract_wiki_links(&updated.content);
        if let Err(e) = s.index.update_links(&note_id, &links).await {
            tracing::warn!(
                "Failed to update links on set-property for {:?}: {}",
                note_id,
                e
            );
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

/// Resolve a block's canonical `<!-- bid:UUID -->` value from `content` by its
/// body-relative `line_num` (web's `ParsedBlock.id = <note_id>:<line>`).
/// Returns `None` when the line is not a known block or carries no bid.
fn resolve_block_bid(content: &str, note_id_str: &str, line_num: usize) -> Option<String> {
    let (_meta, body) = parse_frontmatter(content).ok()?;
    let blocks = parse_blocks(note_id_str, &body);
    let block_id = format!("{}:{}", note_id_str, line_num);
    let block = blocks.iter().find(|b| b.id == block_id)?;
    block.bid.clone()
}

/// Resolve a block_id's suffix to the target block's canonical bid string. The
/// suffix is EITHER a `<line>` number (legacy `<note_id>:<line>`, resolved
/// against the materialized `content`) OR a `<bid>` passed directly — the editor
/// seam's stale-proof address: the line index moves when a note reflows, the bid
/// never does. Returns None only when a numeric line doesn't match a block.
fn block_bid_from_suffix(content: &str, note_id_str: &str, id_suffix: &str) -> Option<String> {
    match id_suffix.parse::<usize>() {
        Ok(line_num) => resolve_block_bid(content, note_id_str, line_num),
        Err(_) => Some(id_suffix.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that block_bid_from_suffix correctly resolves both numeric line
    /// indices and direct bid strings to the same canonical bid. Regression
    /// test for the editor-seam addressing (P1.13) — both addressing forms
    /// must resolve to the same block id (f90eefe/699041b).
    #[test]
    fn test_block_bid_from_suffix_resolves_both_forms() {
        // A simple note with two blocks, both stamped with bids.
        let content = "---\ntitle: Test\n---\n- First block <!-- bid:11111111-1111-1111-1111-111111111111 -->\n- Second block <!-- bid:22222222-2222-2222-2222-222222222222 -->\n";
        let note_id = "test-note";

        // Form 1: numeric line index (line 0 in the parsed body).
        let bid_via_line = block_bid_from_suffix(content, note_id, "0");
        assert_eq!(
            bid_via_line,
            Some("11111111-1111-1111-1111-111111111111".to_string())
        );

        // Form 2: direct bid passed through.
        let bid_direct =
            block_bid_from_suffix(content, note_id, "22222222-2222-2222-2222-222222222222");
        assert_eq!(
            bid_direct,
            Some("22222222-2222-2222-2222-222222222222".to_string())
        );

        // Both forms should resolve to their respective bids.
        // Verify line 1 resolves to the second block's bid.
        let bid_via_line_1 = block_bid_from_suffix(content, note_id, "1");
        assert_eq!(
            bid_via_line_1,
            Some("22222222-2222-2222-2222-222222222222".to_string())
        );

        // Non-existent line should return None.
        let bid_out_of_range = block_bid_from_suffix(content, note_id, "99");
        assert_eq!(bid_out_of_range, None);
    }

    /// Desktop bootstrap-before-author (daily-garble fix, 2026-06-29).
    ///
    /// The relay holds a note's AUTHORITATIVE snapshot (content X). The server
    /// — whose engine does NOT yet hold the doc (disjoint) — authors that note
    /// via `upsert_blocks` (content Y). With the fix, `upsert_blocks` first
    /// pulls X off the relay as a shared base, so the merged note carries BOTH
    /// X and Y. Without it, the fresh `record_local` would fork a disjoint
    /// lineage and the merge would clobber X (the garbled daily).
    mod bootstrap_before_author {
        use super::super::*;
        use std::net::SocketAddr;
        use std::sync::Arc;

        use axum::response::IntoResponse;
        use rand::RngCore;
        use tokio::sync::{broadcast, RwLock};

        use tesela_core::{config::StorageConfig, storage::filesystem::FsNoteStore};
        use tesela_relay::{router, AppState as RelayAppState};
        use tesela_sync::crypto::keys::GroupKey;
        use tesela_sync::device::DeviceId;
        use tesela_sync::group::GroupId;
        use tesela_sync::transport::relay::RelayClient;
        use tesela_sync::{GroupIdentity, Hlc, LoroEngine, OpPayload, SyncEngine};

        use crate::sync_relay::{RelayHandle, RelayState};

        /// Spawn an in-process relay (mirrors `sync_relay::tests::spawn_relay`).
        async fn spawn_relay() -> (reqwest::Url, tempfile::TempDir, tokio::task::JoinHandle<()>) {
            let tmp = tempfile::tempdir().expect("tmp");
            let db = tmp.path().join("relay.sqlite");
            let state = RelayAppState::open(&db, 4_194_304, Some("admin".into()))
                .await
                .expect("relay state");
            let app = router(state);
            let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
                .await
                .expect("bind");
            let addr = listener.local_addr().expect("addr");
            let server = tokio::spawn(async move {
                let _ = axum::serve(
                    listener,
                    app.into_make_service_with_connect_info::<SocketAddr>(),
                )
                .await;
            });
            (
                reqwest::Url::parse(&format!("http://{}", addr)).unwrap(),
                tmp,
                server,
            )
        }

        fn fresh_group() -> (GroupId, GroupKey) {
            let mut gid = [0u8; 16];
            rand::thread_rng().fill_bytes(&mut gid);
            let mut gk = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut gk);
            (GroupId::from_bytes(gid), GroupKey::from_bytes(gk))
        }

        async fn loro_engine_in(tmp: &std::path::Path, device: DeviceId) -> LoroEngine {
            LoroEngine::with_dirs(
                device,
                Arc::new(Hlc::new(device)),
                tmp.join(".tesela").join("loro"),
                Some(tmp.join("notes")),
            )
            .await
            .expect("loro engine")
        }

        #[tokio::test]
        async fn upsert_blocks_bootstraps_relay_snapshot_before_authoring() {
            let (base_url, _relay_tmp, _relay_srv) = spawn_relay().await;
            let (group, key) = fresh_group();

            let slug = "2026_06_29";
            let note_id = stable_uuid_from_slug(slug);
            let alpha_bid = "01010101-0101-0101-0101-010101010101";
            let gamma_bid = "03030303-0303-0303-0303-030303030303";

            // ── Peer authors content X and deposits its snapshot to the relay ──
            let peer_tmp = tempfile::tempdir().unwrap();
            let peer_dev = DeviceId::from_bytes([0xaa; 16]);
            let peer = loro_engine_in(peer_tmp.path(), peer_dev).await;
            peer.record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some(slug.into()),
                title: slug.into(),
                content: format!("- alpha from relay <!-- bid:{alpha_bid} -->\n"),
                created_at_millis: 1,
            })
            .await
            .expect("peer seed NoteUpsert");
            let snapshot_x = peer
                .export_doc_update(note_id, None)
                .await
                .expect("peer snapshot export");
            let peer_client = RelayClient::new(base_url.clone(), group, peer_dev, key.clone());
            peer_client
                .register_or_recover()
                .await
                .expect("peer register");
            peer_client
                .put_snapshots(0, vec![(note_id.to_vec(), snapshot_x)])
                .await
                .expect("peer deposit snapshot");

            // ── Server: fresh engine that does NOT hold the doc (disjoint) ──
            let mosaic = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(mosaic.path().join("notes")).unwrap();
            std::fs::create_dir_all(mosaic.path().join(".tesela")).unwrap();
            let srv_dev = DeviceId::from_bytes([0xbb; 16]);
            let engine: Arc<dyn SyncEngine> =
                Arc::new(loro_engine_in(mosaic.path(), srv_dev).await);
            assert!(
                engine.doc_version(note_id).await.is_none(),
                "server must start without the doc (the disjoint pre-condition)"
            );

            // The note must exist on disk for `upsert_blocks` (it 404s
            // otherwise). A minimal placeholder daily WITHOUT alpha — so alpha
            // in the result can ONLY have come from the relay bootstrap.
            std::fs::write(
                mosaic.path().join(format!("notes/{slug}.md")),
                "---\ntitle: \"2026_06_29\"\n---\n- placeholder <!-- bid:09090909-0909-0909-0909-090909090909 -->\n",
            )
            .unwrap();

            let store = Arc::new(FsNoteStore::new(
                mosaic.path().to_path_buf(),
                StorageConfig::default(),
            ));
            let index = Arc::new(
                tesela_core::db::SqliteIndex::open(&mosaic.path().join(".tesela").join("test.db"))
                    .await
                    .unwrap(),
            );
            let (ws_tx, _) = broadcast::channel(16);
            let (ws_delta_tx, _) = broadcast::channel(16);
            let group_identity = Arc::new(RwLock::new(GroupIdentity {
                group_id: group,
                group_key: key.clone(),
            }));

            // Server relay handle pointing at the spawned relay (same group).
            let relay_client = Arc::new(RelayClient::new(base_url.clone(), group, srv_dev, key));
            relay_client
                .register_or_recover()
                .await
                .expect("server register");
            let relay_handle = RelayHandle {
                url: base_url.to_string(),
                client: relay_client,
                state: Arc::new(RwLock::new(RelayState::default())),
                mosaic_root: mosaic.path().to_path_buf(),
            };

            let state = Arc::new(AppState {
                mosaic_root: mosaic.path().to_path_buf(),
                store,
                index,
                ws_tx,
                ws_delta_tx,
                ws_conn_seq: std::sync::atomic::AtomicU64::new(0),
                type_registry: tesela_core::types::TypeRegistry::load(mosaic.path()),
                auto_sync: Arc::new(crate::reminders::auto::AutoSync::new()),
                sync_engine: Arc::clone(&engine),
                lan_discovery: None,
                group_identity,
                display_name: "test".into(),
                public_url: "http://127.0.0.1:0".into(),
                relay_url: Some(base_url.to_string()),
                relay: Some(relay_handle),
                backup_status: crate::backup_scheduler::BackupStatusHandle::new(
                    crate::backup_scheduler::SchedulerConfig::from_env(),
                ),
            });

            // ── Author content Y via upsert_blocks (adds a NEW block gamma) ──
            let res = upsert_blocks(
                Path(slug.to_string()),
                State(Arc::clone(&state)),
                Json(UpsertBlocksReq {
                    ops: vec![BlockOp::Upsert {
                        bid: gamma_bid.into(),
                        text: "gamma from desktop".into(),
                        parent_bid: None,
                        indent_level: 0,
                        after_bid: None,
                    }],
                }),
            )
            .await;
            assert!(
                res.is_ok(),
                "upsert_blocks should succeed (got {:?})",
                res.err().map(|e| e.into_response().status())
            );

            // The relay's authoritative alpha (X) was bootstrapped as the shared
            // base, and the desktop's gamma (Y) merged onto it — NOT a disjoint
            // clobber that drops alpha. `render_note` is the CRDT truth.
            let merged = engine
                .render_note(note_id)
                .await
                .expect("server renders the note after author");
            assert!(
                merged.contains("alpha from relay"),
                "bootstrapped relay base (X) must survive the author; render:\n{merged}"
            );
            assert!(
                merged.contains("gamma from desktop"),
                "the desktop edit (Y) must be present; render:\n{merged}"
            );
            // Exactly one alpha + one gamma — no disjoint same-bid twins.
            assert_eq!(
                merged.matches(&format!("bid:{alpha_bid}")).count(),
                1,
                "alpha must render exactly once (no twin); render:\n{merged}"
            );
            assert_eq!(
                merged.matches(&format!("bid:{gamma_bid}")).count(),
                1,
                "gamma must render exactly once (no twin); render:\n{merged}"
            );
            // The doc is now resident on the server (bootstrap imported it).
            assert!(
                engine.doc_version(note_id).await.is_some(),
                "the note must be resident after the bootstrap import"
            );
        }

        /// Same convergence fix (M1) on the CREATE path. `create_note` authors
        /// via `record_sync_create`, which does NOT bootstrap internally. The
        /// relay holds slug `relay-note`'s authoritative snapshot (alpha); the
        /// server's engine does NOT hold the doc. Creating that slug locally
        /// must first pull alpha off the relay as a shared base so the merged
        /// note carries BOTH alpha (relay) and gamma (this create) — not a
        /// disjoint clobber that drops alpha.
        #[tokio::test]
        async fn create_note_bootstraps_relay_snapshot_before_authoring() {
            let (base_url, _relay_tmp, _relay_srv) = spawn_relay().await;
            let (group, key) = fresh_group();

            // `sanitize_filename("relay-note") == "relay-note"`, so the create
            // slug matches the slug the relay deposited under.
            let slug = "relay-note";
            let note_id = stable_uuid_from_slug(slug);
            let alpha_bid = "01010101-0101-0101-0101-010101010101";
            let gamma_bid = "03030303-0303-0303-0303-030303030303";

            // ── Peer authors content X and deposits its snapshot to the relay ──
            let peer_tmp = tempfile::tempdir().unwrap();
            let peer_dev = DeviceId::from_bytes([0xaa; 16]);
            let peer = loro_engine_in(peer_tmp.path(), peer_dev).await;
            peer.record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some(slug.into()),
                title: slug.into(),
                content: format!("- alpha from relay <!-- bid:{alpha_bid} -->\n"),
                created_at_millis: 1,
            })
            .await
            .expect("peer seed NoteUpsert");
            let snapshot_x = peer
                .export_doc_update(note_id, None)
                .await
                .expect("peer snapshot export");
            let peer_client = RelayClient::new(base_url.clone(), group, peer_dev, key.clone());
            peer_client
                .register_or_recover()
                .await
                .expect("peer register");
            peer_client
                .put_snapshots(0, vec![(note_id.to_vec(), snapshot_x)])
                .await
                .expect("peer deposit snapshot");

            // ── Server: fresh engine that does NOT hold the doc (disjoint) ──
            let mosaic = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(mosaic.path().join("notes")).unwrap();
            std::fs::create_dir_all(mosaic.path().join(".tesela")).unwrap();
            let srv_dev = DeviceId::from_bytes([0xbb; 16]);
            let engine: Arc<dyn SyncEngine> =
                Arc::new(loro_engine_in(mosaic.path(), srv_dev).await);
            assert!(
                engine.doc_version(note_id).await.is_none(),
                "server must start without the doc (the disjoint pre-condition)"
            );
            // The note must NOT exist on disk — `create_note` creates it (and
            // errors if it already exists). So alpha in the result can ONLY have
            // come from the relay bootstrap, never the local file.
            assert!(
                !mosaic.path().join(format!("notes/{slug}.md")).exists(),
                "the slug must not exist locally before the create"
            );

            let store = Arc::new(FsNoteStore::new(
                mosaic.path().to_path_buf(),
                StorageConfig::default(),
            ));
            let index = Arc::new(
                tesela_core::db::SqliteIndex::open(&mosaic.path().join(".tesela").join("test.db"))
                    .await
                    .unwrap(),
            );
            let (ws_tx, _) = broadcast::channel(16);
            let (ws_delta_tx, _) = broadcast::channel(16);
            let group_identity = Arc::new(RwLock::new(GroupIdentity {
                group_id: group,
                group_key: key.clone(),
            }));

            // Server relay handle pointing at the spawned relay (same group).
            let relay_client = Arc::new(RelayClient::new(base_url.clone(), group, srv_dev, key));
            relay_client
                .register_or_recover()
                .await
                .expect("server register");
            let relay_handle = RelayHandle {
                url: base_url.to_string(),
                client: relay_client,
                state: Arc::new(RwLock::new(RelayState::default())),
                mosaic_root: mosaic.path().to_path_buf(),
            };

            let state = Arc::new(AppState {
                mosaic_root: mosaic.path().to_path_buf(),
                store,
                index,
                ws_tx,
                ws_delta_tx,
                ws_conn_seq: std::sync::atomic::AtomicU64::new(0),
                type_registry: tesela_core::types::TypeRegistry::load(mosaic.path()),
                auto_sync: Arc::new(crate::reminders::auto::AutoSync::new()),
                sync_engine: Arc::clone(&engine),
                lan_discovery: None,
                group_identity,
                display_name: "test".into(),
                public_url: "http://127.0.0.1:0".into(),
                relay_url: Some(base_url.to_string()),
                relay: Some(relay_handle),
                backup_status: crate::backup_scheduler::BackupStatusHandle::new(
                    crate::backup_scheduler::SchedulerConfig::from_env(),
                ),
            });

            // ── Author content Y by CREATING the note at slug `relay-note` ──
            let res = create_note(
                State(Arc::clone(&state)),
                Json(CreateNoteReq {
                    title: slug.to_string(),
                    content: format!("- gamma from desktop <!-- bid:{gamma_bid} -->\n"),
                    tags: None,
                }),
            )
            .await;
            assert!(
                res.is_ok(),
                "create_note should succeed (got {:?})",
                res.err().map(|e| e.into_response().status())
            );

            // The relay's alpha (X) was bootstrapped as the shared base, and the
            // created gamma (Y) merged onto it — NOT a disjoint clobber that
            // drops alpha. `render_note` is the CRDT truth.
            let merged = engine
                .render_note(note_id)
                .await
                .expect("server renders the note after create");
            assert!(
                merged.contains("alpha from relay"),
                "bootstrapped relay base (X) must survive the create; render:\n{merged}"
            );
            assert!(
                merged.contains("gamma from desktop"),
                "the created edit (Y) must be present; render:\n{merged}"
            );
            // Exactly one alpha + one gamma — no disjoint same-bid twins.
            assert_eq!(
                merged.matches(&format!("bid:{alpha_bid}")).count(),
                1,
                "alpha must render exactly once (no twin); render:\n{merged}"
            );
            assert_eq!(
                merged.matches(&format!("bid:{gamma_bid}")).count(),
                1,
                "gamma must render exactly once (no twin); render:\n{merged}"
            );
            // The doc is now resident on the server (bootstrap imported it).
            assert!(
                engine.doc_version(note_id).await.is_some(),
                "the note must be resident after the bootstrap import"
            );
        }

        /// Residual bootstrap-before-author gap (tesela-1fe, 2026-07-02):
        /// `rename_tag`'s corpus-rewrite loop calls `record_sync_update` on
        /// every touched note, but (before the fix) never bootstrapped that
        /// note's doc first. A note the rewrite touches may be non-resident
        /// on this server (evicted, or a `.md`-without-`.bin` import) even
        /// though it already has authoritative history on the relay — a bare
        /// `record_sync_update` would then fork a disjoint lineage and later
        /// clobber that history. Reproduces the exact shape: the relay holds
        /// `referencing-note`'s authoritative snapshot (alpha); the server's
        /// engine does NOT hold that doc; `rename_tag` rewrites its on-disk
        /// `#old-tag` reference to `#new-tag`. With the fix, alpha survives
        /// the rewrite and the rewritten reference merges onto the SAME
        /// lineage — no disjoint twin.
        #[tokio::test]
        async fn rename_tag_bootstraps_relay_snapshot_before_authoring() {
            let (base_url, _relay_tmp, _relay_srv) = spawn_relay().await;
            let (group, key) = fresh_group();

            let ref_slug = "referencing-note";
            let ref_note_id = stable_uuid_from_slug(ref_slug);
            let alpha_bid = "01010101-0101-0101-0101-010101010101";
            let local_bid = "04040404-0404-0404-0404-040404040404";

            // ── Peer authors content X and deposits its snapshot to the relay ──
            let peer_tmp = tempfile::tempdir().unwrap();
            let peer_dev = DeviceId::from_bytes([0xaa; 16]);
            let peer = loro_engine_in(peer_tmp.path(), peer_dev).await;
            peer.record_local(OpPayload::NoteUpsert {
                note_id: ref_note_id,
                display_alias: Some(ref_slug.into()),
                title: ref_slug.into(),
                content: format!("- alpha from relay <!-- bid:{alpha_bid} -->\n"),
                created_at_millis: 1,
            })
            .await
            .expect("peer seed NoteUpsert");
            let snapshot_x = peer
                .export_doc_update(ref_note_id, None)
                .await
                .expect("peer snapshot export");
            let peer_client = RelayClient::new(base_url.clone(), group, peer_dev, key.clone());
            peer_client
                .register_or_recover()
                .await
                .expect("peer register");
            peer_client
                .put_snapshots(0, vec![(ref_note_id.to_vec(), snapshot_x)])
                .await
                .expect("peer deposit snapshot");

            // ── Server: fresh engine that does NOT hold the doc (disjoint) ──
            let mosaic = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(mosaic.path().join("notes")).unwrap();
            std::fs::create_dir_all(mosaic.path().join(".tesela")).unwrap();
            let srv_dev = DeviceId::from_bytes([0xbb; 16]);
            let engine: Arc<dyn SyncEngine> =
                Arc::new(loro_engine_in(mosaic.path(), srv_dev).await);
            assert!(
                engine.doc_version(ref_note_id).await.is_none(),
                "server must start without the doc (the disjoint pre-condition)"
            );

            // The tag's own file (rename_tag requires `from_slug` to be a Tag).
            std::fs::write(
                mosaic.path().join("notes/old-tag.md"),
                "---\ntype: Tag\n---\n- Old Tag\n",
            )
            .unwrap();
            // The note `rename_tag` will rewrite: a LOCAL on-disk body that
            // references `#old-tag`. The server's engine has never authored
            // this note — alpha in the merged result can ONLY have come from
            // the relay bootstrap.
            std::fs::write(
                mosaic.path().join(format!("notes/{ref_slug}.md")),
                format!(
                    "---\ntitle: \"{ref_slug}\"\n---\n- ref #old-tag <!-- bid:{local_bid} -->\n"
                ),
            )
            .unwrap();

            let store = Arc::new(FsNoteStore::new(
                mosaic.path().to_path_buf(),
                StorageConfig::default(),
            ));
            let index = Arc::new(
                tesela_core::db::SqliteIndex::open(&mosaic.path().join(".tesela").join("test.db"))
                    .await
                    .unwrap(),
            );
            let (ws_tx, _) = broadcast::channel(16);
            let (ws_delta_tx, _) = broadcast::channel(16);
            let group_identity = Arc::new(RwLock::new(GroupIdentity {
                group_id: group,
                group_key: key.clone(),
            }));

            // Server relay handle pointing at the spawned relay (same group).
            let relay_client = Arc::new(RelayClient::new(base_url.clone(), group, srv_dev, key));
            relay_client
                .register_or_recover()
                .await
                .expect("server register");
            let relay_handle = RelayHandle {
                url: base_url.to_string(),
                client: relay_client,
                state: Arc::new(RwLock::new(RelayState::default())),
                mosaic_root: mosaic.path().to_path_buf(),
            };

            let state = Arc::new(AppState {
                mosaic_root: mosaic.path().to_path_buf(),
                store,
                index,
                ws_tx,
                ws_delta_tx,
                ws_conn_seq: std::sync::atomic::AtomicU64::new(0),
                type_registry: tesela_core::types::TypeRegistry::load(mosaic.path()),
                auto_sync: Arc::new(crate::reminders::auto::AutoSync::new()),
                sync_engine: Arc::clone(&engine),
                lan_discovery: None,
                group_identity,
                display_name: "test".into(),
                public_url: "http://127.0.0.1:0".into(),
                relay_url: Some(base_url.to_string()),
                relay: Some(relay_handle),
                backup_status: crate::backup_scheduler::BackupStatusHandle::new(
                    crate::backup_scheduler::SchedulerConfig::from_env(),
                ),
            });

            // ── Rename old-tag -> new-tag, rewriting the corpus ──
            let res = rename_tag(
                State(Arc::clone(&state)),
                Json(RenameTagReq {
                    from_slug: "old-tag".into(),
                    to_slug: "new-tag".into(),
                    commit: true,
                }),
            )
            .await;
            assert!(
                res.is_ok(),
                "rename_tag should succeed (got {:?})",
                res.err().map(|e| e.into_response().status())
            );

            // The relay's authoritative alpha (X) was bootstrapped as the shared
            // base, and the rewritten `#new-tag` reference (Y) merged onto it —
            // NOT a disjoint clobber that drops alpha. `render_note` is the CRDT
            // truth.
            let merged = engine
                .render_note(ref_note_id)
                .await
                .expect("server renders the note after rename");
            assert!(
                merged.contains("alpha from relay"),
                "bootstrapped relay base (X) must survive the rewrite; render:\n{merged}"
            );
            assert!(
                merged.contains("#new-tag"),
                "the rewritten tag reference (Y) must be present; render:\n{merged}"
            );
            // Exactly one alpha — no disjoint same-bid twin.
            assert_eq!(
                merged.matches(&format!("bid:{alpha_bid}")).count(),
                1,
                "alpha must render exactly once (no twin); render:\n{merged}"
            );
            // The doc is now resident on the server (bootstrap imported it).
            assert!(
                engine.doc_version(ref_note_id).await.is_some(),
                "the note must be resident after the bootstrap import"
            );
        }
    }

    /// Audit A9a (2026-06-09): `PUT /notes/{id}` must NOT report success
    /// when `record_local` fails. Since the 2026-05-26 redesign,
    /// `record_sync_update` is the SOLE writer on PUT — a swallowed
    /// failure means the edit was silently dropped (never applied, never
    /// synced to peers) while the client got a 200 and believed the save
    /// stuck. These tests inject a SyncEngine whose `record_local` always
    /// errors and assert the handlers surface a 5xx instead.
    mod record_local_failure {
        use super::super::*;
        use std::{
            path::PathBuf,
            sync::{Arc, Mutex},
        };

        use axum::response::IntoResponse;
        use tokio::sync::broadcast;

        use tesela_core::{config::StorageConfig, storage::filesystem::FsNoteStore};
        use tesela_sync::{
            ContentHash, DeviceId, LocalCursor, OpPayload, PeerCursor, SyncEngine, SyncError,
            SyncResult,
        };

        /// A stub engine whose `record_local` always fails — simulates a
        /// Loro insert/serialization failure on the producer path.
        struct FailingRecordEngine;

        #[async_trait::async_trait]
        impl SyncEngine for FailingRecordEngine {
            fn device(&self) -> DeviceId {
                DeviceId::from_bytes([0xfa; 16])
            }
            async fn record_local(&self, _payload: OpPayload) -> SyncResult<ContentHash> {
                Err(SyncError::Storage(
                    "simulated record_local failure".to_string(),
                ))
            }
            async fn local_cursor(&self) -> SyncResult<LocalCursor> {
                Ok(LocalCursor::Earliest)
            }
            async fn peer_cursor(&self, _peer: DeviceId) -> SyncResult<PeerCursor> {
                Ok(PeerCursor::Earliest)
            }
            async fn ack_peer(&self, _peer: DeviceId, _ack: PeerCursor) -> SyncResult<()> {
                Ok(())
            }
        }

        /// Minimal AppState over a tempdir mosaic, with the failing engine
        /// injected. Mirrors the construction in main.rs's WS tests.
        async fn failing_state(mosaic: &std::path::Path) -> Arc<AppState> {
            std::fs::create_dir_all(mosaic.join("notes")).unwrap();
            std::fs::create_dir_all(mosaic.join(".tesela")).unwrap();
            let store = Arc::new(FsNoteStore::new(
                mosaic.to_path_buf(),
                StorageConfig::default(),
            ));
            let index = Arc::new(
                tesela_core::db::SqliteIndex::open(&mosaic.join(".tesela").join("test.db"))
                    .await
                    .unwrap(),
            );
            let (ws_tx, _) = broadcast::channel(16);
            let (ws_delta_tx, _) = broadcast::channel(16);
            let group_identity = Arc::new(tokio::sync::RwLock::new(tesela_sync::GroupIdentity {
                group_id: tesela_sync::GroupId::new_random(),
                group_key: tesela_sync::GroupKey::random(),
            }));
            Arc::new(AppState {
                mosaic_root: mosaic.to_path_buf(),
                store,
                index,
                ws_tx,
                ws_delta_tx,
                ws_conn_seq: std::sync::atomic::AtomicU64::new(0),
                type_registry: tesela_core::types::TypeRegistry::load(mosaic),
                auto_sync: Arc::new(crate::reminders::auto::AutoSync::new()),
                sync_engine: Arc::new(FailingRecordEngine) as Arc<dyn SyncEngine>,
                lan_discovery: None,
                group_identity,
                display_name: "test".into(),
                public_url: "http://127.0.0.1:0".into(),
                relay_url: None,
                relay: None,
                backup_status: crate::backup_scheduler::BackupStatusHandle::new(
                    crate::backup_scheduler::SchedulerConfig::from_env(),
                ),
            })
        }

        const BID: &str = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";

        /// Block-op path: a PUT whose block diff is non-empty must surface
        /// the dropped op as a 5xx, not a 200.
        #[tokio::test]
        async fn put_propagates_block_op_record_local_failure() {
            let tmp = tempfile::TempDir::new().unwrap();
            let state = failing_state(tmp.path()).await;
            let seed = format!("- alpha <!-- bid:{BID} -->\n");
            std::fs::write(tmp.path().join("notes/putfail.md"), &seed).unwrap();

            let result = update_note(
                Path("putfail".to_string()),
                State(Arc::clone(&state)),
                Json(UpdateNoteReq {
                    content: format!("- alpha CHANGED <!-- bid:{BID} -->\n"),
                    base_content: None,
                }),
            )
            .await;

            let err = match result {
                Ok(_) => panic!(
                    "PUT must NOT return 2xx when record_local fails (the sync \
                     op was dropped; the edit was never applied)"
                ),
                Err(e) => e,
            };
            let status = err.into_response().status();
            assert!(status.is_server_error(), "expected a 5xx, got {status}");
        }

        /// NoteUpsert-fallback path (frontmatter-only change, empty block
        /// diff): the same failure must also surface as a 5xx.
        #[tokio::test]
        async fn put_propagates_noteupsert_fallback_record_local_failure() {
            let tmp = tempfile::TempDir::new().unwrap();
            let state = failing_state(tmp.path()).await;
            let seed = format!("---\ntitle: \"Old\"\n---\n\n- alpha <!-- bid:{BID} -->\n");
            std::fs::write(tmp.path().join("notes/fmfail.md"), &seed).unwrap();

            let result = update_note(
                Path("fmfail".to_string()),
                State(Arc::clone(&state)),
                Json(UpdateNoteReq {
                    content: format!("---\ntitle: \"New\"\n---\n\n- alpha <!-- bid:{BID} -->\n"),
                    base_content: None,
                }),
            )
            .await;

            let err = match result {
                Ok(_) => panic!(
                    "frontmatter-only PUT must NOT return 2xx when the \
                     NoteUpsert fallback's record_local fails"
                ),
                Err(e) => e,
            };
            let status = err.into_response().status();
            assert!(status.is_server_error(), "expected a 5xx, got {status}");
        }

        struct StripAppliesPropertyFailsEngine {
            note_path: PathBuf,
            bid: String,
            calls: Arc<Mutex<Vec<&'static str>>>,
        }

        impl StripAppliesPropertyFailsEngine {
            fn write_stripped_block(&self, indent_level: u16, text: &str) -> SyncResult<()> {
                let mut lines = Vec::new();
                let mut text_lines = text.lines();
                let first = text_lines.next().unwrap_or_default();
                let bullet_indent = "  ".repeat(indent_level as usize);
                let continuation_indent = "  ".repeat(indent_level as usize + 1);
                lines.push("---".to_string());
                lines.push("title: \"Ordering\"".to_string());
                lines.push("tags: []".to_string());
                lines.push("---".to_string());
                lines.push(format!(
                    "{bullet_indent}- {first} <!-- bid:{} -->",
                    self.bid
                ));
                for line in text_lines {
                    lines.push(format!("{continuation_indent}{line}"));
                }
                std::fs::write(&self.note_path, lines.join("\n") + "\n")
                    .map_err(|e| SyncError::Storage(format!("write stripped block: {e}")))
            }
        }

        #[async_trait::async_trait]
        impl SyncEngine for StripAppliesPropertyFailsEngine {
            fn device(&self) -> DeviceId {
                DeviceId::from_bytes([0xfb; 16])
            }
            async fn record_local(&self, payload: OpPayload) -> SyncResult<ContentHash> {
                match payload {
                    OpPayload::BlockUpsert {
                        indent_level, text, ..
                    } => {
                        self.calls.lock().unwrap().push("BlockUpsert");
                        self.write_stripped_block(indent_level, &text)?;
                        Ok(ContentHash([0x11; 32]))
                    }
                    OpPayload::BlockPropertySet { .. } => {
                        self.calls.lock().unwrap().push("BlockPropertySet");
                        Err(SyncError::Storage(
                            "simulated BlockPropertySet failure".to_string(),
                        ))
                    }
                    _ => Ok(ContentHash([0x22; 32])),
                }
            }
            async fn local_cursor(&self) -> SyncResult<LocalCursor> {
                Ok(LocalCursor::Earliest)
            }
            async fn peer_cursor(&self, _peer: DeviceId) -> SyncResult<PeerCursor> {
                Ok(PeerCursor::Earliest)
            }
            async fn ack_peer(&self, _peer: DeviceId, _ack: PeerCursor) -> SyncResult<()> {
                Ok(())
            }
        }

        async fn state_with_engine(
            mosaic: &std::path::Path,
            sync_engine: Arc<dyn SyncEngine>,
        ) -> Arc<AppState> {
            std::fs::create_dir_all(mosaic.join("notes")).unwrap();
            std::fs::create_dir_all(mosaic.join(".tesela")).unwrap();
            let store = Arc::new(FsNoteStore::new(
                mosaic.to_path_buf(),
                StorageConfig::default(),
            ));
            let index = Arc::new(
                tesela_core::db::SqliteIndex::open(&mosaic.join(".tesela").join("test.db"))
                    .await
                    .unwrap(),
            );
            let (ws_tx, _) = broadcast::channel(16);
            let (ws_delta_tx, _) = broadcast::channel(16);
            let group_identity = Arc::new(tokio::sync::RwLock::new(tesela_sync::GroupIdentity {
                group_id: tesela_sync::GroupId::new_random(),
                group_key: tesela_sync::GroupKey::random(),
            }));
            Arc::new(AppState {
                mosaic_root: mosaic.to_path_buf(),
                store,
                index,
                ws_tx,
                ws_delta_tx,
                ws_conn_seq: std::sync::atomic::AtomicU64::new(0),
                type_registry: tesela_core::types::TypeRegistry::load(mosaic),
                auto_sync: Arc::new(crate::reminders::auto::AutoSync::new()),
                sync_engine,
                lan_discovery: None,
                group_identity,
                display_name: "test".into(),
                public_url: "http://127.0.0.1:0".into(),
                relay_url: None,
                relay: None,
                backup_status: crate::backup_scheduler::BackupStatusHandle::new(
                    crate::backup_scheduler::SchedulerConfig::from_env(),
                ),
            })
        }

        #[tokio::test]
        async fn set_block_property_leaves_intext_property_when_container_write_fails() {
            let tmp = tempfile::TempDir::new().unwrap();
            std::fs::create_dir_all(tmp.path().join("notes")).unwrap();
            let note_path = tmp.path().join("notes/ordering-fail.md");
            let seed = format!(
                "---\ntitle: \"Ordering\"\ntags: []\n---\n- task <!-- bid:{BID} -->\n  status:: todo\n"
            );
            std::fs::write(&note_path, seed).unwrap();
            let calls = Arc::new(Mutex::new(Vec::new()));
            let engine = Arc::new(StripAppliesPropertyFailsEngine {
                note_path: note_path.clone(),
                bid: BID.to_string(),
                calls: Arc::clone(&calls),
            }) as Arc<dyn SyncEngine>;
            let state = state_with_engine(tmp.path(), engine).await;

            let result = set_block_property(
                State(state),
                Json(SetBlockPropertyReq {
                    block_id: format!("ordering-fail:{BID}"),
                    key: "status".to_string(),
                    value: "done".to_string(),
                }),
            )
            .await;

            let err = match result {
                Ok(_) => panic!("set-property must surface the injected container write failure"),
                Err(e) => e,
            };
            let status = err.into_response().status();
            assert!(status.is_server_error(), "expected a 5xx, got {status}");
            let content = std::fs::read_to_string(&note_path).unwrap();
            assert!(
                content.contains("status:: todo"),
                "a failed container write must not strip the only in-text property value; got:\n{content}"
            );
            assert_eq!(
                calls.lock().unwrap().as_slice(),
                &["BlockPropertySet"],
                "the container write must be attempted before any prose strip"
            );
        }
    }

    /// tesela-ows.1 step 2 (attempt #3) — REGRESSION for the review reject: the
    /// two pre-existing HTTP write paths persisted their recurrence roll as
    /// in-text markdown and relied on the lifecycle keys NOT being container-
    /// resident. Once the engine hook (`apply_block_lifecycle`) makes
    /// deadline/recurrence_done/last_completed/status container-resident after
    /// ANY relay/WS-delivered completion, that assumption broke and the roll
    /// rendered STALE (render-time dedup makes the container value win). The fix
    /// routes BOTH handlers' roll persistence through
    /// `compute_lifecycle_container_sets` + container `BlockPropertySet` ops
    /// (Lead constraint (a)), the SAME mechanism the engine hook uses.
    ///
    /// Both tests seed a block whose lifecycle keys are ALREADY container-
    /// resident (the post-engine-hook state), then complete it via the route
    /// and assert the FULLY-rolled values render. They mirror the reviewer's
    /// empirically-reproduced recipes and FAIL on base 337ac6d2: there
    /// `set_block_property` cleared only `status` (leaving stale sibling keys)
    /// and `update_note` cleared nothing (the whole completion shadowed).
    mod lifecycle_container_roll {
        use super::super::*;
        use std::sync::Arc;

        use tesela_core::{config::StorageConfig, storage::filesystem::FsNoteStore};
        use tesela_sync::{
            DeviceId, GroupId, GroupIdentity, GroupKey, Hlc, LoroEngine, OpPayload, PropOp,
            SyncEngine,
        };

        // "07070707-…" parses to the 16-byte block id [0x07; 16], so the seed
        // `<!-- bid:… -->` marker and the `BlockPropertySet` block_id address the
        // same tree node (mirrors the engine acceptance test).
        const BID_HEX: &str = "07070707-0707-0707-0707-070707070707";
        const BLOCK: [u8; 16] = [0x07; 16];

        /// Build an AppState over a REAL `LoroEngine` (materializing to
        /// `notes/`), seed one recurring block, and drive its lifecycle keys
        /// into the typed props CONTAINER via `BlockPropertySet` — the exact
        /// post-engine-hook shape the reviewer seeded. `relay: None` so the
        /// handlers' `bootstrap_note_if_needed` is a no-op.
        async fn seeded_state(
            tmp: &std::path::Path,
            slug: &str,
            container_props: &[(&str, &str)],
        ) -> Arc<AppState> {
            std::fs::create_dir_all(tmp.join("notes")).unwrap();
            std::fs::create_dir_all(tmp.join(".tesela")).unwrap();
            let device = DeviceId::from_bytes([0xc1; 16]);
            let engine = LoroEngine::with_dirs(
                device,
                Arc::new(Hlc::new(device)),
                tmp.join(".tesela").join("loro"),
                Some(tmp.join("notes")),
            )
            .await
            .expect("loro engine");

            let note_id = stable_uuid_from_slug(slug);
            engine
                .record_local(OpPayload::NoteUpsert {
                    note_id,
                    display_alias: Some(slug.into()),
                    title: slug.into(),
                    content: format!("- water plants <!-- bid:{BID_HEX} -->\n"),
                    created_at_millis: 1,
                })
                .await
                .expect("seed NoteUpsert");
            // Seed each key via `SetScalar` — the REALISTIC shape the engine
            // lifecycle hook (`apply_block_lifecycle`) and pre-round-3 roll
            // persisters actually author, and the shape Taylor's live docs hold.
            // (Round 3 fixed the class where the route's `SetText` write then
            // HARD-ERRORED over this scalar; the write layer now tolerates it, so
            // seeding the production shape is both honest and revert-discriminating
            // — this seed 500'd the route write on base/HEAD.) `status` is seeded
            // LAST as `todo` so no status→done flip fires the roll during seeding.
            for (k, v) in container_props {
                engine
                    .record_local(OpPayload::BlockPropertySet {
                        note_id,
                        block_id: BLOCK,
                        key: (*k).into(),
                        value: PropOp::SetScalar(tesela_sync::PropScalar::Text((*v).into())),
                    })
                    .await
                    .expect("seed BlockPropertySet");
            }

            let engine: Arc<dyn SyncEngine> = Arc::new(engine);
            let store = Arc::new(FsNoteStore::new(tmp.to_path_buf(), StorageConfig::default()));
            let index = Arc::new(
                tesela_core::db::SqliteIndex::open(&tmp.join(".tesela").join("test.db"))
                    .await
                    .unwrap(),
            );
            let (ws_tx, _) = tokio::sync::broadcast::channel(16);
            let (ws_delta_tx, _) = tokio::sync::broadcast::channel(16);
            let group_identity = Arc::new(tokio::sync::RwLock::new(GroupIdentity {
                group_id: GroupId::new_random(),
                group_key: GroupKey::random(),
            }));
            Arc::new(AppState {
                mosaic_root: tmp.to_path_buf(),
                store,
                index,
                ws_tx,
                ws_delta_tx,
                ws_conn_seq: std::sync::atomic::AtomicU64::new(0),
                type_registry: tesela_core::types::TypeRegistry::load(tmp),
                auto_sync: Arc::new(crate::reminders::auto::AutoSync::new()),
                sync_engine: engine,
                lan_discovery: None,
                group_identity,
                display_name: "test".into(),
                public_url: "http://127.0.0.1:0".into(),
                relay_url: None,
                relay: None,
                backup_status: crate::backup_scheduler::BackupStatusHandle::new(
                    crate::backup_scheduler::SchedulerConfig::from_env(),
                ),
            })
        }

        /// The seed: a `daily count 3` recurrence whose 2nd occurrence is due,
        /// with completion memory from the 1st occurrence — all CONTAINER-
        /// resident. Completing it must roll to occurrence #3.
        fn resident_seed() -> Vec<(&'static str, &'static str)> {
            vec![
                ("recurring", "daily count 3"),
                ("deadline", "[[2026-05-08]]"),
                ("recurrence_done", "1"),
                ("last_completed", "[[2026-05-07]]"),
                ("status", "todo"),
            ]
        }

        /// Reviewer repro (1): a SECOND completion via `set_block_property` must
        /// render the FULLY-rolled values, not just `status`. On base 337ac6d2
        /// the handler clears only the `status` key, leaving
        /// deadline/recurrence_done/last_completed at their stale container
        /// values (05-08 / 1 / 05-07).
        #[tokio::test]
        async fn set_block_property_second_completion_rolls_all_container_keys() {
            let tmp = tempfile::TempDir::new().unwrap();
            let slug = "chores";
            let state = seeded_state(tmp.path(), slug, &resident_seed()).await;

            let res = set_block_property(
                State(Arc::clone(&state)),
                Json(SetBlockPropertyReq {
                    block_id: format!("{slug}:{BID_HEX}"),
                    key: "status".to_string(),
                    value: "done".to_string(),
                }),
            )
            .await;
            if let Err(e) = res {
                let status = e.into_response().status();
                panic!("set_block_property failed with status {status}");
            }

            let rendered = state
                .store
                .get(&NoteId::new(slug))
                .await
                .unwrap()
                .expect("note present")
                .content;
            assert!(
                rendered.contains("deadline:: [[2026-05-09]]"),
                "deadline must advance to 05-09 (was stale 05-08 on base); render:\n{rendered}"
            );
            assert!(
                rendered.contains("recurrence_done:: 2"),
                "recurrence_done must roll to 2 (was stale 1 on base); render:\n{rendered}"
            );
            assert!(
                rendered.contains("last_completed:: [[2026-05-08]]"),
                "last_completed must stamp 05-08 (was stale 05-07 on base); render:\n{rendered}"
            );
            assert!(
                rendered.contains("status:: todo") && !rendered.contains("status:: done"),
                "status must reset to todo; render:\n{rendered}"
            );
        }

        /// Reviewer repro (2): a completion via `update_note` (PUT) on a
        /// container-resident block must take effect. On base 337ac6d2 the PUT
        /// clears nothing from the container, so render-time dedup shadows the
        /// entire markdown roll under the stale container values — a silent
        /// no-op (deadline stays 05-08, recurrence_done stays 1).
        #[tokio::test]
        async fn update_note_completion_rolls_on_container_resident_block() {
            let tmp = tempfile::TempDir::new().unwrap();
            let slug = "chores";
            let state = seeded_state(tmp.path(), slug, &resident_seed()).await;

            // The client PUTs the materialized body with the block flipped to
            // done (exactly what a status toggle sends over the whole-body PUT).
            let before = state
                .store
                .get(&NoteId::new(slug))
                .await
                .unwrap()
                .expect("note present")
                .content;
            assert_eq!(
                before.matches("status:: todo").count(),
                1,
                "seed must have exactly one status line; render:\n{before}"
            );
            let flipped = before.replace("status:: todo", "status:: done");

            let res = update_note(
                Path(slug.to_string()),
                State(Arc::clone(&state)),
                Json(UpdateNoteReq {
                    content: flipped,
                    base_content: None,
                }),
            )
            .await;
            assert!(res.is_ok(), "update_note should succeed");

            let rendered = state
                .store
                .get(&NoteId::new(slug))
                .await
                .unwrap()
                .expect("note present")
                .content;
            assert!(
                rendered.contains("deadline:: [[2026-05-09]]"),
                "deadline must advance to 05-09 (silent no-op on base); render:\n{rendered}"
            );
            assert!(
                rendered.contains("recurrence_done:: 2"),
                "recurrence_done must roll to 2 (silent no-op on base); render:\n{rendered}"
            );
            assert!(
                rendered.contains("last_completed:: [[2026-05-08]]"),
                "last_completed must stamp 05-08; render:\n{rendered}"
            );
            assert!(
                rendered.contains("status:: todo") && !rendered.contains("status:: done"),
                "status must render as the rolled todo, not the flipped done; render:\n{rendered}"
            );
        }

        /// Round 3 (t2): a container seeded via `SetScalar` (the engine
        /// lifecycle hook's real shape and Taylor's live-doc shape) then ONE
        /// `set_block_property` completion. On base AND HEAD this 500s — a
        /// PRE-EXISTING bug of the SAME representation-collision class: the
        /// route's `SetText` write over the scalar seed hard-errors ("Expected
        /// value type Text but found Value(String(todo))"). The representation-
        /// tolerant write layer clears the incompatible scalar occupant and
        /// mints the text child, so the completion succeeds and rolls.
        #[tokio::test]
        async fn set_block_property_completion_over_scalar_seed_succeeds() {
            let tmp = tempfile::TempDir::new().unwrap();
            let slug = "chores";
            let state = seeded_state(tmp.path(), slug, &resident_seed()).await;

            let res = set_block_property(
                State(Arc::clone(&state)),
                Json(SetBlockPropertyReq {
                    block_id: format!("{slug}:{BID_HEX}"),
                    key: "status".to_string(),
                    value: "done".to_string(),
                }),
            )
            .await;
            if let Err(e) = res {
                let status = e.into_response().status();
                panic!("completion over a scalar-seeded container must not 500; got {status}");
            }

            let rendered = state
                .store
                .get(&NoteId::new(slug))
                .await
                .unwrap()
                .expect("note present")
                .content;
            assert!(
                rendered.contains("recurrence_done:: 2"),
                "the roll must advance recurrence_done to 2; render:\n{rendered}"
            );
        }

        /// Round 3 (t1): TWO consecutive `set_block_property(status=done)`
        /// completions of ONE recurring block, single device, no relay. On
        /// base/HEAD the SECOND completion 500s — the first completion's roll
        /// leaves `status` a SCALAR, and round 2's `SetText` write over it
        /// hard-errors, making a recurring completion a ONE-SHOT (the reviewer's
        /// empirically-reproduced c1). With the representation-tolerant write
        /// layer + the aligned roll writer, BOTH succeed and the recurrence
        /// advances twice. This test FAILS at HEAD (it is inherently
        /// revert-discriminating for the fix).
        #[tokio::test]
        async fn set_block_property_two_consecutive_completions_advance_recurrence() {
            let tmp = tempfile::TempDir::new().unwrap();
            let slug = "chores";
            // A FRESH `daily count 5` block: `recurrence_done` defaults to 0, so
            // two completions must reach 2. Seeded via `SetScalar` (production
            // shape) — no `recurrence_done`/`last_completed` yet.
            let fresh: Vec<(&str, &str)> = vec![
                ("recurring", "daily count 5"),
                ("deadline", "[[2026-05-08]]"),
                ("status", "todo"),
            ];
            let state = seeded_state(tmp.path(), slug, &fresh).await;

            for round in 1..=2 {
                let res = set_block_property(
                    State(Arc::clone(&state)),
                    Json(SetBlockPropertyReq {
                        block_id: format!("{slug}:{BID_HEX}"),
                        key: "status".to_string(),
                        value: "done".to_string(),
                    }),
                )
                .await;
                if let Err(e) = res {
                    let status = e.into_response().status();
                    panic!("completion round {round} must not 500; got {status}");
                }
            }

            let rendered = state
                .store
                .get(&NoteId::new(slug))
                .await
                .unwrap()
                .expect("note present")
                .content;
            assert!(
                rendered.contains("recurrence_done:: 2"),
                "two completions must advance recurrence_done to 2; render:\n{rendered}"
            );
            assert!(
                rendered.contains("status:: todo") && !rendered.contains("status:: done"),
                "status must reset to todo after the second roll; render:\n{rendered}"
            );
        }

        /// Round 3 (t5): `update_note` must fire `WsEvent::RecurringRolled` from
        /// the GUARDED roll (`persist_lifecycle_rolls`), not the UNGUARDED
        /// `apply_post_save_bumps_with_info` bumps. Seed an occurrence that is
        /// ALREADY completed (`last_completed == deadline`) then re-flip it to
        /// done: the idempotence guard trips, so NOTHING is persisted to the
        /// container (recurrence_done stays 1 — the container value wins render
        /// dedup) and therefore NO `RecurringRolled` event must be emitted. On
        /// base/HEAD the event fires from the unguarded bump, contradicting the
        /// persisted state.
        #[tokio::test]
        async fn update_note_recurring_rolled_event_matches_guarded_roll() {
            let tmp = tempfile::TempDir::new().unwrap();
            let slug = "chores";
            // deadline == last_completed → this occurrence is already recorded,
            // so a re-flip to done trips the idempotence guard.
            let already_completed: Vec<(&str, &str)> = vec![
                ("recurring", "daily count 3"),
                ("deadline", "[[2026-05-08]]"),
                ("recurrence_done", "1"),
                ("last_completed", "[[2026-05-08]]"),
                ("status", "todo"),
            ];
            let state = seeded_state(tmp.path(), slug, &already_completed).await;

            let before = state
                .store
                .get(&NoteId::new(slug))
                .await
                .unwrap()
                .expect("note present")
                .content;
            let flipped = before.replace("status:: todo", "status:: done");

            let mut rx = state.ws_tx.subscribe();
            let res = update_note(
                Path(slug.to_string()),
                State(Arc::clone(&state)),
                Json(UpdateNoteReq {
                    content: flipped,
                    base_content: None,
                }),
            )
            .await;
            assert!(res.is_ok(), "update_note should succeed");

            // Self-validation: the guard tripped, so the container roll authored
            // nothing — recurrence_done must still render 1 (the stale in-text
            // markdown bump loses render-time dedup to the container value).
            let rendered = state
                .store
                .get(&NoteId::new(slug))
                .await
                .unwrap()
                .expect("note present")
                .content;
            assert!(
                rendered.contains("recurrence_done:: 1") && !rendered.contains("recurrence_done:: 2"),
                "guard-tripped re-completion must NOT advance the container; render:\n{rendered}"
            );

            // Revert-discrimination: no RecurringRolled event may be emitted for
            // a roll that never touched the container.
            let mut rolled_events = 0;
            while let Ok(ev) = rx.try_recv() {
                if matches!(ev, WsEvent::RecurringRolled { .. }) {
                    rolled_events += 1;
                }
            }
            assert_eq!(
                rolled_events, 0,
                "a guard-tripped completion must emit NO RecurringRolled event"
            );
        }
    }

    /// Unlinked-reference scanning (tesela-qy4): a plain-text mention of the
    /// title, case-insensitively, is surfaced as an unlinked reference.
    #[test]
    fn find_unlinked_mentions_matches_title_case_insensitively() {
        let content = "- Talked to Alice about Loro Migration today.\n";
        let needles = vec!["loro migration".to_string()];
        let found = find_unlinked_mentions(content, "daily-2026-06-01", &needles);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].target, "daily-2026-06-01");
        assert!(found[0].text.contains("Loro Migration"));
    }

    /// Aliases are scanned in addition to the title.
    #[test]
    fn find_unlinked_mentions_matches_alias() {
        let content = "- Filed a bug against sync-engine yesterday.\n";
        let needles = vec!["loro migration".to_string(), "sync-engine".to_string()];
        let found = find_unlinked_mentions(content, "note-a", &needles);
        assert_eq!(found.len(), 1);
        assert!(found[0].text.contains("sync-engine"));
    }

    /// A line already carrying a `[[wiki-link]]` to the same needle is a
    /// real backlink, not an unlinked mention — must be excluded.
    #[test]
    fn find_unlinked_mentions_skips_already_linked_line() {
        let content = "- See [[Loro Migration]] for details.\n- Loro Migration is slow.\n";
        let needles = vec!["loro migration".to_string()];
        let found = find_unlinked_mentions(content, "note-a", &needles);
        assert_eq!(found.len(), 1, "only the unlinked line should be surfaced");
        assert!(found[0].text.contains("Loro Migration is slow"));
    }

    /// Needles shorter than the minimum length guard are ignored entirely
    /// (avoids matching short common words like "on").
    #[test]
    fn find_unlinked_mentions_applies_min_length_guard() {
        let content = "- Turn the light on before you leave.\n";
        let needles = vec!["on".to_string()];
        let found = find_unlinked_mentions(content, "note-a", &needles);
        assert!(
            found.is_empty(),
            "needles shorter than UNLINKED_MIN_NEEDLE_LEN must be skipped"
        );
    }

    /// Matches inside fenced code blocks are not real prose mentions and
    /// must be skipped.
    #[test]
    fn find_unlinked_mentions_skips_code_fences() {
        let content =
            "- prose mention: Loro Migration is done\n```\nlet x = \"Loro Migration\";\n```\n";
        let needles = vec!["loro migration".to_string()];
        let found = find_unlinked_mentions(content, "note-a", &needles);
        assert_eq!(found.len(), 1, "the fenced match must be excluded");
        assert!(found[0].text.starts_with("- prose mention"));
    }

    /// Word-boundary check still applies: a needle embedded inside a larger
    /// word is not a real mention.
    #[test]
    fn find_unlinked_mentions_requires_word_boundary() {
        let content = "- Superloro Migrations happen weekly.\n";
        let needles = vec!["loro migration".to_string()];
        let found = find_unlinked_mentions(content, "note-a", &needles);
        assert!(found.is_empty());
    }

    /// `code_fence_ranges` covers a closed fence and treats a dangling
    /// opening fence as extending to the end of the content.
    #[test]
    fn code_fence_ranges_covers_closed_and_dangling_fences() {
        let content = "before\n```\ncode\n```\nafter\n```\ndangling\n";
        let ranges = code_fence_ranges(content);
        assert_eq!(ranges.len(), 2);
        let fence_body = &content[ranges[0].clone()];
        assert!(fence_body.contains("code"));
        let dangling_body = &content[ranges[1].clone()];
        assert!(dangling_body.contains("dangling"));
        assert_eq!(ranges[1].end, content.len());
    }
}

/// A block's bid-stripped multi-line prose (`FlatBlock.text` shape — first
/// line + continuation lines, no `<!-- bid -->`) plus its indent level and
/// parent. `parent` is carried so the synthesized prose-strip `BlockUpsert`
/// preserves the node's real `parent` meta instead of resetting it to
/// top-level (the meta BlockDelete's child-reparenting keys on).
struct StrippedBlockProse {
    text: String,
    indent: u16,
    parent: Option<uuid::Uuid>,
}

/// If the block identified by `block_bid` in `content` carries `key` as an
/// in-text `key:: value` continuation line, return its prose with THAT line
/// removed (so the line can be lifted into the typed container without
/// duplicating). Returns `None` when the block has no such in-text line (so
/// the caller skips the redundant prose update). Only the target `key`'s line
/// is stripped — other in-text properties are preserved verbatim.
fn strip_block_intext_prop(
    content: &str,
    block_bid: &str,
    key: &str,
) -> Option<StrippedBlockProse> {
    let tree = parse_note(content);
    let target = uuid::Uuid::parse_str(block_bid).ok()?;
    let block = tree.blocks.iter().find(|b| b.id == target)?;
    let mut kept: Vec<&str> = Vec::new();
    let mut removed_any = false;
    for line in block.text.lines() {
        if let Some((k, _)) = property_kv(line) {
            if k == key {
                removed_any = true;
                continue;
            }
        }
        kept.push(line);
    }
    if !removed_any {
        return None;
    }
    Some(StrippedBlockProse {
        text: kept.join("\n"),
        indent: block.indent,
        parent: block.parent,
    })
}

/// Choose the [`PropOp`]s a `set-property` request maps to, from the
/// property's registry `value_type`. Free-text → one `SetText`; multi-value
/// (`multiselect`, or the `tags` convention) → a `Clear` followed by one
/// `AddToList` per comma-separated item (so a route-driven set replaces the
/// list deterministically); any other type → one `SetScalar` coerced via
/// `parse_scalar`. An unknown property degrades to a `Text` scalar
/// (coerce-and-keep — the registry is advisory, never a write gate).
async fn prop_ops_for_set(s: &Arc<AppState>, key: &str, value: &str) -> Vec<PropOp> {
    let value_type = lookup_value_type(s, key).await;
    match value_type {
        ValueType::Text => vec![PropOp::SetText(value.to_string())],
        ValueType::MultiSelect => list_set_ops(value),
        // The `tags` convention is multi-value even without a registry entry.
        _ if key == "tags" => list_set_ops(value),
        vt => vec![PropOp::SetScalar(parse_scalar(vt, value))],
    }
}

/// `Clear` then one `AddToList` per non-empty comma-separated item.
fn list_set_ops(value: &str) -> Vec<PropOp> {
    let mut ops = vec![PropOp::Clear];
    for item in value.split(',') {
        let item = item.trim();
        if !item.is_empty() {
            ops.push(PropOp::AddToList(PropScalar::Text(item.to_string())));
        }
    }
    ops
}

/// Look up a property's `value_type` from the registry (case-insensitive by
/// key). Degrades to `Text` for an unknown property — the safe default.
async fn lookup_value_type(s: &Arc<AppState>, key: &str) -> ValueType {
    match s.index.get_all_property_defs().await {
        Ok(defs) => defs
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(key))
            .map(|d| ValueType::parse(&d.value_type))
            .unwrap_or(ValueType::Text),
        Err(e) => {
            tracing::warn!("set-property: registry lookup for '{key}' failed: {e}");
            ValueType::Text
        }
    }
}

/// Remove a property from a block and persist. Block-granular counterpart of
/// `set_block_property` for the *clear* case (TagTable / KanbanBoard "unset").
///
/// ## P1.10 — clear through the engine's typed container
/// Emits `OpPayload::BlockPropertySet { value: PropOp::Clear }` through the
/// sync engine, removing the key from the block node's `props`/`prop_keys`
/// containers (the materializer then drops the `key:: value` line). This
/// replaces the old whole-note markdown rewrite + re-diff: clearing one
/// property no longer re-asserts every other block (clobber-prone for
/// concurrent peer edits). Absent key is a safe no-op in the apply arm.
pub async fn clear_block_property(
    State(s): State<Arc<AppState>>,
    Json(req): Json<ClearBlockPropertyReq>,
) -> AppResult<Json<serde_json::Value>> {
    let (note_id_str, id_suffix) = match req.block_id.rsplit_once(':') {
        Some(pair) => pair,
        None => {
            return Err(AppError::Validation(format!(
                "invalid block_id '{}': expected '<note_id>:<line>' or '<note_id>:<bid>'",
                req.block_id
            )))
        }
    };

    let key = req.key.trim().to_lowercase();
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
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

    // Resolve the target block's canonical bid so the clear op addresses the
    // engine's node directly (mirror `set_block_property`): a numeric suffix is
    // a `<note_id>:<line>` resolved against the body, a non-numeric one is a
    // stable bid passed directly by the editor seam.
    let block_bid =
        block_bid_from_suffix(&prev_content, note_id_str, id_suffix).ok_or_else(|| {
            AppError::NotFound(format!(
                "block '{}' not found in note '{}'",
                req.block_id, note_id_str
            ))
        })?;
    let block_id = parse_bid(&block_bid)?;
    let doc_note_id = stable_uuid_from_slug(note_id_str);

    // Bootstrap-before-author (same class as set_block_property): clearing a prop
    // on a non-resident synced daily must land on the relay's lineage, not a
    // disjoint fork. Best-effort + resident-gated.
    bootstrap_note_if_needed(&s, note_id_str).await;

    let payload = OpPayload::BlockPropertySet {
        note_id: doc_note_id,
        block_id,
        key: key.clone(),
        value: PropOp::Clear,
    };
    if let Err(e) = s.sync_engine.record_local(payload).await {
        tracing::warn!(
            "sync: record_local BlockPropertySet(Clear) failed for {}: {e}",
            req.block_id
        );
        return Err(AppError::Internal(anyhow::anyhow!(
            "Failed to record BlockPropertySet(Clear): {e}"
        )));
    }

    let updated = s.store.get(&note_id).await?.ok_or_else(|| {
        AppError::NotFound(format!(
            "Note not found after clear-property: {}",
            note_id_str
        ))
    })?;

    s.index.reindex(&updated).await?;
    {
        use tesela_core::link::extract_wiki_links;
        use tesela_core::traits::link_graph::LinkGraph;
        let links = extract_wiki_links(&updated.content);
        if let Err(e) = s.index.update_links(&note_id, &links).await {
            tracing::warn!(
                "Failed to update links on clear-property for {:?}: {}",
                note_id,
                e
            );
        }
    }
    if updated.content != prev_content {
        if let Err(e) = s
            .index
            .record_version(&note_id, Some(&prev_content), &updated.content, 200)
            .await
        {
            tracing::warn!("Failed to record version on clear-property: {}", e);
        }
    }

    let _ = s.ws_tx.send(WsEvent::NoteUpdated { note: updated });

    tracing::info!("clear-property: {}::{}", req.block_id, key);
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Auto-create tag pages for any tags in the note that don't have a corresponding page.
/// Scans both frontmatter tags AND inline #tags in the body. Tag collection
/// itself (frontmatter + inline `#tag` + block `tags::` lines) is the pure
/// `tesela_core::lifecycle::collect_note_tags` (tesela-ows.1 step 1); this
/// wrapper owns the I/O (slug resolution, page creation, sync fan-out).
async fn ensure_tag_pages(s: &Arc<AppState>, note: &Note) {
    let all_tags = tesela_core::lifecycle::collect_note_tags(note);

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
        let content = tesela_core::lifecycle::tag_page_content(tag);
        match s.store.create(&resolved_slug, &content, &[]).await {
            Ok(tag_note) => {
                let _ = s.index.reindex(&tag_note).await;
                // Bootstrap-before-author (convergence fix M1, 2026-06-29): this
                // auto-created tag-page slug can already exist on the relay
                // (authored on another device); adopt its lineage as a shared
                // base before `record_sync_create` mints a fresh disjoint doc,
                // else they later union into same-bid twins. No-op once resident
                // / absent on the relay.
                bootstrap_note_if_needed(s, tag_note.id.as_str()).await;
                // Sync visibility: peers need a NoteUpsert in the
                // oplog so subsequent BlockUpserts against this
                // page can resolve its slug. `ensure_tag_pages` is a
                // best-effort fan-out (it must not fail the note save
                // that triggered it), so a record failure here logs
                // instead of propagating — unlike the handler-level
                // `record_sync_*` call sites (audit A9a).
                if let Err(e) = record_sync_create(s, &tag_note).await {
                    tracing::warn!("ensure_tag_pages: {e}");
                }
                let _ = s.ws_tx.send(WsEvent::NoteCreated { note: tag_note });
                tracing::info!(
                    "Auto-created tag page at slug '{}' (display name: '{}')",
                    resolved_slug,
                    tag
                );
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to auto-create tag page '{}' at slug '{}': {}",
                    tag,
                    resolved_slug,
                    e
                );
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
async fn resolve_free_tag_slug(
    s: &Arc<AppState>,
    slug_base: &str,
) -> Result<Option<String>, String> {
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

#[cfg(test)]
mod strip_block_intext_prop_tests {
    use super::*;

    /// Block-id resolution accepts BOTH a `<note>:<line>` (resolved against the
    /// materialized body) AND a `<note>:<bid>` (passed straight through — the
    /// editor seam's stale-proof address). Regression guard for the editor
    /// seam fix (f90eefe / 699041b): a non-numeric suffix must be treated as a
    /// bid, not rejected.
    #[test]
    fn block_bid_from_suffix_accepts_line_and_bid() {
        let bid = "019e9a49-cb5c-76a1-b8c2-540abd2362f2";
        let content = format!("---\ntitle: \"T\"\ntags: []\n---\n- buy milk <!-- bid:{bid} -->\n");
        // Numeric suffix → resolved against the body to the block's bid.
        assert_eq!(
            block_bid_from_suffix(&content, "T", "0").as_deref(),
            Some(bid),
            "a `<note>:<line>` suffix resolves to the block's bid",
        );
        // Non-numeric suffix → used directly (the line index goes stale on
        // reflow; the bid never does).
        assert_eq!(
            block_bid_from_suffix(&content, "T", bid).as_deref(),
            Some(bid),
            "a `<note>:<bid>` suffix is used directly",
        );
        // A line index that matches no block → None (the route surfaces 404).
        assert!(block_bid_from_suffix(&content, "T", "99").is_none());
    }

    /// A nested block carrying an in-text property must surface its real
    /// `parent` so the synthesized prose-strip `BlockUpsert` preserves the
    /// node's `parent` meta. Hardcoding `None` here silently reparents the
    /// block to top-level — invisible in materialized markdown (render is
    /// indent-based) but it breaks a later parent BlockDelete's child
    /// reparenting (the reparent loop keys on `parent == deleted_hex`).
    #[test]
    fn strip_preserves_nested_block_parent() {
        let parent_id = uuid::Uuid::now_v7();
        let child_id = uuid::Uuid::now_v7();
        let content = format!(
            "---\ntitle: \"X\"\n---\n\n- parent <!-- bid:{} -->\n  - child <!-- bid:{} -->\n    status:: todo\n",
            parent_id, child_id
        );

        // Sanity: parse_note sees the child as a real child of the parent.
        let tree = parse_note(&content);
        let child = tree
            .blocks
            .iter()
            .find(|b| b.id == child_id)
            .expect("child block parsed");
        assert_eq!(child.parent, Some(parent_id), "child parents to parent");

        let stripped = strip_block_intext_prop(&content, &child_id.to_string(), "status")
            .expect("status:: line is stripped");
        assert_eq!(
            stripped.parent,
            Some(parent_id),
            "stripped prose must carry the real parent, not None",
        );
        assert_eq!(stripped.indent, 1, "nested block keeps its indent");
        assert_eq!(stripped.text, "child", "status:: line removed from prose");
    }

    /// A top-level block has no parent — the stripped prose reports `None`.
    #[test]
    fn strip_top_level_block_parent_is_none() {
        let id = uuid::Uuid::now_v7();
        let content = format!(
            "---\ntitle: \"X\"\n---\n\n- task <!-- bid:{} -->\n  status:: todo\n",
            id
        );

        let stripped = strip_block_intext_prop(&content, &id.to_string(), "status")
            .expect("status:: line is stripped");
        assert_eq!(stripped.parent, None, "top-level block has no parent");
        assert_eq!(stripped.indent, 0);
        assert_eq!(stripped.text, "task");
    }
}
