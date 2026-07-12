use super::*;

impl LoroEngine {
    /// Render a note's current state as markdown by walking its Loro
    /// tree and feeding `tesela_core::serialize_note`, the same renderer
    /// SqliteEngine uses on disk. Gives the divergence check a
    /// byte-identical comparison surface (modulo frontmatter, which is
    /// on the file but not the shadow).
    ///
    /// **Ordering model matches SqliteEngine exactly:** a flat list in
    /// insertion (document) order, each block rendered at its stored
    /// `indent_level`. SqliteEngine never reorders by `order_key` and
    /// keeps document position stable across moves (a move only changes
    /// indent), so the shadow does the same — all blocks live directly
    /// under root in creation order, and `tree.children(Root)` returns
    /// them in that order.
    ///
    /// Returns `None` for unknown note ids.
    pub async fn render_note(&self, note_id: [u8; 16]) -> Option<String> {
        // The views registry doc has no markdown view — `None` keeps every
        // render-driven note walker (divergence checks, CLI backfills)
        // skipping it like an unknown note.
        if Self::is_views_doc(&note_id) {
            return None;
        }
        let doc = self.lazy_load_doc(note_id).await?;
        Some(tesela_core::note_tree::serialize_note(&note_tree_from_doc(
            &doc, None,
        )))
    }

    /// Render the *complete* `.md` file the engine writes to disk as the
    /// authoritative writer: verbatim frontmatter (root `frontmatter` meta)
    /// + page properties + blocks. Identical to
    ///   [`render_note`](Self::render_note) except the frontmatter is
    ///   included, so this is the exact byte stream materialization emits.
    ///   Delegates to [`doc_full_markdown`], which also handles pre-dedup docs
    ///   that still carry the full markdown on root `content`.
    ///
    /// A note whose frontmatter never reached the doc materializes
    /// body-only.
    ///
    /// Returns `None` for unknown note ids.
    pub async fn render_note_full(&self, note_id: [u8; 16]) -> Option<String> {
        // Views registry doc: not a note, no markdown view (see render_note).
        if Self::is_views_doc(&note_id) {
            return None;
        }
        let doc = self.lazy_load_doc(note_id).await?;
        Some(doc_full_markdown(&doc))
    }

    /// Resolve a note's filename slug. Reads the doc's `root.slug` meta
    /// (set on every NoteUpsert), falling back to the index entry. Used
    /// to name the materialized `<slug>.md` file.
    pub(super) async fn slug_for_note(&self, note_id: [u8; 16]) -> Option<String> {
        {
            let docs = self.inner.docs.read().await;
            if let Some(doc) = docs.get(&note_id) {
                let slug = doc
                    .get_map("root")
                    .get("slug")
                    .and_then(|v| v.into_value().ok())
                    .and_then(|v| v.into_string().ok())
                    .map(|s| (*s).clone())
                    .unwrap_or_default();
                if !slug.is_empty() {
                    return Some(slug);
                }
            }
        }
        let key = hex_id(&note_id);
        self.index_entries()
            .await
            .into_iter()
            .find(|e| e.note_id == key)
            .map(|e| e.slug)
            .filter(|s| !s.is_empty())
    }

    /// Write the note's canonical full `.md` (frontmatter + body) to
    /// `<materialize_dir>/<slug>.md` via atomic tmp+rename. No-op when
    /// `materialize_dir` is unset (non-authoritative) or the slug can't
    /// be resolved. This is what makes LoroEngine the sole writer of the
    /// mosaic in authoritative mode.
    pub(super) async fn materialize_note(&self, note_id: [u8; 16]) {
        if let Err(e) = self.materialize_note_checked(note_id).await {
            tracing::warn!(
                "tesela-sync/loro: materialize {}: {e}",
                hex_id(&note_id)
            );
        }
    }

