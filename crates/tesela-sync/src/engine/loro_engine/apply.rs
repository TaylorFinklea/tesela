use super::*;

/// Which apply path is driving [`LoroEngine::apply_import`]. The three public
/// entry points (`import_doc_update`, `apply_doc_update_status`,
/// `import_authoritative_snapshot`) hand-copied the same ~9-step apply body and
/// differed ONLY in how they gate the disjoint-twin heal plan and in the noun
/// used when a poison frame is skipped — captured here so that body lives once
/// (ADR-1, decisions.md 2026-07-01).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ImportMode {
    /// Live WS delta. The heal plan is gated on the note ALREADY carrying
    /// genuine local history before this apply (post-tesela-qql: sampled from
    /// the doc's `len_changes()` AFTER lazy-load, not in-memory residency —
    /// see the call site) — a genuinely new note has no local twin to
    /// protect, so it raw-imports without a per-block plan. A discriminator
    /// error logs a warning (the frame still raw-imports).
    Delta,
    /// Authoritative catch-up snapshot. The plan is computed whenever the note
    /// isn't the views registry doc — local state is NOT consulted (a disjoint
    /// device catching up may not have the note resident yet, but its own
    /// authored twins still need the keep-winner resolve). A discriminator error
    /// is silently ignored (no warning).
    Authoritative,
}

impl ImportMode {
    /// The noun used in the poison-skip log + error ("inbound frame" vs
    /// "authoritative snapshot") — preserves each path's original message.
    fn skip_noun(self) -> &'static str {
        match self {
            ImportMode::Delta => "inbound frame",
            ImportMode::Authoritative => "authoritative snapshot",
        }
    }
}

/// Result of [`LoroEngine::apply_import`]. Carries the only per-path-variable
/// output: whether Loro left ops PENDING after the import (a causal gap
/// surfaced by `apply_doc_update_status`; discarded by the other two publics).
struct ImportOutcome {
    pending: bool,
}

