use super::*;

#[tokio::test]
async fn two_authoritative_engines_converge_through_wire_codec() {
    // The real relay-payload path minus HTTP: A produces relay
    // updates → encode_loro_relay_payload (the TLR2 v2 wire) → decode
    // → B.apply_relay_updates. Both materialize identical files and
    // converge with no flashing, exactly as two Macs over the relay.
    use crate::wire::{decode_loro_relay_payload, encode_loro_relay_payload, LoroDocUpdate};

    let tmp_a = tempfile::tempdir().unwrap();
    let tmp_b = tempfile::tempdir().unwrap();
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_b = DeviceId::from_bytes([0xb2; 16]);
    let a = LoroEngine::with_dirs(
        dev_a,
        Arc::new(Hlc::new(dev_a)),
        tmp_a.path().join("loro"),
        Some(tmp_a.path().join("notes")),
    )
    .await
    .unwrap();
    let b = LoroEngine::with_dirs(
        dev_b,
        Arc::new(Hlc::new(dev_b)),
        tmp_b.path().join("loro"),
        Some(tmp_b.path().join("notes")),
    )
    .await
    .unwrap();

    // Helper: ship A's produced updates to B through the wire codec,
    // then commit A's cursor (simulating a confirmed send).
    async fn ship(from: &LoroEngine, to: &LoroEngine) -> usize {
        let updates = from.produce_relay_updates().await;
        if updates.is_empty() {
            return 0;
        }
        let payload: Vec<LoroDocUpdate> = updates
            .iter()
            .map(|(doc, update_bytes, _vv)| LoroDocUpdate {
                doc: *doc,
                update_bytes: update_bytes.clone(),
            })
            .collect();
        let committed: Vec<([u8; 16], Vec<u8>)> =
            updates.into_iter().map(|(doc, _b, vv)| (doc, vv)).collect();
        let wire = encode_loro_relay_payload(&payload).unwrap();
        let decoded = decode_loro_relay_payload(&wire)
            .unwrap()
            .expect("v2 payload");
        let pairs: Vec<([u8; 16], Vec<u8>)> = decoded
            .into_iter()
            .map(|u| (u.doc, u.update_bytes))
            .collect();
        let n = to.apply_relay_updates(&pairs).await.applied_count();
        from.commit_broadcast_cursors(&committed).await;
        n
    }

    let note = blake3_note_id("shared");
    a.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("shared".into()),
        title: "Shared".into(),
        content: "- base <!-- bid:50505050-5050-5050-5050-505050505050 -->\n".into(),
        created_at_millis: 1,
    })
    .await
    .unwrap();
    // A → B bootstrap.
    assert!(ship(&a, &b).await >= 1, "B received the note");
    assert_eq!(
        a.render_note(note).await,
        b.render_note(note).await,
        "bootstrapped equal"
    );
    // Both have materialized the file.
    let fa = tmp_a.path().join("notes").join("shared.md");
    let fb = tmp_b.path().join("notes").join("shared.md");
    assert!(fa.exists() && fb.exists(), "both materialized shared.md");
    assert_eq!(
        tokio::fs::read_to_string(&fa).await.unwrap(),
        tokio::fs::read_to_string(&fb).await.unwrap(),
        "materialized files identical"
    );

    // Concurrent edits, exchanged both ways.
    a.record_local(OpPayload::BlockUpsert {
        block_id: [0x5a; 16],
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
        block_id: [0x5b; 16],
        note_id: note,
        parent_block_id: None,
        order_key: "b".into(),
        indent_level: 0,
        text: "from B".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    // Two ticks each direction to fully exchange.
    ship(&a, &b).await;
    ship(&b, &a).await;
    ship(&a, &b).await;
    ship(&b, &a).await;

    let ra = a.render_note(note).await.unwrap();
    let rb = b.render_note(note).await.unwrap();
    assert_eq!(ra, rb, "engines converge — no flashing");
    assert!(ra.contains("base") && ra.contains("from A") && ra.contains("from B"));
    assert_eq!(
        tokio::fs::read_to_string(&fa).await.unwrap(),
        tokio::fs::read_to_string(&fb).await.unwrap(),
        "materialized files converge"
    );

    // Steady state: another exchange ships nothing (bounded broadcast).
    assert_eq!(ship(&a, &b).await, 0, "no re-broadcast at steady state");
}

#[tokio::test]
async fn broadcast_cursors_persist_across_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let snap = tmp.path().join("loro");
    let notes = tmp.path().join("notes");
    let dev = test_device();
    let note = blake3_note_id("persist");
    {
        let engine = LoroEngine::with_dirs(
            dev,
            Arc::new(Hlc::new(dev)),
            snap.clone(),
            Some(notes.clone()),
        )
        .await
        .unwrap();
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("persist".into()),
                title: "P".into(),
                content: "- x <!-- bid:60606060-6060-6060-6060-606060606060 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let first = engine.produce_relay_updates().await;
        assert_eq!(first.len(), 1, "first produce emits the note");
        // Commit (confirmed send) advances + persists the cursor.
        let committed: Vec<([u8; 16], Vec<u8>)> =
            first.into_iter().map(|(d, _b, vv)| (d, vv)).collect();
        engine.commit_broadcast_cursors(&committed).await;
    }
    // Reopen: cursor was persisted, so produce emits nothing new.
    let engine = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, Some(notes))
        .await
        .unwrap();
    let again = engine.produce_relay_updates().await;
    assert!(
        again.is_empty(),
        "persisted cursor suppresses re-broadcast after restart"
    );
}

#[tokio::test]
async fn produce_re_emits_when_broadcast_cursor_is_undecodable() {
    // Regression (2026-06-25): a corrupt / incompatible persisted
    // broadcast cursor must NOT permanently strand a note's outbound.
    // Before the fix, export_doc_update returned None on a VersionVector
    // decode failure and produce_relay_updates silently SKIPPED the dirty
    // note (no `else` at the `if let Some(bytes)` push) — so the device
    // never re-broadcast it. On iOS this presented as: a today edit
    // records (splice applied=1) but tick_outbound sends 0 ops, no error,
    // forever → iOS edits never reach the desktop.
    let tmp = tempfile::tempdir().unwrap();
    let snap = tmp.path().join("loro");
    let notes = tmp.path().join("notes");
    let dev = test_device();
    let note = blake3_note_id("stuck");
    let engine = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, Some(notes))
        .await
        .unwrap();
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("stuck".into()),
            title: "S".into(),
            content: "- hi <!-- bid:70707070-7070-7070-7070-707070707070 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    // Corrupt / incompatible persisted cursor for this note (e.g. a
    // version-format change or a stale lineage), so the incremental
    // export from it cannot be produced.
    engine
        .inner
        .broadcast_cursor
        .write()
        .await
        .insert(note, vec![0xff, 0xff, 0xff, 0xff]);
    let out = engine.produce_relay_updates().await;
    assert_eq!(
        out.len(),
        1,
        "a dirty note whose broadcast cursor won't decode must still export \
         (full-snapshot fallback), never be silently skipped"
    );
}

#[tokio::test]
async fn produce_re_emits_snapshot_when_broadcast_cursor_is_ahead_of_current() {
    // Regression (2026-06-29): an authoritative import can rebase a note's
    // doc 'backward' (the convergence/bootstrap heal imports do this),
    // leaving the persisted broadcast cursor AT-OR-AHEAD of the doc's
    // current version. The cursor still DECODES (so the undecodable
    // snapshot fallback never fires) and is != current bytes (so produce's
    // dirty-skip never fires) — but `updates(since_vv)` is then an
    // EMPTY/no-op delta because since_vv already covers current. Before the
    // fix, export_doc_update shipped that empty delta (`.ok()` was
    // `Some(empty)`, so the snapshot `.or_else` never ran) and the note's
    // REAL current content never reached the relay. On iOS this presented
    // as: a today edit records (splice applied=1) but tick_outbound ships a
    // content-less frame, no error, forever → iOS edits never reach the
    // desktop.
    use loro::VersionVector;

    let tmp = tempfile::tempdir().unwrap();
    let snap = tmp.path().join("loro");
    let notes = tmp.path().join("notes");
    let dev = test_device();
    let note = blake3_note_id("ahead");
    let engine = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, Some(notes))
        .await
        .unwrap();
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("ahead".into()),
            title: "A".into(),
            content: "- convergence-canary <!-- bid:80808080-8080-8080-8080-808080808080 -->\n"
                .into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // Realistic path: produce + commit so a decodable cursor is persisted at
    // the doc's current version.
    let first = engine.produce_relay_updates().await;
    assert_eq!(first.len(), 1, "first produce emits the note");
    let committed: Vec<([u8; 16], Vec<u8>)> =
        first.into_iter().map(|(d, _b, vv)| (d, vv)).collect();
    engine.commit_broadcast_cursors(&committed).await;
    assert!(
        engine.produce_relay_updates().await.is_empty(),
        "committed cursor at current → nothing dirty"
    );

    // Now simulate a backward rebase: bump the persisted cursor PAST the
    // doc's current version (the same net state as an authoritative import
    // that rebased current backward). The cursor decodes fine and differs
    // from current bytes, so neither the undecodable fallback nor produce's
    // dirty-skip applies — yet `updates(cursor)` would be empty.
    let current_enc = engine.doc_version(note).await.unwrap();
    let mut ahead = VersionVector::decode(&current_enc).unwrap();
    let bumps: Vec<(u64, i32)> = ahead.iter().map(|(p, c)| (*p, *c)).collect();
    assert!(!bumps.is_empty(), "doc must have ops to bump past");
    for (peer, counter) in bumps {
        ahead.set_end(loro::ID::new(peer, counter + 8));
    }
    let ahead_enc = ahead.encode();
    assert_ne!(
        ahead_enc, current_enc,
        "crafted cursor must differ from current bytes (stay dirty)"
    );
    engine
        .inner
        .broadcast_cursor
        .write()
        .await
        .insert(note, ahead_enc);

    // The dirty note must export a delta that brings a receiver to CURRENT.
    let out = engine.produce_relay_updates().await;
    assert_eq!(
        out.len(),
        1,
        "a dirty note whose cursor is ahead-of-current must still export"
    );
    let (got_id, bytes, _vv) = &out[0];
    assert_eq!(*got_id, note);

    // The exported bytes must import cleanly into a FRESH engine and
    // reproduce the note's CURRENT content — i.e. a real snapshot, not an
    // empty no-op delta.
    let tmp2 = tempfile::tempdir().unwrap();
    let dev2 = DeviceId::from_bytes([2u8; 16]);
    let fresh = LoroEngine::with_dirs(
        dev2,
        Arc::new(Hlc::new(dev2)),
        tmp2.path().join("loro"),
        Some(tmp2.path().join("notes")),
    )
    .await
    .unwrap();
    fresh.import_doc_update(note, bytes).await.unwrap();
    let rendered = fresh
        .render_note(note)
        .await
        .expect("fresh engine should hold the note after import");
    assert!(
        rendered.contains("convergence-canary"),
        "ahead-cursor produce must ship a full snapshot reproducing CURRENT \
         content, not an empty no-op delta; got: {rendered:?}"
    );
}