    pub(super) async fn materialize_note_checked(
        &self,
        note_id: [u8; 16],
    ) -> SyncResult<()> {
        // The views registry doc never materializes to notes/ — it has no
        // slug and is not a note (it would otherwise warn every import).
        if Self::is_views_doc(&note_id) {
            return Ok(());
        }
        let Some(dir) = self.inner.materialize_dir.as_ref() else {
            return Ok(());
        };
        let Some(full) = self.render_note_full(note_id).await else {
            return Err(SyncError::Storage(format!(
                "cannot render {}",
                hex_id(&note_id)
            )));
        };
        let Some(slug) = self.slug_for_note(note_id).await else {
            return Err(SyncError::Storage(format!(
                "cannot materialize {} — no slug",
                hex_id(&note_id)
            )));
        };
        let path = dir.join(format!("{slug}.md"));
        let tmp = unique_tmp(&path);
        tokio::fs::write(&tmp, full.as_bytes())
            .await
            .map_err(|e| {
                SyncError::Storage(format!("materialize write {}: {e}", tmp.display()))
            })?;
        if let Err(e) = tokio::fs::rename(&tmp, &path).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(SyncError::Storage(format!(
                "materialize rename {}: {e}",
                path.display()
            )));
        }
        Ok(())
    }

    /// Remove a materialized `<slug>.md` (authoritative NoteDelete). No-op
    /// when `materialize_dir` is unset or the file is already gone.
    pub(super) async fn remove_materialized(&self, slug: &str) {
        let Some(dir) = self.inner.materialize_dir.as_ref() else {
            return;
        };
        let path = dir.join(format!("{slug}.md"));
        if let Err(e) = tokio::fs::remove_file(&path).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(
                    "tesela-sync/loro: materialize delete {}: {e}",
                    path.display()
                );
            }
        }
    }
}

/// Page-property storage: an ordered `LoroList` named "page_props" on
/// the note doc, holding key, value, key, value, … (interleaved). Page
/// properties arrive wholesale via NoteUpsert (full-content reparse),
/// so we rewrite the whole list each time — clear + repush. Ordered so
/// render reproduces on-disk order deterministically. (When granular
/// per-property merge lands — the deferred multi-value work — this
/// becomes a map/movable-list with per-key updates.)
pub(super) fn set_page_properties(doc: &LoroDoc, props: &[(String, String)]) -> SyncResult<()> {
    let list = doc.get_list("page_props");
    let len = list.len();
    if len > 0 {
        list.delete(0, len)
            .map_err(|e| SyncError::Storage(format!("loro page_props clear: {e}")))?;
    }
    for (k, v) in props {
        list.push(k.as_str())
            .map_err(|e| SyncError::Storage(format!("loro page_props push: {e}")))?;
        list.push(v.as_str())
            .map_err(|e| SyncError::Storage(format!("loro page_props push: {e}")))?;
    }
    Ok(())
}

/// Walk a doc's `blocks` tree into a `NoteTree` — flat blocks in document
/// (insertion) order at their stored indent, plus the ordered page
/// properties — attaching the given `frontmatter`. Shared renderer behind
/// both [`LoroEngine::render_note`] (frontmatter `None`, the shadow
/// comparison surface) and [`LoroEngine::render_note_full`] (frontmatter
/// from the doc's stored content, the exact bytes materialization emits).
fn note_tree_from_doc(
    doc: &LoroDoc,
    frontmatter: Option<String>,
) -> tesela_core::note_tree::NoteTree {
    let tree = doc.get_tree("blocks");
    let mut blocks: Vec<tesela_core::note_tree::FlatBlock> = Vec::new();
    // Live root children in walk order, mirroring the `is_node_deleted`
    // filtering used elsewhere, then collapse any duplicate-bid twins to a
    // single canonical node (Loro unions same-bid nodes minted on disjoint
    // histories — see `dedup_twins_by_block_id`). Render-side heal so an
    // already-corrupted on-disk doc shows each block exactly once.
    let live: Vec<TreeID> = tree
        .children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|n| !matches!(tree.is_node_deleted(n), Ok(true)))
        .collect();
    for node in dedup_twins_by_block_id(&tree, live) {
        if let Some(fb) = flatblock_from_node(&tree, node) {
            // NOTE: blank (empty) bullets are KEPT. They are the editing
            // surface — the web outliner relies on a trailing empty bullet
            // existing so an "empty" day has a focusable row to type into
            // (`JournalView.ensureTrailingEmpty`). Dropping them made empty
            // days zero-block and un-editable (keyboard + mouse), so the
            // 2026-05-29 "drop blank blocks" experiment is reverted.
            // Lifted headings, prose, and fences are ordinary FlatBlocks and
            // therefore follow this exact same render path.
            blocks.push(fb);
        }
    }
    tesela_core::note_tree::NoteTree {
        frontmatter,
        page_properties: page_properties_materialized(doc),
        blocks,
        stamped_any: false,
    }
}

