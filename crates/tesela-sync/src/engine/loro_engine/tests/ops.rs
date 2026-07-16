use super::*;

#[tokio::test]
async fn note_upsert_records_into_doc() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [9u8; 16];

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("smoke".into()),
            title: "Smoke".into(),
            content: "---\ntitle: Smoke\n---\n- Hello\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    assert_eq!(engine.note_count().await, 1);
    // doc exists; content stored on root meta. Detailed
    // materialization tests land as block ops come online.
}

#[tokio::test]
async fn non_noteupsert_ops_are_silent_noops() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let result = engine
        .record_local(OpPayload::BlockDelete {
            block_id: [3u8; 16],
        })
        .await;
    assert!(result.is_ok());
    assert_eq!(engine.note_count().await, 0);
}

#[tokio::test]
async fn block_upsert_builds_indented_tree() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [1u8; 16];
    let root_block = [10u8; 16];
    let child_block = [11u8; 16];

    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: root_block,
            note_id,
            parent_block_id: None,
            order_key: "a0".into(),
            indent_level: 0,
            text: "root block".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: child_block,
            note_id,
            parent_block_id: Some(root_block),
            order_key: "a0a".into(),
            indent_level: 1,
            text: "child block".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    let rendered = engine.render_note(note_id).await.unwrap();
    assert_eq!(
        rendered,
        "- root block <!-- bid:0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a -->\n  \
         - child block <!-- bid:0b0b0b0b-0b0b-0b0b-0b0b-0b0b0b0b0b0b -->\n"
    );
}

#[tokio::test]
async fn snapshot_export_import_preserves_props_only_empty_block() {
    let dev1 = DeviceId::from_bytes([0xe1; 16]);
    let e1 = LoroEngine::new(dev1, Arc::new(Hlc::new(dev1)));
    let dev2 = DeviceId::from_bytes([0xe2; 16]);
    let e2 = LoroEngine::new(dev2, Arc::new(Hlc::new(dev2)));
    let note_id = [0x42; 16];
    let block_id = [0x24; 16];

    e1.record_local(OpPayload::BlockUpsert {
        block_id,
        note_id,
        parent_block_id: None,
        order_key: "a0".into(),
        indent_level: 0,
        text: "".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    e1.record_local(OpPayload::BlockPropertySet {
        note_id,
        block_id,
        key: "priority".into(),
        value: PropOp::SetScalar(PropScalar::Text("p2".into())),
    })
    .await
    .unwrap();

    let rendered = e1.render_note(note_id).await.unwrap();
    assert_eq!(
        rendered,
        "- <!-- bid:24242424-2424-2424-2424-242424242424 -->\n  priority:: p2\n"
    );

    let snapshot = e1.export_doc_update(note_id, None).await.unwrap();
    e2.import_doc_update(note_id, &snapshot).await.unwrap();

    assert_eq!(e2.render_note(note_id).await.unwrap(), rendered);
}

/// tesela-ows.1 step 2 — ACCEPTANCE: a `status:: done` flip arriving over
/// the wire into the ENGINE (relay/WS/FFI `.relay` import path, NOT an HTTP
/// PUT) triggers the recurrence bump exactly ONCE on the receiving device,
/// and the rolled state converges to every peer without a second bump.
///
/// Exercises the hardest realistic shape: the flip is authored as a
/// container `BlockPropertySet` (the FFI path), and the roll is authored
/// back as CONTAINER prop sets (Lead constraint (a)) — no container clear,
/// no in-text eviction. The container value wins render-time dedup, so the
/// rolled deadline/status render with no render-side change.
///
/// Revert-discriminating: on pre-fix code (no engine hook) the import merges
/// `status:: done` with no roll, so the deadline / `recurrence_done` /
/// `status:: todo` assertions below FAIL.
#[tokio::test]
async fn relay_done_flip_triggers_recurrence_bump_once_and_converges() {
    let dev1 = DeviceId::from_bytes([0xe1; 16]);
    let e1 = LoroEngine::new(dev1, Arc::new(Hlc::new(dev1)));
    let dev2 = DeviceId::from_bytes([0xe2; 16]);
    let e2 = LoroEngine::new(dev2, Arc::new(Hlc::new(dev2)));

    let note = [0x77; 16];
    let block: [u8; 16] = [0x07; 16];
    let bid_hex = "07070707-0707-0707-0707-070707070707";

    // e1 seeds a note with one recurring todo block on a shared lineage.
    e1.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("chores".into()),
        title: "Chores".into(),
        content: format!("- water plants <!-- bid:{bid_hex} -->\n"),
        created_at_millis: 1,
    })
    .await
    .unwrap();
    for (k, v) in [
        ("recurring", "daily count 3"),
        ("deadline", "[[2026-05-07]]"),
        ("status", "todo"),
    ] {
        e1.record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: k.into(),
            value: PropOp::SetScalar(PropScalar::Text(v.into())),
        })
        .await
        .unwrap();
    }

    // e2 bootstraps from e1's full state (both share the block's lineage).
    let base = e1.export_doc_update(note, None).await.unwrap();
    e2.import_doc_update(note, &base).await.unwrap();
    assert_eq!(
        e1.render_note(note).await,
        e2.render_note(note).await,
        "bootstrapped equal"
    );

    // e2 (a non-lifecycle writer, e.g. iOS FFI) flips status → done. No bump
    // is authored on e2 — `record_local` (the local author path) is NOT
    // hooked; the roll happens when a peer IMPORTS this flip.
    let e2_before_flip = e2.doc_version(note).await;
    e2.record_local(OpPayload::BlockPropertySet {
        note_id: note,
        block_id: block,
        key: "status".into(),
        value: PropOp::SetScalar(PropScalar::Text("done".into())),
    })
    .await
    .unwrap();
    let flip = e2
        .export_doc_update(note, e2_before_flip.as_deref())
        .await
        .unwrap();
    let e2_after_flip = e2.doc_version(note).await;
    assert!(
        e2.render_note(note).await.unwrap().contains("status:: done"),
        "e2's own flip stays `done` until the bump comes back"
    );

    // Relay delivers e2's flip to e1 → e1's apply_import runs the lifecycle.
    e1.import_doc_update(note, &flip).await.unwrap();
    let r1 = e1.render_note(note).await.unwrap();
    assert!(
        r1.contains("deadline:: [[2026-05-08]]"),
        "deadline advanced one day on e1: {r1:?}"
    );
    assert!(
        r1.contains("status:: todo") && !r1.contains("status:: done"),
        "status rolled back to todo on e1 (no residual done): {r1:?}"
    );
    assert!(
        r1.contains("recurrence_done:: 1"),
        "recurrence_done stamped on e1: {r1:?}"
    );
    assert!(
        r1.contains("last_completed:: [[2026-05-07]]"),
        "completion memory stamped in the CONTAINER on e1: {r1:?}"
    );
    assert_eq!(
        r1.matches("recurrence_done::").count(),
        1,
        "exactly one recurrence_done line — no double bump: {r1:?}"
    );

    // The bump broadcasts back to e2. It converges AND does not re-bump
    // (the frame carries a done→todo transition, never a fresh flip TO
    // done — the recursive-reimport guard).
    let bump = e1
        .export_doc_update(note, e2_after_flip.as_deref())
        .await
        .unwrap();
    e2.import_doc_update(note, &bump).await.unwrap();
    let r2 = e2.render_note(note).await.unwrap();
    assert_eq!(r1, r2, "e1 and e2 converge after the bump broadcast");
    assert_eq!(
        r2.matches("recurrence_done::").count(),
        1,
        "no double bump on e2 after importing the roll: {r2:?}"
    );

    // Re-delivering the ORIGINAL flip to e1 is idempotent — the frame adds
    // nothing causally new (no flip gate) and the guard is the backstop.
    e1.import_doc_update(note, &flip).await.unwrap();
    let r1b = e1.render_note(note).await.unwrap();
    assert_eq!(
        r1b.matches("recurrence_done::").count(),
        1,
        "re-delivered flip must not advance the series again: {r1b:?}"
    );
    assert!(r1b.contains("deadline:: [[2026-05-08]]"), "{r1b:?}");
}

/// tesela-ows.1 step 2 — the data-loss-class REGRESSION (Lead constraint
/// (a), why attempt 2 died): two independent completions of the SAME
/// recurring occurrence authored on DISJOINT lineages and delivered crossed
/// must converge to EXACTLY ONE advance — AND the rolled peer's completion
/// memory (`recurrence_done` / `last_completed`) must SURVIVE the
/// disjoint-twin heal. It survives here precisely because the roll is
/// authored into the typed props CONTAINER, which twin-heal's per-key union
/// preserves; attempt 2 evicted it to in-text and a max-`TreeID` pick on the
/// non-rolling twin wiped it.
///
/// Assertions are robust to the twin-heal union order (which twin's scalar
/// wins a same-key collision): the invariants are "exactly one bump", "no
/// double advance to 05-09", and "completion memory preserved" — never a
/// dependence on which node the max-`TreeID` rule kept.
#[tokio::test]
async fn crossed_duplicate_completions_converge_single_bump_no_dataloss() {
    let mk = |b: u8| {
        let d = DeviceId::from_bytes([b; 16]);
        LoroEngine::new(d, Arc::new(Hlc::new(d)))
    };
    let e_author = mk(0xe0);
    let e1 = mk(0xe1); // roller/target, shares lineage L1 with e_author
    let e2 = mk(0xe2); // disjoint duplicate author (lineage L2)

    let note = [0x77; 16];
    let block: [u8; 16] = [0x07; 16];
    let bid_hex = "07070707-0707-0707-0707-070707070707";

    async fn seed(e: &LoroEngine, note: [u8; 16], block: [u8; 16], bid_hex: &str, status: &str) {
        e.record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("chores".into()),
            title: "Chores".into(),
            content: format!("- water plants <!-- bid:{bid_hex} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();
        for (k, v) in [
            ("recurring", "daily count 5"),
            ("deadline", "[[2026-05-07]]"),
            ("status", status),
        ] {
            e.record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: block,
                key: k.into(),
                value: PropOp::SetScalar(PropScalar::Text(v.into())),
            })
            .await
            .unwrap();
        }
    }

    // Lineage L1: e_author seeds O1 todo; e1 bootstraps from it (SHARED).
    seed(&e_author, note, block, bid_hex, "todo").await;
    let base = e_author.export_doc_update(note, None).await.unwrap();
    e1.import_doc_update(note, &base).await.unwrap();

    // e_author completes O1 (FFI flip); e1 imports → e1 ROLLS O1→O2.
    let before = e_author.doc_version(note).await;
    e_author
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: "status".into(),
            value: PropOp::SetScalar(PropScalar::Text("done".into())),
        })
        .await
        .unwrap();
    let flip = e_author
        .export_doc_update(note, before.as_deref())
        .await
        .unwrap();
    e1.import_doc_update(note, &flip).await.unwrap();
    let r1_roll = e1.render_note(note).await.unwrap();
    assert!(
        r1_roll.contains("deadline:: [[2026-05-08]]")
            && r1_roll.contains("recurrence_done:: 1")
            && r1_roll.contains("last_completed:: [[2026-05-07]]"),
        "e1 rolled O1→O2 exactly once with completion memory: {r1_roll:?}"
    );

    // Lineage L2 (DISJOINT): e2 independently authored the SAME bid at O1 and
    // completed it — a duplicate completion of the SAME occurrence O1.
    seed(&e2, note, block, bid_hex, "done").await;
    let dup = e2.export_doc_update(note, None).await.unwrap();

    // e1 (at O2) imports the disjoint done-twin. It must NOT advance again,
    // and the twin heal must NOT wipe e1's completion memory.
    e1.import_doc_update(note, &dup).await.unwrap();
    let r1 = e1.render_note(note).await.unwrap();
    assert_eq!(
        r1.matches("recurrence_done::").count(),
        1,
        "duplicate completion of O1 must not add a second bump: {r1:?}"
    );
    assert!(
        r1.contains("recurrence_done:: 1") && r1.contains("last_completed:: [[2026-05-07]]"),
        "completion memory PRESERVED through the disjoint-twin heal: {r1:?}"
    );
    assert!(
        !r1.contains("[[2026-05-09]]"),
        "no double advance to O3 (05-09): {r1:?}"
    );

    // All peers converge to the SAME single-bump state.
    let e1_state = e1.export_doc_update(note, None).await.unwrap();
    e_author.import_doc_update(note, &e1_state).await.unwrap();
    e2.import_doc_update(note, &e1_state).await.unwrap();
    let ra = e_author.render_note(note).await.unwrap();
    let rb = e2.render_note(note).await.unwrap();
    assert_eq!(
        ra.matches("recurrence_done::").count(),
        1,
        "e_author converges to one bump: {ra:?}"
    );
    assert_eq!(
        rb.matches("recurrence_done::").count(),
        1,
        "e2 converges to one bump: {rb:?}"
    );
    assert!(
        !ra.contains("[[2026-05-09]]") && !rb.contains("[[2026-05-09]]"),
        "no peer double-advanced: e_author={ra:?} e2={rb:?}"
    );
}

// A new block with an `after_block_id` hint lands ADJACENT to its
// predecessor (between it and the old next block), NOT at document end.
// This is the headline fix: a mid-note split's new half renders in
// place instead of scattering to the bottom.
#[tokio::test]
async fn positional_insert_lands_adjacent() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [40u8; 16];
    let a = [41u8; 16];
    let c = [42u8; 16];
    let b = [43u8; 16];

    // Seed A, C (append).
    upsert_block(&engine, note_id, a, "A", None).await;
    upsert_block(&engine, note_id, c, "C", None).await;
    assert_eq!(block_texts(&engine, note_id).await, vec!["A", "C"]);

    // Insert B AFTER A → expect A, B, C (not A, C, B).
    upsert_block(&engine, note_id, b, "B", Some(a)).await;
    assert_eq!(
        block_texts(&engine, note_id).await,
        vec!["A", "B", "C"],
        "new block with after-hint must land adjacent, not at end"
    );
}