#[tokio::test]
async fn produce_without_commit_re_emits_delta_on_failed_send() {
    // Review finding #1: produce_relay_updates must NOT advance the
    // cursor — only commit_broadcast_cursors does. So a failed relay
    // send (no commit) re-emits the same delta next tick instead of
    // losing it forever.
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
    let note = blake3_note_id("retry");
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("retry".into()),
            title: "R".into(),
            content: "- x <!-- bid:90909090-9090-9090-9090-909090909090 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    // First produce: one delta.
    let first = engine.produce_relay_updates().await;
    assert_eq!(first.len(), 1, "produce emits the note");
    // Simulate a FAILED send: do NOT commit. Next produce must still
    // emit the same delta (not lost).
    let retry = engine.produce_relay_updates().await;
    assert_eq!(
        retry.len(),
        1,
        "failed send re-emits the delta — not dropped"
    );
    assert_eq!(retry[0].0, note);
    // Now commit (confirmed send). Subsequent produce is empty.
    let committed: Vec<([u8; 16], Vec<u8>)> =
        retry.into_iter().map(|(d, _b, vv)| (d, vv)).collect();
    engine.commit_broadcast_cursors(&committed).await;
    assert!(
        engine.produce_relay_updates().await.is_empty(),
        "committed cursor suppresses re-broadcast"
    );
}

#[tokio::test]
async fn local_edits_carry_timestamps_but_the_builtin_views_seed_stays_ts0() {
    // tesela-c7s item 1, precisely scoped. Two invariants that MUST hold
    // together:
    //  (a) a REAL local authoring op carries a wall-clock timestamp (> 0),
    //      so a strand investigation can see when a note last changed;
    //  (b) the DETERMINISTIC `builtin_views_seed_update` stays ts == 0, so
    //      its bytes are byte-identical on every device (the fresh-device-
    //      clobber invariant — two independent seeds must author the SAME
    //      op ids, which a per-device wall-clock stamp would break).
    //
    // REVERT-DISCRIMINATING both ways: removing `set_record_timestamp(true)`
    // from `set_doc_peer` drops (a) to ts == 0; flipping the seed builder to
    // `set_record_timestamp(true)` raises (b) above 0 (and would also break
    // `views_seed_update_is_deterministic`).
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    let note = blake3_note_id("stamped");
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("stamped".into()),
            title: "S".into(),
            content: "- hi <!-- bid:e5e5e5e5-e5e5-e5e5-e5e5-e5e5e5e5e5e5 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    let snapshot = engine.export_doc_update(note, None).await.unwrap();
    let meta = LoroDoc::decode_import_blob_meta(&snapshot, false).unwrap();
    assert!(
        meta.end_timestamp > 0,
        "a real local edit must record a wall-clock change timestamp; got {}",
        meta.end_timestamp
    );

    let seed = builtin_views_seed_update().unwrap();
    let seed_meta = LoroDoc::decode_import_blob_meta(&seed, false).unwrap();
    assert_eq!(
        seed_meta.end_timestamp, 0,
        "the deterministic builtin-views seed MUST stay ts=0 (byte-identical \
         across devices); got {}",
        seed_meta.end_timestamp
    );
}

