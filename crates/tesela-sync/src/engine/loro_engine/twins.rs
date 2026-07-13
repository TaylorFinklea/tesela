use super::*;

impl LoroEngine {
    /// One-shot LOCAL repair scan (tesela-49d): report every note's DISJOINT
    /// twins (a block_id on >1 live tree node — the residue of pre-fix disjoint
    /// authoring) WITHOUT mutating anything. Returns `(note_id, block_id_hex,
    /// candidate_texts)` per twin block_id so a CLI can show what a repair would
    /// collapse. Read-only. (Pre-collapsed UNION *concatenation* on a single node
    /// is NOT a twin and is not reported here — that residue stays manual.)
    pub async fn scan_disjoint_twins(&self) -> Vec<([u8; 16], String, Vec<String>)> {
        let mut out = Vec::new();
        for note_id in self.note_ids().await {
            if Self::is_views_doc(&note_id) {
                continue;
            }
            let doc = self.doc_for_note_mut(note_id).await;
            let twin_bids = duplicate_block_ids(&doc);
            if twin_bids.is_empty() {
                continue;
            }
            let tree = doc.get_tree("blocks");
            let live: Vec<TreeID> = tree
                .children(TreeParentId::Root)
                .unwrap_or_default()
                .into_iter()
                .filter(|n| !matches!(tree.is_node_deleted(n), Ok(true)))
                .collect();
            let mut by_bid: HashMap<String, Vec<String>> = HashMap::new();
            for node in live {
                let Some(bid) = read_meta_str(&tree, node, "block_id") else {
                    continue;
                };
                if twin_bids.contains(&bid) {
                    by_bid
                        .entry(bid)
                        .or_default()
                        .push(read_block_text(&tree, node).unwrap_or_default());
                }
            }
            for (bid, texts) in by_bid {
                out.push((note_id, bid, texts));
            }
        }
        out
    }

    /// One-shot LOCAL repair (tesela-49d): collapse every note's DISJOINT twins
    /// to the deterministic max-`TreeID` survivor — the SAME rule the relay apply
    /// paths use ([`tombstone_duplicate_twins`] for the node keep +
    /// [`twin_winners_for`] for the props union), but FRAMELESS (operates on the
    /// doc's OWN existing twins, so residue can be healed without waiting for an
    /// inbound sync). Each changed note is persisted + materialized. Returns the
    /// `(note_id, block_id_hex)` collapsed. Idempotent: a second run finds no twins.
    ///
    /// Gated by the CALLER (dry-run default + mosaic lock in the CLI). Twins now
    /// also self-heal on the next relay round via the apply-path heal; this is
    /// the offline/force path for existing residue.
    ///
    /// Runs its own plan (`twin_winners_for`) → tombstone → prop-reassert
    /// sequence per note — the SAME shape as `apply_import`'s, and subject to
    /// the SAME interleave risk against a concurrent `apply_import` (or
    /// `record_local`) for that note (tesela-4ju REVIEW REJECT, 2026-07-02).
    /// Takes that note's `apply_locks` guard for the whole per-note sequence
    /// (see `Inner::apply_locks` for the ordering rule) before touching
    /// `docs`, closing the same window `apply_import` closes for itself.
    pub async fn heal_disjoint_twins(&self) -> Vec<([u8; 16], String)> {
        let mut healed = Vec::new();
        for note_id in self.note_ids().await {
            if Self::is_views_doc(&note_id) {
                continue;
            }
            let apply_lock = self.apply_lock_for_note(note_id).await;
            let _apply_guard = apply_lock.lock().await;
            let doc = self.doc_for_note_mut(note_id).await;
            let changes = twin_winners_for(&doc);
            if changes.is_empty() {
                continue;
            }
            let ownership_guard = self.inner.ownership_transition.lock().await;
            tombstone_duplicate_twins(&doc, note_id);
            self.reassert_prop_heals_under_ownership(note_id, &changes)
                .await
                .ok();
            self.refresh_note_derived_under_ownership(note_id, &doc)
                .await;
            drop(ownership_guard);
            if let Some(dir) = self.inner.snapshot_dir.as_ref() {
                self.save_snapshot(dir, note_id).await;
            }
            if self.inner.materialize_dir.is_some() {
                self.materialize_note(note_id).await;
            }
            for c in &changes {
                healed.push((note_id, hex_id(&c.block_id)));
            }
        }
        healed
    }

