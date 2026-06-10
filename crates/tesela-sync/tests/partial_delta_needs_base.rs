//! Delivery-layer invariant: the server's live WS delta is exported as
//! `export_doc_update(note, Some(pre_vv))` — a PARTIAL update relative to the
//! note's version BEFORE the edit. A receiving device that has NOT first
//! acquired that base (empty doc) cannot apply the partial update — Loro
//! buffers it as pending and it never materializes. This is WHY a device must
//! be bootstrapped (VV catch-up / snapshot import) on note-open/connect, not
//! merely fed live deltas. Documents the requirement the 2026-05-31 delivery
//! redesign is built on.

use std::sync::Arc;

use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, SyncEngine};

const ALPHA_BID: [u8; 16] = [0x01; 16];

fn engine(b: u8) -> LoroEngine {
    let d = DeviceId::from_bytes([b; 16]);
    LoroEngine::new(d, Arc::new(Hlc::new(d)))
}

#[tokio::test]
async fn partial_delta_into_empty_doc_does_not_apply() {
    let note_id = [0xCD; 16];
    let server = engine(0x11);

    // Seed a base, capture the pre-edit version, then make a live edit.
    server
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("note".into()),
            title: "Note".into(),
            content: "- alpha <!-- bid:01010101-0101-0101-0101-010101010101 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .expect("seed");
    let pre_vv = server.doc_version(note_id).await.expect("pre_vv");
    server
        .record_local(OpPayload::BlockUpsert {
            block_id: ALPHA_BID,
            note_id,
            parent_block_id: None,
            order_key: "a0".into(),
            indent_level: 0,
            text: "alpha EDITED".into(),
            after_block_id: None,
        })
        .await
        .expect("edit");

    // The live WS delta the server would push: ops since pre_vv only.
    let partial = server
        .export_doc_update(note_id, Some(&pre_vv))
        .await
        .expect("partial export");

    // A fresh device with NO base imports the partial delta.
    let device = engine(0x22);
    let applied = device.apply_relay_updates(&[(note_id, partial.clone())]).await;

    // The edit must NOT be visible — the partial update is missing its base
    // dependency, so it can't materialize. This is the bug behind
    // "iOS never updated with web edits".
    let rendered = device.render_note(note_id).await.unwrap_or_default();
    eprintln!("applied={applied:?} rendered={rendered:?}");
    assert!(
        !rendered.contains("alpha EDITED"),
        "partial delta should NOT apply without the base; got: {rendered:?}"
    );

    // Now give the device the base FIRST (what bootstrap-on-open does), then
    // re-deliver the same partial delta — it must now apply.
    let base_snapshot = {
        let s2 = engine(0x11);
        s2.record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("note".into()),
            title: "Note".into(),
            content: "- alpha <!-- bid:01010101-0101-0101-0101-010101010101 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .expect("reseed");
        // Same device id 0x11 → same peer → identical base history as `server`.
        s2.export_doc_update(note_id, None).await.expect("base snapshot")
    };
    let device2 = engine(0x33);
    device2
        .apply_relay_updates(&[(note_id, base_snapshot)])
        .await;
    device2.apply_relay_updates(&[(note_id, partial)]).await;
    let rendered2 = device2.render_note(note_id).await.unwrap_or_default();
    eprintln!("after base: rendered2={rendered2:?}");
    assert!(
        rendered2.contains("alpha EDITED"),
        "with the base first, the partial delta applies; got: {rendered2:?}"
    );
}