#[tokio::test]
async fn since_vv_delta_is_smaller_than_snapshot_and_converges() {
    // iOS #150 (block-granular-writes spec, Stage 4): the live WS frame
    // ships a DELTA relative to the last-pushed VV, not a full snapshot
    // every keystroke. This proves the two properties the iOS change
    // relies on: (1) `export_doc_update(note, Some(vv_before_edit))` after
    // a single edit is byte-SMALLER than the full snapshot, and (2) a peer
    // that already holds `vv_before_edit` converges after importing only
    // that delta. Together: the steady-state WS frame shrinks AND stays
    // loss-free.
    let author = LoroEngine::new(
        DeviceId::from_bytes([0xe1; 16]),
        Arc::new(Hlc::new(DeviceId::from_bytes([0xe1; 16]))),
    );
    let note = [0x91; 16];

    // Seed a multi-block base so the snapshot has real heft.
    author
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("shared".into()),
            title: "Shared".into(),
            content: "- alpha <!-- bid:01010101-0101-0101-0101-010101010101 -->\n\
                      - beta <!-- bid:02020202-0202-0202-0202-020202020202 -->\n\
                      - gamma <!-- bid:03030303-0303-0303-0303-030303030303 -->\n"
                .into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // A peer bootstraps from the full snapshot — this is the base the
    // delta will be relative to (mirrors iOS's `lastPushedVV[slug]`
    // tracking the VV as of the last push the peer received).
    let peer = LoroEngine::new(
        DeviceId::from_bytes([0xf2; 16]),
        Arc::new(Hlc::new(DeviceId::from_bytes([0xf2; 16]))),
    );
    let snapshot = author.export_doc_update(note, None).await.unwrap();
    peer.import_doc_update(note, &snapshot).await.unwrap();
    assert_eq!(
        author.render_note(note).await,
        peer.render_note(note).await,
        "peer bootstrapped to the same base"
    );

    // Capture the VV AS OF the last push (the value iOS records as
    // `lastPushedVV[slug]` after `recordAndPush`), then author one edit.
    let vv_before_edit = author.doc_version(note).await.expect("vv before edit");
    author
        .record_local(OpPayload::BlockUpsert {
            block_id: [0x02; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "b".into(),
            indent_level: 0,
            text: "beta EDITED".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    // The steady-state WS frame (delta) vs. the full snapshot iOS used to
    // ship every keystroke.
    let delta = author
        .export_doc_update(note, Some(&vv_before_edit))
        .await
        .expect("delta export");
    let full_snapshot = author
        .export_doc_update(note, None)
        .await
        .expect("snapshot");
    assert!(
        delta.len() < full_snapshot.len(),
        "since_vv delta ({} bytes) must be smaller than the full snapshot ({} bytes)",
        delta.len(),
        full_snapshot.len(),
    );

    // The peer holding `vv_before_edit` applies ONLY the delta and
    // converges — no full-snapshot resend needed (loss-free).
    peer.import_doc_update(note, &delta).await.unwrap();
    let rendered = peer.render_note(note).await.unwrap();
    assert!(
        rendered.contains("beta EDITED"),
        "peer converges from the delta alone; got: {rendered:?}"
    );
    assert_eq!(
        author.render_note(note).await,
        peer.render_note(note).await,
        "author + peer converge after the delta-only exchange"
    );
}

// ── WS-push clobber guard (2026-06-02) ───────────────────────────
//
// The FINAL data-loss vector: a device ships a WHOLE-NOTE SNAPSHOT
// carrying its STALE value for a block another peer (the server, via
// HTTP) just edited. The stale op is CONCURRENT with the server's
// edit and WINS the LWW tiebreak → a raw `doc.import` reverts the
// server's edit on the authoritative doc. `import_doc_update` must
// apply ONLY the blocks the peer GENUINELY (causally) re-authored,
// never a stale re-assertion the peer merely re-shipped.

// FLIPPED by tesela-fte (pure max-`TreeID`): formerly
// `ws_apply_stale_snapshot_does_not_revert_peer_edit`, which asserted the
// stale-guard preserved the server's newer "Awesome sweet". Under pure
// max-`TreeID` the survivor per bid is ONLY the higher-`TreeID`
// (higher-peer) twin: the device (0x7f) outranks the server (0x5e), so A
// resolves to the device's re-shipped stale "Awesome" and the server's
// "Awesome sweet" is dropped. B still resolves to the device's genuine "B
// device". Product-approved 2026-07-01: higher-TreeID text wins over the
// genuine-edit/stale-guard preference.
#[tokio::test]
async fn ws_apply_disjoint_conflict_resolves_to_max_treeid_twin() {
    let sdev = DeviceId::from_bytes([0x5e; 16]);
    let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
    let ddev = DeviceId::from_bytes([0x7f; 16]);
    let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
    let note = blake3_note_id("daily");

    seed_disjoint(&server, &device, note).await;

    // Server edits A via HTTP-style block op (the newer value).
    server
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "Awesome sweet".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    // Device (stale: never saw the server edit) re-authors A back to the
    // stale value AND genuinely edits B. Then exports a FULL SNAPSHOT —
    // the cold-launch first-push frame that triggered the incident.
    device
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "Awesome".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    device
        .record_local(OpPayload::BlockUpsert {
            block_id: B_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "B device".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    let snapshot = device.export_doc_update(note, None).await.unwrap();

    // Server applies the device's snapshot via the WS-apply path.
    server.import_doc_update(note, &snapshot).await.unwrap();

    let a = block_text(&server, note, A_BID_BYTES)
        .await
        .unwrap_or_default();
    let b = block_text(&server, note, B_BID_BYTES)
        .await
        .unwrap_or_default();
    assert_eq!(
        a, "Awesome",
        "pure max-`TreeID`: the higher-peer (device 0x7f) twin wins, even a \
         stale re-ship — stale-guard dropped (got {a:?})"
    );
    assert_eq!(
        b, "B device",
        "B: the higher-peer (device) twin's genuine edit (got {b:?})"
    );
}

#[tokio::test]
async fn ws_apply_genuine_edit_applies() {
    // No competing server edit on B: the device's genuine B edit must
    // land on the server (don't invert the bug into "always keep server").
    let sdev = DeviceId::from_bytes([0x5e; 16]);
    let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
    let ddev = DeviceId::from_bytes([0x7f; 16]);
    let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
    let note = blake3_note_id("daily");

    seed_disjoint(&server, &device, note).await;

    device
        .record_local(OpPayload::BlockUpsert {
            block_id: B_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "B device".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    // Device ships a since_vv DELTA of just B (the steady-state frame).
    let delta = device.export_doc_update(note, None).await.unwrap();
    server.import_doc_update(note, &delta).await.unwrap();

    assert_eq!(
        block_text(&server, note, A_BID_BYTES).await.as_deref(),
        Some("Awesome"),
        "A unchanged"
    );
    assert_eq!(
        block_text(&server, note, B_BID_BYTES).await.as_deref(),
        Some("B device"),
        "genuine B edit applied"
    );
}

#[tokio::test]
async fn ws_apply_stale_snapshot_is_idempotent() {
    // Applying the same disjoint snapshot twice must not corrupt state; the
    // second apply is a no-op (both blocks stable). FLIPPED by tesela-fte:
    // under pure max-`TreeID` A resolves to the higher-peer device twin's
    // "Awesome" (not the server's "Awesome sweet" — stale-guard dropped).
    let sdev = DeviceId::from_bytes([0x5e; 16]);
    let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
    let ddev = DeviceId::from_bytes([0x7f; 16]);
    let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
    let note = blake3_note_id("daily");

    seed_disjoint(&server, &device, note).await;

    server
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "Awesome sweet".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    device
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "Awesome".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    device
        .record_local(OpPayload::BlockUpsert {
            block_id: B_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "B device".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    let snapshot = device.export_doc_update(note, None).await.unwrap();

    server.import_doc_update(note, &snapshot).await.unwrap();
    let a1 = block_text(&server, note, A_BID_BYTES)
        .await
        .unwrap_or_default();
    let b1 = block_text(&server, note, B_BID_BYTES)
        .await
        .unwrap_or_default();
    // Second apply of the identical frame.
    server.import_doc_update(note, &snapshot).await.unwrap();
    let a2 = block_text(&server, note, A_BID_BYTES)
        .await
        .unwrap_or_default();
    let b2 = block_text(&server, note, B_BID_BYTES)
        .await
        .unwrap_or_default();

    assert_eq!(a1, "Awesome", "pure max-`TreeID`: higher-peer device twin wins");
    assert_eq!(b1, "B device");
    assert_eq!(a1, a2, "A stable across re-apply");
    assert_eq!(b1, b2, "B stable across re-apply");
}

#[tokio::test]
async fn ws_apply_shared_register_concurrent_edit_merges_via_loro_text() {
    // When the server + device SHARE the Loro lineage for a block (one
    // LoroText) and BOTH edit it concurrently, the protected apply must
    // DEFER to Loro's own LoroText merge — NOT force one side's whole value
    // and NOT restore the other. Block text is a sequence CRDT now, so the
    // two whole-text edits INTERLEAVE: both sides converge to the SAME
    // merged value, both contributions survive, and re-apply is stable (no
    // oscillation, no clobber). (Pre-LoroText this was an LWW whole-string
    // pick; the merge is the deepest fix.)
    let sdev = DeviceId::from_bytes([0x5e; 16]);
    let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
    let ddev = DeviceId::from_bytes([0x7f; 16]);
    let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
    let note = blake3_note_id("daily");

    // SHARED base: device imports the server's snapshot (same TreeIDs).
    server
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("daily".into()),
            title: "Daily".into(),
            content: format!("- base <!-- bid:{A_BID} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    let base = server.export_doc_update(note, None).await.unwrap();
    device.import_doc_update(note, &base).await.unwrap();

    // Capture the device's pre-edit VV so it can ship a true since-vv
    // DELTA of just its own concurrent edit on the SHARED register.
    let dev_vv = device.doc_version(note).await;
    // Concurrent edits to the SAME shared block.
    server
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "server edit".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    device
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "device edit".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    let delta = device
        .export_doc_update(note, dev_vv.as_deref())
        .await
        .unwrap();

    // Server applies the device's delta. Loro's LoroText merge picks the
    // SAME converged value on both sides — and re-applying must be stable.
    server.import_doc_update(note, &delta).await.unwrap();
    // Round-trip the server's state back to the device to converge.
    let dev_vv2 = device.doc_version(note).await;
    let srv_delta = server
        .export_doc_update(note, dev_vv2.as_deref())
        .await
        .unwrap();
    device.import_doc_update(note, &srv_delta).await.unwrap();

    let sa = block_text(&server, note, A_BID_BYTES)
        .await
        .unwrap_or_default();
    let da = block_text(&device, note, A_BID_BYTES)
        .await
        .unwrap_or_default();
    assert_eq!(
        sa, da,
        "shared-register concurrent edit converges on both sides"
    );
    // The LoroText merge INTERLEAVES both whole-text edits rather than
    // LWW-picking one: the result is NEITHER whole string (no clobber) and
    // is longer than either input — both sides contributed characters.
    assert_ne!(
        sa, "server edit",
        "not an LWW pick of the server's whole edit"
    );
    assert_ne!(
        sa, "device edit",
        "not an LWW pick of the device's whole edit"
    );
    assert!(
        sa.len() > "server edit".len() && sa.contains("device"),
        "both concurrent edits' contributions survive the merge: {sa:?}"
    );

    // Stable: re-applying the same delta does not flip the value.
    server.import_doc_update(note, &delta).await.unwrap();
    let sa2 = block_text(&server, note, A_BID_BYTES)
        .await
        .unwrap_or_default();
    assert_eq!(sa, sa2, "no oscillation on re-apply");
}

// ── Same-block concurrent text MERGE (2026-06-02 LoroText fix) ────
//
// The DEEPEST data-loss vector: two replicas on a SHARED Loro lineage
// each apply a DIFFERENT whole-text BlockUpsert to the SAME block,
// concurrently. With the legacy LWW map register one side's typing
// vanished. With block text stored as a nested `LoroText`, each
// replica's whole-text `update()` Myers-diffs into the minimal
// splices against the shared sequence, so cross-import INTERLEAVES
// both contributions instead of clobbering.
#[tokio::test]
async fn concurrent_same_block_text_merges_not_clobbers() {
    let note = blake3_note_id("merge");

    // Replica A builds the shared base for block X.
    let dev_a = DeviceId::from_bytes([0xa7; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    a.record_local(OpPayload::BlockUpsert {
        block_id: A_BID_BYTES,
        note_id: note,
        parent_block_id: None,
        order_key: "00000000".into(),
        indent_level: 0,
        text: "The quick fox".into(),
        after_block_id: None,
    })
    .await
    .unwrap();

    // Replica B imports the base so both share the same TreeID lineage
    // for X (the merge precondition — NOT disjoint twins).
    let dev_b = DeviceId::from_bytes([0xb7; 16]);
    let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
    let base = a.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &base).await.unwrap();
    assert_eq!(
        block_text(&b, note, A_BID_BYTES).await.as_deref(),
        Some("The quick fox"),
        "shared base seeded on B"
    );

    // Capture each replica's pre-edit VV so each ships only its own
    // concurrent edit as a since-vv delta.
    let a_vv = a.doc_version(note).await;
    let b_vv = b.doc_version(note).await;

    // Concurrent whole-text edits to the SAME shared block X.
    a.record_local(OpPayload::BlockUpsert {
        block_id: A_BID_BYTES,
        note_id: note,
        parent_block_id: None,
        order_key: "00000000".into(),
        indent_level: 0,
        text: "The quick brown fox".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    b.record_local(OpPayload::BlockUpsert {
        block_id: A_BID_BYTES,
        note_id: note,
        parent_block_id: None,
        order_key: "00000000".into(),
        indent_level: 0,
        text: "The quick red fox jumps".into(),
        after_block_id: None,
    })
    .await
    .unwrap();

    // Cross-import each replica's delta into the other, then converge.
    let a_delta = a.export_doc_update(note, a_vv.as_deref()).await.unwrap();
    let b_delta = b.export_doc_update(note, b_vv.as_deref()).await.unwrap();
    b.import_doc_update(note, &a_delta).await.unwrap();
    a.import_doc_update(note, &b_delta).await.unwrap();

    let ta = block_text(&a, note, A_BID_BYTES).await.unwrap_or_default();
    let tb = block_text(&b, note, A_BID_BYTES).await.unwrap_or_default();

    // Byte-identical on both replicas.
    assert_eq!(ta, tb, "replicas converge to the same merged text");
    // NOT the LWW whole-string pick: neither whole edit wholly won.
    assert_ne!(
        ta, "The quick brown fox",
        "must not be A's whole-string LWW pick"
    );
    assert_ne!(
        ta, "The quick red fox jumps",
        "must not be B's whole-string LWW pick"
    );
    // INTERLEAVED merge: both sides' contributions survive — A added
    // "brown", B added "red" and "jumps". Neither was wholly dropped.
    assert!(
        ta.contains("brown"),
        "A's edit (\"brown\") must survive the merge: {ta:?}"
    );
    assert!(
        ta.contains("red"),
        "B's edit (\"red\") must survive the merge: {ta:?}"
    );
    assert!(
        ta.contains("jumps"),
        "B's edit (\"jumps\") must survive the merge: {ta:?}"
    );
}

// ── Character-level splice API (collab editing C1 foundation) ─────
//
// `splice_block_text` lets a client send the user's ACTUAL keystroke
// (insert at offset / delete a range) instead of re-authoring the whole
// block text. Re-authoring Myers-diffs into DELETEs of a concurrent
// peer's characters → clobber; a splice is a single insert/delete on the
// block's `text_seq` LoroText, so concurrent splices INTERLEAVE.

#[tokio::test]
async fn splice_block_text_concurrent_inserts_interleave() {
    // Two replicas on a SHARED text_seq lineage each splice an insert at
    // offset 0 of an EMPTY block concurrently. Cross-importing each
    // other's since-vv delta must INTERLEAVE both inserts — both replicas
    // byte-identical, both "AAA" and "BBB" present (neither overwritten).
    let note = blake3_note_id("splice-interleave");
    // Start from an empty block so offset 0 is unambiguous on both sides.
    let (a, b) = splice_shared_base(note, "").await;

    // Capture each replica's pre-edit VV so each ships only its own splice.
    let a_vv = a.doc_version(note).await;
    let b_vv = b.doc_version(note).await;

    // Concurrent splices: A inserts "AAA" at 0, B inserts "BBB" at 0.
    let na = a
        .splice_block_text(note, A_BID_BYTES, 0, 0, "AAA")
        .await
        .unwrap();
    let nb = b
        .splice_block_text(note, A_BID_BYTES, 0, 0, "BBB")
        .await
        .unwrap();
    assert_eq!(na, 1, "A's splice applied");
    assert_eq!(nb, 1, "B's splice applied");

    // Cross-import each replica's delta into the other, then converge.
    let a_delta = a.export_doc_update(note, a_vv.as_deref()).await.unwrap();
    let b_delta = b.export_doc_update(note, b_vv.as_deref()).await.unwrap();
    b.import_doc_update(note, &a_delta).await.unwrap();
    a.import_doc_update(note, &b_delta).await.unwrap();

    let ta = block_text(&a, note, A_BID_BYTES).await.unwrap_or_default();
    let tb = block_text(&b, note, A_BID_BYTES).await.unwrap_or_default();

    assert_eq!(ta, tb, "replicas converge to the same merged text");
    assert!(
        ta.contains("AAA"),
        "A's splice (\"AAA\") must survive the interleave: {ta:?}"
    );
    assert!(
        ta.contains("BBB"),
        "B's splice (\"BBB\") must survive the interleave: {ta:?}"
    );
    // A real interleave: both 3-char inserts land, so the merged text is
    // 6 chars — neither side OVERWROTE the other (that would be 3 chars).
    assert_eq!(
        ta.chars().count(),
        6,
        "both inserts present, neither overwritten: {ta:?}"
    );
}

#[tokio::test]
async fn splice_block_text_utf16_offsets_handle_multibyte() {
    // The block holds "a😀b". The emoji is 2 UTF-16 code units, so the
    // offset JUST AFTER it is 3 (a=1, 😀=2 → 1+2). Splicing an insert at
    // UTF-16 offset 3 must land between 😀 and "b" — proving the offset is
    // UTF-16, not a Unicode-scalar index (which would be 2) or a byte
    // index (which would be 5).
    let note = blake3_note_id("splice-utf16");
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "a😀b".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    let n = engine
        .splice_block_text(note, A_BID_BYTES, 3, 0, "X")
        .await
        .unwrap();
    assert_eq!(n, 1, "splice applied");

    let got = block_text(&engine, note, A_BID_BYTES)
        .await
        .unwrap_or_default();
    assert_eq!(
        got, "a😀Xb",
        "insert at UTF-16 offset 3 lands after the 2-unit emoji: {got:?}"
    );
}

#[tokio::test]
async fn splice_block_text_delete_then_insert_replaces() {
    // A single splice with delete_len>0 AND a non-empty insert at the
    // same offset replaces the range: "hello world" → delete "world"
    // (offset 6, len 5) + insert "there" → "hello there".
    let note = blake3_note_id("splice-replace");
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "hello world".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    let n = engine
        .splice_block_text(note, A_BID_BYTES, 6, 5, "there")
        .await
        .unwrap();
    assert_eq!(n, 1, "replace splice applied");

    let got = block_text(&engine, note, A_BID_BYTES)
        .await
        .unwrap_or_default();
    assert_eq!(got, "hello there", "the range was replaced: {got:?}");
}

#[tokio::test]
async fn read_block_text_returns_merged_text_after_splice() {
    // The inbound live-apply read (C1-inbound): after a splice mutates a
    // block's text_seq, the public read_block_text(note, block) returns the
    // current merged text — this is what iOS reads to reconcile the open
    // editor with a remote peer's concurrent edit. Unknown note/block → None.
    let note = blake3_note_id("read-block-text");
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "hello".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    engine
        .splice_block_text(note, A_BID_BYTES, 5, 0, " world")
        .await
        .unwrap();

    let got = engine.read_block_text(note, A_BID_BYTES).await;
    assert_eq!(
        got.as_deref(),
        Some("hello world"),
        "reads the merged text_seq content"
    );

    assert_eq!(
        engine.read_block_text(note, [0xcc; 16]).await,
        None,
        "unknown block → None"
    );
    assert_eq!(
        engine
            .read_block_text(blake3_note_id("nope"), A_BID_BYTES)
            .await,
        None,
        "unknown note → None"
    );
}

#[tokio::test]
async fn splice_block_text_unknown_block_is_noop() {
    // A splice targeting a block_id that has no live node is a no-op and
    // returns Ok(0) — a splice is an in-place edit, the block must exist.
    let note = blake3_note_id("splice-missing");
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "present".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    // B_BID_BYTES was never created in this note.
    let n = engine
        .splice_block_text(note, B_BID_BYTES, 0, 0, "X")
        .await
        .unwrap();
    assert_eq!(n, 0, "missing block → no-op Ok(0)");
    // The existing block is untouched.
    assert_eq!(
        block_text(&engine, note, A_BID_BYTES).await.as_deref(),
        Some("present"),
        "the present block is unaffected"
    );
}

// ── Convergent whole-text writes (the same-bid lineage-union fix) ──────
//
// `write_block_text` is the WHOLE-TEXT authoring path (BlockUpsert / the
// disjoint-twin heal / reconcile). Two replicas on a SHARED text_seq
// lineage that each REWRITE the block to a DIFFERENT whole string must
// converge to ONE coherent value — the shared base preserved ONCE, only
// the divergent tails char-merging — NOT the two full strings
// concatenated (the "Bothnice onenice one" signature). And re-applying
// the SAME whole text (the heal's idempotent re-issue) must never grow or
// duplicate runs.

#[tokio::test]
async fn write_block_text_concurrent_rewrites_one_coherent_value() {
    // Shared base "hello"; A rewrites whole-text → "hello world", B
    // rewrites whole-text → "hello there", concurrently. After merge both
    // replicas converge AND the shared base "hello" survives exactly ONCE
    // (a minimal-diff char-merge), not "hello worldhello there" (the
    // whole-replace union that duplicates the base).
    let note = blake3_note_id("write-converge");
    let (a, b) = splice_shared_base(note, "hello").await;

    let a_vv = a.doc_version(note).await;
    let b_vv = b.doc_version(note).await;

    // Concurrent WHOLE-TEXT rewrites (the BlockUpsert authoring path).
    upsert_block(&a, note, A_BID_BYTES, "hello world", None).await;
    upsert_block(&b, note, A_BID_BYTES, "hello there", None).await;

    let a_delta = a.export_doc_update(note, a_vv.as_deref()).await.unwrap();
    let b_delta = b.export_doc_update(note, b_vv.as_deref()).await.unwrap();
    b.import_doc_update(note, &a_delta).await.unwrap();
    a.import_doc_update(note, &b_delta).await.unwrap();

    let ta = block_text(&a, note, A_BID_BYTES).await.unwrap_or_default();
    let tb = block_text(&b, note, A_BID_BYTES).await.unwrap_or_default();

    assert_eq!(ta, tb, "replicas converge to the same merged text: {ta:?}");
    // The shared base appears exactly once — the divergent tails merge,
    // the common prefix is NOT re-inserted by both writers.
    assert_eq!(
        ta.matches("hello").count(),
        1,
        "shared base 'hello' preserved once, not concatenated: {ta:?}"
    );
    // Both divergent edits survive the char-merge.
    assert!(ta.contains("world"), "A's edit survives: {ta:?}");
    assert!(ta.contains("there"), "B's edit survives: {ta:?}");
}

#[tokio::test]
async fn peer_hidden_blank_reservations_become_distinct_authored_blocks() {
    let note = blake3_note_id("local-only-blank-leaves");
    let a_blank = [0xd1; 16];
    let b_blank = [0xd2; 16];
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_b = DeviceId::from_bytes([0xb1; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));

    upsert_block(&a, note, a_blank, "", None).await;
    let a_reservation = a.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &a_reservation).await.unwrap();
    let a_bid = uuid::Uuid::from_bytes(a_blank).to_string();
    assert!(!b.render_note(note).await.unwrap().contains(&a_bid));

    let a_vv = a.doc_version(note).await;
    let b_vv = b.doc_version(note).await;
    upsert_block(&b, note, b_blank, "", None).await;
    let a_spliced = a
        .splice_block_text(
            note,
            a_blank,
            0,
            0,
            "Is our conductor arena duplicating terminal bench?",
        )
        .await
        .unwrap();
    let b_spliced = b
        .splice_block_text(
            note,
            b_blank,
            0,
            0,
            "OpenCode desktop app\nstatus:: todo\ntags:: Task",
        )
        .await
        .unwrap();
    assert_eq!(a_spliced, 1, "creator can splice its hidden reservation");
    assert_eq!(b_spliced, 1, "peer can splice its own reservation");

    let a_delta = a.export_doc_update(note, a_vv.as_deref()).await.unwrap();
    let b_delta = b.export_doc_update(note, b_vv.as_deref()).await.unwrap();
    a.import_doc_update(note, &b_delta).await.unwrap();
    b.import_doc_update(note, &a_delta).await.unwrap();

    for engine in [&a, &b] {
        let rendered = engine.render_note(note).await.unwrap();
        assert!(rendered.contains("Is our conductor arena"));
        assert!(rendered.contains("OpenCode desktop app"));
        assert!(!rendered.contains("tags:: TaskIs our conductor arena"));
        assert_eq!(
            block_text(engine, note, a_blank).await.as_deref(),
            Some("Is our conductor arena duplicating terminal bench?")
        );
        assert_eq!(
            block_text(engine, note, b_blank).await.as_deref(),
            Some("OpenCode desktop app\nstatus:: todo\ntags:: Task")
        );
    }
}

#[tokio::test]
async fn write_block_text_empty_base_concurrent_char_merges() {
    // Shared EMPTY placeholder (the daily empty block); two devices type
    // DIFFERENT whole text into it concurrently, then merge. With no
    // common ancestor content to anchor a diff, this is the irreducible
    // char-merge case (same semantics as the splice interleave): both
    // replicas converge to ONE shared value and BOTH fragments survive
    // (neither replica clobbers the other). The fix's job is convergence +
    // no compounding, NOT to magically pick a single fork's text here.
    let note = blake3_note_id("write-empty-merge");
    let (a, b) = splice_shared_base(note, "").await;
    let a_vv = a.doc_version(note).await;
    let b_vv = b.doc_version(note).await;
    upsert_block(&a, note, A_BID_BYTES, "Both", None).await;
    upsert_block(&b, note, A_BID_BYTES, "nice one", None).await;
    let a_delta = a.export_doc_update(note, a_vv.as_deref()).await.unwrap();
    let b_delta = b.export_doc_update(note, b_vv.as_deref()).await.unwrap();
    b.import_doc_update(note, &a_delta).await.unwrap();
    a.import_doc_update(note, &b_delta).await.unwrap();
    let ta = block_text(&a, note, A_BID_BYTES).await.unwrap_or_default();
    let tb = block_text(&b, note, A_BID_BYTES).await.unwrap_or_default();
    assert_eq!(ta, tb, "replicas converge: A={ta:?} B={tb:?}");
    assert!(ta.contains("Both"), "A's authoring survives: {ta:?}");
    assert!(ta.contains("nice one"), "B's authoring survives: {ta:?}");
    // No compounding: each fragment appears exactly once (no third run).
    assert_eq!(ta.matches("Both").count(), 1, "no duplicate run: {ta:?}");
    assert_eq!(ta.matches("nice one").count(), 1, "no duplicate run: {ta:?}");
}

#[tokio::test]
async fn write_block_text_reapply_is_idempotent() {
    // Re-authoring a block with the SAME whole text (the disjoint-twin
    // heal re-issues record_local(BlockUpsert{text}) on every import) must
    // be a true no-op on the text_seq — never appending a second run that
    // grows/duplicates the value, and never growing the doc's op history
    // (the lever that compounds under multi-round relay re-broadcast).
    let note = blake3_note_id("write-idempotent");
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    upsert_block(&engine, note, A_BID_BYTES, "nice one", None).await;
    let v0 = engine.doc_version(note).await;
    // Re-apply the identical whole text several times.
    for _ in 0..3 {
        upsert_block(&engine, note, A_BID_BYTES, "nice one", None).await;
    }
    let v1 = engine.doc_version(note).await;
    assert_eq!(
        block_text(&engine, note, A_BID_BYTES).await.as_deref(),
        Some("nice one"),
        "re-applying identical whole text never grows/duplicates the value"
    );
    assert_eq!(
        v0, v1,
        "re-applying identical whole text never grows the op history"
    );
}

#[tokio::test]
async fn write_block_text_distinct_new_blocks_stay_separate() {
    // Two replicas on a shared base each ADD a NEW block with a DISTINCT
    // bid (the iOS fresh-v4 case). After merge the note holds BOTH new
    // blocks as separate values — distinct bids never share a text_seq, so
    // there is no concatenation.
    let note = blake3_note_id("write-distinct");
    let (a, b) = splice_shared_base(note, "base").await;

    let a_vv = a.doc_version(note).await;
    let b_vv = b.doc_version(note).await;

    // Distinct fresh bids, one per replica.
    let new_a: [u8; 16] = [0xc1; 16];
    let new_b: [u8; 16] = [0xc2; 16];
    upsert_block(&a, note, new_a, "alpha block", None).await;
    upsert_block(&b, note, new_b, "beta block", None).await;

    let a_delta = a.export_doc_update(note, a_vv.as_deref()).await.unwrap();
    let b_delta = b.export_doc_update(note, b_vv.as_deref()).await.unwrap();
    b.import_doc_update(note, &a_delta).await.unwrap();
    a.import_doc_update(note, &b_delta).await.unwrap();

    for (label, eng) in [("a", &a), ("b", &b)] {
        assert_eq!(
            block_text(eng, note, new_a).await.as_deref(),
            Some("alpha block"),
            "{label}: A's new block is its own coherent value"
        );
        assert_eq!(
            block_text(eng, note, new_b).await.as_deref(),
            Some("beta block"),
            "{label}: B's new block is its own coherent value"
        );
    }
}

// ── BOOTSTRAP-BEFORE-AUTHOR: multi-device daily convergence ──────────
//
// The production garble (Taylor's daily, bid c35861c0):
// `Bothnice onenice one` = SEPARATE intended blocks' text concatenated
// into ONE block, plus persistent divergence. Root cause: a device
// authored today's daily on a FRESH DISJOINT LoroDoc; when it later
// received the relay's authoritative version of the same bid (created on
// another device + synced), `apply_relay_updates` merged the disjoint
// lineages → per-block `text_seq` UNION / same-bid twins. The fix is
// CLIENT-SIDE bootstrap-before-author: import the relay's authoritative
// doc (shared base) BEFORE the first local edit, so the edit lands on the
// existing lineage → clean char-merge.

// CASE-GARBLE (reproduces the bug). REALISTIC — no hardcoded shared
// placeholder bid. Engine A creates today's daily + a block with a content
// bid + text, exports the authoritative snapshot (what the relay holds).
// Engine B authored its OWN fresh DISJOINT daily doc and edited the SAME
// bid (the bid reached B via materialized markdown). When B then imports
// A's authoritative ops through the relay apply path (no shared base), the
// two disjoint `text_seq` lineages UNION into one garbled block and/or
// leave disjoint same-bid twins (divergence). This asserts the failure
// reproduces, so the fix below is proven against a real garble.
// FORMERLY `daily_disjoint_author_then_relay_import_garbles`, which asserted
// the BUG reproduced (union / twin divergence / data loss). tesela-y11's
// deterministic disjoint-twin resolution now converges this case CLEANLY on
// the relay apply path WITHOUT needing bootstrap-before-author: the twins are
// deduped to ONE node whose text is the deterministic winner (pure max-`TreeID`,
// tesela-fte), never a `Both`+`nice one` concatenation and never a
// persistent twin. (One genuine value wins a same-block conflict — inherent
// to single-line text; the char-MERGE where both survive is the shared-base
// path in the next test.)
#[tokio::test]
async fn daily_disjoint_author_then_relay_import_converges_no_garble() {
    let note = blake3_note_id("2026-06-29");
    let bid = content_bid("c35861c0-daily-block");

    // Engine A: fresh daily, authors the block, exports the authoritative
    // snapshot (the relay's version of this note).
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    upsert_block(&a, note, bid, "Both", None).await;
    let auth = a.export_doc_update(note, None).await.unwrap();

    // Engine B: its OWN fresh DISJOINT daily, edits the SAME bid (got from
    // materialized markdown) — a disjoint twin of the same block_id.
    let dev_b = DeviceId::from_bytes([0xb1; 16]);
    let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
    upsert_block(&b, note, bid, "nice one", None).await;

    // B imports A's authoritative ops via the relay apply path. No shared
    // base → disjoint-lineage merge, now resolved deterministically.
    let _ = b.apply_relay_updates(&[(note, auth)]).await;

    let tb = block_text(&b, note, bid).await.unwrap_or_default();
    let twins = block_twin_count(&b, note, bid).await;

    assert!(
        !(tb.contains("Both") && tb.contains("nice one")),
        "disjoint merge must NOT union/garble the two runs: {tb:?}"
    );
    assert_eq!(twins, 1, "disjoint merge must dedup to ONE live node: twins={twins}");
    assert!(
        tb == "Both" || tb == "nice one",
        "must keep ONE coherent genuine value (no garble, no empty): {tb:?}"
    );
}

// CASE-FIXED. Engine B imports A's authoritative snapshot
// (`import_authoritative_snapshot`) BEFORE its first local edit of the
// shared block, so it authors into A's EXISTING lineage. A and B then edit
// the SAME block concurrently → clean char-merge: ONE coherent block, no
// concatenation, no twins. ALSO: a local un-broadcast edit on B (its own
// new block, never synced) must SURVIVE the bootstrap import (the import is
// a non-destructive merge, never a wholesale replace).
#[tokio::test]
async fn daily_bootstrap_before_author_converges_clean() {
    let note = blake3_note_id("2026-06-29");
    let bid = content_bid("c35861c0-daily-block");

    // Engine A: authoritative — fresh daily + block, exported snapshot.
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    upsert_block(&a, note, bid, "Both", None).await;
    let auth = a.export_doc_update(note, None).await.unwrap();

    // Engine B: has a LOCAL UN-BROADCAST edit (its own new block) BEFORE
    // bootstrap — must survive the authoritative import.
    let dev_b = DeviceId::from_bytes([0xb1; 16]);
    let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
    let local_bid = content_bid("b-local-unbroadcast");
    upsert_block(&b, note, local_bid, "local draft", None).await;

    // BOOTSTRAP-BEFORE-AUTHOR: import A's authoritative doc (shared base)
    // BEFORE B's first edit of the shared block.
    b.import_authoritative_snapshot(note, &auth).await.unwrap();

    // Clobber guard: B's local un-broadcast edit survived the bootstrap.
    assert_eq!(
        block_text(&b, note, local_bid).await.as_deref(),
        Some("local draft"),
        "local un-broadcast edit must survive bootstrap import"
    );
    // B now shares A's lineage for `bid` — exactly one node, A's value.
    assert_eq!(
        block_text(&b, note, bid).await.as_deref(),
        Some("Both"),
        "bootstrap establishes A's value as the shared base"
    );
    assert_eq!(
        block_twin_count(&b, note, bid).await,
        1,
        "bootstrap leaves a single shared node, no twin"
    );

    // CONCURRENT edits on the SHARED lineage: A appends "!" (offset 4),
    // B appends " yeah" (offset 4).
    a.splice_block_text(note, bid, 4, 0, "!").await.unwrap();
    b.splice_block_text(note, bid, 4, 0, " yeah").await.unwrap();

    // Converge by cross-importing each replica's FULL doc. (A full export
    // is used rather than a since-vv delta because B's pre-bootstrap local
    // block is an op A has never seen — a since-vv delta of just B's splice
    // would import PENDING against that causal gap. The full snapshot
    // carries the local block too, so it lands and the splices char-merge.)
    let a_full = a.export_doc_update(note, None).await.unwrap();
    let b_full = b.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &a_full).await.unwrap();
    a.import_doc_update(note, &b_full).await.unwrap();

    let ta = block_text(&a, note, bid).await.unwrap_or_default();
    let tb = block_text(&b, note, bid).await.unwrap_or_default();

    // Clean convergence: byte-identical, ONE node, no concatenation.
    assert_eq!(ta, tb, "replicas converge: A={ta:?} B={tb:?}");
    assert_eq!(
        block_twin_count(&a, note, bid).await,
        1,
        "no twins on A after concurrent edit: {ta:?}"
    );
    assert_eq!(
        block_twin_count(&b, note, bid).await,
        1,
        "no twins on B after concurrent edit: {tb:?}"
    );
    // The shared base "Both" survives exactly once (char-merge, not the
    // disjoint union that duplicated runs).
    assert_eq!(
        ta.matches("Both").count(),
        1,
        "shared base preserved once, not concatenated: {ta:?}"
    );
    // Both concurrent contributions survive the merge.
    assert!(ta.contains('!'), "A's concurrent edit survives: {ta:?}");
    assert!(ta.contains("yeah"), "B's concurrent edit survives: {ta:?}");
    // B's local un-broadcast block is still intact and coherent.
    assert_eq!(
        block_text(&b, note, local_bid).await.as_deref(),
        Some("local draft"),
        "local block still coherent after converge"
    );
}

// ---- P1.4 property ops ----

use tesela_core::property::PropScalar;

// (a) BlockPropertySet SetScalar on a block → read it back via the engine.
#[tokio::test]
async fn block_property_set_scalar_round_trips() {
    let note = blake3_note_id("prop-scalar");
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    upsert_block(&engine, note, A_BID_BYTES, "a block", None).await;

    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: A_BID_BYTES,
            key: "status".into(),
            value: PropOp::SetScalar(PropScalar::Text("doing".into())),
        })
        .await
        .unwrap();

    assert_eq!(
        block_prop_scalar(&engine, note, A_BID_BYTES, "status").await,
        Some(PropScalar::Text("doing".into())),
        "scalar property reads back after BlockPropertySet"
    );
}

// A property set on a block that doesn't exist is a safe no-op, NOT a crash.
#[tokio::test]
async fn block_property_set_on_missing_block_is_noop() {
    let note = blake3_note_id("prop-missing-block");
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    // B_BID_BYTES is never created in this note.
    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: B_BID_BYTES,
            key: "status".into(),
            value: PropOp::SetScalar(PropScalar::Text("doing".into())),
        })
        .await
        .expect("property set on a missing block must not error");
    assert_eq!(
        block_prop_scalar(&engine, note, B_BID_BYTES, "status").await,
        None,
        "no node was created for the missing block"
    );
}

// (b) ⭐ Shared base: A splices prose on block X, B sets a property on the
// SAME block X. Exchange both ways → BOTH survive (prose carries A's edit
// AND the property is set — neither clobbers the other).
#[tokio::test]
async fn concurrent_prose_splice_and_property_set_both_survive() {
    let note = blake3_note_id("prose-vs-prop");
    let block = A_BID_BYTES;

    // Engine A builds the shared base (one block, text "Hello").
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    upsert_block(&a, note, block, "Hello", None).await;

    // Engine B imports the base so both share Loro history (same TreeID).
    let dev_b = DeviceId::from_bytes([0xb2; 16]);
    let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
    let base = a.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &base).await.unwrap();
    assert_eq!(block_text(&b, note, block).await.as_deref(), Some("Hello"));

    // Concurrent, neither has seen the other:
    //   A appends " world" to the SAME block's prose.
    //   B sets a `status` property on the SAME block.
    a.splice_block_text(note, block, 5, 0, " world")
        .await
        .unwrap();
    b.record_local(OpPayload::BlockPropertySet {
        note_id: note,
        block_id: block,
        key: "status".into(),
        value: PropOp::SetScalar(PropScalar::Text("doing".into())),
    })
    .await
    .unwrap();

    // Exchange updates both ways.
    let ua = a.export_doc_update(note, None).await.unwrap();
    let ub = b.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &ua).await.unwrap();
    a.import_doc_update(note, &ub).await.unwrap();

    // Both survive on BOTH replicas: the prose carries A's edit AND the
    // property is set — neither clobbers the other.
    for (label, e) in [("A", &a), ("B", &b)] {
        assert_eq!(
            block_text(e, note, block).await.as_deref(),
            Some("Hello world"),
            "{label}: prose edit must survive the concurrent property set"
        );
        assert_eq!(
            block_prop_scalar(e, note, block, "status").await,
            Some(PropScalar::Text("doing".into())),
            "{label}: property must survive the concurrent prose edit"
        );
    }
}

// (c) AddToList of two DISTINCT values on the same block's "tags" from two
// engines on a shared base → union after merge.
//
// The "tags" LoroList must exist in SHARED history before the two engines
// diverge: Loro derives a child container's id from the op that created
// it, so two peers each minting the list for the FIRST time concurrently
// produce rival containers and one branch is overwritten (documented
// "Container ID And Overwrite Hazards"). The realistic product path tags
// an EXISTING block, so we seed the list once on the base (one initial
// AddToList, imported by B) and THEN add distinct values concurrently —
// which unions correctly because both push into the same shared container.
#[tokio::test]
async fn concurrent_add_to_list_unions() {
    let note = blake3_note_id("tags-union");
    let block = A_BID_BYTES;

    let dev_a = DeviceId::from_bytes([0xc1; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    upsert_block(&a, note, block, "a block", None).await;
    // Seed the shared "tags" list on the base so both replicas share its
    // container id (see the doc comment above).
    a.record_local(OpPayload::BlockPropertySet {
        note_id: note,
        block_id: block,
        key: "tags".into(),
        value: PropOp::AddToList(PropScalar::Text("Base".into())),
    })
    .await
    .unwrap();

    let dev_b = DeviceId::from_bytes([0xc2; 16]);
    let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
    let base = a.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &base).await.unwrap();

    // Concurrent AddToList of DISTINCT values to the same (shared) "tags" list.
    a.record_local(OpPayload::BlockPropertySet {
        note_id: note,
        block_id: block,
        key: "tags".into(),
        value: PropOp::AddToList(PropScalar::Text("Task".into())),
    })
    .await
    .unwrap();
    b.record_local(OpPayload::BlockPropertySet {
        note_id: note,
        block_id: block,
        key: "tags".into(),
        value: PropOp::AddToList(PropScalar::Text("Urgent".into())),
    })
    .await
    .unwrap();

    let ua = a.export_doc_update(note, None).await.unwrap();
    let ub = b.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &ua).await.unwrap();
    a.import_doc_update(note, &ub).await.unwrap();

    // Union on both replicas — both distinct values present.
    for (label, e) in [("A", &a), ("B", &b)] {
        let mut tags: Vec<String> = block_prop_list(e, note, block, "tags")
            .await
            .into_iter()
            .map(|s| match s {
                PropScalar::Text(t) => t,
                other => format!("{other:?}"),
            })
            .collect();
        tags.sort();
        assert_eq!(
            tags,
            vec!["Base".to_string(), "Task".to_string(), "Urgent".to_string()],
            "{label}: concurrent AddToList must union both distinct values \
             (alongside the shared base value)"
        );
    }
}

// ---- P1.9 disjoint-twin heal carries props ----

// ⭐ Two devices each author the SAME block_id INDEPENDENTLY (disjoint Loro
// lineages via `seed_disjoint`), and each first-sets a DISTINCT scalar
// property on its own twin. When one device's snapshot is applied via the
// WS-apply path, `tombstone_duplicate_twins` keeps ONE twin (max-`TreeID`)
// and tombstones the loser — so the loser-twin's property would VANISH
// without the heal carrying it forward. The heal must read every twin's
// props in the fork BEFORE the tombstone, merge per key, and re-assert each
// onto the survivor → BOTH distinct properties survive.
#[tokio::test]
async fn disjoint_twins_each_with_distinct_property_both_survive() {
    let sdev = DeviceId::from_bytes([0x5e; 16]);
    let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
    let ddev = DeviceId::from_bytes([0x7f; 16]);
    let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
    let note = blake3_note_id("daily");

    // Disjoint lineages: server + device each author blocks A and B
    // independently (distinct TreeIDs for the same block_ids).
    seed_disjoint(&server, &device, note).await;

    // Each device first-sets a DISTINCT scalar property on block A — on its
    // OWN twin (each set mints a `props` container on a rival node).
    server
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: A_BID_BYTES,
            key: "status".into(),
            value: PropOp::SetScalar(PropScalar::Text("doing".into())),
        })
        .await
        .unwrap();
    device
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: A_BID_BYTES,
            key: "priority".into(),
            value: PropOp::SetScalar(PropScalar::Int(3)),
        })
        .await
        .unwrap();

    // Device exports a FULL SNAPSHOT; server applies it via the WS path.
    let snapshot = device.export_doc_update(note, None).await.unwrap();
    server.import_doc_update(note, &snapshot).await.unwrap();

    // BOTH the server's own property AND the device-twin's property must
    // survive on the surviving node after the tombstone.
    assert_eq!(
        block_prop_scalar(&server, note, A_BID_BYTES, "status").await,
        Some(PropScalar::Text("doing".into())),
        "the server's own property must survive the twin dedup"
    );
    assert_eq!(
        block_prop_scalar(&server, note, A_BID_BYTES, "priority").await,
        Some(PropScalar::Int(3)),
        "the tombstoned twin's property must be carried onto the survivor"
    );
}

// ⭐ Two disjoint twins each AddToList a DISTINCT value to the SAME list key.
// The heal must UNION the loser-twin's missing members onto the survivor's
// list (via per-key AddToList re-assert), never replace the winner's list
// wholesale → survivor list = [x, y] deduped.
#[tokio::test]
async fn disjoint_twins_each_add_to_same_list_key_union() {
    let sdev = DeviceId::from_bytes([0x5e; 16]);
    let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
    let ddev = DeviceId::from_bytes([0x7f; 16]);
    let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
    let note = blake3_note_id("daily");

    seed_disjoint(&server, &device, note).await;

    // Each twin adds a DISTINCT value to the SAME list key `tags` on block A
    // — on rival list containers (disjoint lineage, no shared base list).
    server
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: A_BID_BYTES,
            key: "tags".into(),
            value: PropOp::AddToList(PropScalar::Text("x".into())),
        })
        .await
        .unwrap();
    device
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: A_BID_BYTES,
            key: "tags".into(),
            value: PropOp::AddToList(PropScalar::Text("y".into())),
        })
        .await
        .unwrap();

    let snapshot = device.export_doc_update(note, None).await.unwrap();
    server.import_doc_update(note, &snapshot).await.unwrap();

    let mut tags: Vec<String> = block_prop_list(&server, note, A_BID_BYTES, "tags")
        .await
        .into_iter()
        .map(|s| match s {
            PropScalar::Text(t) => t,
            other => format!("{other:?}"),
        })
        .collect();
    tags.sort();
    assert_eq!(
        tags,
        vec!["x".to_string(), "y".to_string()],
        "the heal must UNION both twins' list members onto the survivor \
         (deduped), not replace the winner's list wholesale"
    );
}