// Backward compatibility: a BlockUpsert with NO positional hint appends
// at document end — exactly today's behavior. Receive-only devices and
// pre-hint producers depend on this.
#[tokio::test]
async fn positional_insert_no_hint_appends() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [44u8; 16];
    let a = [45u8; 16];
    let c = [46u8; 16];
    let b = [47u8; 16];

    upsert_block(&engine, note_id, a, "A", None).await;
    upsert_block(&engine, note_id, c, "C", None).await;
    // No hint → append at end.
    upsert_block(&engine, note_id, b, "B", None).await;
    assert_eq!(block_texts(&engine, note_id).await, vec!["A", "C", "B"]);
}

// An `after_block_id` that doesn't resolve to a live node (the engine
// never saw the predecessor, or it was deleted) falls back to append.
// Loss-free: the block is still created and rendered; only its position
// degrades to end-of-document (today's behavior).
#[tokio::test]
async fn positional_insert_unknown_predecessor_appends() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [48u8; 16];
    let a = [49u8; 16];
    let b = [50u8; 16];
    let ghost = [99u8; 16]; // never created

    upsert_block(&engine, note_id, a, "A", None).await;
    upsert_block(&engine, note_id, b, "B", Some(ghost)).await;
    // Ghost predecessor → append; B still present, at end.
    assert_eq!(block_texts(&engine, note_id).await, vec!["A", "B"]);
}

// Insert-at-top: `after_block_id == None` appends, but a hint pointing
// at the FIRST block puts the new block second. (Top-of-document insert
// is exercised by the diff path's pos==0 → None = append for a fresh
// note; an explicit top insert in an existing note is rare and falls to
// append, which is loss-free.)
#[tokio::test]
async fn positional_insert_after_first_is_second() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [51u8; 16];
    let a = [52u8; 16];
    let b = [53u8; 16];
    let x = [54u8; 16];

    upsert_block(&engine, note_id, a, "A", None).await;
    upsert_block(&engine, note_id, b, "B", None).await;
    upsert_block(&engine, note_id, x, "X", Some(a)).await; // after A
    assert_eq!(block_texts(&engine, note_id).await, vec!["A", "X", "B"]);
}

// CONVERGENCE: two engines that share a base (A, C) each insert a
// DIFFERENT new block at the SAME adjacent position (after A),
// concurrently. Cross-importing their updates must converge to the SAME
// deterministic order on BOTH engines — no divergence, no panic. This
// is the load-bearing CRDT invariant for `create_at`.
#[tokio::test]
async fn positional_insert_concurrent_converges() {
    let note_id = [55u8; 16];
    let a = [56u8; 16];
    let c = [57u8; 16];
    let b1 = [58u8; 16];
    let b2 = [59u8; 16];

    // Engine 1 builds the shared base A, C.
    let dev1 = DeviceId::from_bytes([0xd1; 16]);
    let e1 = LoroEngine::new(dev1, Arc::new(Hlc::new(dev1)));
    upsert_block(&e1, note_id, a, "A", None).await;
    upsert_block(&e1, note_id, c, "C", None).await;

    // Engine 2 imports the base so both share history (same TreeIDs for
    // A and C — the convergence precondition the cutover relies on).
    let dev2 = DeviceId::from_bytes([0xd2; 16]);
    let e2 = LoroEngine::new(dev2, Arc::new(Hlc::new(dev2)));
    let base = e1.export_doc_update(note_id, None).await.unwrap();
    e2.import_doc_update(note_id, &base).await.unwrap();
    assert_eq!(block_texts(&e2, note_id).await, vec!["A", "C"]);

    // Concurrent adjacent inserts: e1 inserts B1 after A, e2 inserts B2
    // after A — neither has seen the other yet.
    upsert_block(&e1, note_id, b1, "B1", Some(a)).await;
    upsert_block(&e2, note_id, b2, "B2", Some(a)).await;

    // Cross-import both directions.
    let u1 = e1.export_doc_update(note_id, None).await.unwrap();
    let u2 = e2.export_doc_update(note_id, None).await.unwrap();
    e2.import_doc_update(note_id, &u1).await.unwrap();
    e1.import_doc_update(note_id, &u2).await.unwrap();

    let t1 = block_texts(&e1, note_id).await;
    let t2 = block_texts(&e2, note_id).await;
    assert_eq!(
        t1, t2,
        "engines diverged after concurrent positional insert"
    );
    // Both new blocks survive, A first and C last (the inserts went
    // between them).
    assert_eq!(t1.first().map(String::as_str), Some("A"));
    assert_eq!(t1.last().map(String::as_str), Some("C"));
    assert!(t1.contains(&"B1".to_string()) && t1.contains(&"B2".to_string()));
    assert_eq!(t1.len(), 4);
}

#[tokio::test]
async fn block_move_reparents_in_tree() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [3u8; 16];
    let a = [30u8; 16];
    let b = [31u8; 16];
    let c = [32u8; 16];

    // Set up: a (root), b (root), c child of a → render = "a / b / \tc"
    for (id, parent, indent, text) in [
        (a, None, 0u16, "a"),
        (b, None, 0u16, "b"),
        (c, Some(a), 1u16, "c"),
    ] {
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: id,
                note_id,
                parent_block_id: parent,
                order_key: "a0".into(),
                indent_level: indent,
                text: text.into(),
                after_block_id: None,
            })
            .await
            .unwrap();
    }

    engine
        .record_local(OpPayload::BlockMove {
            block_id: c,
            new_parent: Some(b),
            new_order_key: "b0".into(),
        })
        .await
        .unwrap();

    let rendered = engine.render_note(note_id).await.unwrap();
    assert_eq!(
        rendered,
        "- a <!-- bid:1e1e1e1e-1e1e-1e1e-1e1e-1e1e1e1e1e1e -->\n\
         - b <!-- bid:1f1f1f1f-1f1f-1f1f-1f1f-1f1f1f1f1f1f -->\n  \
         - c <!-- bid:20202020-2020-2020-2020-202020202020 -->\n"
    );
}

#[tokio::test]
async fn block_delete_removes_from_render() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [4u8; 16];
    let a = [40u8; 16];
    let b = [41u8; 16];

    for (id, text) in [(a, "keep"), (b, "delete me")] {
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: id,
                note_id,
                parent_block_id: None,
                order_key: "a0".into(),
                indent_level: 0,
                text: text.into(),
                after_block_id: None,
            })
            .await
            .unwrap();
    }

    engine
        .record_local(OpPayload::BlockDelete { block_id: b })
        .await
        .unwrap();

    let rendered = engine.render_note(note_id).await.unwrap();
    assert_eq!(
        rendered,
        "- keep <!-- bid:28282828-2828-2828-2828-282828282828 -->\n"
    );
}

#[tokio::test]
async fn block_move_or_delete_for_unknown_block_is_noop() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);

    let res = engine
        .record_local(OpPayload::BlockMove {
            block_id: [99u8; 16],
            new_parent: None,
            new_order_key: "z".into(),
        })
        .await;
    assert!(res.is_ok());

    let res = engine
        .record_local(OpPayload::BlockDelete {
            block_id: [99u8; 16],
        })
        .await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn render_uses_insertion_order_ignoring_order_key() {
    // SqliteEngine renders by document/insertion order and ignores
    // order_key entirely (apply_block_move's new_order_key param is
    // unused). The shadow must match: blocks render in creation
    // order regardless of the order_key carried on the op.
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [70u8; 16];

    for (id, order, text) in [
        ([70u8; 16], "a5", "created first"),
        ([71u8; 16], "a0", "created second"),
        ([72u8; 16], "ar", "created third"),
    ] {
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: id,
                note_id,
                parent_block_id: None,
                order_key: order.into(),
                indent_level: 0,
                text: text.into(),
                after_block_id: None,
            })
            .await
            .unwrap();
    }

    let rendered = engine.render_note(note_id).await.unwrap();
    assert_eq!(
        rendered,
        "- created first <!-- bid:46464646-4646-4646-4646-464646464646 -->\n\
         - created second <!-- bid:47474747-4747-4747-4747-474747474747 -->\n\
         - created third <!-- bid:48484848-4848-4848-4848-484848484848 -->\n"
    );
}

#[tokio::test]
async fn block_move_changes_indent_not_position() {
    // Reproduces the 2026-05-28 nursery-rhyme divergence: a move
    // must change only the block's indent, never its document
    // position — matching SqliteEngine.
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [73u8; 16];
    let a = [0xa0; 16];
    let b = [0xb0; 16];
    let c = [0xc0; 16];

    // Create three flat top-level blocks: a, b, c.
    for (id, text) in [(a, "a"), (b, "b"), (c, "c")] {
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: id,
                note_id,
                parent_block_id: None,
                order_key: "x".into(),
                indent_level: 0,
                text: text.into(),
                after_block_id: None,
            })
            .await
            .unwrap();
    }
    // Move c under a. SqliteEngine would set c.indent = a.indent+1
    // = 1 and leave c at document position 3 (last). Order stays
    // a, b, c; only c's indent changes.
    engine
        .record_local(OpPayload::BlockMove {
            block_id: c,
            new_parent: Some(a),
            new_order_key: "y".into(),
        })
        .await
        .unwrap();

    let rendered = engine.render_note(note_id).await.unwrap();
    assert_eq!(
        rendered,
        "- a <!-- bid:a0a0a0a0-a0a0-a0a0-a0a0-a0a0a0a0a0a0 -->\n\
         - b <!-- bid:b0b0b0b0-b0b0-b0b0-b0b0-b0b0b0b0b0b0 -->\n  \
         - c <!-- bid:c0c0c0c0-c0c0-c0c0-c0c0-c0c0c0c0c0c0 -->\n"
    );
}

#[tokio::test]
async fn block_upsert_after_delete_recreates_node() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [80u8; 16];
    let block = [81u8; 16];

    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: block,
            note_id,
            parent_block_id: None,
            order_key: "a0".into(),
            indent_level: 0,
            text: "before".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    engine
        .record_local(OpPayload::BlockDelete { block_id: block })
        .await
        .unwrap();
    // After delete, a re-upsert (e.g. peer revives the same block_id)
    // must create a fresh node — without the tombstone filter this
    // would error with "TreeID is deleted or does not exist".
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: block,
            note_id,
            parent_block_id: None,
            order_key: "a0".into(),
            indent_level: 0,
            text: "after".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    let rendered = engine.render_note(note_id).await.unwrap();
    assert_eq!(
        rendered,
        "- after <!-- bid:51515151-5151-5151-5151-515151515151 -->\n"
    );
}

#[tokio::test]
async fn snapshot_round_trip_survives_reload() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("loro");

    // First engine — write a block + verify snapshot file lands.
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
        .await
        .unwrap();
    let note_id = [0xee; 16];
    let block = [0xff; 16];
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: block,
            note_id,
            parent_block_id: None,
            order_key: "a0".into(),
            indent_level: 0,
            text: "persisted".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    drop(engine);

    // Second engine — points at the same dir, loads snapshot,
    // render should match without replaying any oplog ops.
    let hlc2 = Arc::new(Hlc::new(test_device()));
    let reloaded = LoroEngine::with_snapshot_dir(test_device(), hlc2, dir.clone())
        .await
        .unwrap();
    assert_eq!(reloaded.note_count().await, 1);
    let rendered = reloaded.render_note(note_id).await.unwrap();
    assert_eq!(
        rendered,
        "- persisted <!-- bid:ffffffff-ffff-ffff-ffff-ffffffffffff -->\n"
    );
}

#[tokio::test]
async fn note_upsert_after_snapshot_load_does_not_duplicate_blocks() {
    // Regression: a NoteUpsert re-save of the SAME (stamped) content
    // after a snapshot reload must be a no-op — no duplicate nodes
    // AND stable block identity (the tree_matches_blocks fast path).
    // Content carries bid markers, as a real note file does after
    // its first write (unstamped content would mint fresh ids each
    // parse, which is not a realistic re-save).
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("loro");

    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
        .await
        .unwrap();
    let note_id = [0x10; 16];
    let content = "---\ntitle: T\n---\n- a <!-- bid:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa -->\n- b <!-- bid:bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb -->\n";

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("t".into()),
            title: "T".into(),
            content: content.into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    let after_first = engine.render_note(note_id).await.unwrap();
    drop(engine);

    // Reload from snapshot, then re-fire NoteUpsert with same body.
    let hlc2 = Arc::new(Hlc::new(test_device()));
    let reloaded = LoroEngine::with_snapshot_dir(test_device(), hlc2, dir)
        .await
        .unwrap();
    reloaded
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("t".into()),
            title: "T".into(),
            content: content.into(),
            created_at_millis: 2,
        })
        .await
        .unwrap();
    let after_second = reloaded.render_note(note_id).await.unwrap();

    assert_eq!(
        after_first, after_second,
        "second NoteUpsert after snapshot load must not duplicate blocks"
    );
}

