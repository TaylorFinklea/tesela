//! Catch-up invariant (2026-06-02 iOS engine-render fix): importing a server
//! snapshot into a RESIDENT doc that holds a LOCAL-ONLY block MERGES — it keeps
//! BOTH the local-only block and the server's blocks. This is what makes the
//! iOS "catch-up on open" safe: `bootstrapNoteIfNeeded` re-fetches the full
//! server snapshot and imports it into the already-resident daily, and that
//! must NOT clobber an iOS-authored block the server hasn't seen yet.
//!
//! Mirrors `import_doc_update`/`apply_relay_updates` (the path the FFI
//! `importNoteSnapshot` drives) — a full Loro snapshot import is commutative +
//! idempotent, so the resident doc unions the two histories.

use std::sync::Arc;

use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, SyncEngine};

// Base block bid (authored inline in the NoteUpsert content): 0a0a…
const SERVER_BID: [u8; 16] = [0x0C; 16];
const LOCAL_BID: [u8; 16] = [0x0B; 16];

fn engine(b: u8) -> LoroEngine {
    let d = DeviceId::from_bytes([b; 16]);
    LoroEngine::new(d, Arc::new(Hlc::new(d)))
}

#[tokio::test]
async fn snapshot_import_into_resident_doc_keeps_local_only_block() {
    let note_id = [0xEE; 16];

    // --- Shared base (both sides start from the same note doc). ---
    // The server (device 0x11) authors the note with one block ("server base").
    let server = engine(0x11);
    server
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("daily".into()),
            title: "Daily".into(),
            content: "- server base <!-- bid:0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a -->\n"
                .into(),
            created_at_millis: 1,
        })
        .await
        .expect("server base");
    let base_snapshot = server
        .export_doc_update(note_id, None)
        .await
        .expect("base snapshot");

    // The device (0x22) imports the base so the doc is RESIDENT, then authors a
    // LOCAL-ONLY block the server has never seen ("ios local block").
    let device = engine(0x22);
    device
        .apply_relay_updates(&[(note_id, base_snapshot.clone())])
        .await;
    assert!(
        device.doc_version(note_id).await.is_some(),
        "device doc must be resident after base import"
    );
    device
        .record_local(OpPayload::BlockUpsert {
            block_id: LOCAL_BID,
            note_id,
            parent_block_id: None,
            order_key: "z9".into(),
            indent_level: 0,
            text: "ios local block".into(),
            after_block_id: None,
        })
        .await
        .expect("local-only block");

    // --- Concurrently, the server adds a NEW block (the "web-authored new
    // block" of the bug) and exports a fresh FULL snapshot. ---
    server
        .record_local(OpPayload::BlockUpsert {
            block_id: SERVER_BID,
            note_id,
            parent_block_id: None,
            order_key: "m5".into(),
            indent_level: 0,
            text: "web new block".into(),
            after_block_id: None,
        })
        .await
        .expect("server new block");
    let server_snapshot = server
        .export_doc_update(note_id, None)
        .await
        .expect("server snapshot");

    // --- Catch-up: import the server's full snapshot into the RESIDENT device
    // doc (what iOS `importNoteSnapshot` does on open). MERGE, not clobber. ---
    LoroEngine::import_doc_update(&device, note_id, &server_snapshot)
        .await
        .expect("snapshot merge import");

    let rendered = device.render_note(note_id).await.unwrap_or_default();
    eprintln!("merged render = {rendered:?}");

    // BOTH must survive the merge.
    assert!(
        rendered.contains("ios local block"),
        "local-only block must survive snapshot catch-up; got: {rendered:?}"
    );
    assert!(
        rendered.contains("web new block"),
        "server's new block must appear after catch-up; got: {rendered:?}"
    );
    // The shared base block is still there too.
    assert!(
        rendered.contains("server base"),
        "base block must remain; got: {rendered:?}"
    );
}