// ---- P1.5 container-property materialization ----

// A scalar block property materializes as a `key:: value` continuation
// line AFTER the block's prose in the rendered markdown.
#[tokio::test]
async fn render_materializes_block_scalar_property() {
    let note = blake3_note_id("mat-scalar");
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    upsert_block(&engine, note, A_BID_BYTES, "Task", None).await;

    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: A_BID_BYTES,
            key: "status".into(),
            value: PropOp::SetScalar(PropScalar::Text("doing".into())),
        })
        .await
        .unwrap();

    let full = engine.render_note_full(note).await.unwrap();
    assert_eq!(
        full,
        format!(
            "- Task <!-- bid:{} -->\n  status:: doing\n",
            uuid::Uuid::from_bytes(A_BID_BYTES),
        ),
        "scalar prop renders as a continuation line after the prose"
    );
}

// A multi-value (list) property materializes as a single comma-joined
// `key:: a, b` line (the `tags::` join convention), stable-deduped.
#[tokio::test]
async fn render_materializes_block_multi_value_property() {
    let note = blake3_note_id("mat-multi");
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    upsert_block(&engine, note, A_BID_BYTES, "Task", None).await;

    for v in ["Task", "Urgent"] {
        engine
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: A_BID_BYTES,
                key: "tags".into(),
                value: PropOp::AddToList(PropScalar::Text(v.into())),
            })
            .await
            .unwrap();
    }

    let full = engine.render_note_full(note).await.unwrap();
    assert_eq!(
        full,
        format!(
            "- Task <!-- bid:{} -->\n  tags:: Task, Urgent\n",
            uuid::Uuid::from_bytes(A_BID_BYTES),
        ),
        "multi-value prop renders comma-joined in list order"
    );
}