#[tokio::test]
async fn identical_unstamped_raw_note_upsert_is_idempotent() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note_id = [0x11; 16];
    let raw = "# Heading\n\nProse\n\n```text\n- payload\n```\n";

    let payload = || OpPayload::NoteUpsert {
        note_id,
        display_alias: Some("raw-reapply".into()),
        title: "Raw reapply".into(),
        content: raw.into(),
        created_at_millis: 1,
    };
    engine.record_local(payload()).await.unwrap();
    let first_ids = tesela_core::note_tree::parse_note(&engine.render_note(note_id).await.unwrap())
        .blocks
        .into_iter()
        .map(|block| block.id)
        .collect::<Vec<_>>();
    engine.record_local(payload()).await.unwrap();

    let rendered = engine.render_note(note_id).await.unwrap();
    let tree = tesela_core::note_tree::parse_note(&rendered);
    assert_eq!(
        tree.blocks.len(),
        3,
        "raw reapply must not duplicate blocks"
    );
    assert_eq!(
        tree.blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>(),
        vec!["# Heading", "Prose", "```text\n- payload\n```"]
    );
    assert_eq!(
        tree.blocks.iter().map(|block| block.id).collect::<Vec<_>>(),
        first_ids,
        "exact raw replay must retain resident identity"
    );
    let docs = engine.inner.docs.read().await;
    let doc = docs.get(&note_id).unwrap();
    let loro_tree = doc.get_tree("blocks");
    let live = loro_tree
        .children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|node| !matches!(loro_tree.is_node_deleted(node), Ok(true)))
        .count();
    assert_eq!(live, 3, "exact replay must not create hidden live twins");
    assert!(duplicate_block_ids(doc).is_empty());
}

#[tokio::test]
async fn partially_stamped_exact_reapply_preserves_explicit_and_minted_ids() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note_id = [0x13; 16];
    let explicit = uuid::Uuid::from_bytes([0x45; 16]);
    let content = format!("# Raw heading\n\n- Explicit <!-- bid:{explicit} -->\n");
    let payload = || OpPayload::NoteUpsert {
        note_id,
        display_alias: Some("partial-stamp".into()),
        title: "Partial stamp".into(),
        content: content.clone(),
        created_at_millis: 1,
    };

    engine.record_local(payload()).await.unwrap();
    let first = tesela_core::note_tree::parse_note(&engine.render_note(note_id).await.unwrap());
    engine.record_local(payload()).await.unwrap();
    let second = tesela_core::note_tree::parse_note(&engine.render_note(note_id).await.unwrap());

    assert_eq!(
        second
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        first
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(second.blocks[1].id, explicit);
}

#[tokio::test]
async fn changed_unstamped_reapply_fails_without_duplicate_or_resurrection() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("loro");
    let device = test_device();
    let engine = LoroEngine::with_snapshot_dir(device, Arc::new(Hlc::new(device)), dir.clone())
        .await
        .unwrap();
    let note_id = [0x14; 16];
    let raw = "First raw\n\nSecond raw\n";
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("raw-policy".into()),
            title: "Raw policy".into(),
            content: raw.into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    let seeded = tesela_core::note_tree::parse_note(&engine.render_note(note_id).await.unwrap());

    let edited = engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("raw-policy".into()),
            title: "Raw policy".into(),
            content: "First raw edited\n\nSecond raw\n".into(),
            created_at_millis: 2,
        })
        .await;
    assert!(matches!(edited, Err(SyncError::Protocol(_))));
    assert_eq!(
        tesela_core::note_tree::parse_note(&engine.render_note(note_id).await.unwrap())
            .blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>(),
        vec!["First raw", "Second raw"]
    );

    engine
        .record_local(OpPayload::BlockDelete {
            block_id: *seeded.blocks[0].id.as_bytes(),
        })
        .await
        .unwrap();
    let stale = engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("raw-policy".into()),
            title: "Raw policy".into(),
            content: raw.into(),
            created_at_millis: 3,
        })
        .await;
    assert!(matches!(stale, Err(SyncError::Protocol(_))));
    assert_eq!(
        tesela_core::note_tree::parse_note(&engine.render_note(note_id).await.unwrap())
            .blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Second raw"]
    );
    drop(engine);

    let reloaded = LoroEngine::with_snapshot_dir(device, Arc::new(Hlc::new(device)), dir)
        .await
        .unwrap();
    assert_eq!(
        tesela_core::note_tree::parse_note(&reloaded.render_note(note_id).await.unwrap())
            .blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Second raw"]
    );
}

#[tokio::test]
async fn deleting_one_lifted_region_preserves_adjacent_regions() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note_id = [0x12; 16];
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("lifted-delete".into()),
            title: "Lifted delete".into(),
            content: "First raw\n\nSecond raw\n\nThird raw\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    let before = engine.render_note(note_id).await.unwrap();
    let middle = *tesela_core::note_tree::parse_note(&before).blocks[1]
        .id
        .as_bytes();

    engine
        .record_local(OpPayload::BlockDelete { block_id: middle })
        .await
        .unwrap();
    let rendered = engine.render_note(note_id).await.unwrap();
    assert!(rendered.contains("First raw"));
    assert!(!rendered.contains("Second raw"));
    assert!(rendered.contains("Third raw"));
}

#[tokio::test]
async fn corrupt_snapshot_skipped_on_load() {
    // Write a garbage .bin file with a valid-looking hex name.
    // Load should warn + skip without panicking, and the engine
    // should still be functional.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("loro");
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let bad_id = [0xab; 16];
    let bad_path = dir.join(format!("{}.bin", hex::encode(bad_id)));
    tokio::fs::write(&bad_path, b"this is not a Loro snapshot")
        .await
        .unwrap();

    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
        .await
        .unwrap();
    // Corrupt note didn't load; engine works for a fresh note.
    assert_eq!(engine.note_count().await, 0);

    let good_id = [0xcd; 16];
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: [0xef; 16],
            note_id: good_id,
            parent_block_id: None,
            order_key: "a0".into(),
            indent_level: 0,
            text: "still works".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    assert!(engine.render_note(good_id).await.is_some());
}

#[tokio::test]
async fn snapshot_dir_created_when_missing() {
    // Construct with a path that doesn't exist yet — should be
    // created, not error.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("loro").join("nested").join("path");
    assert!(!dir.exists());

    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
        .await
        .unwrap();
    assert!(dir.exists());
    assert_eq!(engine.note_count().await, 0);
}

#[tokio::test]
async fn note_delete_retains_tombstone_snapshot() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("loro");
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
        .await
        .unwrap();
    let note_id = [0xdd; 16];

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("doomed".into()),
            title: "Doomed".into(),
            content: "---\ntitle: Doomed\n---\n- bye\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    let path = dir.join(format!("{}.bin", hex::encode(note_id)));
    assert!(path.exists(), "snapshot should land for new note");

    engine
        .record_local(OpPayload::NoteDelete {
            note_id,
            display_alias: Some("doomed".into()),
        })
        .await
        .unwrap();
    assert!(
        path.exists(),
        "NoteDelete must retain its tombstone snapshot for relay and restart durability"
    );
    drop(engine);

    let reopened = LoroEngine::with_snapshot_dir(
        test_device(),
        Arc::new(Hlc::new(test_device())),
        dir,
    )
    .await
    .unwrap();
    assert_eq!(reopened.note_count().await, 0);
    assert!(reopened.render_note(note_id).await.is_none());
}

#[tokio::test]
async fn note_upsert_renders_page_properties() {
    // A page-property-only note (query page) must round-trip its
    // properties through the shadow — previously rendered empty.
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [0x5a; 16];
    let content = "---\ntitle: Saved\n---\n\nquery:: kind:page\nsort:: modified desc\n";

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("saved".into()),
            title: "Saved".into(),
            content: content.into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    let rendered = engine.render_note(note_id).await.unwrap();
    // render_note omits frontmatter (lives on disk, not the shadow);
    // page properties render in order.
    assert_eq!(rendered, "query:: kind:page\nsort:: modified desc\n");
}

#[tokio::test]
async fn render_note_full_includes_frontmatter() {
    // The cutover dry-run surface: render_note_full must reproduce the
    // verbatim frontmatter (from the doc's stored content) + body, so
    // it equals what materialization would write to disk. For a note
    // whose source is itself canonical, this round-trips byte-for-byte.
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [0x7f; 16];
    let content =
        "---\ntitle: Full\n---\n\n- hello <!-- bid:00000000-0000-0000-0000-000000000001 -->\n";

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("full".into()),
            title: "Full".into(),
            content: content.into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // render_note (body only) drops the frontmatter…
    let body = engine.render_note(note_id).await.unwrap();
    assert!(
        !body.starts_with("---"),
        "render_note must omit frontmatter, got: {body:?}"
    );
    // …render_note_full prepends it back, byte-identical to the source.
    let full = engine.render_note_full(note_id).await.unwrap();
    assert_eq!(full, content, "render_note_full should reproduce source");
    assert!(
        full.starts_with("---\ntitle: Full\n---\n"),
        "render_note_full must carry frontmatter, got: {full:?}"
    );
}

#[tokio::test]
async fn note_upsert_updates_property_shaped_text_inside_fence() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note_id = [0x7d; 16];
    let before =
        "- <!-- bid:33333333-3333-3333-3333-333333333333 -->\n  ```text\n  status:: todo\n  ```\n";
    let after = before.replace("status:: todo", "status:: done");

    for content in [before.to_string(), after.clone()] {
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("fenced-status".into()),
                title: "Fenced status".into(),
                content,
                created_at_millis: 1,
            })
            .await
            .unwrap();
    }

    assert_eq!(
        engine.render_note(note_id).await.as_deref(),
        Some(after.as_str())
    );
}

#[tokio::test]
async fn typed_property_dedup_does_not_delete_same_key_inside_fence() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note_id = [0x7e; 16];
    let block_id = [0x34; 16];

    engine
        .record_local(OpPayload::BlockUpsert {
            block_id,
            note_id,
            parent_block_id: None,
            order_key: "a".into(),
            indent_level: 0,
            text: "```text\nstatus:: literal payload\n```".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id,
            block_id,
            key: "status".into(),
            value: PropOp::SetScalar(PropScalar::Text("done".into())),
        })
        .await
        .unwrap();

    let rendered = engine.render_note(note_id).await.unwrap();
    assert!(
        rendered.contains("status:: literal payload"),
        "fenced payload survives typed-property dedup: {rendered:?}"
    );
    assert!(
        rendered.ends_with("  status:: done\n"),
        "typed property still materializes after the fence: {rendered:?}"
    );
}

#[tokio::test]
async fn migrate_on_apply_keeps_fenced_property_as_text() {
    let device = test_device();
    let engine = LoroEngine::new_migrating(device, Arc::new(Hlc::new(device)));
    let note_id = [0x7f; 16];
    let block_id = [0x35; 16];
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id,
            note_id,
            parent_block_id: None,
            order_key: "a".into(),
            indent_level: 0,
            text: "```text\nstatus:: literal payload\n```".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    assert_eq!(
        engine.render_note(note_id).await.as_deref(),
        Some(
            "- <!-- bid:35353535-3535-3535-3535-353535353535 -->\n  ```text\n  status:: literal payload\n  ```\n"
        )
    );
}

#[tokio::test]
async fn note_upsert_stores_lean_frontmatter_not_full_content() {
    // The dedup invariant: a NoteUpsert must NOT duplicate the body onto
    // root meta. Storing the full markdown there doubled every snapshot
    // (a 1.3 MB page → +1.3 MB of redundant history past the relay's body
    // limit). The body lives only in the tree; root carries just the
    // verbatim frontmatter, and the full markdown is reconstructed on
    // demand. tags (frontmatter) + links (body) must still index from the
    // reconstruction — proving nothing was lost by not storing content.
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [0x6c; 16];
    let content =
        "---\ntitle: Lean\ntags: [alpha]\n---\n\n- see [[target]] #beta <!-- bid:00000000-0000-0000-0000-00000000000a -->\n";

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("lean".into()),
            title: "Lean".into(),
            content: content.into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    {
        let docs = engine.inner.docs.read().await;
        let root = docs.get(&note_id).unwrap().get_map("root");
        assert!(
            root.get("content").is_none(),
            "lean schema must not store full content on root meta"
        );
        assert_eq!(
            root.get("frontmatter")
                .and_then(|v| v.into_value().ok())
                .and_then(|v| v.into_string().ok())
                .map(|s| (*s).clone()),
            Some("---\ntitle: Lean\ntags: [alpha]\n---\n".to_string()),
            "verbatim frontmatter stored on root meta"
        );
    }

    // Reconstruction round-trips the source byte-for-byte…
    let full = engine.render_note_full(note_id).await.unwrap();
    assert_eq!(full, content, "render_note_full reconstructs from the tree");

    // …and the index still derives the frontmatter tag + body tag/link
    // from the reconstruction (not from a stored copy of content).
    let entry = engine
        .index_entries()
        .await
        .into_iter()
        .find(|e| e.note_id == hex_id(&note_id))
        .unwrap();
    assert!(
        entry.tags.contains(&"alpha".to_string()),
        "frontmatter tag: {:?}",
        entry.tags
    );
    assert!(
        entry.tags.contains(&"beta".to_string()),
        "inline body tag: {:?}",
        entry.tags
    );
    assert_eq!(entry.links, vec!["target".to_string()], "body link indexed");
}

