use super::*;
use std::collections::BTreeSet;

pub(super) type BlockOwners = BTreeSet<[u8; 16]>;
pub(super) type BlockIndex = HashMap<[u8; 16], BlockOwners>;

/// Every live block id in a Loro tree, including nodes whose structural
/// parent is another node. Ownership is independent of the flat renderer's
/// root-child projection, so legacy/non-canonical nesting must remain indexed.
pub(super) fn live_block_ids(tree: &LoroTree) -> impl Iterator<Item = [u8; 16]> + '_ {
    tree.nodes().into_iter().filter_map(|node| {
        if matches!(tree.is_node_deleted(&node), Ok(true)) {
            return None;
        }
        read_meta_str(tree, node, "block_id").and_then(|hex| parse_note_id_from_hex(&hex))
    })
}

/// Schema version of the index doc's entry shape. Bump whenever the
/// per-entry fields OR their encoding change so a stale on-disk index is
/// rebuilt from the (self-describing) per-note docs on boot — no manual
/// cache clear. v1 = {title, slug}. v2 = + {tags, links} (comma-joined).
/// v3 = tags/links newline-joined (comma collided with link targets like
/// `[[Smith, John]]` — review finding [7]). v4 = fenced regions excluded
/// from inline tag/link derivation.
pub(super) const INDEX_SCHEMA_VERSION: i64 = 4;

/// Delimiter for the multi-valued tags/links fields stored as a single
/// string in an index entry. Newline can't appear in a tag name
/// (`[A-Za-z0-9_/-]`) or a single-line `[[wiki-link]]` target, so it's
/// collision-free — unlike the comma it replaced.
const INDEX_LIST_SEP: char = '\n';

/// Join a multi-valued index field with the collision-free separator.
fn join_list(items: &[String]) -> String {
    items.join(&INDEX_LIST_SEP.to_string())
}

/// Build the block_id → owning note ids map from a set of loaded per-note
/// docs by reading each block node's `block_id` meta. Used at boot.
pub(super) fn build_block_index(docs: &HashMap<[u8; 16], LoroDoc>) -> BlockIndex {
    let mut out = BlockIndex::new();
    for (note_id, doc) in docs.iter() {
        if note_doc_is_deleted(doc) {
            continue;
        }
        let tree = doc.get_tree("blocks");
        for bid in live_block_ids(&tree) {
            out.entry(bid).or_default().insert(*note_id);
        }
    }
    out
}

/// Best-effort frontmatter `title:` extraction for index rebuild
/// fallback. Returns None if there's no frontmatter title.
pub(super) fn frontmatter_title(content: &str) -> Option<String> {
    tesela_core::storage::markdown::parse_frontmatter(content)
        .ok()
        .and_then(|(meta, _)| meta.title)
        .filter(|t| !t.is_empty())
}

/// Derive a note's index metadata `(tags, links)` from its content +
/// parsed page properties. Tags come from three sources (frontmatter
/// `tags:`, the `tags::` page property, inline `#tags`); links are
/// `[[wiki-link]]` targets. Both deduped + sorted.
fn extract_index_metadata(
    content: &str,
    page_properties: &[(String, String)],
) -> (Vec<String>, Vec<String>) {
    use std::collections::BTreeSet;
    let mut tags: BTreeSet<String> = BTreeSet::new();

    // Frontmatter `tags:` (gray_matter via tesela_core).
    if let Ok((meta, _body)) = tesela_core::storage::markdown::parse_frontmatter(content) {
        for t in meta.tags {
            if !t.is_empty() {
                tags.insert(t);
            }
        }
    }
    // `tags::` page property (comma- or space-separated).
    for (k, v) in page_properties {
        if k == "tags" {
            for t in v.split([',', ' ']) {
                let t = t.trim().trim_start_matches('#');
                if !t.is_empty() {
                    tags.insert(t.to_string());
                }
            }
        }
    }
    // The shared extractors use the structural note scanner, so fenced
    // code/query payload is inert even inside nested canonical blocks.
    for t in tesela_core::block::extract_tags_from_note(content) {
        if !t.is_empty() {
            tags.insert(t);
        }
    }

    let links: BTreeSet<String> = tesela_core::link::extract_wiki_links(content)
        .into_iter()
        .map(|l| l.target)
        .filter(|t| !t.is_empty())
        .collect();

    (tags.into_iter().collect(), links.into_iter().collect())
}