// A page-level scalar property materializes at the body top, per the
// `split_page_properties` convention.
#[tokio::test]
async fn render_materializes_page_property() {
    let note = blake3_note_id("mat-page");
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    upsert_block(&engine, note, A_BID_BYTES, "Body", None).await;

    engine
        .record_local(OpPayload::PagePropertySet {
            note_id: note,
            key: "type".into(),
            value: PropOp::SetScalar(PropScalar::Text("Tag".into())),
        })
        .await
        .unwrap();

    let full = engine.render_note_full(note).await.unwrap();
    assert_eq!(
        full,
        format!(
            "type:: Tag\n- Body <!-- bid:{} -->\n",
            uuid::Uuid::from_bytes(A_BID_BYTES),
        ),
        "page prop renders at the body top before the bullets"
    );
}

// ⭐ REVIEW-GATE determinism test: the SAME set of property ops applied
// in DIFFERENT orders to two FRESH engines, converged via export/import,
// must render BYTE-IDENTICAL markdown. Determinism is the whole point of
// `prop_keys` + canonical formatting + stable-dedup.
#[tokio::test]
async fn render_is_byte_identical_regardless_of_prop_op_order() {
    let note = blake3_note_id("mat-determinism");

    // A shared base both replicas import, so block + list containers
    // share Loro ids (the union-merge precondition the engine relies on).
    let dev_seed = DeviceId::from_bytes([0xd0; 16]);
    let seed = LoroEngine::new(dev_seed, Arc::new(Hlc::new(dev_seed)));
    upsert_block(&seed, note, A_BID_BYTES, "Task", None).await;
    // Seed the shared "tags" list (one initial value) so concurrent
    // AddToList unions instead of minting rival containers.
    seed.record_local(OpPayload::BlockPropertySet {
        note_id: note,
        block_id: A_BID_BYTES,
        key: "tags".into(),
        value: PropOp::AddToList(PropScalar::Text("Base".into())),
    })
    .await
    .unwrap();
    let base = seed.export_doc_update(note, None).await.unwrap();

    // The SAME logical op set, in two different orders.
    let ops_order_1 = vec![
        (
            "status",
            PropOp::SetScalar(PropScalar::Text("doing".into())),
        ),
        ("priority", PropOp::SetScalar(PropScalar::Int(3))),
        ("tags", PropOp::AddToList(PropScalar::Text("Task".into()))),
        ("tags", PropOp::AddToList(PropScalar::Text("Urgent".into()))),
        ("note", PropOp::SetText("freeform".into())),
    ];
    let ops_order_2 = vec![
        ("tags", PropOp::AddToList(PropScalar::Text("Urgent".into()))),
        ("note", PropOp::SetText("freeform".into())),
        ("priority", PropOp::SetScalar(PropScalar::Int(3))),
        (
            "status",
            PropOp::SetScalar(PropScalar::Text("doing".into())),
        ),
        ("tags", PropOp::AddToList(PropScalar::Text("Task".into()))),
    ];

    async fn build(
        note: [u8; 16],
        base: &[u8],
        peer: u8,
        ops: &[(&str, PropOp)],
    ) -> LoroEngine {
        let dev = DeviceId::from_bytes([peer; 16]);
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        engine.import_doc_update(note, base).await.unwrap();
        for (key, value) in ops {
            engine
                .record_local(OpPayload::BlockPropertySet {
                    note_id: note,
                    block_id: A_BID_BYTES,
                    key: (*key).into(),
                    value: value.clone(),
                })
                .await
                .unwrap();
        }
        engine
    }

    let a = build(note, &base, 0xa1, &ops_order_1).await;
    let b = build(note, &base, 0xb2, &ops_order_2).await;

    // Converge: exchange full updates both ways.
    let ua = a.export_doc_update(note, None).await.unwrap();
    let ub = b.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &ua).await.unwrap();
    a.import_doc_update(note, &ub).await.unwrap();

    let ra = a.render_note_full(note).await.unwrap();
    let rb = b.render_note_full(note).await.unwrap();
    assert_eq!(
        ra, rb,
        "converged replicas must render byte-identical markdown \
         regardless of the order property ops were applied"
    );
    // And the rendered form must actually carry every property (guards
    // against the trivial both-empty pass).
    for needle in [
        "status:: doing",
        "priority:: 3",
        "tags:: Base, Task, Urgent",
        "note:: freeform",
    ] {
        assert!(
            ra.contains(needle),
            "converged render missing {needle}: {ra}"
        );
    }

    // Migrated-vs-unmigrated byte equality (P1.6 determinism gate): a block
    // whose `status` arrives as a TYPED scalar prop op (the unmigrated /
    // already-clean path) and a block whose `status` arrives as an in-text
    // `status:: doing` line lifted by migrate-on-apply must render
    // byte-identical markdown. Same CRDT state → same bytes, no matter how
    // the property got there.
    let note2 = blake3_note_id("mat-migrate-determinism");

    let dev_clean = DeviceId::from_bytes([0xe1; 16]);
    let clean = LoroEngine::new(dev_clean, Arc::new(Hlc::new(dev_clean)));
    upsert_block(&clean, note2, A_BID_BYTES, "buy milk", None).await;
    clean
        .record_local(OpPayload::BlockPropertySet {
            note_id: note2,
            block_id: A_BID_BYTES,
            key: "status".into(),
            value: PropOp::SetScalar(PropScalar::Text("doing".into())),
        })
        .await
        .unwrap();

    let dev_mig = DeviceId::from_bytes([0xe2; 16]);
    let migrating = LoroEngine::new_migrating(dev_mig, Arc::new(Hlc::new(dev_mig)));
    migrating
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note2,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "buy milk\nstatus:: doing".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    assert_eq!(
        clean.render_note_full(note2).await.unwrap(),
        migrating.render_note_full(note2).await.unwrap(),
        "a typed-prop-op block and a migrate-lifted in-text block render \
         byte-identical markdown"
    );
    // The migrating engine must have ACTUALLY lifted the property into a
    // typed container (prose-only text_seq), not merely left it in-text and
    // coincidentally rendered the same bytes.
    assert_eq!(
        block_text(&migrating, note2, A_BID_BYTES).await.as_deref(),
        Some("buy milk"),
        "migrate lifted the property — block text is prose-only"
    );
    assert_eq!(
        block_prop_scalar(&migrating, note2, A_BID_BYTES, "status").await,
        Some(PropScalar::Text("doing".into())),
        "migrate produced a typed container value"
    );
}

