//! Phase 1 (multi-device presence foundation): stable cursors + EphemeralStore
//! presence, the primitives the FFI will expose. TDD:
//!   - a loro `Cursor` minted on a block's text is OP-ANCHORED — it survives a
//!     concurrent insert BEFORE it (shifts to the new offset, not the stale
//!     index). This is what lets a remote caret stay correct through the
//!     other peer's typing.
//!   - presence (EphemeralStore) round-trips between two engines and expires.

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

#[tokio::test]
async fn presence_round_trips_between_engines() {
    let a = engine(0x0a);
    let b = engine(0x0b);

    // A publishes its presence (opaque bytes — the FFI's encoded cursor+meta).
    let payload = b"a@block5".to_vec();
    let delta = a.set_local_presence("device-a".into(), payload.clone());
    assert!(
        !delta.is_empty(),
        "set_local_presence returns a non-empty broadcast delta"
    );

    // Before applying, B sees no peers.
    assert!(b.presence_peers().is_empty(), "B starts with no peer presence");

    // B applies A's presence delta → B now sees A's cursor.
    assert!(b.apply_presence(&delta), "B applies A's presence delta");
    assert_eq!(
        b.presence_peers(),
        vec![("device-a".to_string(), payload)],
        "B reads A's exact presence value under A's key"
    );
}

#[tokio::test]
async fn presence_aggregates_multiple_peers_and_lww_updates() {
    let hub = engine(0x01);
    let a = engine(0x0a);
    let c = engine(0x0c);

    hub.apply_presence(&a.set_local_presence("a".into(), b"a1".to_vec()));
    hub.apply_presence(&c.set_local_presence("c".into(), b"c1".to_vec()));

    let mut peers = hub.presence_peers();
    peers.sort();
    assert_eq!(
        peers,
        vec![
            ("a".to_string(), b"a1".to_vec()),
            ("c".to_string(), b"c1".to_vec())
        ],
        "the hub aggregates presence from both peers"
    );

    // A moves its cursor → last write wins for A's key, C untouched.
    hub.apply_presence(&a.set_local_presence("a".into(), b"a2".to_vec()));
    let mut peers = hub.presence_peers();
    peers.sort();
    assert_eq!(
        peers,
        vec![
            ("a".to_string(), b"a2".to_vec()),
            ("c".to_string(), b"c1".to_vec())
        ],
        "A's presence updates in place (LWW); C's stays"
    );
}