impl LoroEngine {
    /// The single apply orchestrator (ADR-1, decisions.md 2026-07-01) behind
    /// `import_doc_update`, `apply_doc_update_status`, and
    /// `import_authoritative_snapshot`. Runs the shared sequence:
    /// poison-probe → import plan ([`peer_import_plan`]: twins props union +
    /// lifecycle status-flip gate) → `doc.import` → max-`TreeID` twin tombstone
    /// ([`tombstone_duplicate_twins`]) → prop re-assert → derived refresh →
    /// block lifecycle ([`Self::apply_block_lifecycle`], tesela-ows.1 step 2) →
    /// snapshot persist → materialize. `mode` selects ONLY the heal-plan
    /// residency gate + the poison-skip noun; the body is otherwise identical
    /// across the three public wrappers (see each for the per-path rationale).
    async fn apply_import(
        &self,
        note_id: [u8; 16],
        bytes: &[u8],
        mode: ImportMode,
    ) -> SyncResult<ImportOutcome> {
        // Serialize the WHOLE plan→import→tombstone sequence per note_id
        // (tesela-4ju): held until this fn returns, so a concurrent
        // `apply_import` for the SAME note can't interleave its import
        // between this call's props-plan fork and its twin tombstone (which
        // would size the tombstone to a twin set the second import just
        // changed, dropping the second caller's genuine edit). Different
        // notes never contend — this is a per-note, not global, lock.
        let apply_lock = self.apply_lock_for_note(note_id).await;
        let _apply_guard = apply_lock.lock().await;

        let doc = self.doc_for_note_mut(note_id).await;
        let is_views = Self::is_views_doc(&note_id);

        // Sample "does this doc already carry genuine local history" AFTER
        // `doc_for_note_mut` (which may have just lazy-loaded an on-disk
        // snapshot for a note that wasn't in-memory resident) but BEFORE
        // `doc.import` mutates it below. Only the Delta path gates its heal
        // plan on it; the authoritative path never consults this.
        //
        // This replaces the former in-memory-RESIDENCY sample (the
        // tesela-qql landmine, tesela-engc.5 audit): residency and "has
        // local state" were equivalent ONLY while `doc_for_note_mut`
        // unconditionally fabricated an empty doc on every map miss — a
        // non-resident note was ALWAYS empty. Now that a miss can lazy-load
        // real disk state, a note can be non-resident-in-memory yet have
        // genuine local history; gating on residency there would raw-import
        // an inbound delta with no twin protection and revert real local
        // edits. `len_changes() == 0` is true for both "genuinely brand new"
        // and "freshly lazy-loaded from an empty/nonexistent snapshot" —
        // exactly the cases with nothing local to protect.
        let has_local_state = match mode {
            ImportMode::Delta => doc.len_changes() > 0,
            ImportMode::Authoritative => false,
        };

        // POISON GUARD (2026-06-26): loro 1.12 can PANIC *inside* its richtext
        // apply (`insert_elem_at_entity_index` index-out-of-bounds) on certain
        // concurrent / disjoint-lineage frames. An unguarded `import` aborts the
        // whole process. Probe on a throwaway snapshot round-trip into a FRESH
        // doc (`doc.fork()` does NOT isolate — it shares the poisoned mutex): a
        // frame that panics (or errors) is SKIPPED (returned as an error so the
        // tick counts it failed + bounded-retries), never applied. A clean probe
        // here guarantees the real `import` below won't panic either.
        if let Err(reason) = probe_import_poison(&doc, bytes) {
            tracing::error!(
                "tesela-sync/loro: SKIPPING {} for {} — {reason}",
                mode.skip_noun(),
                hex_id(&note_id)
            );
            return Err(SyncError::Storage(format!(
                "{} skipped for {}: {reason}",
                mode.skip_noun(),
                hex_id(&note_id)
            )));
        }

        // Compute the disjoint-twin PROPS plan BEFORE mutating the doc: the
        // per-key union of every twin block's props, re-asserted onto the
        // max-`TreeID` survivor after the merge so a tombstoned loser's props
        // aren't lost (the node/text keep needs no plan — it's the pure max
        // rule). Skipped for the VIEWS registry doc (no "blocks" tree — the raw
        // import IS the merge) and, on the Delta path, for a not-yet-resident
        // note (nothing local to protect). A plan failure is graceful — raw-
        // import without per-block prop protection, never panic — but the Delta
        // path additionally warns.
        let plan_gate = match mode {
            ImportMode::Delta => has_local_state && !is_views,
            ImportMode::Authoritative => !is_views,
        };
        let plan = if plan_gate {
            match peer_import_plan(&doc, bytes) {
                Ok(p) => Some(p),
                Err(e) => {
                    if matches!(mode, ImportMode::Delta) {
                        tracing::warn!(
                            "tesela-sync/loro: WS-apply discriminator failed for {} ({e}); \
                             raw-importing without per-block protection",
                            hex_id(&note_id)
                        );
                    }
                    None
                }
            }
        } else {
            None
        };

        // Raw-import the frame. Genuinely-new blocks arrive with Loro's native,
        // convergent ordering (the multi-engine relay path relies on it) and
        // shared-lineage text merges; the clobber it can cause on EXISTING
        // disjoint-twin blocks is healed below. Surface Loro's PENDING status (a
        // causal gap) for the `apply_doc_update_status` caller; the other two
        // publics discard it.
        let status = doc
            .import(bytes)
            .map_err(|e| SyncError::Storage(format!("loro import: {e}")))?;
        let pending = status
            .pending
            .as_ref()
            .map(|p| !p.is_empty())
            .unwrap_or(false);

        // ── Disjoint-twin heal ───────────────────────────────────────────────
        // Collapse each twin bid to the global-max `TreeID` node (pure rule,
        // tesela-fte): a pure function of the merged twin set, so every replica
        // keeps the IDENTICAL node/text in ONE round — no cross-tombstone, no
        // dependence on emptiness/staleness. Shared-lineage blocks need no heal
        // (their splices already merged). The `plan` carries only the props
        // union, re-asserted below onto whichever node survives.
        if !is_views {
            tombstone_duplicate_twins(&doc, note_id);
        }
        // Props half of the heal: re-assert each tombstoned twin's resolved props
        // onto the surviving winner (per-key, idempotency-guarded). Goes
        // through `record_local_locked` (this note's `apply_locks` guard is
        // already held here), which re-acquires the docs lock + re-fetches
        // the doc.
        if let Some(p) = &plan {
            self.reassert_prop_heals(note_id, &p.twins).await?;
        }

        self.refresh_note_derived(note_id, &doc).await;

        // ── Engine-side block lifecycle (tesela-ows.1 step 2) ────────────────
        // A `done` flip reaching the engine over ANY writer that IMPORTS (WS
        // live-apply, relay import, iOS `.relay` import) — not just an HTTP PUT
        // — must trigger the recurrence bump + same-note dependency unblock that
        // previously lived only in `tesela-server`'s route handlers. Gated
        // (Lead constraint (b)) on a real non-done→done flip that
        // [`peer_import_plan`] already detected while forking for the twins
        // plan, so the common text-edit delta pays no extra render. `prev_md`
        // is the pre-import full markdown; `doc` now holds the post-import,
        // twin-healed state. The roll is authored as CONTAINER prop sets
        // (constraint (a)) — never in-text, never clearing the container — so
        // lifecycle state stays where twin-heal's union protects it.
        //
        // Hook point rationale (post-heal materialize, NOT `record_local`): the
        // acceptance is a flip delivered via relay/import, and `apply_import` is
        // the single orchestrator behind every import wrapper. `record_local`
        // (the local author path) is deliberately NOT hooked — the HTTP handler
        // already rolls there (zero-behavior-change requirement) and hooking it
        // would double-fire; a locally-authored FFI flip rolls once a peer
        // imports it (matches the acceptance test's "author does not self-roll").
        if !is_views {
            if let Some(prev_md) = plan.as_ref().and_then(|p| p.lifecycle_prev.as_deref()) {
                self.apply_block_lifecycle(note_id, prev_md, &doc).await?;
            }
        }

        if let Some(dir) = self.inner.snapshot_dir.as_ref() {
            self.save_snapshot(dir, note_id).await;
        }
        // Authoritative-writer mode: a peer's edit must land on disk too.
        if self.inner.materialize_dir.is_some() {
            self.materialize_note(note_id).await;
        }
        Ok(ImportOutcome { pending })
    }

