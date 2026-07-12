//! Phase 1 (multi-device presence foundation): stable cursors, the primitive
//! the FFI exposes for placing a remote caret. TDD:
//!   - a loro `Cursor` minted on a block's text is OP-ANCHORED — it survives a
//!     concurrent insert BEFORE it (shifts to the new offset, not the stale
//!     index). This is what lets a remote caret stay correct through the
//!     other peer's typing.
//!
//! The engine's EphemeralStore-backed presence surface (set/apply/peers) was
//! deleted per ADR-8 (decisions.md 2026-07-01, tesela-engc.7) — no production
//! caller on any platform; presence transport is CF-DO-WS
//! (`presence_relay.rs` / iOS `PresenceRelaySocket`).

use std::sync::Arc;

use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, SyncEngine};

fn engine(b: u8) -> LoroEngine {
    let d = DeviceId::from_bytes([b; 16]);
    LoroEngine::new(d, Arc::new(Hlc::new(d)))
}

fn note_id(slug: &str) -> [u8; 16] {
    tesela_core::stable_uuid_from_slug(slug)
}

/// Seed a note with a single block carrying `text` under the given bid.
async fn seed_block(eng: &LoroEngine, note: [u8; 16], bid: [u8; 16], text: &str) {
    eng.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("daily".into()),
        title: "Daily".into(),
        content: "- seed\n".into(),
        created_at_millis: 1,
    })
    .await
    .unwrap();
    eng.record_local(OpPayload::BlockUpsert {
        block_id: bid,
        note_id: note,
        parent_block_id: None,
        order_key: "00000000".into(),
        indent_level: 0,
        text: text.into(),
        after_block_id: None,
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn block_cursor_survives_concurrent_insert_before() {
    let eng = engine(0x01);
    let note = note_id("2026-06-27");
    let bid = [0xab; 16];
    seed_block(&eng, note, bid, "hello").await;

    // Mint a cursor at the end of "hello" (utf16 offset 5).
    let cur = eng
        .mint_block_cursor(note, bid, 5)
        .await
        .expect("mint a cursor on an existing block");
    assert_eq!(
        eng.resolve_block_cursor(note, &cur).await,
        Some(5),
        "a freshly minted cursor resolves to where it was minted"
    );

    // Insert "XYZ" BEFORE the cursor (at offset 0) → text becomes "XYZhello".
    eng.splice_block_text(note, bid, 0, 0, "XYZ").await.unwrap();

    // The SAME cursor must now resolve to 8 — it FOLLOWED the concurrent
    // insert-before, instead of staying at the stale index 5.
    assert_eq!(
        eng.resolve_block_cursor(note, &cur).await,
        Some(8),
        "an op-anchored cursor follows a concurrent insert-before"
    );
}

#[tokio::test]
async fn cursor_resolves_across_engines_on_shared_doc() {
    // The real cross-device case: A's caret cursor bytes must resolve to the
    // same offset on B, which shares A's doc lineage (via relay/bootstrap).
    let a = engine(0x0a);
    let b = engine(0x0b);
    let note = note_id("2026-06-27-x");
    let bid = [0xcd; 16];
    seed_block(&a, note, bid, "hello world").await;

    // B imports A's doc — same `text_seq` container id on both sides.
    let snap = a.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &snap).await.unwrap();

    // A mints a caret at offset 6 (start of "world") and ships the bytes.
    let cur = a.mint_block_cursor(note, bid, 6).await.expect("A mints a cursor");
    assert_eq!(
        b.resolve_block_cursor(note, &cur).await,
        Some(6),
        "A's encoded cursor resolves to the same offset on B's shared-lineage copy"
    );
}