impl LoroEngine {
    /// Confirm that the persisted derived index is the exact projection of
    /// the durable note snapshots loaded at boot. Note-id coverage alone is
    /// insufficient: a failed batch-final checkpoint can leave current-schema
    /// title/tag/link metadata stale for an existing note.
    pub(super) async fn index_matches_loaded_docs(&self) -> bool {
        let existing: std::collections::HashMap<String, crate::engine::IndexEntry> = self
            .index_entries()
            .await
            .into_iter()
            .map(|entry| (entry.note_id.clone(), entry))
            .collect();
        let docs = self.inner.docs.read().await;
        let note_count = docs
            .iter()
            .filter(|(note_id, doc)| !Self::is_special_doc(note_id) && !note_doc_is_deleted(doc))
            .count();
        if existing.len() != note_count {
            return false;
        }

        for (note_id, doc) in docs.iter() {
            if Self::is_special_doc(note_id) || note_doc_is_deleted(doc) {
                continue;
            }
            let key = hex_id(note_id);
            let Some(entry) = existing.get(&key) else {
                return false;
            };
            let root = doc.get_map("root");
            let read = |field: &str| -> String {
                root.get(field)
                    .and_then(|value| value.into_value().ok())
                    .and_then(|value| value.into_string().ok())
                    .map(|value| (*value).clone())
                    .unwrap_or_default()
            };
            let content = doc_full_markdown(doc);
            let slug = match read("slug") {
                value if !value.is_empty() => value,
                _ => entry.slug.clone(),
            };
            let title = match read("title") {
                value if !value.is_empty() => value,
                _ if !entry.title.is_empty() => entry.title.clone(),
                _ => frontmatter_title(&content).unwrap_or_else(|| slug.clone()),
            };
            let parsed = tesela_core::note_tree::parse_note(&content);
            let (tags, links) = extract_index_metadata(&content, &parsed.page_properties);
            if entry.title != title
                || entry.slug != slug
                || entry.tags != tags
                || entry.links != links
            {
                return false;
            }
        }
        true
    }

    /// Rebuild every index entry from the loaded per-note docs. Each doc's
    /// full markdown is reconstructed via `doc_full_markdown` (frontmatter +
    /// rendered body, or the legacy root `content` for pre-dedup docs), and
    /// slug + title come from root meta, so the index is a derived
    /// projection. tags/links are always re-derived from that markdown.
    /// title/slug prefer the doc's root meta, then fall back to the
    /// existing index entry (so a rebuild against docs written by an
    /// older engine — which lack slug/title on root meta — doesn't lose
    /// the slugs the prior index already had), then to a frontmatter
    /// title. Stamps the current schema version.
    pub(super) async fn rebuild_index_from_docs(&self) {
        // Snapshot existing index title/slug as fallback.
        let existing: std::collections::HashMap<String, (String, String)> = self
            .index_entries()
            .await
            .into_iter()
            .map(|e| (e.note_id, (e.title, e.slug)))
            .collect();

        let docs = self.inner.docs.read().await;

        // Prune index entries that have no backing doc AT ALL, so the
        // rebuild is a TRUE projection — not an upsert-merge that leaves
        // ghost entries (review finding [6]). A doc can be genuinely absent
        // because its snapshot was corrupt/unreadable on load; its index
        // entry must not survive as a phantom note.
        //
        // "Backing doc" is deliberately checked against ON-DISK snapshots
        // (when `snapshot_dir` is set), NOT `docs.keys()` alone
        // (tesela-engc.5 audit, highest-severity unstubbed item): this
        // function currently only ever runs right after boot's eager
        // `load_snapshots_from_dir`, where the two sets are identical, but
        // `docs.keys()` alone would silently prune the index entry of any
        // note that's merely not memory-resident the instant that stops
        // being true (a future partial/lazy boot, or an evicted note) —
        // indistinguishable from "genuinely gone". An in-memory-only engine
        // (no `snapshot_dir`) has no disk to consult, so memory stays the
        // live set there.
        let live: std::collections::HashSet<String> = match self.inner.snapshot_dir.as_ref() {
            Some(dir) => snapshot_note_ids_on_disk(dir).await,
            None => docs.keys().map(hex_id).collect(),
        };
        let notes_map = self.inner.index.get_map("notes");
        let stale: Vec<String> = existing
            .keys()
            .filter(|k| !live.contains(*k))
            .cloned()
            .collect();
        for key in stale {
            let _ = notes_map.delete(&key);
        }

        for (note_id, doc) in docs.iter() {
            // The views registry doc is not a note — never index it.
            if Self::is_special_doc(note_id) {
                continue;
            }
            if note_doc_is_deleted(doc) {
                let _ = notes_map.delete(&hex_id(note_id));
                continue;
            }
            let root = doc.get_map("root");
            let read = |k: &str| -> String {
                root.get(k)
                    .and_then(|v| v.into_value().ok())
                    .and_then(|v| v.into_string().ok())
                    .map(|s| (*s).clone())
                    .unwrap_or_default()
            };
            let content = doc_full_markdown(doc);
            let key = hex_id(note_id);
            let prior = existing.get(&key);
            let mut slug = read("slug");
            if slug.is_empty() {
                slug = prior.map(|(_, s)| s.clone()).unwrap_or_default();
            }
            let mut title = read("title");
            if title.is_empty() {
                title = prior
                    .map(|(t, _)| t.clone())
                    .filter(|t| !t.is_empty())
                    .or_else(|| frontmatter_title(&content))
                    .unwrap_or_else(|| slug.clone());
            }
            let parsed = tesela_core::note_tree::parse_note(&content);
            self.index_upsert(
                *note_id,
                Some(slug.as_str()).filter(|s| !s.is_empty()),
                &title,
                &content,
                &parsed.page_properties,
            );
        }
        // Stamp schema version (index_upsert already stamps, but ensure
        // it's set even when there are zero docs).
        let _ = self
            .inner
            .index
            .get_map("meta")
            .insert("schema_version", INDEX_SCHEMA_VERSION);
        self.inner.index.commit();
    }