    /// Apply a peer's Loro update bytes into the addressed note's doc —
    /// **the protected WS-apply path** (2026-06-02; LoroText era).
    ///
    /// Block text is now a nested [`LoroText`] sequence CRDT, not an LWW map
    /// register. On a SHARED Loro lineage (one node per block_id) a raw
    /// `doc.import` of the peer's frame MERGES the peer's text splices with the
    /// server's — it can neither revert the server nor drop the peer's edit. So
    /// shared-lineage blocks need no protection and defer entirely to Loro's
    /// merge.
    ///
    /// The ONE residual data-loss vector is the DISJOINT-TWIN case (the
    /// `project_multidevice_convergence` residue): when a block_id has >1 live
    /// node post-import, the twins hold two INDEPENDENT LoroTexts Loro can't
    /// merge, plus per-twin `props` containers, and one of them must be
    /// tombstoned.
    ///
    /// The resolution has two halves when the server already holds this note:
    ///
    /// 1. **Props plan (before mutating):** ask [`peer_import_plan`] for its
    ///    `.twins` — the per-key UNION of every disjoint-twin block's props — so
    ///    the tombstoned loser's props aren't lost. Empty plan ⇒ no twins ⇒ nothing
    ///    to heal. The surviving node/TEXT needs no plan — it is resolved purely
    ///    by the max-`TreeID` node keep (the higher-`TreeID` twin's text wins,
    ///    product-approved 2026-07-01, even a stale re-ship).
    /// 2. **Raw-import + heal:** raw-import the frame so genuinely-NEW blocks
    ///    arrive with Loro's native, convergent ordering (the multi-engine
    ///    relay path depends on this — re-authoring new blocks under the
    ///    server's peer would diverge across devices) and shared-lineage text
    ///    merges, then [`tombstone_duplicate_twins`] keeps the global-max
    ///    `TreeID` node per bid and the props union is re-asserted onto it.
    ///    Shared-lineage blocks are never touched.
    ///
    /// Bootstrap (the server has no doc for this note yet) is a plain raw
    /// import: there's nothing to clobber, and the peer's full state is exactly
    /// what the server should adopt.
    ///
    /// Idempotent + commutative: a re-applied frame keeps the same max-`TreeID`
    /// survivor + finds every prop already set → no change. A decode/fork
    /// failure logs + falls back to a plain raw import (never panics).
    pub async fn import_doc_update(&self, note_id: [u8; 16], bytes: &[u8]) -> SyncResult<()> {
        self.apply_import(note_id, bytes, ImportMode::Delta)
            .await
            .map(|_| ())
    }

