//! Deterministic repro of the concurrent-edit CLOBBER (2026-06-02).
//!
//! Real-world mapping (the confirmed live bug):
//! - A note has two blocks, alpha + beta, both stamped with explicit bids.
//! - A PEER (Roshar) edits beta concurrently; that edit lands on the
//!   AUTHORITATIVE server engine first (BlockUpsert beta -> "beta EDITED
//!   BY PEER"). The server's on-disk `<slug>.md` now reflects the peer
//!   edit.
//! - A STALE web client — whose in-memory view of beta is still the OLD
//!   text because the peer edit hasn't merged into it yet — saves via
//!   PUT /notes/{id} with the FULL note body: it changed alpha
//!   ("alpha CHANGED") but its beta is STALE ("beta").
//! - The server's `record_sync_update` does:
//!       old = parse_note(server_current_file)   // has "beta EDITED BY PEER"
//!       new = parse_note(web_put_body)          // has stale "beta"
//!       ops = diff_note_trees_with_options(old, new, emit_deletes:false)
//!       for op in ops { record_local(op) }
//!   The whole-body diff sees beta's text differ (server "beta EDITED BY
//!   PEER" vs PUT "beta") and emits a BlockUpsert reverting beta to the
//!   STALE text — clobbering the peer's concurrent edit. emit_deletes:false
//!   stops deletions but NOT stale-text re-assertion.
//!
//! `whole_body_diff_clobbers_concurrent_peer_edit` reproduces the loss.
//! `block_granular_write_preserves_both_edits` proves the fix shape: when
//! the client sends ONLY a BlockUpsert for the block it actually edited
//! (alpha), it can never re-assert stale text for beta, so both edits
//! survive.

use std::sync::Arc;

use tesela_core::note_tree::parse_note;
use tesela_sync::diff::{diff_note_trees_with_options, DiffOptions};
use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, SyncEngine};

const ALPHA_BID: [u8; 16] = [0x01; 16];
const BETA_BID: [u8; 16] = [0x02; 16];

const NOTE_ID: [u8; 16] = [0xAB; 16];

fn engine(device_byte: u8) -> LoroEngine {
    let device = DeviceId::from_bytes([device_byte; 16]);
    let hlc = Arc::new(Hlc::new(device));
    LoroEngine::new(device, hlc)
}

/// Initial seed body: alpha + beta, both stamped with explicit bids so
/// `parse_note` keys each FlatBlock to the same id the engine authored.
fn seed_body() -> String {
    "- alpha <!-- bid:01010101-0101-0101-0101-010101010101 -->\n\
     - beta <!-- bid:02020202-0202-0202-0202-020202020202 -->\n"
        .to_string()
}

/// The STALE web PUT body: web edited alpha ("alpha CHANGED") but its
/// beta is the OLD pre-peer-edit text ("beta"). This is the whole-document
/// body the web client ships on PUT /notes/{id}.
fn stale_web_put_body() -> String {
    "- alpha CHANGED <!-- bid:01010101-0101-0101-0101-010101010101 -->\n\
     - beta <!-- bid:02020202-0202-0202-0202-020202020202 -->\n"
        .to_string()
}

/// Seed the server engine and land the PEER's concurrent edit to beta on
/// it (mirrors Roshar's edit reaching the authoritative engine first).
/// Returns the server engine with beta == "beta EDITED BY PEER".
async fn server_with_peer_edit() -> LoroEngine {
    let server = engine(0x11);

    // Server seeds the note from the materialized markdown.
    server
        .record_local(OpPayload::NoteUpsert {
            note_id: NOTE_ID,
            display_alias: Some("note".into()),
            title: "Note".into(),
            content: seed_body(),
            created_at_millis: 1,
        })
        .await
        .expect("server seed NoteUpsert");

    // PEER edit lands on the server first: beta -> "beta EDITED BY PEER".
    server
        .record_local(OpPayload::BlockUpsert {
            block_id: BETA_BID,
            note_id: NOTE_ID,
            parent_block_id: None,
            order_key: "00000001".into(),
            indent_level: 0,
            text: "beta EDITED BY PEER".into(),
        })
        .await
        .expect("peer BlockUpsert beta");

    server
}