    /// Re-assert the disjoint-twin heal's RESOLVED props (captured from the fork
    /// BEFORE the tombstone) onto each survivor — the props half of the heal.
    /// Emits ONE [`OpPayload::BlockPropertySet`] PER resolved key, each
    /// idempotency-guarded against the survivor's CURRENT value:
    /// - **list** → `AddToList` only the MISSING members (a present value is a
    ///   no-op via the helper's union), so the winner's list is grown, never
    ///   replaced wholesale;
    /// - **scalar / text** → `SetScalar` / `SetText` only when the survivor's
    ///   current value differs from the target (LWW register — re-asserting an
    ///   equal value is a no-op skipped here to keep the apply idempotent).
    ///
    /// Called only while `apply_import` or `heal_disjoint_twins` holds both the
    /// note's apply guard and the global ownership-transition guard, so writes
    /// dispatch through `record_local_locked_under_ownership` rather than
    /// re-entering either non-reentrant mutex. A `BlockPropertySet` on a block
    /// whose survivor went missing is itself a safe no-op.
    pub(super) async fn reassert_prop_heals_under_ownership(
        &self,
        note_id: [u8; 16],
        changes: &[PeerBlockChange],
    ) -> SyncResult<()> {
        // Collect the (block_id, key, op) re-asserts from the current doc,
        // then author them through the already-guarded path.
        let mut block_ops: Vec<([u8; 16], String, PropOp)> = Vec::new();
        {
            let Some(doc) = self.lazy_load_doc(note_id).await else {
                return Ok(());
            };
            let tree = doc.get_tree("blocks");
            for c in changes {
                if c.props.is_empty() {
                    continue;
                }
                let bid_hex = hex_id(&c.block_id);
                let current: Vec<(String, ResolvedValue)> = find_node_by_block_id(&tree, &bid_hex)
                    .and_then(|node| tree.get_meta(node).ok())
                    .and_then(|meta| prop_containers::read_node_prop_containers(&meta))
                    .map(|(props, prop_keys)| prop_containers::read_props_typed(&props, &prop_keys))
                    .unwrap_or_default();
                for (key, target) in &c.props {
                    let cur = current.iter().find(|(k, _)| k == key).map(|(_, v)| v);
                    match target {
                        ResolvedValue::Scalar(s) => {
                            if cur != Some(&ResolvedValue::Scalar(s.clone())) {
                                block_ops.push((
                                    c.block_id,
                                    key.clone(),
                                    PropOp::SetScalar(s.clone()),
                                ));
                            }
                        }
                        ResolvedValue::Text(t) => {
                            if cur != Some(&ResolvedValue::Text(t.clone())) {
                                block_ops.push((
                                    c.block_id,
                                    key.clone(),
                                    PropOp::SetText(t.clone()),
                                ));
                            }
                        }
                        ResolvedValue::List(members) => {
                            let present = match cur {
                                Some(ResolvedValue::List(have)) => have.clone(),
                                _ => Vec::new(),
                            };
                            for m in members {
                                if !present.contains(m) {
                                    block_ops.push((
                                        c.block_id,
                                        key.clone(),
                                        PropOp::AddToList(m.clone()),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        for (block_id, key, value) in block_ops {
            // Both callers already hold this note's apply guard and the global
            // ownership-transition guard; do not re-enter either mutex.
            self.record_local_locked_under_ownership(OpPayload::BlockPropertySet {
                note_id,
                block_id,
                key,
                value,
            })
            .await?;
        }
        Ok(())
    }
}

/// Collapse duplicate-`block_id` twins to a single canonical node, returning
/// the survivors in their original `nodes` walk order.
///
/// **Why this exists (the bug):** Loro tree node identity is the internal
/// `TreeID` (peer + counter), NOT the `block_id` meta. When two engines that
/// never shared a Loro base both author the same bid (e.g. the Mac server
/// seeds a note from disk while iOS re-authors blocks from its own markdown),
/// each mints a DIFFERENT `TreeID` for that bid. On merge Loro UNIONS the
/// nodes, so two live nodes carry the same `block_id` meta → the renderer
/// emits the block twice and block-diff saves update only one twin (leaving a
/// stale ghost = "my web edit reverted on refresh"). This dedups them.
///
/// **Tie-break rule — lexicographically-MAX `TreeID` (higher `peer`, then
/// higher `counter`).** loro 1.12's `TreeID` exposes public `peer: u64` /
/// `counter: i32` fields and derives `Ord` over `(peer, counter)`, so this is a
/// stable, process-restart-independent comparator. MAX (not min) is the
/// provably-convergent pure rule (tesela-fte, decisions.md 2026-07-01): the
/// global-max `TreeID` twin ALWAYS survives on every replica in ONE round, so a
/// disjoint conflict can never cross-tombstone or split-brain. We deliberately
/// do NOT use a "most-recently-edited" rule: the `text` meta is a plain LWW map-register
/// (`meta.insert("text", ...)`), and loro 1.12's `LoroMap` only exposes
/// `get_last_editor(key) -> PeerID` — the *peer* that last wrote a key, not a
/// comparable per-update lamport/timestamp. `LoroTree::get_last_move_id`
/// reflects the last STRUCTURAL (create/move) op, not text-meta updates, so it
/// can't order twins by text recency either. With no reliable cross-peer
/// recency signal available, max-`TreeID` is the deterministic choice. It is
/// NOT recency-aware: in a disjoint merge it may keep a stale twin's text (the
/// product trade-off accepted 2026-07-01) — the way to preserve a specific
/// edit's text is a shared base before authoring (then both sides resolve to
/// the same `TreeID`). This helper guarantees no duplicate render + the
/// global-max-`TreeID` survivor on every replica.
pub(super) fn dedup_twins_by_block_id(tree: &LoroTree, nodes: Vec<TreeID>) -> Vec<TreeID> {
    // First pass: for each block_id, find the canonical (max-TreeID) survivor.
    let mut canonical: HashMap<String, TreeID> = HashMap::new();
    for node in &nodes {
        if let Some(hex) = read_meta_str(tree, *node, "block_id") {
            canonical
                .entry(hex)
                .and_modify(|kept| {
                    if node > kept {
                        *kept = *node;
                    }
                })
                .or_insert(*node);
        }
    }
    // Second pass: keep nodes in original walk order, emitting each block_id's
    // canonical survivor exactly once. Nodes with no block_id meta are kept
    // (they can't be twins by bid; preserve existing behavior).
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        match read_meta_str(tree, node, "block_id") {
            Some(hex) => {
                if canonical.get(&hex) == Some(&node) {
                    out.push(node);
                }
            }
            None => out.push(node),
        }
    }
    out
}

/// Permanently tombstone every non-canonical duplicate-`block_id` twin in a
/// doc's `blocks` tree, committing if anything was deleted. This is the
/// persistent counterpart to the render-side heal in `note_tree_from_doc`:
/// after a peer's snapshot is imported (which unions same-bid twins), it
/// removes the strays from the doc itself so later block-diff saves can't
/// resurrect or update a ghost.
///
/// Uses the same max-`TreeID` survivor rule as `dedup_twins_by_block_id`, so
/// the survivor a render shows is the one that stays in the doc — and, because
/// the global-max `TreeID` is a pure function of the merged twin set, EVERY
/// replica keeps the IDENTICAL node in ONE round (tesela-fte: no cross-tombstone,
/// no split-brain). Idempotent: after one pass each bid has exactly one live
/// node, so a re-import (which merges identical state) finds nothing to delete
/// and returns `false` without committing. `note_id` is accepted for log/parity
/// with the other per-note helpers (the doc is already addressed).
pub(super) fn tombstone_duplicate_twins(doc: &LoroDoc, _note_id: [u8; 16]) -> bool {
    let tree = doc.get_tree("blocks");
    let live: Vec<TreeID> = tree
        .children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|n| !matches!(tree.is_node_deleted(n), Ok(true)))
        .collect();
    let kept = dedup_twins_by_block_id(&tree, live.clone());
    let mut deleted_any = false;
    for node in live {
        if !kept.contains(&node) {
            // Already-deleted nodes were filtered out above, so this only
            // hits live non-canonical twins; delete is safe (no double-free).
            if tree.delete(node).is_ok() {
                deleted_any = true;
            }
        }
    }
    if deleted_any {
        doc.commit();
    }
    deleted_any
}

/// The resolution for a DISJOINT-twin block_id (>1 live node): the union of
/// every twin's props to re-assert onto the surviving node.
///
/// The surviving NODE is NOT carried here — it is the global-max `TreeID`,
/// chosen deterministically by [`tombstone_duplicate_twins`] (pure max rule,
/// tesela-fte), identical on every replica in ONE round. This value only
/// supplies the PROPS union so a tombstoned twin's props survive onto whichever
/// node wins; emptiness/staleness influence NOTHING about which node is kept.
///
/// Only DISJOINT twins (>1 live node for the block_id) are emitted. A
/// SHARED-register block (single node) defers entirely to Loro's own LoroText
/// merge and is never healed.
pub(super) struct PeerBlockChange {
    block_id: [u8; 16],
    /// The RESOLVED typed property set the surviving node must carry — the
    /// per-key union across every disjoint twin read in the fork BEFORE the
    /// tombstone drops the losers. Re-asserted onto the survivor as one
    /// `BlockPropertySet` per key (lists via `AddToList` = union; scalars/text
    /// idempotency-guarded), so a tombstoned twin's props are never lost. See
    /// [`reconcile_orphaned_prop_containers`].
    props: Vec<(String, ResolvedValue)>,
}

/// A property value resolved from a Loro `props` container into plain Rust —
/// the typed analog the disjoint-twin heal merges + re-asserts. Decoupled from
/// `loro::LoroValue` (same discipline as [`PropScalar`] on the wire).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum ResolvedValue {
    Scalar(PropScalar),
    Text(String),
    List(Vec<PropScalar>),
}

/// Read + merge the props of EVERY live twin node for `owner` (a block_id hex)
/// in `doc` into one resolved typed set — the shared recovery path used by the
/// disjoint-twin heal (reading twins in the fork BEFORE the tombstone drops a
/// loser) AND the union re-assert (the apply emits one `BlockPropertySet` per
/// resolved key onto the survivor). Building ONE reconcile keeps the two halves
/// from diverging.
///
/// Merge semantics per key (matching the §4 design):
/// - **list** → union all twins' members, first-occurrence-stable dedup
///   (`prop_get_list_dedup` semantics), so the survivor carries every twin's
///   adds; the apply re-asserts via `AddToList` (union), never wholesale.
/// - **scalar** → union distinct KEYS; a same-key value collision is LWW — the
///   FIRST twin (deterministic node-walk order) wins, no recency analog.
/// - **text** → the first twin's value (a free-text register collision has no
///   union primitive; the genuine-edit discrimination lives in the text path,
///   not here).
///
/// A key that appears as different kinds across twins keeps the FIRST kind seen
/// (deterministic). Idempotent: re-reading a single-twin block returns its props
/// unchanged.
fn reconcile_orphaned_prop_containers(doc: &LoroDoc, owner: &str) -> Vec<(String, ResolvedValue)> {
    let tree = doc.get_tree("blocks");
    let live: Vec<TreeID> = tree
        .children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|n| !matches!(tree.is_node_deleted(n), Ok(true)))
        .collect();
    // Ordered keys + per-key merged value. A Vec preserves first-seen key order
    // (deterministic) while we look up by key for the union.
    let mut order: Vec<String> = Vec::new();
    let mut merged: HashMap<String, ResolvedValue> = HashMap::new();
    for node in live {
        if read_meta_str(&tree, node, "block_id").as_deref() != Some(owner) {
            continue;
        }
        let Ok(meta) = tree.get_meta(node) else {
            continue;
        };
        let Some((props, prop_keys)) = prop_containers::read_node_prop_containers(&meta) else {
            continue;
        };
        for (key, value) in prop_containers::read_props_typed(&props, &prop_keys) {
            match merged.get_mut(&key) {
                None => {
                    order.push(key.clone());
                    merged.insert(key, value);
                }
                // List → union the new members (first-occurrence-stable dedup).
                Some(ResolvedValue::List(existing)) => {
                    if let ResolvedValue::List(incoming) = value {
                        for v in incoming {
                            if !existing.contains(&v) {
                                existing.push(v);
                            }
                        }
                    }
                }
                // Scalar / text already seen → keep the first (LWW / register).
                Some(_) => {}
            }
        }
    }
    order
        .into_iter()
        .filter_map(|k| merged.remove(&k).map(|v| (k, v)))
        .collect()
}

/// Capture the PROPS-union plan for every DISJOINT twin (block_id with >1 live
/// node) in `doc` — the SINGLE plan shared by the relay apply paths (called on a
/// fork of auth+frame) AND the one-shot local repair (called on the live doc, so
/// an existing twin can be collapsed WITHOUT an inbound frame). Empty for a doc
/// with no twins.
///
/// The surviving NODE is NOT chosen here: text is resolved purely by the
/// global-max `TreeID` node keep ([`tombstone_duplicate_twins`] /
/// [`dedup_twins_by_block_id`], tesela-fte) — a pure function of the merged twin
/// set, identical on every replica, converging in ONE round with no
/// cross-tombstone and no dependence on emptiness/staleness/recency. This
/// function only unions each twin block's props ([`reconcile_orphaned_prop_containers`])
/// so a tombstoned loser's props are re-asserted onto whichever node survives.
pub(super) fn twin_winners_for(doc: &LoroDoc) -> Vec<PeerBlockChange> {
    let twin_bids = duplicate_block_ids(doc);
    if twin_bids.is_empty() {
        return Vec::new();
    }
    twin_bids
        .into_iter()
        .filter_map(|bid_hex| {
            let bid = parse_note_id_from_hex(&bid_hex)?;
            let props = reconcile_orphaned_prop_containers(doc, &bid_hex);
            Some(PeerBlockChange {
                block_id: bid,
                props,
            })
        })
        .collect()
}

/// The set of block_ids (hex) that have MORE THAN ONE live node in a doc's
/// `blocks` tree — disjoint-lineage twins. A block_id with a single live node
/// is a SHARED Loro register whose value Loro's LWW resolves authoritatively.
pub(super) fn duplicate_block_ids(doc: &LoroDoc) -> std::collections::HashSet<String> {
    let tree = doc.get_tree("blocks");
    let mut counts: HashMap<String, u32> = HashMap::new();
    for node in tree.children(TreeParentId::Root).unwrap_or_default() {
        if matches!(tree.is_node_deleted(&node), Ok(true)) {
            continue;
        }
        if let Some(bid) = read_meta_str(&tree, node, "block_id") {
            *counts.entry(bid).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(b, _)| b)
        .collect()
}