    /// Apply the server's FULL snapshot as a catch-up re-base — the
    /// disjoint-device recovery a live delta can't do (its ops reference a
    /// lineage the device never imported, so they land PENDING).
    ///
    /// A device that authored a note WITHOUT first importing the server's
    /// snapshot is on a DISJOINT Loro lineage: it minted its own `TreeID` for
    /// each block_id. Raw-importing the server snapshot UNIONS the lineages into
    /// same-bid twins. This path resolves them with the EXACT SAME deterministic
    /// keep-winner rule as [`import_doc_update`] (pure global-max `TreeID`,
    /// tesela-fte) — so the catch-up path and the
    /// WS path always pick the IDENTICAL survivor and can never cross-tombstone a
    /// block out of existence. After the collapse the device shares ONE lineage
    /// per block_id, so later concurrent edits MERGE through the block's
    /// `LoroText` instead of forking new twins.
    ///
    /// Block_ids the device has but the snapshot does NOT are KEPT untouched —
    /// those are the device's genuine unsynced new blocks (they aren't twins).
    ///
    /// Otherwise mirrors [`import_doc_update`]'s tail: refresh derived
    /// projections, persist the snapshot, materialize the note to disk.
    pub async fn import_authoritative_snapshot(
        &self,
        note_id: [u8; 16],
        bytes: &[u8],
    ) -> SyncResult<()> {
        let outcome = self
            .apply_import(note_id, bytes, ImportMode::Authoritative)
            .await
            .map(|_| ());
        // This IS the causal-gap heal (tesela-c7s item 2): a note queued for
        // snapshot catch-up because a live delta landed pending has now
        // re-based on the relay's authoritative full state, so its gap is
        // closed — clear it from the ledger. Only on success; a failed import
        // leaves it queued for the next attempt.
        if outcome.is_ok() {
            self.clear_pending_import(note_id).await;
        }
        outcome
    }

    /// Like [`import_doc_update`](Self::import_doc_update) but RETURNS whether
    /// the imported update was left PENDING by Loro — i.e. it referenced ops the
    /// doc is missing (a causal gap / disjoint-lineage signal). `import_doc_update`
    /// discards loro's `ImportStatus`; this surfaces it so a caller (the iOS
    /// delta path) can trigger an authoritative-snapshot catch-up when a live
    /// delta can't fully integrate.
    ///
    /// Returns `Ok(true)` when `ImportStatus.pending` is non-empty. The apply
    /// itself runs the SAME protected path as `import_doc_update` (twin
    /// tombstone + disjoint-twin heal + derived refresh + persist), so behavior
    /// is identical; only the pending bool is additionally reported.
    pub async fn apply_doc_update_status(
        &self,
        note_id: [u8; 16],
        bytes: &[u8],
    ) -> SyncResult<bool> {
        self.apply_import(note_id, bytes, ImportMode::Delta)
            .await
            .map(|o| o.pending)
    }