/// Regression DOCUMENTATION of the bug the block-granular write path fixes:
/// the server's whole-body diff (the legacy `PUT /notes/{id}` path) re-asserts
/// the stale beta and CLOBBERS the peer's concurrent edit. This test asserts
/// the buggy behavior happens so it stays GREEN — it pins the exact reason the
/// whole-body PUT must not be used for block edits. The fix lives on a
/// different path (`block_granular_write_preserves_both_edits` + the
/// `POST /notes/{id}/blocks` endpoint), so this legacy path is intentionally
/// unchanged. If this test ever flips (the clobber stops happening on the
/// whole-body diff), revisit whether the PUT path was made stale-aware.
///
/// Mirrors `record_sync_update` exactly: parse the server's current file
/// as `old`, parse the stale PUT body as `new`, diff with
/// `emit_deletes:false`, and `record_local` every resulting op.
#[tokio::test]
async fn whole_body_diff_clobbers_concurrent_peer_edit() {
    let server = server_with_peer_edit().await;

    // The server's authoritative on-disk file == its current render. After
    // the peer edit this contains "beta EDITED BY PEER".
    let server_current_file = server
        .render_note(NOTE_ID)
        .await
        .expect("server render after peer edit");
    eprintln!("=== SERVER FILE (pre-PUT, has peer edit) ===\n{server_current_file}\n=== END ===");
    assert!(
        server_current_file.contains("beta EDITED BY PEER"),
        "precondition: server file should hold the peer's edit; got:\n{server_current_file}"
    );

    // --- record_sync_update path ---
    let old_tree = parse_note(&server_current_file);
    let new_tree = parse_note(&stale_web_put_body());
    let ops = diff_note_trees_with_options(
        NOTE_ID,
        &old_tree,
        &new_tree,
        DiffOptions { emit_deletes: false },
    );
    eprintln!("=== OPS EMITTED BY WHOLE-BODY DIFF ===\n{ops:#?}\n=== END ===");

    for op in ops {
        server.record_local(op).await.expect("apply diff op");
    }

    let after = server
        .render_note(NOTE_ID)
        .await
        .expect("server render after PUT");
    eprintln!("=== SERVER RENDER AFTER STALE PUT ===\n{after}\n=== END ===");

    // Web's own edit lands.
    assert!(
        after.contains("alpha CHANGED"),
        "web's own edit (alpha) should land; got:\n{after}"
    );
    // THE BUG, asserted as documentation: the stale whole-body PUT re-asserted
    // the old "beta", so the peer's concurrent "beta EDITED BY PEER" is LOST.
    // This is exactly why block edits must use the block-granular endpoint
    // (see `block_granular_write_preserves_both_edits`), not the whole-body
    // PUT. Asserting the clobber keeps this test green while pinning the bug.
    assert!(
        !after.contains("beta EDITED BY PEER"),
        "expected the legacy whole-body diff to clobber the peer's beta edit \
         (documents the bug); got:\n{after}"
    );
    assert!(
        after.contains("- beta <!--") || after.contains("- beta\n") || after.contains("- beta "),
        "expected the stale 'beta' to have been re-asserted by the whole-body \
         diff; got:\n{after}"
    );
}

/// Fix shape (EXPECTED TO PASS): the client sends ONLY the block it
/// actually edited (alpha). With no op for beta, the peer's beta edit can
/// never be re-asserted stale, so both edits survive.
#[tokio::test]
async fn block_granular_write_preserves_both_edits() {
    let server = server_with_peer_edit().await;

    let before = server
        .render_note(NOTE_ID)
        .await
        .expect("server render before granular write");
    assert!(
        before.contains("beta EDITED BY PEER"),
        "precondition: server file should hold the peer's edit; got:\n{before}"
    );

    // Block-granular write: ONLY the block web actually changed (alpha).
    // No beta op is emitted, so beta's peer edit is untouched.
    server
        .record_local(OpPayload::BlockUpsert {
            block_id: ALPHA_BID,
            note_id: NOTE_ID,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "alpha CHANGED".into(),
        })
        .await
        .expect("granular BlockUpsert alpha");

    let after = server
        .render_note(NOTE_ID)
        .await
        .expect("server render after granular write");
    eprintln!("=== SERVER RENDER AFTER GRANULAR WRITE ===\n{after}\n=== END ===");

    assert!(
        after.contains("alpha CHANGED"),
        "web's own edit (alpha) should land; got:\n{after}"
    );
    assert!(
        after.contains("beta EDITED BY PEER"),
        "peer's concurrent beta edit MUST survive a block-granular write; got:\n{after}"
    );
    // No stale pre-peer-edit beta ghost.
    let stale_beta = after
        .lines()
        .filter(|l| {
            let t = l.trim_start_matches([' ', '\t', '-']).trim();
            t.starts_with("beta") && !t.starts_with("beta EDITED BY PEER")
        })
        .count();
    assert_eq!(
        stale_beta, 0,
        "no stale 'beta' bullet should remain; got:\n{after}"
    );
}