    /// Update the index entry for a note. Called on NoteUpsert. Stores
    /// title + slug + tags + outbound link targets — all derived from
    /// the note content and overwritten wholesale (the index is a
    /// derived projection of the notes).
    pub(super) fn index_upsert(
        &self,
        note_id: [u8; 16],
        slug: Option<&str>,
        title: &str,
        content: &str,
        page_properties: &[(String, String)],
    ) {
        let (tags, links) = extract_index_metadata(content, page_properties);
        let notes = self.inner.index.get_map("notes");
        let key = hex_id(&note_id);
        let entry = match notes.get(&key) {
            Some(loro::ValueOrContainer::Container(loro::Container::Map(m))) => m,
            _ => match notes.insert_container(&key, loro::LoroMap::new()) {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("tesela-sync/loro: index insert_container: {e}");
                    return;
                }
            },
        };
        let _ = entry.insert("title", title);
        let _ = entry.insert("slug", slug.unwrap_or(""));
        // Tags + links as comma-joined strings (derived, overwritten
        // wholesale; structured per-tag containers can come if granular
        // tag merge is ever needed).
        let _ = entry.insert("tags", join_list(&tags));
        let _ = entry.insert("links", join_list(&links));
        // Stamp the schema version so a freshly-built index (e.g. from
        // disk-seed) is recognized as current and not needlessly rebuilt
        // on the next boot.
        let _ = self
            .inner
            .index
            .get_map("meta")
            .insert("schema_version", INDEX_SCHEMA_VERSION);
        self.inner.index.commit();
    }

    /// Remove a note's index entry (NoteDelete).
    pub(super) fn index_remove(&self, note_id: [u8; 16]) {
        let notes = self.inner.index.get_map("notes");
        let _ = notes.delete(&hex_id(&note_id));
        self.inner.index.commit();
    }

    /// List all index entries. The hybrid model's note list — sourced
    /// from the always-resident index, no per-note docs loaded.
    pub async fn index_entries(&self) -> Vec<crate::engine::IndexEntry> {
        let notes = self.inner.index.get_map("notes");
        let value = notes.get_deep_value();
        let mut out = Vec::new();
        if let loro::LoroValue::Map(m) = value {
            for (key, v) in m.iter() {
                if let loro::LoroValue::Map(entry) = v {
                    let get = |k: &str| {
                        entry.get(k).and_then(|x| {
                            if let loro::LoroValue::String(s) = x {
                                Some((**s).to_string())
                            } else {
                                None
                            }
                        })
                    };
                    let split = |k: &str| -> Vec<String> {
                        get(k)
                            .filter(|s| !s.is_empty())
                            .map(|s| s.split(INDEX_LIST_SEP).map(|t| t.to_string()).collect())
                            .unwrap_or_default()
                    };
                    out.push(crate::engine::IndexEntry {
                        note_id: key.to_string(),
                        title: get("title").unwrap_or_default(),
                        slug: get("slug").unwrap_or_default(),
                        tags: split("tags"),
                        links: split("links"),
                    });
                }
            }
        }
        out.sort_by(|a, b| a.note_id.cmp(&b.note_id));
        out
    }

    /// Persist the index doc to `<dir>/_index.bin`. Best-effort.
    pub(super) async fn save_index_snapshot(&self, dir: &Path) {
        let _persist_guard = self.inner.index_persist_lock.lock().await;
        let bytes = match self.inner.index.export(ExportMode::Snapshot) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("tesela-sync/loro: index snapshot export: {e}");
                return;
            }
        };
        let path = dir.join("_index.bin");
        let tmp = unique_tmp(&path);
        if tokio::fs::write(&tmp, &bytes).await.is_ok() {
            if tokio::fs::rename(&tmp, &path).await.is_err() {
                let _ = tokio::fs::remove_file(&tmp).await;
            }
        } else {
            let _ = tokio::fs::remove_file(&tmp).await;
        }
    }
}
