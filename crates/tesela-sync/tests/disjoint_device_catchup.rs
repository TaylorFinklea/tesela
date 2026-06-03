//! Reproduction: a device that authored a note WITHOUT the server's base
//! (e.g. an edit made in mock mode, or before bootstrap) is on a DISJOINT
//! Loro lineage. When it later "catches up" by importing the server's
//! snapshot (the exact thing `bootstrapNoteIfNeeded` does), does it adopt the
//! server's authoritative text, or stay stuck on its disjoint copy?
//!
//! This is the suspected root cause of "web edits don't reach devices / web
//! gets clobbered" on devices that didn't cleanly bootstrap: the catch-up
//! raw-imports the server snapshot, which UNIONS the two disjoint lineages
//! into same-bid twins, then the deterministic min-`TreeID` dedup picks a
//! survivor by peer id (NOT by authority/recency) — so a device whose peer id
//! sorts below the server's keeps its OWN stale twin and never converges.

use std::sync::Arc;

use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, SyncEngine};

fn engine(b: u8) -> LoroEngine {
    let d = DeviceId::from_bytes([b; 16]);
    LoroEngine::new(d, Arc::new(Hlc::new(d)))
}

fn note_id(slug: &str) -> [u8; 16] {
    let h = blake3::hash(slug.as_bytes());
    let mut o = [0u8; 16];
    o.copy_from_slice(&h.as_bytes()[..16]);
    o
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
    // the server peer (0x5e), so min-TreeID dedup favors the device's twin.
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