#[tokio::test]
async fn note_upsert_indexes_resident_typed_page_tags_from_materialized_doc() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note_id = [0x6b; 16];
    let content = "- Body <!-- bid:4a4a4a4a-4a4a-4a4a-4a4a-4a4a4a4a4a4a -->\n";
    let upsert = || OpPayload::NoteUpsert {
        note_id,
        display_alias: Some("typed-page-tag".into()),
        title: "Typed page tag".into(),
        content: content.into(),
        created_at_millis: 1,
    };
    engine.record_local(upsert()).await.unwrap();
    engine
        .record_local(OpPayload::PagePropertySet {
            note_id,
            key: "tags".into(),
            value: PropOp::AddToList(PropScalar::Text("resident".into())),
        })
        .await
        .unwrap();

    // This whole-content payload omits the typed page property. Its legacy
    // list is replaced, but the typed container remains authoritative.
    engine.record_local(upsert()).await.unwrap();

    assert!(engine
        .render_note_full(note_id)
        .await
        .unwrap()
        .contains("tags:: resident"));
    let entry = engine
        .index_entries()
        .await
        .into_iter()
        .find(|entry| entry.note_id == hex_id(&note_id))
        .unwrap();
    assert!(entry.tags.contains(&"resident".to_string()));
}

#[tokio::test]
async fn large_lifted_fence_snapshot_does_not_duplicate_body() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note_id = [0x6d; 16];
    let payload = "x".repeat(128 * 1024);
    let content = format!("```text\n{payload}\n```\n");
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("large-fence".into()),
            title: "Large fence".into(),
            content: content.clone(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    let snapshot = engine.export_doc_update(note_id, None).await.unwrap();
    assert!(
        snapshot.len() < content.len() + content.len() / 2,
        "snapshot must carry one body copy, not the old doubled root mirror: snapshot={} source={}",
        snapshot.len(),
        content.len()
    );
    let docs = engine.inner.docs.read().await;
    assert!(docs
        .get(&note_id)
        .unwrap()
        .get_map("root")
        .get("content")
        .is_none());
}

#[tokio::test]
async fn matching_legacy_root_content_lifts_missing_regions_before_retirement() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note_id = [0x6e; 16];
    let old_body_id = [0x44; 16];
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: old_body_id,
            note_id,
            parent_block_id: None,
            order_key: "a".into(),
            indent_level: 0,
            text: "Body".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    let legacy = "# Heading\n\nLegacy prose\n\n- Body\n";
    {
        let doc = engine.doc_for_note_mut(note_id).await;
        doc.get_map("root").insert("content", legacy).unwrap();
        doc.commit();
    }

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("legacy-lift".into()),
            title: "Legacy lift".into(),
            content: legacy.into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    let rendered = engine.render_note_full(note_id).await.unwrap();
    assert!(tesela_core::note_tree::canonicalization_preserves_structure(legacy, &rendered));
    let blocks = tesela_core::note_tree::parse_note(&rendered).blocks;
    assert_eq!(
        blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>(),
        vec!["# Heading", "Legacy prose", "Body"]
    );
    assert_eq!(blocks[2].id.as_bytes(), &old_body_id);
    let docs = engine.inner.docs.read().await;
    assert!(docs
        .get(&note_id)
        .unwrap()
        .get_map("root")
        .get("content")
        .is_none());
}

#[tokio::test]
async fn stale_partial_note_upsert_cannot_retire_legacy_root_content() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note_id = [0x6f; 16];
    let body_id = uuid::Uuid::from_bytes([0x46; 16]);
    let legacy = format!("# Heading\n\nLegacy prose\n\n- Body <!-- bid:{body_id} -->\n");
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: *body_id.as_bytes(),
            note_id,
            parent_block_id: None,
            order_key: "a".into(),
            indent_level: 0,
            text: "Body".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    {
        let doc = engine.doc_for_note_mut(note_id).await;
        doc.get_map("root")
            .insert("content", legacy.as_str())
            .unwrap();
        doc.commit();
    }

    let stale = engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("legacy-stale".into()),
            title: "Legacy stale".into(),
            content: format!("- Body <!-- bid:{body_id} -->\n"),
            created_at_millis: 1,
        })
        .await;
    assert!(matches!(stale, Err(SyncError::Protocol(_))));
    assert_eq!(
        engine.render_note_full(note_id).await.as_deref(),
        Some(legacy.as_str())
    );
    let docs = engine.inner.docs.read().await;
    assert!(docs
        .get(&note_id)
        .unwrap()
        .get_map("root")
        .get("content")
        .is_some());
}

#[tokio::test]
async fn legacy_root_content_prunes_reservation_without_rewriting_retained_bytes() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note_id = [0x70; 16];
    let empty_id = uuid::Uuid::from_bytes([0x71; 16]);
    let legacy = format!("# Heading\n\nLegacy prose\n\n- <!-- bid:{empty_id} -->\n");
    let expected = "# Heading\n\nLegacy prose\n\n";
    {
        let doc = engine.doc_for_note_mut(note_id).await;
        doc.get_map("root")
            .insert("content", legacy.as_str())
            .unwrap();
        doc.commit();
    }

    assert_eq!(
        engine.render_note_full(note_id).await.as_deref(),
        Some(expected)
    );
    assert_eq!(
        engine.render_note_full(note_id).await.as_deref(),
        Some(expected),
        "repeated legacy projection must be byte-stable"
    );
}

#[tokio::test]
async fn bare_leaf_blocks_are_hidden_from_rendered_projection() {
    let engine = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    let note_id = [0x4b; 16];
    let real_id = uuid::Uuid::from_bytes([0x4a; 16]);
    let empty_id = uuid::Uuid::from_bytes([0x4b; 16]);
    let content = format!("- real <!-- bid:{real_id} -->\n- <!-- bid:{empty_id} -->\n");
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("blank-leaf".into()),
            title: "Blank leaf".into(),
            content,
            created_at_millis: 1,
        })
        .await
        .unwrap();

    let rendered = engine.render_note(note_id).await.unwrap();
    assert!(rendered.contains(&real_id.to_string()));
    assert!(!rendered.contains(&empty_id.to_string()));
}

#[tokio::test]
async fn empty_parent_with_child_remains_in_rendered_projection() {
    let engine = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    let note_id = [0x4c; 16];
    let parent_id = uuid::Uuid::from_bytes([0x4c; 16]);
    let child_id = uuid::Uuid::from_bytes([0x4d; 16]);
    let content = format!("- <!-- bid:{parent_id} -->\n  - child <!-- bid:{child_id} -->\n");
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("empty-parent".into()),
            title: "Empty parent".into(),
            content,
            created_at_millis: 1,
        })
        .await
        .unwrap();

    let rendered = engine.render_note(note_id).await.unwrap();
    assert!(rendered.contains(&parent_id.to_string()));
    assert!(rendered.contains(&child_id.to_string()));
}

#[tokio::test]
async fn render_note_full_body_only_when_no_frontmatter() {
    // A note whose content never carried frontmatter materializes
    // body-only — render_note_full == render_note in that case.
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [0x80; 16];
    let content = "- bare <!-- bid:00000000-0000-0000-0000-000000000002 -->\n";

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("bare".into()),
            title: "bare".into(),
            content: content.into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    let body = engine.render_note(note_id).await.unwrap();
    let full = engine.render_note_full(note_id).await.unwrap();
    assert_eq!(full, body, "no frontmatter → full equals body");
    assert_eq!(full, content);
}

#[tokio::test]
async fn index_doc_tracks_notes() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: [1u8; 16],
            display_alias: Some("alpha".into()),
            title: "Alpha".into(),
            content: "- a\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: [2u8; 16],
            display_alias: Some("beta".into()),
            title: "Beta".into(),
            content: "- b\n".into(),
            created_at_millis: 2,
        })
        .await
        .unwrap();

    let entries = engine.index_entries().await;
    assert_eq!(entries.len(), 2);
    let titles: Vec<_> = entries.iter().map(|e| e.title.as_str()).collect();
    assert!(titles.contains(&"Alpha"));
    assert!(titles.contains(&"Beta"));
    let slugs: Vec<_> = entries.iter().map(|e| e.slug.as_str()).collect();
    assert!(slugs.contains(&"alpha"));

    // Delete removes the index entry.
    engine
        .record_local(OpPayload::NoteDelete {
            note_id: [1u8; 16],
            display_alias: Some("alpha".into()),
        })
        .await
        .unwrap();
    let entries = engine.index_entries().await;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "Beta");
}

#[tokio::test]
async fn note_upsert_reconciles_drifted_tree() {
    // Review finding [2], revised 2026-06-10: a full-content NoteUpsert
    // re-syncs an already-populated tree NON-destructively — it heals
    // the blocks its body carries (text/indent, in place, lineage
    // preserved) but must NOT remove live blocks absent from the body
    // (the stale-PUT anti-clobber rule; on the real fleet "absent from
    // a whole-content save" is routinely a peer's concurrent block, and
    // the old clear+reseed deleting it was data-loss vector #2 — and
    // the same reseed RESURRECTED explicitly-deleted blocks, the
    // 2026-06-10 iOS delete-revert bug). Removal flows ONLY through an
    // explicit BlockDelete.
    let tmp = tempfile::tempdir().unwrap();
    let engine = LoroEngine::with_dirs(
        test_device(),
        Arc::new(Hlc::new(test_device())),
        tmp.path().join("loro"),
        Some(tmp.path().join("notes")),
    )
    .await
    .unwrap();
    let note_id = [0x55; 16];
    let body = "- one <!-- bid:11111111-1111-1111-1111-111111111111 -->\n- two <!-- bid:22222222-2222-2222-2222-222222222222 -->\n";
    let up = |content: String| OpPayload::NoteUpsert {
        note_id,
        display_alias: Some("n".into()),
        title: "N".into(),
        content,
        created_at_millis: 1,
    };
    engine.record_local(up(body.to_string())).await.unwrap();
    assert_eq!(engine.render_note(note_id).await.unwrap(), body);

    // Drift the tree out of band: a stale extra block AND a text drift
    // on a body block.
    let stale_bid: [u8; 16] = [0x33; 16];
    {
        let doc = engine.doc_for_note_mut(note_id).await;
        let tree = doc.get_tree("blocks");
        let n = tree.create(TreeParentId::Root).unwrap();
        let m = tree.get_meta(n).unwrap();
        m.insert("block_id", hex_id(&stale_bid).as_str()).unwrap();
        write_block_text(&m, "STALE").unwrap();
        m.insert("indent_level", 0i64).unwrap();
        let drifted = find_node_by_block_id(&tree, "11111111111111111111111111111111").unwrap();
        let dm = tree.get_meta(drifted).unwrap();
        write_block_text(&dm, "one DRIFTED").unwrap();
        doc.commit();
        // On a real flow a peer block arrives via import, whose
        // `refresh_note_derived` registers it in the block_index — do
        // the same so the explicit delete below can resolve its doc.
        engine.refresh_note_derived(note_id, &doc).await;
    }
    assert!(engine.render_note(note_id).await.unwrap().contains("STALE"));
    assert!(engine
        .render_note(note_id)
        .await
        .unwrap()
        .contains("one DRIFTED"));

    // Re-save the canonical body: body blocks heal in place; the
    // unknown live block SURVIVES (no destructive reseed).
    engine.record_local(up(body.to_string())).await.unwrap();
    let rendered = engine.render_note(note_id).await.unwrap();
    assert!(
        !rendered.contains("one DRIFTED") && rendered.contains("- one"),
        "body-block text drift heals in place: {rendered:?}"
    );
    assert!(
        rendered.contains("STALE"),
        "a live block absent from the body must survive a whole-content save: {rendered:?}"
    );

    // Removal is explicit-only.
    engine
        .record_local(OpPayload::BlockDelete {
            block_id: stale_bid,
        })
        .await
        .unwrap();
    let rendered = engine.render_note(note_id).await.unwrap();
    assert!(!rendered.contains("STALE"), "{rendered:?}");
    assert_eq!(
        rendered, body,
        "explicit delete restores the canonical body"
    );
}