/// Page-level properties for materialization: container `props`/`prop_keys`
/// at the doc root FIRST (canonical order), then any LEGACY `page_props`
/// entry whose key the container hasn't already supplied. Container props
/// and legacy `page_props` are disjoint stores at this stage (P1.5);
/// migrate-on-write that folds legacy into the container and CLEARS it is
/// P1.6, so for now we surface both without double-emitting a shared key.
pub(super) fn page_properties_materialized(doc: &LoroDoc) -> Vec<(String, String)> {
    let (props, prop_keys) = prop_containers::page_prop_containers(doc);
    let mut out = prop_containers::materialize_props(&props, &prop_keys);
    let seen: std::collections::HashSet<String> = out.iter().map(|(k, _)| k.clone()).collect();
    for (k, v) in read_page_properties(doc) {
        if !seen.contains(&k) {
            out.push((k, v));
        }
    }
    out
}

/// Read the ordered page properties back out of the "page_props" list.
fn read_page_properties(doc: &LoroDoc) -> Vec<(String, String)> {
    let list = doc.get_list("page_props");
    let len = list.len();
    let mut out = Vec::with_capacity(len / 2);
    let mut i = 0;
    while i + 1 < len {
        let k = list
            .get(i)
            .and_then(|v| v.into_value().ok())
            .and_then(|v| v.into_string().ok())
            .map(|s| (*s).clone());
        let v = list
            .get(i + 1)
            .and_then(|v| v.into_value().ok())
            .and_then(|v| v.into_string().ok())
            .map(|s| (*s).clone());
        if let (Some(k), Some(v)) = (k, v) {
            out.push((k, v));
        }
        i += 2;
    }
    out
}

/// Read a per-note doc's verbatim frontmatter. Current-version docs store
/// it directly on root `frontmatter` (the lean schema — the body lives in
/// the tree, so the full markdown is never duplicated on root meta).
/// Pre-dedup docs instead stored the full markdown on root `content`; fall
/// back to parsing that so their frontmatter still renders until a reseed
/// rebuilds them lean. Returns `None` when neither is present (body-only).
fn doc_frontmatter(doc: &LoroDoc) -> Option<String> {
    let root = doc.get_map("root");
    let read = |k: &str| -> String {
        root.get(k)
            .and_then(|v| v.into_value().ok())
            .and_then(|v| v.into_string().ok())
            .map(|s| (*s).clone())
            .unwrap_or_default()
    };
    let fm = read("frontmatter");
    if !fm.is_empty() {
        return Some(fm);
    }
    let content = read("content");
    if !content.is_empty() {
        return tesela_core::note_tree::parse_note(&content).frontmatter;
    }
    None
}

/// Reconstruct the full `.md` for a per-note doc — frontmatter + rendered
/// body — which equals what materialization writes to disk and what the
/// index derives tags/links from. Lean (current-version) docs reconstruct
/// from the tree; pre-dedup docs that still carry the full markdown on root
/// `content` return it verbatim (matching the old derivation exactly until
/// a reseed converts them).
pub(super) fn doc_full_markdown(doc: &LoroDoc) -> String {
    let content = doc
        .get_map("root")
        .get("content")
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| (*s).clone())
        .unwrap_or_default();
    if !content.is_empty() {
        return content;
    }
    tesela_core::note_tree::serialize_note(&note_tree_from_doc(doc, doc_frontmatter(doc)))
}

