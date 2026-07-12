//! tesela-c7s — the OUTBOUND DEPOSIT STRAND class (the 2026-07-03 live fleet
//! wedge + the same-block "data loss" that was really a never-deposited op).
//!
//! A device whose outbound broadcast cursor is stranded — stale-AHEAD of the
//! doc's current version (the 2026-06-29 class), or UNDECODABLE (2026-06-25)
//! — ships an empty/no-op incremental frame every tick and deposits nothing.
//! `export_doc_update` already rescues correctness with a full-snapshot
//! fallback, but nothing (a) made the strand OBSERVABLE (item 3) or (b) HEALED
//! the cursor after a confirmed snapshot deposit (item 4), so the strand looped
//! forever ("fresh edits, ZERO incremental PUT /ops"). Separately, an inbound
//! causal-gap that Loro left PENDING was only `tracing::warn`'d, never recorded
//! durably or auto-escalated to a snapshot catch-up (item 2).
//!
//! Each test is REVERT-DISCRIMINATING: it fails if its guard is neutralized
//! (see the per-test note for exactly which line to break to watch it fail).

use std::sync::Arc;

use loro::VersionVector;
use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, SyncEngine};

fn engine(b: u8) -> LoroEngine {
    let d = DeviceId::from_bytes([b; 16]);
    LoroEngine::new(d, Arc::new(Hlc::new(d)))
}

fn note_id(slug: &str) -> [u8; 16] {
    tesela_core::stable_uuid_from_slug(slug)
}

/// Seed a note + produce + commit so its broadcast cursor sits exactly at the
/// doc's current version (the healthy steady state). Returns the note id.
async fn seed_broadcast(e: &LoroEngine, slug: &str, bid: &str) -> [u8; 16] {
    let note = note_id(slug);
    e.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some(slug.into()),
        title: slug.into(),
        content: format!("- seed <!-- bid:{bid} -->\n"),
        created_at_millis: 1,
    })
    .await
    .expect("seed note");
    let first = e.produce_relay_updates().await;
    assert_eq!(first.len(), 1, "first produce emits the note");
    let committed: Vec<([u8; 16], Vec<u8>)> =
        first.into_iter().map(|(d, _b, vv)| (d, vv)).collect();
    e.commit_broadcast_cursors(&committed).await;
    assert!(
        e.produce_relay_updates().await.is_empty(),
        "cursor at current → nothing dirty"
    );
    note
}

/// Craft a broadcast cursor that is stale-AHEAD of the note's current version
/// by `bump` counters on every peer, and install it — the exact net state an
/// authoritative import that rebased current 'backward' leaves behind.
async fn strand_cursor_ahead(e: &LoroEngine, note: [u8; 16], bump: i32) {
    let current_enc = e.doc_version(note).await.unwrap();
    let mut ahead = VersionVector::decode(&current_enc).unwrap();
    let peers: Vec<(u64, i32)> = ahead.iter().map(|(p, c)| (*p, *c)).collect();
    assert!(!peers.is_empty(), "doc must have ops to bump past");
    for (peer, counter) in peers {
        ahead.set_end(loro::ID::new(peer, counter + bump));
    }
    e.commit_broadcast_cursors(&[(note, ahead.encode())]).await;
}

// ─────────────────────────────────────────────────────────────────────────
// Item 3 — outbound strand ALARM (stale-ahead + undecodable)
// ─────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn stale_ahead_cursor_raises_outbound_strand_alarm_and_still_ships() {
    // REVERT-DISCRIMINATING: neutralize the `outbound_strand_alarms.fetch_add`
    // (or the `outbound_cursor_stranded` classification) in
    // `produce_relay_updates` and the alarm count stays 0 while a dirty note
    // silently snapshots every tick — exactly the wedge that shipped no
    // observability.
    let e = engine(0x11);
    let note = seed_broadcast(&e, "ahead", "60606060-6060-6060-6060-606060606060").await;

    strand_cursor_ahead(&e, note, 100).await;

    let before = e.outbound_strand_alarm_count();
    let out = e.produce_relay_updates().await;
    let after = e.outbound_strand_alarm_count();

    assert_eq!(
        out.len(),
        1,
        "a stale-ahead dirty note must STILL export (snapshot fallback), never be silent"
    );
    assert_eq!(
        after,
        before + 1,
        "the stale-ahead strand must raise exactly one outbound strand alarm"
    );
}