#[tokio::test]
async fn three_engines_converge_via_broadcast_relay() {
    // PHASE 5: the relay BROADCAST cursor model (the recon's #1
    // flagged risk). Three engines edit the same note; each tick
    // every engine broadcasts its per-note deltas and every other
    // engine imports them idempotently. Assert all three converge,
    // and that a steady-state tick produces nothing (bounded
    // re-broadcast, no infinite loop).
    let mk = |seed: u8| {
        let d = DeviceId::from_bytes([seed; 16]);
        LoroEngine::new(d, Arc::new(Hlc::new(d)))
    };
    let a = mk(0xa1);
    let b = mk(0xb2);
    let c = mk(0xc3);
    let note = [0x88; 16];

    // A creates the note; one broadcast round seeds B and C.
    a.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("n".into()),
        title: "N".into(),
        content: "- base <!-- bid:01010101-0101-0101-0101-010101010101 -->\n".into(),
        created_at_millis: 1,
    })
    .await
    .unwrap();

    // Helper: one full relay round — everyone broadcasts, everyone
    // else imports.
    async fn relay_round(engines: &[&LoroEngine]) {
        let mut bus: Vec<([u8; 16], Vec<u8>)> = Vec::new();
        for e in engines {
            let produced = e.produce_relay_updates().await;
            let committed: Vec<([u8; 16], Vec<u8>)> =
                produced.iter().map(|(d, _, vv)| (*d, vv.clone())).collect();
            for (d, b, _) in &produced {
                bus.push((*d, b.clone()));
            }
            // Simulate a confirmed send → advance the cursor.
            e.commit_broadcast_cursors(&committed).await;
        }
        for e in engines {
            e.apply_relay_updates(&bus).await;
        }
    }
    let all = [&a, &b, &c];
    relay_round(&all).await;
    relay_round(&all).await; // second round propagates any transitive deltas

    // Concurrent edits on all three.
    a.record_local(OpPayload::BlockUpsert {
        block_id: [0xaa; 16],
        note_id: note,
        parent_block_id: None,
        order_key: "a".into(),
        indent_level: 0,
        text: "A edit".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    b.record_local(OpPayload::BlockUpsert {
        block_id: [0xbb; 16],
        note_id: note,
        parent_block_id: None,
        order_key: "b".into(),
        indent_level: 0,
        text: "B edit".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    c.record_local(OpPayload::BlockUpsert {
        block_id: [0xcc; 16],
        note_id: note,
        parent_block_id: None,
        order_key: "c".into(),
        indent_level: 0,
        text: "C edit".into(),
        after_block_id: None,
    })
    .await
    .unwrap();

    // A couple of relay rounds to fully propagate.
    relay_round(&all).await;
    relay_round(&all).await;

    let ra = a.render_note(note).await.unwrap();
    let rb = b.render_note(note).await.unwrap();
    let rc = c.render_note(note).await.unwrap();
    assert_eq!(ra, rb, "A and B converge");
    assert_eq!(rb, rc, "B and C converge");
    for needle in ["base", "A edit", "B edit", "C edit"] {
        assert!(ra.contains(needle), "converged state has {needle}: {ra}");
    }

    // Steady state: a further round broadcasts nothing new.
    let nothing: usize = {
        let mut n = 0;
        for e in &all {
            n += e.produce_relay_updates().await.len();
        }
        n
    };
    assert_eq!(
        nothing, 0,
        "no new broadcasts at steady state (bounded re-broadcast)"
    );
}

#[tokio::test]
async fn trait_level_delta_methods_converge_cursor_free() {
    // INSTANT-MULTIDEVICE PHASE 0: the live WS path holds `dyn SyncEngine`
    // and exchanges deltas via the NEW trait-level doc_version /
    // export_doc_update / import_doc_update. This proves (1) those methods
    // are reachable + correct through the trait object (the FFI/server
    // holder shape), and (2) the live export is CURSOR-FREE — it must NOT
    // advance the relay's broadcast cursor, so the relay path still sees
    // the note as pending (spec finding #3: no WS/relay cursor contention).
    let a_concrete = LoroEngine::new(
        DeviceId::from_bytes([0xc1; 16]),
        Arc::new(Hlc::new(DeviceId::from_bytes([0xc1; 16]))),
    );
    let b_concrete = LoroEngine::new(
        DeviceId::from_bytes([0xd2; 16]),
        Arc::new(Hlc::new(DeviceId::from_bytes([0xd2; 16]))),
    );
    let note = [0x88; 16];

    a_concrete
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("shared".into()),
            title: "Shared".into(),
            content: "- base <!-- bid:02020202-0202-0202-0202-020202020202 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // Drive the exchange THROUGH the trait object, exactly as the live WS
    // path (and the FFI) will.
    let a: &dyn SyncEngine = &a_concrete;
    let b: &dyn SyncEngine = &b_concrete;

    let bootstrap = a.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &bootstrap).await.unwrap();
    assert_eq!(
        a.render_note(note).await,
        b.render_note(note).await,
        "bootstrap via trait methods converges"
    );

    // Cursor-free invariant: the live export above did NOT advance the
    // broadcast cursor, so the relay producer still owes this note. (If
    // export had consumed the cursor, produce_relay_updates would return
    // nothing — the finding-#3 bug.)
    let pending = a.produce_relay_updates().await;
    assert!(
        pending.iter().any(|(nid, _, _)| *nid == note),
        "cursor-free export must leave the note pending for the relay path"
    );

    // Concurrent edits, exchanged both ways via the trait object.
    a_concrete
        .record_local(OpPayload::BlockUpsert {
            block_id: [0xca; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "a".into(),
            indent_level: 0,
            text: "from A".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    b_concrete
        .record_local(OpPayload::BlockUpsert {
            block_id: [0xcb; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "b".into(),
            indent_level: 0,
            text: "from B".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    let b_vv = b.doc_version(note).await;
    let a_upd = a.export_doc_update(note, b_vv.as_deref()).await.unwrap();
    b.import_doc_update(note, &a_upd).await.unwrap();
    let a_vv = a.doc_version(note).await;
    let b_upd = b.export_doc_update(note, a_vv.as_deref()).await.unwrap();
    a.import_doc_update(note, &b_upd).await.unwrap();

    let ra = a.render_note(note).await.unwrap();
    let rb = b.render_note(note).await.unwrap();
    assert_eq!(ra, rb, "trait-level exchange converges — no flashing");
    assert!(ra.contains("base") && ra.contains("from A") && ra.contains("from B"));
}

#[tokio::test]
async fn two_engines_converge_on_concurrent_edits_no_flashing() {
    // PHASE 4 KEYSTONE: the flashing fix at the engine level. Two
    // LoroEngines (distinct devices/PeerIDs) edit the SAME note
    // concurrently, exchange Loro updates, and converge to one
    // deterministic state on both sides — stable across repeated
    // exchange (no ping-pong). The hand-rolled engine could not do
    // this; that's the whole reason for the migration.
    let a = LoroEngine::new(
        DeviceId::from_bytes([0xa1; 16]),
        Arc::new(Hlc::new(DeviceId::from_bytes([0xa1; 16]))),
    );
    let b = LoroEngine::new(
        DeviceId::from_bytes([0xb2; 16]),
        Arc::new(Hlc::new(DeviceId::from_bytes([0xb2; 16]))),
    );
    assert_ne!(
        a.peer_id(),
        b.peer_id(),
        "devices must have distinct peer ids"
    );
    let note = [0x77; 16];

    // A creates the note with one stamped block; B bootstraps from
    // A's full state (the new-device-join path).
    a.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("shared".into()),
        title: "Shared".into(),
        content: "- base <!-- bid:01010101-0101-0101-0101-010101010101 -->\n".into(),
        created_at_millis: 1,
    })
    .await
    .unwrap();
    let bootstrap = a.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &bootstrap).await.unwrap();
    assert_eq!(
        a.render_note(note).await,
        b.render_note(note).await,
        "bootstrapped equal"
    );

    // Concurrent edits: A appends a block, B appends a different one.
    a.record_local(OpPayload::BlockUpsert {
        block_id: [0xaa; 16],
        note_id: note,
        parent_block_id: None,
        order_key: "a".into(),
        indent_level: 0,
        text: "from A".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    b.record_local(OpPayload::BlockUpsert {
        block_id: [0xbb; 16],
        note_id: note,
        parent_block_id: None,
        order_key: "b".into(),
        indent_level: 0,
        text: "from B".into(),
        after_block_id: None,
    })
    .await
    .unwrap();

    // Exchange updates both ways (two relay ticks), using each peer's
    // version vector as the cursor.
    let b_vv = b.doc_version(note).await;
    let a_upd = a.export_doc_update(note, b_vv.as_deref()).await.unwrap();
    b.import_doc_update(note, &a_upd).await.unwrap();
    let a_vv = a.doc_version(note).await;
    let b_upd = b.export_doc_update(note, a_vv.as_deref()).await.unwrap();
    a.import_doc_update(note, &b_upd).await.unwrap();

    let ra = a.render_note(note).await.unwrap();
    let rb = b.render_note(note).await.unwrap();
    assert_eq!(ra, rb, "engines converge to identical state — no flashing");
    assert!(ra.contains("base") && ra.contains("from A") && ra.contains("from B"));

    // Re-exchange must be a stable no-op (no oscillation).
    let b_vv2 = b.doc_version(note).await;
    if let Some(u) = a.export_doc_update(note, b_vv2.as_deref()).await {
        if !u.is_empty() {
            b.import_doc_update(note, &u).await.unwrap();
        }
    }
    assert_eq!(
        a.render_note(note).await.unwrap(),
        ra,
        "stable after re-exchange"
    );
    assert_eq!(
        b.render_note(note).await.unwrap(),
        rb,
        "stable after re-exchange"
    );
}

#[tokio::test]
async fn concurrent_first_property_set_on_shared_block_both_survive() {
    // P1.9b FOUNDATION: two devices share a base note carrying ONE
    // propsless block (seeded via NoteUpsert, so the block node reaches
    // SHARED history before the peers diverge). A first-sets scalar
    // `status`, B first-sets scalar `priority` — DISTINCT keys —
    // concurrently. After a bidirectional exchange BOTH keys must be
    // present on BOTH replicas.
    //
    // Without eager-seeding the per-block `props`/`prop_keys` containers
    // at the shared-base creation site, each device's FIRST property set
    // MINTS a rival `props` map (Loro derives the child container id from
    // the creating op). On merge the two rival maps collide at the same
    // node-meta register and one OVERWRITES the other (LWW, not union),
    // so one device's property vanishes. Seeding the empty containers
    // into shared history first makes both get-or-create resolve to the
    // SAME child id → union.
    let a = LoroEngine::new(
        DeviceId::from_bytes([0xa1; 16]),
        Arc::new(Hlc::new(DeviceId::from_bytes([0xa1; 16]))),
    );
    let b = LoroEngine::new(
        DeviceId::from_bytes([0xb2; 16]),
        Arc::new(Hlc::new(DeviceId::from_bytes([0xb2; 16]))),
    );
    let note = [0x88; 16];
    // The seeded block's id is the bid in the comment: bytes [0x07; 16].
    let block: [u8; 16] = [0x07; 16];

    a.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("shared".into()),
        title: "Shared".into(),
        content: "- base <!-- bid:07070707-0707-0707-0707-070707070707 -->\n".into(),
        created_at_millis: 1,
    })
    .await
    .unwrap();

    // B bootstraps from A's full state — the shared base now lives in
    // both peers' history (the propsless block node included).
    let bootstrap = a.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &bootstrap).await.unwrap();
    assert_eq!(
        a.render_note(note).await,
        b.render_note(note).await,
        "bootstrapped equal"
    );

    // Concurrent FIRST property sets on the SAME shared block, DISTINCT
    // keys. Neither device has seen the other's set yet.
    a.record_local(OpPayload::BlockPropertySet {
        note_id: note,
        block_id: block,
        key: "status".into(),
        value: PropOp::SetScalar(crate::PropScalar::Text("doing".into())),
    })
    .await
    .unwrap();
    b.record_local(OpPayload::BlockPropertySet {
        note_id: note,
        block_id: block,
        key: "priority".into(),
        value: PropOp::SetScalar(crate::PropScalar::Int(3)),
    })
    .await
    .unwrap();

    // Exchange both ways, using each peer's version vector as the cursor.
    let b_vv = b.doc_version(note).await;
    let a_upd = a.export_doc_update(note, b_vv.as_deref()).await.unwrap();
    b.import_doc_update(note, &a_upd).await.unwrap();
    let a_vv = a.doc_version(note).await;
    let b_upd = b.export_doc_update(note, a_vv.as_deref()).await.unwrap();
    a.import_doc_update(note, &b_upd).await.unwrap();

    let ra = a.render_note(note).await.unwrap();
    let rb = b.render_note(note).await.unwrap();
    assert_eq!(ra, rb, "replicas converge to identical state");
    assert!(
        ra.contains("status:: doing"),
        "A's property must survive on the merged replica, got: {ra:?}"
    );
    assert!(
        ra.contains("priority:: 3"),
        "B's property must survive on the merged replica, got: {ra:?}"
    );
}

#[tokio::test]
async fn concurrent_same_key_scalar_set_is_deterministic_lww() {
    // P1.9b: a same-key concurrent scalar set is LWW-by-HLC (the v1
    // product decision — a scalar has no union semantics). The invariant
    // we DO require is that both replicas pick the IDENTICAL winner after
    // a bidirectional exchange (deterministic, no oscillation), and the
    // losing key isn't dropped wholesale.
    let a = LoroEngine::new(
        DeviceId::from_bytes([0xc1; 16]),
        Arc::new(Hlc::new(DeviceId::from_bytes([0xc1; 16]))),
    );
    let b = LoroEngine::new(
        DeviceId::from_bytes([0xd2; 16]),
        Arc::new(Hlc::new(DeviceId::from_bytes([0xd2; 16]))),
    );
    let note = [0x99; 16];
    let block: [u8; 16] = [0x07; 16];

    a.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("shared".into()),
        title: "Shared".into(),
        content: "- base <!-- bid:07070707-0707-0707-0707-070707070707 -->\n".into(),
        created_at_millis: 1,
    })
    .await
    .unwrap();
    let bootstrap = a.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &bootstrap).await.unwrap();

    // Same key `status`, conflicting concurrent values.
    a.record_local(OpPayload::BlockPropertySet {
        note_id: note,
        block_id: block,
        key: "status".into(),
        value: PropOp::SetScalar(crate::PropScalar::Text("doing".into())),
    })
    .await
    .unwrap();
    b.record_local(OpPayload::BlockPropertySet {
        note_id: note,
        block_id: block,
        key: "status".into(),
        value: PropOp::SetScalar(crate::PropScalar::Text("done".into())),
    })
    .await
    .unwrap();

    let b_vv = b.doc_version(note).await;
    let a_upd = a.export_doc_update(note, b_vv.as_deref()).await.unwrap();
    b.import_doc_update(note, &a_upd).await.unwrap();
    let a_vv = a.doc_version(note).await;
    let b_upd = b.export_doc_update(note, a_vv.as_deref()).await.unwrap();
    a.import_doc_update(note, &b_upd).await.unwrap();

    let ra = a.render_note(note).await.unwrap();
    let rb = b.render_note(note).await.unwrap();
    assert_eq!(
        ra, rb,
        "same-key scalar LWW converges to one winner on both"
    );
    assert!(
        ra.contains("status:: doing") || ra.contains("status:: done"),
        "exactly one of the conflicting values must win, got: {ra:?}"
    );

    // Re-exchange must be a stable no-op (no oscillation between winners).
    let b_vv2 = b.doc_version(note).await;
    if let Some(u) = a.export_doc_update(note, b_vv2.as_deref()).await {
        if !u.is_empty() {
            b.import_doc_update(note, &u).await.unwrap();
        }
    }
    assert_eq!(
        a.render_note(note).await.unwrap(),
        ra,
        "stable after re-exchange"
    );
    assert_eq!(
        b.render_note(note).await.unwrap(),
        rb,
        "stable after re-exchange"
    );
}

#[tokio::test]
async fn note_upsert_does_not_clobber_concurrent_block_property() {
    // P1.8: prop ops are the SOLE writers of `props`. After a property
    // migrates into a typed `props` container (prose-only `text_seq`), an
    // OLD-PEER full-content NoteUpsert re-injects the property as an
    // in-text `key:: value` continuation line. `parse_note` folds that
    // line back into `FlatBlock.text` (and leaves `FlatBlock.properties`
    // empty), so the incoming block's text is `"buy milk\nstatus:: doing"`
    // while the live tree's block text is the prose-only `"buy milk"`. The
    // OLD `tree_matches_blocks` compares raw text → MISMATCH → reseed →
    // the typed `status` container is destroyed and the property collapses
    // back into prose text (re-embedded, no longer a mergeable container).
    //
    // The fix: strip recognized `key:: value` lines from the incoming body
    // before comparing prose, AND compare materialized props per block.
    // Stripped prose (`buy milk`) matches the tree's prose AND the body's
    // lifted props (`status:: doing`) match the container's materialized
    // props → NOT drifted → no reseed → the container survives.
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    let note = [0x71; 16];
    let block: [u8; 16] = [0x07; 16];

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("n".into()),
            title: "N".into(),
            content: "- buy milk <!-- bid:07070707-0707-0707-0707-070707070707 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // A property set lands on the block as a typed container (the SOLE
    // writer of `props`); the block's prose stays prose-only.
    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: "status".into(),
            value: PropOp::SetScalar(crate::PropScalar::Text("doing".into())),
        })
        .await
        .unwrap();
    assert!(
        engine
            .render_note(note)
            .await
            .unwrap()
            .contains("status:: doing"),
        "property is set before the re-save"
    );

    // An OLD-PEER full-content NoteUpsert that carries the property as an
    // IN-TEXT continuation line (the un-migrated shape). It must be
    // recognized as the same block (stripped prose + props both match) and
    // must NOT reseed — leaving the typed container intact.
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("n".into()),
            title: "N".into(),
            content: "- buy milk <!-- bid:07070707-0707-0707-0707-070707070707 -->\n  status:: doing\n".into(),
            created_at_millis: 2,
        })
        .await
        .unwrap();

    let rendered = engine.render_note(note).await.unwrap();
    assert!(
        rendered.contains("status:: doing"),
        "property must survive an old-peer in-text NoteUpsert, got: {rendered:?}"
    );
    assert!(
        rendered.contains("buy milk"),
        "prose must survive too, got: {rendered:?}"
    );
    // The property must remain a TYPED container — the block's prose-only
    // `text_seq` must NOT have the property re-embedded into it (a reseed
    // would fold `status:: doing` back into block text).
    {
        let doc = engine.doc_for_note_mut(note).await;
        let tree = doc.get_tree("blocks");
        let node = find_node_by_block_id(&tree, &hex::encode(block)).unwrap();
        assert_eq!(
            read_block_text(&tree, node).as_deref(),
            Some("buy milk"),
            "block text stays prose-only — the property is NOT folded back into text"
        );
        let meta = tree.get_meta(node).unwrap();
        let (props, _) = prop_containers::read_node_prop_containers(&meta).unwrap();
        assert_eq!(
            prop_containers::prop_get_scalar(&props, "status"),
            Some(crate::PropScalar::Text("doing".into())),
            "the typed container survives"
        );
    }
}