// A legacy `key:: value` line embedded in a block's TEXT (the pre-P1.6
// form, before migrate-on-write lifts it into `props`) round-trips
// unchanged — container props and legacy-in-text props are DISJOINT at
// this stage, so the materializer must NOT double-emit.
#[tokio::test]
async fn legacy_in_text_property_round_trips_without_double_emit() {
    let note = blake3_note_id("mat-legacy");
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    let bid = uuid::Uuid::from_bytes(A_BID_BYTES);
    // The legacy form: the property lives INSIDE the block text (folded
    // continuation), with NO container `props` set.
    let content = format!("- Task <!-- bid:{} -->\n  status:: doing\n", bid);

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("legacy".into()),
            title: "legacy".into(),
            content: content.clone(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    let full = engine.render_note_full(note).await.unwrap();
    assert_eq!(
        full, content,
        "legacy in-text property round-trips unchanged — no container, no double-emit"
    );
}

// A4 — render-time dedup: when a block carries BOTH a legacy in-text
// `status:: a` line (flag OFF, never lifted) AND a container `status`
// property, the materializer must emit the property ONCE, with the
// CONTAINER value winning. Guards the un-migrated legacy/dual-write dup.
#[tokio::test]
async fn render_dedups_intext_property_when_container_prop_exists() {
    let note = blake3_note_id("mat-dedup");
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    let bid = uuid::Uuid::from_bytes(A_BID_BYTES);
    // Legacy in-text `status:: a` lands in text_seq and is NOT lifted
    // (non-migrating engine).
    let content = format!("- Task <!-- bid:{} -->\n  status:: a\n", bid);
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("dedup".into()),
            title: "dedup".into(),
            content,
            created_at_millis: 1,
        })
        .await
        .unwrap();
    // A container `status` property for the SAME key, different value.
    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: A_BID_BYTES,
            key: "status".into(),
            value: PropOp::SetScalar(PropScalar::Text("b".into())),
        })
        .await
        .unwrap();

    let full = engine.render_note_full(note).await.unwrap();
    assert_eq!(
        full,
        format!("- Task <!-- bid:{} -->\n  status:: b\n", bid),
        "container prop wins; the duplicate in-text status line is dropped at render"
    );
    assert_eq!(
        full.matches("status::").count(),
        1,
        "exactly one status line"
    );
}

