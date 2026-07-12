//! Reproduction: a device that authored a note WITHOUT the server's base
//! (e.g. an edit made in mock mode, or before bootstrap) is on a DISJOINT
//! Loro lineage. When it later "catches up" by importing the server's
//! snapshot (the exact thing `bootstrapNoteIfNeeded` does), does it adopt the
//! server's authoritative text, or stay stuck on its disjoint copy?
//!
//! This is the suspected root cause of "web edits don't reach devices / web
//! gets clobbered" on devices that didn't cleanly bootstrap: the catch-up
//! raw-imports the server snapshot, which UNIONS the two disjoint lineages
//! into same-bid twins, then the deterministic global-max `TreeID` dedup
//! (tesela-fte) picks a survivor by peer id (NOT by authority/recency). Here
//! the server peer (0x5e) OUTRANKS the device (0x11), so the server's
//! authoritative text wins and the device converges to it.

use std::sync::Arc;

use loro::{LoroDoc, TreeParentId};
use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, SyncEngine};

fn engine(b: u8) -> LoroEngine {
    let d = DeviceId::from_bytes([b; 16]);
    LoroEngine::new(d, Arc::new(Hlc::new(d)))
}

fn note_id(slug: &str) -> [u8; 16] {
    tesela_core::stable_uuid_from_slug(slug)
}

/// Count the LIVE (non-tombstoned) nodes in a doc snapshot's `blocks` tree
/// whose `block_id` meta equals `target_hex`. Used to prove a re-base
/// tombstoned the device's disjoint twin (so exactly one live node remains).
fn live_nodes_for_block(snapshot: &[u8], target_hex: &str) -> usize {
    let doc = LoroDoc::new();
    doc.import(snapshot).unwrap();
    let tree = doc.get_tree("blocks");
    let mut count = 0;
    for node in tree.children(TreeParentId::Root).unwrap_or_default() {
        if matches!(tree.is_node_deleted(&node), Ok(true)) {
            continue;
        }
        let meta = tree.get_meta(node).unwrap();
        if let Some(v) = meta.get("block_id") {
            if let Ok(val) = v.into_value() {
                if let Ok(s) = val.into_string() {
                    if *s == target_hex {
                        count += 1;
                    }
                }
            }
        }
    }
    count
}