#[tokio::test]
async fn note_upsert_does_not_clobber_concurrent_block_property_on_server() {
    // P1.8 — the SERVER variant of the clobber test. This is the actual
    // TDD proof that the `tree_matches_blocks` prose-strip is load-bearing.
    //
    // On a DEVICE engine the reseed gate (`tree_is_empty ||
    // materialize_dir.is_some()`) skips the reseed entirely, so the typed
    // container survives an old-peer in-text NoteUpsert no matter what
    // `tree_matches_blocks` returns — the device variant proves nothing
    // about the prose-strip. The reseed only fires on the AUTHORITATIVE
    // writer (materialize_dir set). Here, if the prose-strip is reverted to
    // the old raw `read_block_text(...) == block.text` compare, the live
    // tree's prose-only `"buy milk"` won't equal the body's
    // `"buy milk\nstatus:: doing"` → drift → reseed → the new block is
    // seeded from the body with the property folded BACK into block text,
    // so `read_block_text` would return `Some("buy milk\nstatus:: doing")`
    // and the `block text stays prose-only` assertion below FAILS. The
    // prose-strip makes the old-peer body NOT count as drift, so the
    // server never reseeds and the prose stays prose-only.
    let tmp = tempfile::tempdir().unwrap();
    let dev = test_device();
    let engine = LoroEngine::with_dirs(
        dev,
        Arc::new(Hlc::new(dev)),
        tmp.path().join("loro"),
        Some(tmp.path().join("notes")),
    )
    .await
    .unwrap();
    let note = blake3_note_id("server-clobber");
    let block: [u8; 16] = [0x07; 16];

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("n".into()),
            title: "N".into(),
            content: "- buy milk <!-- bid:07070707-0707-0707-0707-070707070707 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // A property set lands on the block as a typed container; prose stays
    // prose-only.
    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: "status".into(),
            value: PropOp::SetScalar(crate::PropScalar::Text("doing".into())),
        })
        .await
        .unwrap();
    assert!(
        engine
            .render_note(note)
            .await
            .unwrap()
            .contains("status:: doing"),
        "property is set before the re-save"
    );

    // An OLD-PEER full-content NoteUpsert carrying the property as an
    // IN-TEXT continuation line. On the SERVER the reseed gate is open, so
    // the ONLY thing keeping this from reseeding (and folding the property
    // back into block text) is the prose-strip in `tree_matches_blocks`.
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("n".into()),
            title: "N".into(),
            content: "- buy milk <!-- bid:07070707-0707-0707-0707-070707070707 -->\n  status:: doing\n".into(),
            created_at_millis: 2,
        })
        .await
        .unwrap();

    let rendered = engine.render_note(note).await.unwrap();
    assert!(
        rendered.contains("status:: doing"),
        "property must survive an old-peer in-text NoteUpsert on the server, got: {rendered:?}"
    );
    assert!(
        rendered.contains("buy milk"),
        "prose must survive too, got: {rendered:?}"
    );
    // The load-bearing assertion: the block's prose-only `text_seq` must
    // NOT have the property re-embedded. A reseed (which the prose-strip
    // prevents) would seed the block from the body's
    // `"buy milk\nstatus:: doing"` and this would be `Some(...status...)`.
    let doc = engine.doc_for_note_mut(note).await;
    let tree = doc.get_tree("blocks");
    let node = find_node_by_block_id(&tree, &hex::encode(block)).unwrap();
    assert_eq!(
        read_block_text(&tree, node).as_deref(),
        Some("buy milk"),
        "block text stays prose-only — the server did NOT reseed the old-peer in-text body"
    );
    let meta = tree.get_meta(node).unwrap();
    let (props, _) = prop_containers::read_node_prop_containers(&meta).unwrap();
    assert_eq!(
        prop_containers::prop_get_scalar(&props, "status"),
        Some(crate::PropScalar::Text("doing".into())),
        "the typed container survives on the server path"
    );
}

#[tokio::test]
async fn note_upsert_drift_reseed_preserves_props() {
    // P1.8: when a reseed is GENUINELY unavoidable (structural drift the
    // block-granular diff didn't capture — here a brand-new block in the
    // body), the surviving block_id's materialized props must be
    // snapshotted before `clear_block_tree` and replayed after the
    // reseed. Reseed stays SERVER-ONLY (gate on materialize_dir), so this
    // runs on an authoritative engine. Without the snapshot/replay, the
    // reseed drops the property (clear_block_tree tombstones the node and
    // the body never carried the prop).
    let tmp = tempfile::tempdir().unwrap();
    let dev = test_device();
    let engine = LoroEngine::with_dirs(
        dev,
        Arc::new(Hlc::new(dev)),
        tmp.path().join("loro"),
        Some(tmp.path().join("notes")),
    )
    .await
    .unwrap();
    let note = blake3_note_id("drift");
    let block_a: [u8; 16] = [0x07; 16];
    let body_one = "- one <!-- bid:07070707-0707-0707-0707-070707070707 -->\n";

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("drift".into()),
            title: "Drift".into(),
            content: body_one.into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block_a,
            key: "status".into(),
            value: PropOp::SetScalar(crate::PropScalar::Text("doing".into())),
        })
        .await
        .unwrap();
    assert!(engine
        .render_note(note)
        .await
        .unwrap()
        .contains("status:: doing"));

    // A NoteUpsert whose body has the SAME first block PLUS a brand-new
    // second block — genuine drift forcing a reseed.
    let body_two = "- one <!-- bid:07070707-0707-0707-0707-070707070707 -->\n\
                    - two <!-- bid:08080808-0808-0808-0808-080808080808 -->\n";
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("drift".into()),
            title: "Drift".into(),
            content: body_two.into(),
            created_at_millis: 2,
        })
        .await
        .unwrap();

    let rendered = engine.render_note(note).await.unwrap();
    assert!(
        rendered.contains("two"),
        "the drift body's new block must land, got: {rendered:?}"
    );
    assert!(
        rendered.contains("status:: doing"),
        "the surviving block's property must be replayed across the reseed, got: {rendered:?}"
    );
}

#[tokio::test]
async fn note_upsert_never_destructively_reseeds() {
    // P1.8 regression, generalized 2026-06-10: a destructive reseed
    // re-mints the block tree (fresh node ids), minting rival container
    // ids that overwrite instead of merge across peers. Post-cutover
    // EVERY engine is an authoritative writer, so the old "server-only"
    // materialize_dir gate was vacuous — NoteUpsert now reconciles
    // non-destructively on every engine: a drifting full-content
    // NoteUpsert must NOT remove blocks absent from its body; they
    // converge via explicit block ops / the twin heal instead.
    let dev = test_device();
    let device_engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    let note = [0x73; 16];
    let body_one = "- one <!-- bid:07070707-0707-0707-0707-070707070707 -->\n";
    device_engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("n".into()),
            title: "N".into(),
            content: body_one.into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // Drift the shadow tree out of band: append a stale block.
    {
        let doc = device_engine.doc_for_note_mut(note).await;
        let tree = doc.get_tree("blocks");
        let n = tree.create(TreeParentId::Root).unwrap();
        let m = tree.get_meta(n).unwrap();
        m.insert("block_id", "33333333-3333-3333-3333-333333333333")
            .unwrap();
        write_block_text(&m, "STALE").unwrap();
        m.insert("indent_level", 0i64).unwrap();
        doc.commit();
    }
    assert!(device_engine
        .render_note(note)
        .await
        .unwrap()
        .contains("STALE"));

    // A drifting full-content NoteUpsert on the NON-authoritative engine
    // must NOT reseed (which would re-mint rival container ids); the stale
    // block stays until block ops / the twin heal converge it.
    device_engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("n".into()),
            title: "N".into(),
            content: body_one.into(),
            created_at_millis: 2,
        })
        .await
        .unwrap();
    assert!(
        device_engine
            .render_note(note)
            .await
            .unwrap()
            .contains("STALE"),
        "a device (non-authoritative) engine must NOT reseed on drift"
    );
}

#[tokio::test]
async fn note_upsert_does_not_delete_absent_blocks_on_authoritative_engine() {
    // Data-loss vector #2 at the ENGINE level (2026-06-10): on an
    // AUTHORITATIVE engine (materialize_dir set — post-cutover that is
    // every engine: iOS, desktop, server), a stale full-content
    // NoteUpsert whose body LACKS a block a peer added must NOT delete
    // it. The old clear+reseed did exactly that.
    let tmp = tempfile::tempdir().unwrap();
    let dev = test_device();
    let engine = LoroEngine::with_dirs(
        dev,
        Arc::new(Hlc::new(dev)),
        tmp.path().join("loro"),
        Some(tmp.path().join("notes")),
    )
    .await
    .unwrap();
    let note = blake3_note_id("anticlobber");
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("anticlobber".into()),
            title: "A".into(),
            content: "- mine <!-- bid:07070707-0707-0707-0707-070707070707 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    // A peer's block lands via an explicit BlockUpsert.
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: [0x08; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "00000001".into(),
            indent_level: 0,
            text: "peer block".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    // A STALE whole-content upsert (authored before the peer's block).
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("anticlobber".into()),
            title: "A".into(),
            content: "- mine <!-- bid:07070707-0707-0707-0707-070707070707 -->\n".into(),
            created_at_millis: 2,
        })
        .await
        .unwrap();
    let rendered = engine.render_note(note).await.unwrap();
    assert!(
        rendered.contains("peer block"),
        "a stale NoteUpsert must not delete blocks absent from its body: {rendered:?}"
    );
    assert!(rendered.contains("mine"), "{rendered:?}");
}