// A4 case-fold: the in-text key is compared case-insensitively to the
// container keys, so an in-text `status:: a` is still deduped when the
// container key was set with different case (`Status`). Container wins.
#[tokio::test]
async fn render_dedups_intext_property_case_insensitively() {
    let note = blake3_note_id("mat-dedup-case");
    let dev = test_device();
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    let bid = uuid::Uuid::from_bytes(A_BID_BYTES);
    let content = format!("- Task <!-- bid:{} -->\n  status:: a\n", bid);
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("dedup-case".into()),
            title: "dedup-case".into(),
            content,
            created_at_millis: 1,
        })
        .await
        .unwrap();
    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: A_BID_BYTES,
            key: "Status".into(),
            value: PropOp::SetScalar(PropScalar::Text("b".into())),
        })
        .await
        .unwrap();

    let full = engine.render_note_full(note).await.unwrap();
    assert!(
        !full.contains("status:: a"),
        "the lowercase in-text dup is dropped despite the container key's case: {full:?}"
    );
    assert!(
        full.contains("Status:: b"),
        "the container value (verbatim key) is kept: {full:?}"
    );
}

// P1.6 — migrate-on-apply. With the flag ON, a `BlockUpsert` whose incoming
// text carries a SOLELY `key:: value` continuation line lifts it OUT of the
// prose into the typed `props`/`prop_keys` container: the block's
// `text_seq` becomes prose-only and the property reads back as a typed
// scalar. Re-applying the SAME (already-clean) BlockUpsert is a no-op (the
// prose is already stripped → nothing to lift → no double-set).
#[tokio::test]
async fn migrate_on_apply_lifts_intext_prop_and_is_idempotent() {
    let note = blake3_note_id("migrate-lift");
    let dev = test_device();
    let engine = LoroEngine::new_migrating(dev, Arc::new(Hlc::new(dev)));

    // A BlockUpsert carrying an in-text property (the un-migrated shape a
    // mixed-fleet old peer authors): prose line + a solely-`key:: value`
    // continuation line, joined by '\n' the way `parse_note` folds it.
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "buy milk\nstatus:: doing".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    // The property was LIFTED: prose-only text_seq + a typed container.
    assert_eq!(
        block_text(&engine, note, A_BID_BYTES).await.as_deref(),
        Some("buy milk"),
        "migrate strips the property line from prose"
    );
    assert_eq!(
        block_prop_scalar(&engine, note, A_BID_BYTES, "status").await,
        Some(PropScalar::Text("doing".into())),
        "migrate folds the stripped line into the typed props container"
    );
    // The rendered VIEW still emits the property as a `key:: value` line
    // (dual-read: an old reader still SEES it).
    let rendered = engine.render_note(note).await.unwrap();
    assert!(
        rendered.contains("status:: doing"),
        "rendered view re-emits the lifted property, got: {rendered:?}"
    );

    // Idempotent: re-applying the SAME logical block (now already clean
    // prose, no in-text property) finds nothing to lift and leaves the
    // container untouched (one value, not a re-set duplicate).
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "buy milk".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    assert_eq!(
        block_text(&engine, note, A_BID_BYTES).await.as_deref(),
        Some("buy milk"),
        "re-apply of clean prose leaves text_seq prose-only"
    );
    assert_eq!(
        block_prop_scalar(&engine, note, A_BID_BYTES, "status").await,
        Some(PropScalar::Text("doing".into())),
        "re-apply does not disturb the already-lifted property"
    );
}