#[tokio::test]
async fn disjoint_device_catches_up_to_server_lineage() {
    let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let note = note_id("2026-06-03");

    // Server authors the daily (lineage S).
    let server = engine(0x5e);
    server
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("daily".into()),
            title: "Daily".into(),
            content: format!("- server base <!-- bid:{bid} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // Device authors the SAME daily INDEPENDENTLY (mock-mode style — no server
    // base imported first) → disjoint lineage D. Device peer (0x11) sorts BELOW
    // the server peer (0x5e), so global-max `TreeID` dedup (tesela-fte) keeps
    // the SERVER's twin — the device converges to the server's authoritative text.
    let device = engine(0x11);
    device
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("daily".into()),
            title: "Daily".into(),
            content: format!("- device mock <!-- bid:{bid} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // Web edits the block on the SERVER → "web edit" (the value devices should
    // converge to).
    server
        .record_local(OpPayload::BlockUpsert {
            block_id: [0x0a; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "web edit".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    // Device CATCHES UP: import the server's full snapshot, exactly as
    // `bootstrapNoteIfNeeded` → `importNoteSnapshot` → `import_doc_update`.
    let snap = server.export_doc_update(note, None).await.unwrap();
    device.import_doc_update(note, &snap).await.unwrap();

    let rendered = device.render_note(note).await.unwrap();
    assert!(
        rendered.contains("web edit"),
        "device must adopt the server's authoritative text after catch-up; got: {rendered:?}"
    );
    assert!(
        !rendered.contains("device mock"),
        "the device's disjoint text must not survive catch-up; got: {rendered:?}"
    );
}

#[tokio::test]
async fn disjoint_device_live_delta_does_not_converge() {
    // Same disjoint setup, but the device receives the web edit as the LIVE
    // DELTA `upsert_blocks` actually broadcasts (export since the pre-edit VV),
    // NOT a full snapshot. The delta's ops modify the SERVER's tree node for
    // the block — a container created in the server's base, which a disjoint
    // device never imported — so Loro can't place them. This is why live
    // web->device fails while a hard refresh (full-snapshot catch-up) works.
    let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let note = note_id("2026-06-03");

    let server = engine(0x5e);
    server
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("daily".into()),
            title: "Daily".into(),
            content: format!("- server base <!-- bid:{bid} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    let device = engine(0x11);
    device
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("daily".into()),
            title: "Daily".into(),
            content: format!("- device mock <!-- bid:{bid} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // Capture the server's VV BEFORE the web edit (what upsert_blocks does).
    let pre_vv = server.doc_version(note).await;
    server
        .record_local(OpPayload::BlockUpsert {
            block_id: [0x0a; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "web edit".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    // The LIVE delta the server broadcasts to devices.
    let delta = server
        .export_doc_update(note, pre_vv.as_deref())
        .await
        .unwrap();
    device.import_doc_update(note, &delta).await.unwrap();

    // INVARIANT (the bug's root): a live delta CANNOT converge a disjoint
    // device — the server-lineage ops have no home on the device's tree, so
    // the device stays on its own stale text. This is why live web->device
    // silently fails while a hard refresh works.
    let after_delta = device.render_note(note).await.unwrap();
    assert!(
        !after_delta.contains("web edit"),
        "documents the invariant: a live delta should NOT converge a disjoint device; got: {after_delta:?}"
    );

    // THE HEAL the client must fall back to: a full-snapshot catch-up DOES
    // converge it (same as test 1). The fix makes the client trigger this when
    // a live delta fails to apply.
    let snap = server.export_doc_update(note, None).await.unwrap();
    device.import_doc_update(note, &snap).await.unwrap();
    let after_catchup = device.render_note(note).await.unwrap();
    assert!(
        after_catchup.contains("web edit"),
        "a full-snapshot catch-up must heal the disjoint device; got: {after_catchup:?}"
    );
}

#[tokio::test]
async fn disjoint_device_authoritative_rebase_then_converges() {
    // The fix: a disjoint device catches up via `import_authoritative_snapshot`
    // (server-wins re-base), which TOMBSTONES the device's stale twin and KEEPS
    // the server's snapshot-origin node — so the device truly RE-BASES onto the
    // server's lineage instead of staying on its own. Proof has three parts:
    //   (a) the device renders the server's "web edit";
    //   (b) the device's blocks tree has exactly ONE live node for the bid (the
    //       server's — the device's disjoint twin was tombstoned);
    //   (c) a SUBSEQUENT device edit + a CONCURRENT web edit MERGE via the
    //       shared LoroText (both contributions survive), NOT a clobber.
    let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let bid_hex = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
    let note = note_id("2026-06-03");

    // Server authors the daily (lineage S).
    let server = engine(0x5e);
    server
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("daily".into()),
            title: "Daily".into(),
            content: format!("- server base <!-- bid:{bid} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // Device authors the SAME daily INDEPENDENTLY (disjoint lineage D).
    let device = engine(0x11);
    device
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("daily".into()),
            title: "Daily".into(),
            content: format!("- device mock <!-- bid:{bid} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // Web edits the block on the SERVER → "web edit".
    server
        .record_local(OpPayload::BlockUpsert {
            block_id: [0x0a; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "web edit".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    // Device CATCHES UP via the AUTHORITATIVE re-base (server-wins).
    let snap = server.export_doc_update(note, None).await.unwrap();
    device
        .import_authoritative_snapshot(note, &snap)
        .await
        .unwrap();

    // (a) Re-base proof — the device adopts the server's authoritative text.
    let rendered = device.render_note(note).await.unwrap();
    assert!(
        rendered.contains("web edit"),
        "device must adopt the server's text after authoritative re-base; got: {rendered:?}"
    );
    assert!(
        !rendered.contains("device mock"),
        "the device's disjoint text must not survive re-base; got: {rendered:?}"
    );

    // (b) Re-base proof — exactly ONE live node for the bid (the server's; the
    // device's disjoint twin was tombstoned, not merely render-deduped).
    let device_snap = device.export_doc_update(note, None).await.unwrap();
    assert_eq!(
        live_nodes_for_block(&device_snap, bid_hex),
        1,
        "the device's disjoint twin must be tombstoned, leaving one live node"
    );

    // (c) Re-base proof — the device now shares the server's lineage, so a
    // subsequent device edit and a concurrent web edit to the SAME block MERGE
    // through the shared LoroText instead of one clobbering the other.
    device
        .record_local(OpPayload::BlockUpsert {
            block_id: [0x0a; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "web edit + device".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    server
        .record_local(OpPayload::BlockUpsert {
            block_id: [0x0a; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "WEB! web edit".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    // Cross-import the two concurrent deltas (a relay round-trip). Each side
    // exports the updates the OTHER is missing — i.e. since the RECEIVER's
    // current version vector (the `produce_relay_updates`/`sync_one_way`
    // contract). The server has never seen the device's lineage (it never
    // imported the device's re-base), so the device must ship its full delta
    // since the SERVER's vv — otherwise its text edit, which causally depends
    // on the device-only re-base op, lands PENDING.
    let server_vv = server.doc_version(note).await;
    let device_vv = device.doc_version(note).await;
    let device_delta = device
        .export_doc_update(note, server_vv.as_deref())
        .await
        .unwrap();
    let server_delta = server
        .export_doc_update(note, device_vv.as_deref())
        .await
        .unwrap();
    server.import_doc_update(note, &device_delta).await.unwrap();
    device.import_doc_update(note, &server_delta).await.unwrap();

    let server_final = server.render_note(note).await.unwrap();
    let device_final = device.render_note(note).await.unwrap();
    // BOTH contributions survive on BOTH sides (shared lineage → LoroText
    // merge), and the two sides converge — not a clobber where one edit wins.
    assert!(
        server_final.contains("device") && server_final.contains("WEB!"),
        "both concurrent edits must merge on the server side; got: {server_final:?}"
    );
    assert!(
        device_final.contains("device") && device_final.contains("WEB!"),
        "both concurrent edits must merge on the device side; got: {device_final:?}"
    );
    assert_eq!(
        server_final, device_final,
        "the re-based device and the server must converge to the same text"
    );
}