#[tokio::test]
async fn block_delete_tombstones_every_same_bid_twin() {
    // 2026-06-10 (the iOS delete-revert product bug, twin half): docs in
    // the wild can carry same-bid TWINS (disjoint-lineage residue) that
    // the renderer dedups via `dedup_twins_by_block_id` — the user sees
    // ONE block. A BlockDelete that tombstones only the first matching
    // node leaves the survivor rendering, so the delete silently
    // reverts on the next materialize. Author intent is bid-level.
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    let note = [0x74; 16];
    let bid: [u8; 16] = [0x07; 16];
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("twins".into()),
            title: "T".into(),
            content: "- keep <!-- bid:09090909-0909-0909-0909-090909090909 -->\n- doomed <!-- bid:07070707-0707-0707-0707-070707070707 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    // Inject a rival live node with the SAME bid (what a disjoint
    // lineage union leaves behind when the dedup can't run).
    {
        let doc = engine.doc_for_note_mut(note).await;
        let tree = doc.get_tree("blocks");
        let n = tree.create(TreeParentId::Root).unwrap();
        let m = tree.get_meta(n).unwrap();
        m.insert("block_id", hex_id(&bid).as_str()).unwrap();
        write_block_text(&m, "doomed twin").unwrap();
        m.insert("indent_level", 0i64).unwrap();
        doc.commit();
    }
    engine
        .record_local(OpPayload::BlockDelete { block_id: bid })
        .await
        .unwrap();
    let rendered = engine.render_note(note).await.unwrap();
    assert!(
        !rendered.contains("doomed"),
        "BlockDelete must tombstone EVERY live node carrying the bid: {rendered:?}"
    );
    assert!(rendered.contains("keep"), "{rendered:?}");
}

#[tokio::test]
async fn block_op_resolves_seeded_block_via_index() {
    // A block created via NoteUpsert seed (not BlockUpsert) must be
    // resolvable by a later block-only op through the block_index.
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [0x66; 16];
    // Seed two stamped blocks via NoteUpsert.
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("n".into()),
            title: "N".into(),
            content: "- keep <!-- bid:10101010-1010-1010-1010-101010101010 -->\n- drop <!-- bid:20202020-2020-2020-2020-202020202020 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    // BlockDelete the second block by id — only resolvable via the
    // block_index (the op carries no note_id).
    let drop_id = [0x20; 16];
    engine
        .record_local(OpPayload::BlockDelete { block_id: drop_id })
        .await
        .unwrap();
    let rendered = engine.render_note(note_id).await.unwrap();
    assert_eq!(
        rendered, "- keep <!-- bid:10101010-1010-1010-1010-101010101010 -->\n",
        "seeded block resolved + deleted via block_index"
    );
}

#[tokio::test]
async fn block_delete_reparents_direct_children_to_indent_0() {
    // Review finding [1]/[9]: deleting a parent must flatten its
    // DIRECT children to indent 0 (matching SqliteEngine), while
    // grandchildren keep their indent.
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [0x44; 16];
    let a = [0xa1; 16];
    let b = [0xb1; 16]; // direct child of a (indent 1)
    let c = [0xc1; 16]; // child of b (indent 2, grandchild of a)

    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: a,
            note_id,
            parent_block_id: None,
            order_key: "a".into(),
            indent_level: 0,
            text: "A".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: b,
            note_id,
            parent_block_id: Some(a),
            order_key: "b".into(),
            indent_level: 1,
            text: "B".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: c,
            note_id,
            parent_block_id: Some(b),
            order_key: "c".into(),
            indent_level: 2,
            text: "C".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    // Delete A (the parent with a direct child B).
    engine
        .record_local(OpPayload::BlockDelete { block_id: a })
        .await
        .unwrap();

    let rendered = engine.render_note(note_id).await.unwrap();
    // B (direct child) flattened to indent 0; C (grandchild) keeps
    // indent 2 — exactly SqliteEngine's apply_block_delete behavior.
    assert_eq!(
        rendered,
        "- B <!-- bid:b1b1b1b1-b1b1-b1b1-b1b1-b1b1b1b1b1b1 -->\n    - C <!-- bid:c1c1c1c1-c1c1-c1c1-c1c1-c1c1c1c1c1c1 -->\n"
    );
}

#[tokio::test]
async fn index_link_with_comma_is_one_edge() {
    // Review finding [7]: a wiki-link target containing a comma must
    // remain a single link, not fragment into two.
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: [8u8; 16],
            display_alias: Some("c".into()),
            title: "C".into(),
            content: "- see [[Smith, John]] and [[plain]]\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    let entries = engine.index_entries().await;
    assert_eq!(entries.len(), 1);
    let mut links = entries[0].links.clone();
    links.sort();
    assert_eq!(links, vec!["Smith, John".to_string(), "plain".to_string()]);
}

#[tokio::test]
async fn index_rebuild_prunes_ghost_entries() {
    // Review finding [6]: rebuild must drop index entries that have
    // no backing per-note doc, not leave them as phantoms.
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    // One real note with a doc.
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: [1u8; 16],
            display_alias: Some("real".into()),
            title: "Real".into(),
            content: "- x\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    // Inject a ghost index entry with no backing doc.
    {
        let notes = engine.inner.index.get_map("notes");
        let ghost = notes
            .insert_container(&hex_id(&[0x99u8; 16]), loro::LoroMap::new())
            .unwrap();
        ghost.insert("title", "Ghost").unwrap();
        ghost.insert("slug", "ghost").unwrap();
        engine.inner.index.commit();
    }
    assert_eq!(
        engine.index_entries().await.len(),
        2,
        "ghost present pre-rebuild"
    );

    engine.rebuild_index_from_docs().await;
    let entries = engine.index_entries().await;
    assert_eq!(entries.len(), 1, "ghost pruned");
    assert_eq!(entries[0].title, "Real");
}

#[tokio::test]
async fn index_self_heals_when_schema_stale() {
    // Simulate a stale on-disk index: write notes, then hand-corrupt
    // the persisted index's schema_version to 1 (pre-tags/links) and
    // strip the tags field, then reload — the boot rebuild should
    // restore tags/links from the self-describing per-note docs.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("loro");
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
        .await
        .unwrap();
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: [3u8; 16],
            display_alias: Some("n".into()),
            title: "N".into(),
            content: "---\ntitle: N\ntags: [alpha]\n---\n\n- see [[target]]\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    drop(engine);

    // Downgrade the persisted index schema marker to force a rebuild.
    let idx_path = dir.join("_index.bin");
    let idx = LoroDoc::new();
    idx.import(&tokio::fs::read(&idx_path).await.unwrap())
        .unwrap();
    idx.get_map("meta").insert("schema_version", 1i64).unwrap();
    idx.commit();
    tokio::fs::write(&idx_path, idx.export(ExportMode::Snapshot).unwrap())
        .await
        .unwrap();

    // Reload: boot rebuild should fire and restore tags/links.
    let hlc2 = Arc::new(Hlc::new(test_device()));
    let reloaded = LoroEngine::with_snapshot_dir(test_device(), hlc2, dir)
        .await
        .unwrap();
    let entries = reloaded.index_entries().await;
    assert_eq!(entries.len(), 1);
    assert!(
        entries[0].tags.contains(&"alpha".to_string()),
        "tags: {:?}",
        entries[0].tags
    );
    assert_eq!(entries[0].links, vec!["target".to_string()]);
}

#[tokio::test]
async fn index_rebuild_preserves_slug_when_doc_lacks_it() {
    // The live upgrade scenario: per-note docs written by an older
    // engine carry "content" but NOT slug/title on root meta, while
    // the prior index DOES have the slug. Rebuild must keep the slug
    // (from the prior index) rather than blanking it.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("loro");
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
        .await
        .unwrap();
    let note_id = [4u8; 16];
    // Build a per-note doc WITHOUT slug/title on root (simulate old
    // engine): only content. Then a prior index entry with the slug.
    {
        let doc = engine.doc_for_note_mut(note_id).await;
        doc.get_map("root")
            .insert("content", "---\ntitle: Kept\ntags: [z]\n---\n\n- body\n")
            .unwrap();
        doc.commit();
    }
    // Prior index entry (title+slug only, no tags) — like step 1.
    {
        let notes = engine.inner.index.get_map("notes");
        let entry = notes
            .insert_container(&hex_id(&note_id), loro::LoroMap::new())
            .unwrap();
        entry.insert("title", "Kept").unwrap();
        entry.insert("slug", "kept-slug").unwrap();
        engine.inner.index.commit();
    }

    engine.rebuild_index_from_docs().await;
    let entries = engine.index_entries().await;
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].slug, "kept-slug",
        "slug preserved from prior index"
    );
    assert_eq!(entries[0].title, "Kept");
    assert!(
        entries[0].tags.contains(&"z".to_string()),
        "tags derived: {:?}",
        entries[0].tags
    );
}

#[tokio::test]
async fn index_doc_captures_tags_and_links() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let content = "---\ntitle: T\ntags: [daily]\n---\n\ntags:: project\n- see [[other-note]] and #urgent stuff\n";

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: [7u8; 16],
            display_alias: Some("t".into()),
            title: "T".into(),
            content: content.into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    let entries = engine.index_entries().await;
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    // tags from frontmatter (daily), page property (project), inline (#urgent)
    assert!(
        e.tags.contains(&"daily".to_string()),
        "frontmatter tag: {:?}",
        e.tags
    );
    assert!(
        e.tags.contains(&"project".to_string()),
        "page-prop tag: {:?}",
        e.tags
    );
    assert!(
        e.tags.contains(&"urgent".to_string()),
        "inline tag: {:?}",
        e.tags
    );
    // link target
    assert_eq!(e.links, vec!["other-note".to_string()]);
}

#[tokio::test]
async fn index_excludes_fenced_tags_and_links_before_and_after_reload() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("loro");
    let device = test_device();
    let engine = LoroEngine::with_snapshot_dir(device, Arc::new(Hlc::new(device)), dir.clone())
        .await
        .unwrap();
    let content = concat!(
        "---\ntitle: Fence Index\ntags: [front]\n---\n\n",
        "- outside #real [[outside]] <!-- bid:55555555-5555-5555-5555-555555555555 -->\n",
        "- Parent <!-- bid:66666666-6666-6666-6666-666666666666 -->\n",
        "  - Child <!-- bid:77777777-7777-7777-7777-777777777777 -->\n",
        "    ```text\n    #nested-fake [[nested-secret]]\n    ```\n",
        "  - ```text <!-- bid:88888888-8888-8888-8888-888888888888 -->\n",
        "    #same-line-fake [[same-line-secret]]\n    ```\n",
    );
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: [8u8; 16],
            display_alias: Some("fence-index".into()),
            title: "Fence Index".into(),
            content: content.into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    let assert_metadata = |entry: &crate::engine::IndexEntry| {
        assert!(entry.tags.contains(&"front".to_string()));
        assert!(entry.tags.contains(&"real".to_string()));
        assert!(!entry.tags.contains(&"fake".to_string()));
        assert_eq!(entry.links, vec!["outside".to_string()]);
    };
    let entries = engine.index_entries().await;
    assert_metadata(&entries[0]);

    // Simulate the persisted v3 projection, where fence payload still leaked
    // into derived metadata. Reopen must notice schema v4 and rebuild from the
    // note doc rather than trusting these stale values.
    {
        let key = hex_id(&[8u8; 16]);
        let notes = engine.inner.index.get_map("notes");
        let entry = match notes.get(&key).unwrap() {
            loro::ValueOrContainer::Container(loro::Container::Map(map)) => map,
            other => panic!("unexpected index entry: {other:?}"),
        };
        entry.insert("tags", "front\nreal\nnested-fake").unwrap();
        entry.insert("links", "outside\nnested-secret").unwrap();
        engine
            .inner
            .index
            .get_map("meta")
            .insert("schema_version", 3i64)
            .unwrap();
        engine.inner.index.commit();
        tokio::fs::write(
            dir.join("_index.bin"),
            engine.inner.index.export(ExportMode::Snapshot).unwrap(),
        )
        .await
        .unwrap();
    }
    drop(engine);

    let reloaded = LoroEngine::with_snapshot_dir(device, Arc::new(Hlc::new(device)), dir)
        .await
        .unwrap();
    let entries = reloaded.index_entries().await;
    assert_metadata(&entries[0]);
}

#[tokio::test]
async fn index_doc_survives_reload() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("loro");
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
        .await
        .unwrap();
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: [9u8; 16],
            display_alias: Some("kept".into()),
            title: "Kept".into(),
            content: "- x\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    drop(engine);

    let hlc2 = Arc::new(Hlc::new(test_device()));
    let reloaded = LoroEngine::with_snapshot_dir(test_device(), hlc2, dir)
        .await
        .unwrap();
    let entries = reloaded.index_entries().await;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "Kept");
    assert_eq!(entries[0].slug, "kept");
}

#[tokio::test]
async fn note_upsert_overwrites_page_properties() {
    // A second NoteUpsert with different props replaces the first
    // wholesale (no stale leftovers).
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [0x5b; 16];

    for content in [
        "query:: kind:page\nsort:: modified desc\n",
        "query:: kind:block\n",
    ] {
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("q".into()),
                title: "Q".into(),
                content: content.into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
    }

    let rendered = engine.render_note(note_id).await.unwrap();
    assert_eq!(rendered, "query:: kind:block\n", "wholesale overwrite");
}

#[tokio::test]
async fn note_delete_hides_doc_from_live_projection() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [60u8; 16];

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("doomed".into()),
            title: "Doomed".into(),
            content: "---\ntitle: Doomed\n---\n- bye\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    assert_eq!(engine.note_count().await, 1);

    engine
        .record_local(OpPayload::NoteDelete {
            note_id,
            display_alias: Some("doomed".into()),
        })
        .await
        .unwrap();
    assert_eq!(engine.note_count().await, 0);
    assert!(engine.render_note(note_id).await.is_none());
}