/// Seed a flat tree from `tesela_core::FlatBlock`s parsed out of a
/// NoteUpsert's body content. Used when LoroEngine sees a note for the
/// first time and the only op is the NoteUpsert.
///
/// All blocks are created directly under root in document order so
/// `tree.children(Root)` later returns them in that order — matching
/// SqliteEngine's flat-document-order model. `indent_level` carries the
/// visual hierarchy; the tree is intentionally flat.
/// Split a parsed block's `text` into prose-only lines plus the recognized
/// `key:: value` properties it carries in-text — the SAME conservative
/// migrate-strip classification the migrate-on-apply path (P1.6) uses, so a
/// prose-only migrated tree isn't seen as drifted vs an old peer's
/// in-text-property body.
///
/// `parse_note` folds property continuation lines INTO `FlatBlock.text`
/// (joined by `'\n'`) and leaves `FlatBlock.properties` empty, so the incoming
/// body's properties live in `text`. We lift a line ONLY when, after a trim, it
/// is SOLELY `key:: value` (a false-positive mid-prose strip is irreversible
/// text loss — same conservatism as the migrate path). Keys are lowercased to
/// match the engine's case-folding-on-write; values are kept verbatim, matching
/// the canonical string form `materialize_props` renders (a `Text` scalar /
/// comma-joined list both round-trip their stored string). NON-property lines
/// (prose, blanks-collapsed-already-by-the-parser) are kept as prose.
pub(super) fn classify_block_prose_and_props(text: &str) -> (String, Vec<(String, String)>) {
    let mut prose: Vec<&str> = Vec::new();
    let mut props: Vec<(String, String)> = Vec::new();
    let mut fence = tesela_core::note_tree::MarkdownFenceTracker::default();
    for line in text.split('\n') {
        if fence.line_is_fenced(line) {
            prose.push(line);
        } else if let Some((key, value)) = solely_property_line(line) {
            props.push((key, value));
        } else {
            prose.push(line);
        }
    }
    if props.is_empty() {
        (text.to_string(), props)
    } else {
        (prose.join("\n"), props)
    }
}

/// If `line` (after trim) is SOLELY a `key:: value` property — key a leading
/// identifier (`[A-Za-z_][A-Za-z0-9_]*`), then exactly `:: `, then a non-empty
/// value — return `(lowercased_key, value)`; otherwise `None`. Conservative:
/// anything that isn't the whole line being a property is left as prose.
fn solely_property_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    let (key, value) = trimmed.split_once(":: ")?;
    if value.is_empty() {
        return None;
    }
    let mut chars = key.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some((key.to_ascii_lowercase(), value.to_string()))
}

/// A4 render-time dedup: drop legacy in-text `key:: value` lines whose key
/// matches a container property key, so the materializer emits each property
/// ONCE (the container value wins). Keys are compared case-insensitively
/// (`solely_property_line` lowercases the in-text key; container keys are
/// stored verbatim, so we lowercase those too). Returns `text` byte-for-byte
/// unchanged when there are no container props OR no line is dropped, so the
/// common no-duplicate path is untouched.
pub(super) fn dedup_intext_props_against_container(
    text: String,
    properties: &[(String, String)],
) -> String {
    if properties.is_empty() {
        return text;
    }
    let container_keys: std::collections::HashSet<String> = properties
        .iter()
        .map(|(k, _)| k.to_ascii_lowercase())
        .collect();
    let mut fence = tesela_core::note_tree::MarkdownFenceTracker::default();
    let kept: Vec<&str> = text
        .split('\n')
        .filter(|line| {
            fence.line_is_fenced(line)
                || solely_property_line(line)
                    .map(|(k, _)| !container_keys.contains(&k))
                    .unwrap_or(true)
        })
        .collect();
    if kept.len() == text.split('\n').count() {
        // Nothing dropped — preserve the exact original bytes (incl. any
        // trailing-newline nuance) so non-duplicate blocks are unaffected.
        return text;
    }
    kept.join("\n")
}