    /// Engine-side block lifecycle (tesela-ows.1 step 2, Lead-tier). Invoked
    /// from [`Self::apply_import`] ONLY when [`peer_import_plan`] detected a real
    /// non-done→done status flip in the just-imported frame, so every writer
    /// that IMPORTS (WS live-apply, relay import, iOS `.relay` import) — not just
    /// the HTTP PUT — triggers the recurrence bump + same-note dependency unblock
    /// that previously lived only in `tesela-server`'s route handlers.
    ///
    /// - `prev_md` is the note's full markdown BEFORE the import; the current
    ///   `doc` holds the post-import, twin-HEALED state (tombstone + prop
    ///   re-assert already ran), re-rendered here as `next_md`.
    /// - The recurrence bump runs through the IDEMPOTENCE-GUARDED core
    ///   ([`tesela_core::lifecycle::compute_lifecycle_container_sets`],
    ///   constraint (a)): a concurrently- or re-delivered done-flip can't
    ///   double-advance the series (the guard anchors on the completed
    ///   occurrence — the EARLIER of prev/next — so it is robust to a concurrent
    ///   peer's roll landing on either side of the merge).
    ///
    /// ## Constraint (a): lifecycle props STAY in the typed props container
    /// The roll is authored as CONTAINER [`OpPayload::BlockPropertySet`] sets —
    /// the SAME mechanism [`Self::reassert_prop_heals`] uses — NOT as an in-text
    /// markdown rewrite, and the container is NEVER cleared. Lifecycle state
    /// (`last_completed`, `recurrence_done`, the rolled dates, `status`) lives in
    /// the container where disjoint-twin heal's per-key union
    /// ([`reconcile_orphaned_prop_containers`]) protects it. Attempt 2 evicted
    /// these to in-text lines (clearing the container so the roll would render);
    /// because twin-heal unions CONTAINER props only, a max-`TreeID` pick landing
    /// on the non-rolling twin then silently WIPED completion memory. Keeping the
    /// roll in the container renders correctly for free — the container value
    /// wins render-time dedup ([`dedup_intext_props_against_container`]) — with
    /// no clearing and no render-side change.
    ///
    /// ## Recursive-reimport guard (idempotence when the roll propagates)
    /// The bump authors `status:: todo`; when a peer imports the resulting frame
    /// it sees a done→todo transition (LWW: the roll's HLC is later than the
    /// original flip's), never a fresh flip TO done, so the flip gate does not
    /// re-trigger. The idempotence guard is the backstop for any residual
    /// re-delivery.
    ///
    /// ## Lock discipline (tesela-4ju)
    /// Called from inside `apply_import`, which already holds this note's
    /// `apply_locks` guard for its whole body. Authoring goes through
    /// `record_local_locked` (NOT the public `record_local`, which would deadlock
    /// re-acquiring the non-reentrant guard) — the same discipline as
    /// `reassert_prop_heals`.
    async fn apply_block_lifecycle(
        &self,
        note_id: [u8; 16],
        prev_md: &str,
        doc: &LoroDoc,
    ) -> SyncResult<()> {
        let next_md = doc_full_markdown(doc);
        // Same note-id string the HTTP path passes to the lifecycle fns: the
        // slug (so same-note `blocked_by::` refs resolve), falling back to the
        // hex id when no slug is resident. `slug_for_note` is read-only — it
        // does not acquire `apply_locks`, so it is safe under our guard.
        let note_str = self
            .slug_for_note(note_id)
            .await
            .unwrap_or_else(|| hex_id(&note_id));
        let rolls =
            tesela_core::lifecycle::compute_lifecycle_container_sets(prev_md, &next_md, &note_str);
        for roll in rolls {
            // Address the container node by the block's canonical bid. An
            // unstamped block (no bid) can't be addressed — skip it (the engine
            // tree always stamps a block_id, so resident notes always carry one).
            let Some(block_id) = roll
                .bid
                .as_deref()
                .and_then(|b| uuid::Uuid::parse_str(b).ok())
                .map(|u| *u.as_bytes())
            else {
                continue;
            };
            for (key, value) in roll.props {
                // Representation alignment (tesela-ows.1 step 2, round 3): author
                // each rolled key in the representation the container ALREADY
                // holds for it — a text-typed key (the route's free-text default)
                // stays a `LoroText`; a scalar or not-yet-present key stays a
                // scalar. The engine has no property registry, so it PRESERVES the
                // established representation instead of guessing it. This stops
                // the engine writer from flip-flopping a key scalar<->text on each
                // inbound completion (which orphans the old child container). The
                // write layer ([`prop_containers::clear_incompatible_child`])
                // still tolerates any residual mix already in a live doc.
                let value_op = if block_prop_is_text(doc, block_id, &key) {
                    PropOp::SetText(value)
                } else {
                    PropOp::SetScalar(PropScalar::Text(value))
                };
                self.record_local_locked(OpPayload::BlockPropertySet {
                    note_id,
                    block_id,
                    key,
                    value: value_op,
                })
                .await?;
            }
        }
        Ok(())
    }
}