// P1.6 — `tags::` routes to AddToList (a list container), NOT a scalar, so a
// migrated tags line union-merges across replicas instead of LWW-clobbering.
#[tokio::test]
async fn migrate_on_apply_routes_tags_to_list() {
    let note = blake3_note_id("migrate-tags");
    let dev = test_device();
    let engine = LoroEngine::new_migrating(dev, Arc::new(Hlc::new(dev)));

    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "a task\ntags:: urgent".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    assert_eq!(
        block_text(&engine, note, A_BID_BYTES).await.as_deref(),
        Some("a task"),
        "tags line stripped from prose"
    );
    assert_eq!(
        block_prop_list(&engine, note, A_BID_BYTES, "tags").await,
        vec![PropScalar::Text("urgent".into())],
        "tags:: routes to a list container (AddToList), not a scalar"
    );
    // It is NOT a scalar.
    assert_eq!(
        block_prop_scalar(&engine, note, A_BID_BYTES, "tags").await,
        None,
        "tags must be a list, never a scalar register"
    );
}

// P1.6 mixed-fleet — an OLD peer that can't read containers re-injects the
// property as an in-text `key:: value` line on a NoteUpsert / BlockUpsert.
// With migrate ON the line is lifted back into the container; the rendered
// view emits the property exactly ONCE (no double-emit from a container
// value PLUS a re-injected in-text line). Mirrors
// `legacy_in_text_property_round_trips_without_double_emit`.
#[tokio::test]
async fn mixed_fleet_old_peer_reinjects_no_double_emit() {
    let note = blake3_note_id("migrate-mixed-fleet");
    let dev = test_device();
    let engine = LoroEngine::new_migrating(dev, Arc::new(Hlc::new(dev)));

    // First apply lifts the property into the container.
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "buy milk\nstatus:: doing".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    // An OLD peer re-broadcasts the block with the property STILL in-text
    // (it never learned to read the container). Migrate lifts it again →
    // prose-only + one container value, never two.
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "buy milk\nstatus:: doing".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    assert_eq!(
        block_text(&engine, note, A_BID_BYTES).await.as_deref(),
        Some("buy milk"),
        "the re-injected in-text property is lifted again, not re-embedded"
    );
    let rendered = engine.render_note(note).await.unwrap();
    // Exactly one `status:: doing` line — the container value emitted once,
    // NOT a container value plus a lingering in-text line.
    assert_eq!(
        rendered.matches("status:: doing").count(),
        1,
        "no double-emit: property renders exactly once, got: {rendered:?}"
    );
}

// P1.6 — two devices on a SHARED lineage each migrate the SAME block's
// in-text `status::` property concurrently. Because migrate is
// deterministic-shape (same incoming text + same classification → same
// prose-strip + same scalar set), both replicas converge to identical props
// after exchange (same-key scalar collision = LWW, identical winner).
#[tokio::test]
async fn concurrent_migrate_same_block_converges() {
    let note = blake3_note_id("migrate-concurrent");

    // Shared base: a block with prose only, on a shared Loro lineage so the
    // `props` map container is in shared history before peers diverge (the
    // eager-seed precondition from P1.9b).
    let dev_seed = DeviceId::from_bytes([0xc0; 16]);
    let seed = LoroEngine::new(dev_seed, Arc::new(Hlc::new(dev_seed)));
    upsert_block(&seed, note, A_BID_BYTES, "buy milk", None).await;
    let base = seed.export_doc_update(note, None).await.unwrap();

    // Two migrating replicas import the shared base.
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let a = LoroEngine::new_migrating(dev_a, Arc::new(Hlc::new(dev_a)));
    a.import_doc_update(note, &base).await.unwrap();
    let dev_b = DeviceId::from_bytes([0xb2; 16]);
    let b = LoroEngine::new_migrating(dev_b, Arc::new(Hlc::new(dev_b)));
    b.import_doc_update(note, &base).await.unwrap();

    // Each concurrently applies a BlockUpsert that carries the SAME in-text
    // property — both migrate it identically.
    let a_vv = a.doc_version(note).await;
    let b_vv = b.doc_version(note).await;
    for engine in [&a, &b] {
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "buy milk\nstatus:: doing".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
    }

    // Exchange concurrent deltas both ways.
    let a_delta = a.export_doc_update(note, a_vv.as_deref()).await.unwrap();
    let b_delta = b.export_doc_update(note, b_vv.as_deref()).await.unwrap();
    b.import_doc_update(note, &a_delta).await.unwrap();
    a.import_doc_update(note, &b_delta).await.unwrap();

    // Both replicas converge: identical typed props AND identical rendered
    // markdown.
    assert_eq!(
        block_prop_scalar(&a, note, A_BID_BYTES, "status").await,
        block_prop_scalar(&b, note, A_BID_BYTES, "status").await,
        "concurrent migrators converge on the same scalar (LWW winner)"
    );
    assert_eq!(
        block_prop_scalar(&a, note, A_BID_BYTES, "status").await,
        Some(PropScalar::Text("doing".into())),
        "the converged scalar is the migrated value"
    );
    let ra = a.render_note_full(note).await.unwrap();
    let rb = b.render_note_full(note).await.unwrap();
    assert_eq!(
        ra, rb,
        "concurrent migrators render byte-identical markdown"
    );
}