#[tokio::test]
async fn undecodable_cursor_raises_outbound_strand_alarm() {
    // REVERT-DISCRIMINATING: the `Err(_) => true` arm of
    // `outbound_cursor_stranded` is what classifies an undecodable cursor as a
    // strand; break it (return false) and the alarm never fires.
    let e = engine(0x12);
    let note = seed_broadcast(&e, "corrupt", "70707070-7070-7070-7070-707070707070").await;

    // A corrupt / version-incompatible persisted cursor.
    e.commit_broadcast_cursors(&[(note, vec![0xff, 0xff, 0xff, 0xff])])
        .await;

    let before = e.outbound_strand_alarm_count();
    let out = e.produce_relay_updates().await;
    let after = e.outbound_strand_alarm_count();

    assert_eq!(out.len(), 1, "undecodable-cursor note still exports (snapshot)");
    assert_eq!(
        after,
        before + 1,
        "an undecodable broadcast cursor is a strand and must raise the alarm"
    );
}

#[tokio::test]
async fn healthy_and_disjoint_broadcasts_raise_no_alarm() {
    // Guard against a false-positive alarm: a genuinely-new note (no cursor,
    // first snapshot) and a note that ships a real forward delta must NOT be
    // flagged as strands.
    let e = engine(0x13);
    let note = note_id("fresh");
    e.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("fresh".into()),
        title: "fresh".into(),
        content: "- one <!-- bid:80808080-8080-8080-8080-808080808080 -->\n".into(),
        created_at_millis: 1,
    })
    .await
    .unwrap();

    // First broadcast: since == None → snapshot, but NOT a strand.
    let before = e.outbound_strand_alarm_count();
    let first = e.produce_relay_updates().await;
    assert_eq!(first.len(), 1);
    let committed: Vec<([u8; 16], Vec<u8>)> =
        first.into_iter().map(|(d, _b, vv)| (d, vv)).collect();
    e.commit_broadcast_cursors(&committed).await;

    // A real forward edit: cursor is BEHIND current → genuine delta, not a strand.
    e.record_local(OpPayload::BlockUpsert {
        block_id: note_id("fresh-b2"),
        note_id: note,
        parent_block_id: None,
        order_key: "z".into(),
        indent_level: 0,
        text: "two".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    let out = e.produce_relay_updates().await;
    assert_eq!(out.len(), 1, "the forward edit ships");
    let after = e.outbound_strand_alarm_count();
    assert_eq!(
        after, before,
        "neither a first-snapshot nor a behind-cursor forward delta is a strand"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// Item 4 — a confirmed snapshot deposit REPAIRS the stranded cursor
// ─────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn snapshot_deposit_repairs_stranded_cursor() {
    // REVERT-DISCRIMINATING: make `repair_broadcast_cursors_after_snapshot` a
    // no-op (or break `broadcast_cursor_needs_repair` to always return false)
    // and the post-repair `produce` still finds the note dirty (stale-ahead) —
    // the "produce is now EMPTY" assertion fails, i.e. the strand was masked,
    // not healed.
    let e = engine(0x21);
    let note = seed_broadcast(&e, "repair", "90909090-9090-9090-9090-909090909090").await;

    // Strand it, well beyond any subsequent small edit.
    strand_cursor_ahead(&e, note, 500).await;
    assert_eq!(
        e.produce_relay_updates().await.len(),
        1,
        "stranded: the dirty note re-exports (snapshot) instead of nothing"
    );

    // Simulate the deposit: capture the vv AT SNAPSHOT TIME, then repair after
    // the (confirmed) PUT.
    let snap_vv = e.doc_version(note).await.unwrap();
    e.repair_broadcast_cursors_after_snapshot(&[(note, snap_vv)])
        .await;

    // HEALED: the cursor is re-anchored to the deposited version, so with no
    // new edits produce ships NOTHING (cursor == current) rather than looping
    // on the snapshot fallback.
    let alarm_before = e.outbound_strand_alarm_count();
    assert!(
        e.produce_relay_updates().await.is_empty(),
        "after repair the cursor sits at current → strand healed, no re-broadcast"
    );
    assert_eq!(
        e.outbound_strand_alarm_count(),
        alarm_before,
        "a healed cursor raises no further alarm"
    );

    // And the NEXT local edit ships a real INCREMENTAL delta (not a snapshot,
    // not empty) — the whole point of healing the cursor.
    e.record_local(OpPayload::BlockUpsert {
        block_id: note_id("repair-b2"),
        note_id: note,
        parent_block_id: None,
        order_key: "z".into(),
        indent_level: 0,
        text: "after-heal".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    let alarm_before = e.outbound_strand_alarm_count();
    let out = e.produce_relay_updates().await;
    assert_eq!(out.len(), 1, "the post-heal edit ships");
    assert_eq!(
        e.outbound_strand_alarm_count(),
        alarm_before,
        "post-heal the edit ships incrementally — no strand alarm"
    );
}

#[tokio::test]
async fn repair_uses_snapshot_time_vv_and_never_swallows_a_concurrent_edit() {
    // The "reset to the vv AT SNAPSHOT TIME, not at confirm time" rule: an edit
    // recorded DURING the deposit (after the snapshot was cut, before the PUT
    // confirmed) must survive the repair and still reach peers.
    //
    // REVERT-DISCRIMINATING: if repair anchored to the doc's CURRENT version at
    // confirm time (V2) instead of the passed snapshot-time vv (V1), the cursor
    // would land at current and `produce` would ship NOTHING — the concurrent
    // edit swallowed. The assertions below (out.len()==1 + the delta renders
    // the edit) fail in that case.
    let base = engine(0x31);
    let note = seed_broadcast(&base, "swallow", "a1a1a1a1-a1a1-a1a1-a1a1-a1a1a1a1a1a1").await;

    // Snapshot-time state: capture BOTH the base snapshot and its vv (V1).
    let base_snapshot = base.export_doc_update(note, None).await.unwrap();
    let snap_vv = base.doc_version(note).await.unwrap();

    // Strand the cursor.
    strand_cursor_ahead(&base, note, 500).await;

    // A concurrent edit lands DURING the deposit → current advances to V2.
    base.record_local(OpPayload::BlockUpsert {
        block_id: note_id("swallow-b2"),
        note_id: note,
        parent_block_id: None,
        order_key: "z".into(),
        indent_level: 0,
        text: "concurrent-during-deposit".into(),
        after_block_id: None,
    })
    .await
    .unwrap();

    // Repair with the SNAPSHOT-TIME vv (V1) — what the deposit captured.
    base.repair_broadcast_cursors_after_snapshot(&[(note, snap_vv)])
        .await;

    // The concurrent edit is NOT swallowed: produce ships a forward delta.
    let out = base.produce_relay_updates().await;
    assert_eq!(
        out.len(),
        1,
        "the edit made during the deposit must still ship (not be swallowed)"
    );
    let (_id, delta, _vv) = &out[0];

    // The shipped frame is a genuine forward delta: applied on top of the
    // snapshot-time base it reproduces the concurrent edit.
    let peer = engine(0x32);
    peer.import_doc_update(note, &base_snapshot).await.unwrap();
    peer.import_doc_update(note, delta).await.unwrap();
    let rendered = peer.render_note(note).await.unwrap_or_default();
    assert!(
        rendered.contains("concurrent-during-deposit"),
        "the delta carries the during-deposit edit forward; got: {rendered:?}"
    );
}

#[tokio::test]
async fn repair_leaves_a_healthy_cursor_untouched() {
    // A cursor already at current (healthy) must NOT be rewound by a repair
    // call — otherwise every heal-deposit would force a redundant re-broadcast.
    let e = engine(0x41);
    let note = seed_broadcast(&e, "healthy", "b2b2b2b2-b2b2-b2b2-b2b2-b2b2b2b2b2b2").await;

    // Cursor == current. A repair to the current vv is a no-op.
    let snap_vv = e.doc_version(note).await.unwrap();
    e.repair_broadcast_cursors_after_snapshot(&[(note, snap_vv)])
        .await;

    assert!(
        e.produce_relay_updates().await.is_empty(),
        "repairing a healthy cursor must not make it re-broadcast"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// Item 2 — durable inbound causal-gap ledger + auto snapshot catch-up
// ─────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn pending_ledger_records_then_auto_heals_round_trip() {
    // REVERT-DISCRIMINATING on TWO guards:
    //  - remove `record_pending_import` in `apply_relay_updates` → the ledger
    //    is empty even though the note is stuck (the "recorded" asserts fail);
    //  - remove `clear_pending_import` in `import_authoritative_snapshot` → the
    //    note lingers in the ledger after the heal (the "cleared" asserts fail).
    let note = note_id("gap");

    // Author a base + a tail edit; export ONLY the tail delta (from pre-tail vv).
    let author = engine(0x51);
    author
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("gap".into()),
            title: "gap".into(),
            content: "- alpha <!-- bid:c3c3c3c3-c3c3-c3c3-c3c3-c3c3c3c3c3c3 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    let pre_vv = author.doc_version(note).await.unwrap();
    author
        .record_local(OpPayload::BlockUpsert {
            block_id: note_id("gap-tail"),
            note_id: note,
            parent_block_id: None,
            order_key: "z".into(),
            indent_level: 0,
            text: "tail-only-no-base".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    let tail = author.export_doc_update(note, Some(&pre_vv)).await.unwrap();

    // Device with NO base imports the tail → Loro buffers it PENDING.
    let device = engine(0x52);
    let report = device.apply_relay_updates(&[(note, tail.clone())]).await;
    assert_eq!(report.pending, vec![note], "causal gap surfaced as pending");

    // The gap is RECORDED durably (not just a warn), naming the note + peers.
    let ledger = device.pending_import_notes().await;
    assert_eq!(ledger.len(), 1, "pending note recorded in the ledger");
    assert_eq!(ledger[0].note_id, note);
    assert!(
        !ledger[0].from_peers.is_empty(),
        "the stuck frame's peers are captured as the missing base's from_peer"
    );

    // Just-pending this pass: withheld one tick so a same-session missing-base
    // delta can still integrate it first.
    assert!(
        device.notes_needing_snapshot_catchup().await.is_empty(),
        "a note that only just went pending is not yet escalated"
    );

    // Still pending after ANOTHER apply pass → escalate to snapshot catch-up.
    let _ = device.apply_relay_updates(&[(note, tail)]).await;
    assert_eq!(
        device.notes_needing_snapshot_catchup().await,
        vec![note],
        "a note pending past one pass needs an authoritative-snapshot catch-up"
    );

    // The heal: import the author's authoritative full snapshot.
    let snapshot = author.export_doc_update(note, None).await.unwrap();
    device
        .import_authoritative_snapshot(note, &snapshot)
        .await
        .unwrap();

    // Ledger CLEARED — the gap closed, so nothing is queued anymore.
    assert!(
        device.pending_import_notes().await.is_empty(),
        "an authoritative-snapshot heal clears the ledger"
    );
    assert!(
        device.notes_needing_snapshot_catchup().await.is_empty(),
        "nothing left to catch up after the heal"
    );

    // And the content actually converged.
    let rendered = device.render_note(note).await.unwrap_or_default();
    assert!(
        rendered.contains("alpha") && rendered.contains("tail-only-no-base"),
        "device converged to the authoritative content; got: {rendered:?}"
    );
}

#[tokio::test]
async fn clean_inbound_apply_clears_a_prior_pending_ledger_entry() {
    // The other heal path (item 2): a note stuck pending is cleared when the
    // MISSING BASE later arrives as a normal delta (no snapshot catch-up
    // needed). REVERT-DISCRIMINATING: drop the `clear_pending_import` on the
    // `Ok(false)` arm of `apply_relay_updates` and the note lingers forever.
    let note = note_id("late-base");

    let author = engine(0x61);
    author
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("late".into()),
            title: "late".into(),
            content: "- base <!-- bid:d4d4d4d4-d4d4-d4d4-d4d4-d4d4d4d4d4d4 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    let full_snapshot = author.export_doc_update(note, None).await.unwrap();
    let pre_vv = author.doc_version(note).await.unwrap();
    author
        .record_local(OpPayload::BlockUpsert {
            block_id: note_id("late-tail"),
            note_id: note,
            parent_block_id: None,
            order_key: "z".into(),
            indent_level: 0,
            text: "tail".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    let tail = author.export_doc_update(note, Some(&pre_vv)).await.unwrap();

    let device = engine(0x62);
    // Tail first (no base) → pending + recorded.
    let _ = device.apply_relay_updates(&[(note, tail)]).await;
    assert_eq!(device.pending_import_notes().await.len(), 1);

    // The base arrives as a normal delta and cleanly integrates → ledger clears.
    let report = device.apply_relay_updates(&[(note, full_snapshot)]).await;
    assert_eq!(report.applied, vec![note], "the base applied cleanly");
    assert!(
        device.pending_import_notes().await.is_empty(),
        "a clean apply of the arriving base clears the pending ledger entry"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// F3 — bounded catch-up escalation for a PERMANENTLY-gapped note
// ─────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn permanently_gapped_note_escalation_is_bounded_then_terminal() {
    // A note whose base no peer ever deposits can never heal, so
    // `notes_needing_snapshot_catchup` must NOT re-escalate it every pass
    // forever (each escalation drives a wasted relay `fetch_snapshots`). It is
    // spaced by exponential backoff and capped at MAX_CATCHUP_ATTEMPTS, after
    // which the note goes TERMINAL (a permanent gap surfaced in the ledger for
    // the sync-health UI) and stops escalating entirely.
    //
    // REVERT-DISCRIMINATING: remove the `catchup_exhausted` gate / the
    // MAX_CATCHUP_ATTEMPTS cap (or the backoff `continue`) in
    // `notes_needing_snapshot_catchup` and the escalation count over this long
    // run blows past MAX (unbounded / every-pass), failing the equality below.
    let note = note_id("permagap");

    // Author a base + a tail edit; export ONLY the tail delta (no base).
    let author = engine(0x71);
    author
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("permagap".into()),
            title: "permagap".into(),
            content: "- alpha <!-- bid:e5e5e5e5-e5e5-e5e5-e5e5-e5e5e5e5e5e5 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    let pre_vv = author.doc_version(note).await.unwrap();
    author
        .record_local(OpPayload::BlockUpsert {
            block_id: note_id("permagap-tail"),
            note_id: note,
            parent_block_id: None,
            order_key: "z".into(),
            indent_level: 0,
            text: "orphan-tail".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    let tail = author.export_doc_update(note, Some(&pre_vv)).await.unwrap();

    // Device with NO base: the tail stays pending on every apply (its base
    // never arrives — a permanent causal gap).
    let device = engine(0x72);

    // Drive many apply passes (each bumps the ledger's import pass) and count
    // how many times the note is ESCALATED to a catch-up. A long horizon so the
    // full exponential-backoff schedule plus several post-terminal passes are
    // exercised.
    let mut escalations = 0usize;
    for _ in 0..256 {
        let _ = device.apply_relay_updates(&[(note, tail.clone())]).await;
        let due = device.notes_needing_snapshot_catchup().await;
        if due.contains(&note) {
            escalations += 1;
        }
    }

    // BOUNDED: exactly MAX_CATCHUP_ATTEMPTS escalations over the whole run,
    // never one-per-pass.
    assert_eq!(
        escalations,
        tesela_sync::engine::MAX_CATCHUP_ATTEMPTS as usize,
        "a permanently-gapped note escalates at most MAX_CATCHUP_ATTEMPTS times, \
         not once per pass"
    );

    // TERMINAL: the note is still in the ledger (visible to sync-health) but
    // marked exhausted, and no longer escalates.
    let ledger = device.pending_import_notes().await;
    assert_eq!(ledger.len(), 1, "the permanent gap stays in the ledger");
    assert!(
        ledger[0].catchup_exhausted,
        "a note past the escalation bound is marked a terminal/permanent gap"
    );
    assert_eq!(
        ledger[0].catchup_attempts,
        tesela_sync::engine::MAX_CATCHUP_ATTEMPTS,
        "attempts stopped exactly at the bound"
    );
    assert!(
        device.notes_needing_snapshot_catchup().await.is_empty(),
        "a terminal (exhausted) note never escalates again"
    );

    // But a genuine heal still clears it — the terminal state is not a
    // dead-end: if the base finally arrives, the whole entry drops.
    let full = author.export_doc_update(note, None).await.unwrap();
    device
        .import_authoritative_snapshot(note, &full)
        .await
        .unwrap();
    assert!(
        device.pending_import_notes().await.is_empty(),
        "an authoritative heal clears even a terminal ledger entry"
    );
}