/// **The disjoint-twin heal PROPS plan** (2026-06-02, LoroText era; pure-max
/// rewrite tesela-fte). Given the server's current authoritative doc + a peer's
/// frame bytes, return for every DISJOINT-twin block the union of its twins'
/// PROPS to re-assert onto the surviving node. The surviving node itself is NOT
/// chosen here — it is the global-max `TreeID`, tombstoned deterministically by
/// [`tombstone_duplicate_twins`]. Emptiness/staleness/recency influence NOTHING
/// about which node survives.
///
/// ## Why this is scoped to disjoint twins
/// Block text is now a nested [`LoroText`] sequence CRDT, not an LWW map
/// register. On a SHARED Loro lineage (one node per block_id) the peer's frame
/// carries only its own LoroText splices, which MERGE with the server's — the
/// import neither reverts the server nor drops the peer's edit. So shared-
/// lineage blocks need NO heal and are skipped entirely (never override Loro's
/// merge).
///
/// The DISJOINT-twin case (the `project_multidevice_convergence` residue)
/// survives: when a block_id has >1 live node post-import the twins hold two
/// INDEPENDENT LoroTexts that Loro CANNOT merge, plus per-twin `props`
/// containers that would be dropped when the loser is tombstoned. The text is
/// resolved purely by the max-`TreeID` node keep (the higher-`TreeID` twin's
/// text wins — product-approved 2026-07-01, even if that's a stale re-ship);
/// the PROPS are the one thing that must be carried across the tombstone, so
/// this plan reads every twin's props in the fork BEFORE the merge drops a
/// loser, unions them per key, and the apply re-asserts them onto the survivor.
///
/// ## The plan
/// 1. `server_vv = auth.oplog_vv()`. Fork the auth doc, import the frame into
///    the fork (full-history clone — never touches the auth doc). Nothing
///    causally new → idempotent no-op.
/// 2. `twin_bids` = block_ids with >1 live node in the fork (disjoint twins).
///    None → nothing to heal.
/// 3. Per twin block_id, union every live twin's props ([`twin_winners_for`] →
///    [`reconcile_orphaned_prop_containers`]) — a pure function of the merged
///    twin set, identical on every replica, so it converges in ONE round.
///
/// Idempotent: the re-assert only sets a key whose current value differs, so a
/// re-applied frame is a no-op.
/// Probe whether `bytes` can be imported into a fork of `doc` WITHOUT a Loro
/// panic. loro 1.12's richtext `apply_diff` can panic (e.g.
/// `insert_elem_at_entity_index` index-out-of-bounds) on certain concurrent /
/// disjoint-lineage frames; an unguarded `import` aborts the whole process
/// (it crash-looped the desktop on every relay tick, 2026-06-26). Forks the
/// doc (so the probe never mutates the live doc) and imports under
/// `catch_unwind`. `Ok(())` ⇒ the same `import` on the live doc is safe;
/// `Err(reason)` ⇒ the caller MUST skip the frame (a clean import error is
/// also surfaced so the caller skips rather than half-applies). The fork is
/// dropped normally after the catch, so a caught panic never escalates to a
/// panic-in-cleanup.
pub(super) fn probe_import_poison(doc: &LoroDoc, bytes: &[u8]) -> Result<(), String> {
    // Probe on a FULLY INDEPENDENT copy — NOT `doc.fork()`. A fork shares the
    // original's internal `LoroMutex`, so a panic during the probe import
    // poisons the LIVE doc's mutex and the next access aborts with a
    // non-unwinding "poisoned LoroMutex" panic (the fork-based first attempt
    // still crash-looped, 2026-06-26). A snapshot round-trip into a fresh
    // `LoroDoc` gives the probe its own mutex, fully isolating any panic.
    let snapshot = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        doc.export(ExportMode::Snapshot)
    })) {
        Ok(Ok(s)) => s,
        // Can't snapshot the live doc → don't block sync on the probe; let the
        // real import proceed (this is not the poison case we're guarding).
        _ => return Ok(()),
    };
    let probe = LoroDoc::new();
    if probe.import(&snapshot).is_err() {
        return Ok(());
    }
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| probe.import(bytes))) {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(format!("loro import error: {e}")),
        Err(_) => {
            // The probe's OWN mutex is now poisoned; dropping it would re-lock
            // it and abort. Leak the throwaway probe — a few KB on a RARE
            // poison frame is far better than crashing the process. The live
            // `doc` was never touched, so it stays healthy.
            std::mem::forget(probe);
            Err("loro PANICKED on import (poison frame)".to_string())
        }
    }
}