#[tokio::test]
async fn block_upsert_with_same_block_id_updates_text() {
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::new(test_device(), hlc);
    let note_id = [2u8; 16];
    let block = [20u8; 16];

    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: block,
            note_id,
            parent_block_id: None,
            order_key: "a0".into(),
            indent_level: 0,
            text: "first".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: block,
            note_id,
            parent_block_id: None,
            order_key: "a0".into(),
            indent_level: 0,
            text: "second".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    let rendered = engine.render_note(note_id).await.unwrap();
    assert_eq!(
        rendered,
        "- second <!-- bid:14141414-1414-1414-1414-141414141414 -->\n"
    );
}

// ── Authoritative-writer cutover ─────────────────────────────────

#[tokio::test]
async fn authoritative_engine_materializes_and_deletes_md_files() {
    let tmp = tempfile::tempdir().unwrap();
    let snap = tmp.path().join("loro");
    let notes = tmp.path().join("notes");
    let dev = test_device();
    let engine = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, Some(notes.clone()))
        .await
        .unwrap();
    let note_id = blake3_note_id("daily");
    let content =
        "---\ntitle: Daily\n---\n\n- one <!-- bid:30303030-3030-3030-3030-303030303030 -->\n";
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("daily".into()),
            title: "Daily".into(),
            content: content.into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    let path = notes.join("daily.md");
    let on_disk = tokio::fs::read_to_string(&path).await.unwrap();
    assert_eq!(on_disk, content, "NoteUpsert materializes the full file");

    // A block append rewrites the file with both bullets.
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: [0x31; 16],
            note_id,
            parent_block_id: None,
            order_key: "b".into(),
            indent_level: 0,
            text: "two".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    let on_disk = tokio::fs::read_to_string(&path).await.unwrap();
    assert!(
        on_disk.contains("- one ") && on_disk.contains("- two "),
        "block append materialized: {on_disk:?}"
    );
    assert!(
        on_disk.starts_with("---\ntitle: Daily\n---\n"),
        "frontmatter preserved"
    );

    // NoteDelete removes the file.
    engine
        .record_local(OpPayload::NoteDelete {
            note_id,
            display_alias: Some("daily".into()),
        })
        .await
        .unwrap();
    assert!(!path.exists(), "NoteDelete removes the materialized file");
}

#[tokio::test]
async fn authoritative_materialization_hides_reservation_until_creator_splices() {
    let tmp = tempfile::tempdir().unwrap();
    let dev = test_device();
    let notes = tmp.path().join("notes");
    let engine = LoroEngine::with_dirs(
        dev,
        Arc::new(Hlc::new(dev)),
        tmp.path().join("loro"),
        Some(notes.clone()),
    )
    .await
    .unwrap();
    let note_id = blake3_note_id("local-reservation-materialization");
    let kept_id = uuid::Uuid::from_bytes([0x72; 16]);
    let empty_id = [0x73; 16];
    let empty_uuid = uuid::Uuid::from_bytes(empty_id);
    let content = format!("- kept <!-- bid:{kept_id} -->\n");
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("local-reservation".into()),
            title: "Local reservation".into(),
            content,
            created_at_millis: 1,
        })
        .await
        .unwrap();
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: empty_id,
            note_id,
            parent_block_id: None,
            order_key: "b".into(),
            indent_level: 0,
            text: String::new(),
            after_block_id: None,
        })
        .await
        .unwrap();

    let path = notes.join("local-reservation.md");
    let hidden = tokio::fs::read_to_string(&path).await.unwrap();
    assert_eq!(hidden, format!("- kept <!-- bid:{kept_id} -->\n"));
    {
        let docs = engine.inner.docs.read().await;
        let tree = docs.get(&note_id).unwrap().get_tree("blocks");
        assert!(
            find_node_by_block_id(&tree, &hex_id(&empty_id)).is_some(),
            "hidden reservation remains in the creator's Loro tree"
        );
    }

    assert_eq!(
        engine
            .splice_block_text(note_id, empty_id, 0, 0, "creator text")
            .await
            .unwrap(),
        1
    );
    let visible = tokio::fs::read_to_string(path).await.unwrap();
    assert!(visible.contains("creator text"));
    assert!(visible.contains(&empty_uuid.to_string()));
}

#[tokio::test]
async fn note_delete_without_alias_still_removes_file() {
    // Review finding: a NoteDelete whose op carries no display_alias
    // (op.rs: "None means the producer did not know the slug") must
    // still remove the materialized file — the slug is resolved from
    // the resident doc/index BEFORE the inner apply drops them.
    let tmp = tempfile::tempdir().unwrap();
    let dev = test_device();
    let engine = LoroEngine::with_dirs(
        dev,
        Arc::new(Hlc::new(dev)),
        tmp.path().join("loro"),
        Some(tmp.path().join("notes")),
    )
    .await
    .unwrap();
    let note_id = blake3_note_id("orphan");
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("orphan".into()),
            title: "Orphan".into(),
            content: "- x <!-- bid:33333333-3333-3333-3333-333333333333 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    let path = tmp.path().join("notes").join("orphan.md");
    assert!(path.exists(), "materialized");
    engine
        .record_local(OpPayload::NoteDelete {
            note_id,
            display_alias: None,
        })
        .await
        .unwrap();
    assert!(
        !path.exists(),
        "NoteDelete with display_alias=None must still remove the file"
    );
}

#[tokio::test]
async fn non_authoritative_engine_writes_no_md_files() {
    // Without materialize_dir, the engine must not touch the notes dir.
    let tmp = tempfile::tempdir().unwrap();
    let snap = tmp.path().join("loro");
    let dev = test_device();
    let engine = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, None)
        .await
        .unwrap();
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: blake3_note_id("x"),
            display_alias: Some("x".into()),
            title: "X".into(),
            content: "- hi <!-- bid:32323232-3232-3232-3232-323232323232 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    // Only the snapshot dir should exist; no notes/ dir was created.
    assert!(
        !tmp.path().join("notes").exists(),
        "no .md materialization when non-authoritative"
    );
}

#[tokio::test]
async fn reseed_from_disk_tracks_and_canonicalizes() {
    let tmp = tempfile::tempdir().unwrap();
    let snap = tmp.path().join("loro");
    let notes = tmp.path().join("notes");
    tokio::fs::create_dir_all(&notes).await.unwrap();
    // A canonical note and a non-canonical one (bullet missing its bid
    // — reseed will stamp + re-render canonically).
    tokio::fs::write(
        notes.join("alpha.md"),
        "---\ntitle: Alpha\n---\n\n- a1 <!-- bid:40404040-4040-4040-4040-404040404040 -->\n",
    )
    .await
    .unwrap();
    tokio::fs::write(notes.join("beta.md"), "- just text\n")
        .await
        .unwrap();
    let dev = test_device();
    let engine = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, Some(notes.clone()))
        .await
        .unwrap();
    let n = engine.reseed_from_disk(&notes).await.unwrap();
    assert_eq!(n, 2, "both .md files reseeded");
    // Both notes are now tracked + render their content.
    let alpha = blake3_note_id("alpha");
    let rendered = engine.render_note(alpha).await.unwrap();
    assert!(rendered.contains("a1"), "alpha block present: {rendered:?}");
    // beta got a canonical bid stamped on its bullet (was bare).
    let beta = blake3_note_id("beta");
    let rb = engine.render_note(beta).await.unwrap();
    assert!(
        rb.contains("- just text") && rb.contains("<!-- bid:"),
        "beta canonicalized: {rb:?}"
    );
}

#[tokio::test]
async fn canonical_lift_reseed_survives_reload_reimport_and_replica_delta() {
    let tmp = tempfile::tempdir().unwrap();
    let snap = tmp.path().join("loro-a");
    let notes = tmp.path().join("notes");
    tokio::fs::create_dir_all(&notes).await.unwrap();
    let source = "---\ntitle: Mixed\n---\n\n# Heading\n\nProse one\nprose two\n\n```query\nstatus:: done\n- payload, not a block\n```\n\n- Existing <!-- bid:22222222-2222-2222-2222-222222222222 -->\n";
    let path = notes.join("mixed.md");
    tokio::fs::write(&path, source).await.unwrap();

    let device_a = DeviceId::from_bytes([0xa7; 16]);
    let engine = LoroEngine::with_dirs(
        device_a,
        Arc::new(Hlc::new(device_a)),
        snap.clone(),
        Some(notes.clone()),
    )
    .await
    .unwrap();
    assert_eq!(engine.reseed_from_disk(&notes).await.unwrap(), 1);

    let note = blake3_note_id("mixed");
    let canonical = tokio::fs::read_to_string(&path).await.unwrap();
    assert_ne!(
        canonical, source,
        "explicit reseed canonicalizes raw regions"
    );
    assert!(tesela_core::note_tree::canonicalization_preserves_structure(source, &canonical));
    let parsed = tesela_core::note_tree::parse_note(&canonical);
    assert_eq!(
        parsed
            .blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>(),
        vec![
            "# Heading",
            "Prose one\nprose two",
            "```query\nstatus:: done\n- payload, not a block\n```",
            "Existing",
        ]
    );
    let ids = parsed
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<Vec<_>>();
    assert_eq!(
        engine.render_note_full(note).await.as_deref(),
        Some(canonical.as_str())
    );
    {
        let docs = engine.inner.docs.read().await;
        let root = docs.get(&note).unwrap().get_map("root");
        assert!(
            root.get("content").is_none(),
            "lifted body must not be duplicated on root content"
        );
    }

    assert_eq!(engine.reseed_from_disk(&notes).await.unwrap(), 1);
    let after_reimport = engine.render_note_full(note).await.unwrap();
    assert_eq!(after_reimport, canonical);
    assert_eq!(
        tesela_core::note_tree::parse_note(&after_reimport)
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        ids,
        "unchanged reimport keeps stable block ids"
    );
    drop(engine);

    let reloaded = LoroEngine::with_dirs(
        device_a,
        Arc::new(Hlc::new(device_a)),
        snap,
        Some(notes.clone()),
    )
    .await
    .unwrap();
    assert_eq!(
        reloaded.render_note_full(note).await.as_deref(),
        Some(canonical.as_str())
    );

    let device_b = DeviceId::from_bytes([0xb8; 16]);
    let peer = LoroEngine::new(device_b, Arc::new(Hlc::new(device_b)));
    let bootstrap = reloaded.export_doc_update(note, None).await.unwrap();
    peer.import_doc_update(note, &bootstrap).await.unwrap();
    assert_eq!(
        reloaded.render_note(note).await,
        peer.render_note(note).await
    );

    let prose_id = *ids[1].as_bytes();
    let a_version = reloaded.doc_version(note).await;
    peer.record_local(OpPayload::BlockUpsert {
        block_id: prose_id,
        note_id: note,
        parent_block_id: None,
        order_key: "b".into(),
        indent_level: 0,
        text: "Prose edited on peer".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    let delta = peer
        .export_doc_update(note, a_version.as_deref())
        .await
        .unwrap();
    reloaded.import_doc_update(note, &delta).await.unwrap();

    let a_rendered = reloaded.render_note(note).await.unwrap();
    let b_rendered = peer.render_note(note).await.unwrap();
    assert_eq!(a_rendered, b_rendered);
    assert!(a_rendered.contains("Prose edited on peer"));
    assert!(a_rendered.contains("# Heading"));
    assert!(a_rendered.contains("- payload, not a block"));
    assert_eq!(
        tokio::fs::read_to_string(&path).await.unwrap(),
        reloaded.render_note_full(note).await.unwrap(),
        "inbound peer delta materializes the converged canonical file"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn reseed_from_disk_canonicalizes_heading_and_hydrates_pure_bullets() {
    let tmp = tempfile::tempdir().unwrap();
    let snap = tmp.path().join("loro");
    let notes = tmp.path().join("notes");
    tokio::fs::create_dir_all(&notes).await.unwrap();

    let source = "# Heading\n\nA prose paragraph.\n\n- retained on disk\n  - nested bullet\n";
    let source_path = notes.join("heading.md");
    tokio::fs::write(&source_path, source).await.unwrap();
    tokio::fs::write(
        notes.join("bullets.md"),
        "- pure bullet\n  - nested bullet\n",
    )
    .await
    .unwrap();

    #[derive(Clone)]
    struct LogWriter(Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for LogWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let logs = Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer_logs = Arc::clone(&logs);
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_max_level(tracing::Level::WARN)
        .with_writer(move || LogWriter(Arc::clone(&writer_logs)))
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);

    let dev = test_device();
    let engine = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, Some(notes.clone()))
        .await
        .unwrap();
    let count = engine.reseed_from_disk(&notes).await.unwrap();

    drop(guard);
    let warnings = String::from_utf8(logs.lock().unwrap().clone()).unwrap();
    assert_eq!(count, 2, "both structurally preserving notes are reseeded");
    let canonical = tokio::fs::read_to_string(&source_path).await.unwrap();
    assert_ne!(
        canonical, source,
        "explicit reseed canonicalizes source syntax"
    );
    assert!(
        tesela_core::note_tree::canonicalization_preserves_structure(source, &canonical),
        "canonicalization must preserve the parsed structure"
    );
    assert!(
        engine
            .doc_version(blake3_note_id("heading"))
            .await
            .is_some(),
        "the lifted note gains a Loro doc"
    );
    assert!(
        engine
            .doc_version(blake3_note_id("bullets"))
            .await
            .is_some(),
        "the pure-bullet note hydrates normally"
    );
    assert!(
        !warnings.contains("reseed skip heading"),
        "a structurally preserving lift must not warn as skipped: {warnings:?}"
    );
}