/// The pre-mutation analysis of an inbound frame, computed on a SINGLE fork of
/// the auth doc (tesela-ows.1 step 2 folds the lifecycle status-flip gate into
/// the existing disjoint-twin props plan so the common delta forks only once —
/// Lead constraint (b): the common text-edit delta pays no extra markdown
/// render).
struct ImportPlan {
    /// Per-block props union for every disjoint twin — re-asserted onto the
    /// max-`TreeID` survivor after the merge (the unchanged twins-heal plan).
    twins: Vec<PeerBlockChange>,
    /// `Some(prev_markdown)` when this frame flips at least one block's status
    /// from non-`done` → `done`: the note's full markdown BEFORE the import, so
    /// [`LoroEngine::apply_block_lifecycle`] can diff prev→post. `None` in the
    /// common case (no such flip), letting the caller skip the lifecycle and pay
    /// no markdown render.
    lifecycle_prev: Option<String>,
}

/// Fork `auth`, merge `frame`, and compute BOTH the disjoint-twin props plan and
/// the status-flip-to-done signal in one pass. The fork is a full-history clone,
/// so the auth doc is never touched. A frame that adds nothing causally new is an
/// idempotent no-op (empty plan, no flip).
fn peer_import_plan(auth: &LoroDoc, frame: &[u8]) -> SyncResult<ImportPlan> {
    let server_vv = auth.oplog_vv();
    // Fork (full-history clone) so the import never touches the auth doc.
    let fork = auth.fork();
    fork.import(frame)
        .map_err(|e| SyncError::Storage(format!("fork import: {e}")))?;
    let fork_vv = fork.oplog_vv();
    // Nothing causally new from the peer → idempotent no-op.
    if fork_vv == server_vv {
        return Ok(ImportPlan {
            twins: Vec::new(),
            lifecycle_prev: None,
        });
    }

    // The merged fork now holds both lineages; capture each disjoint twin's
    // props union so the max-`TreeID` node keep can't drop a loser's props.
    let twins = twin_winners_for(&fork);

    // Lifecycle gate: only pay for a full markdown render when a block actually
    // flipped non-done→done (`auth` = pre-merge, `fork` = post-merge). The
    // common text-edit delta returns `None` here after a cheap per-block status
    // scan and never renders markdown.
    let lifecycle_prev = if any_status_flip_to_done(auth, &fork) {
        Some(doc_full_markdown(auth))
    } else {
        None
    };

    Ok(ImportPlan {
        twins,
        lifecycle_prev,
    })
}

/// A block's effective `status`: the typed `props` container value if present,
/// else a legacy in-text `status:: value` continuation line. Mirrors render-time
/// dedup (container wins), so the flip gate reads the SAME status the note
/// materializes.
fn block_status_of(fb: &tesela_core::note_tree::FlatBlock) -> Option<String> {
    if let Some((_, v)) = fb.properties.iter().find(|(k, _)| k == "status") {
        return Some(v.clone());
    }
    let mut fence = tesela_core::note_tree::MarkdownFenceTracker::default();
    for line in fb.text.lines() {
        if fence.line_is_fenced(line) {
            continue;
        }
        if let Some((k, v)) = tesela_core::lifecycle::property_kv(line) {
            if k == "status" {
                return Some(v);
            }
        }
    }
    None
}

/// `true` when any block is `done` in `after` (post-merge) but was NOT `done` in
/// `before` (pre-merge) — a genuine completion the engine lifecycle must act on.
/// Reads status per block (container or in-text); pays no markdown render.
fn any_status_flip_to_done(before: &LoroDoc, after: &LoroDoc) -> bool {
    let after_tree = after.get_tree("blocks");
    let before_tree = before.get_tree("blocks");
    for node in after_tree.children(TreeParentId::Root).unwrap_or_default() {
        if matches!(after_tree.is_node_deleted(&node), Ok(true)) {
            continue;
        }
        let Some(fb) = flatblock_from_node(&after_tree, node) else {
            continue;
        };
        if block_status_of(&fb).as_deref() != Some("done") {
            continue;
        }
        let bid_hex = hex_id(fb.id.as_bytes());
        let before_done = find_node_by_block_id(&before_tree, &bid_hex)
            .and_then(|n| flatblock_from_node(&before_tree, n))
            .and_then(|b| block_status_of(&b))
            .as_deref()
            == Some("done");
        if !before_done {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod fence_status_tests {
    use super::*;

    fn block(text: &str, properties: Vec<(String, String)>) -> tesela_core::note_tree::FlatBlock {
        tesela_core::note_tree::FlatBlock {
            id: uuid::Uuid::nil(),
            parent: None,
            indent: 0,
            text: text.to_string(),
            properties,
        }
    }

    #[test]
    fn fenced_status_line_is_not_lifecycle_state() {
        let fenced = block("```text\nstatus:: done\n```", Vec::new());
        assert_eq!(block_status_of(&fenced), None);

        let real = block(
            "before\n```text\nstatus:: done\n```\nstatus:: todo",
            Vec::new(),
        );
        assert_eq!(block_status_of(&real).as_deref(), Some("todo"));
    }

    #[test]
    fn typed_status_still_wins_over_fenced_payload() {
        let block = block(
            "```text\nstatus:: literal\n```",
            vec![("status".into(), "done".into())],
        );
        assert_eq!(block_status_of(&block).as_deref(), Some("done"));
    }
}
