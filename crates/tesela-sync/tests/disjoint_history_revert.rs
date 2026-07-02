//! Multi-device convergence anchors for the duplicate-block_id bug.
//!
//! Real-world mapping:
//! - The Mac server seeds note docs from disk (NoteUpsert per file).
//! - Each iOS device's `recordNoteDiff` re-authors blocks from its own
//!   materialized markdown, minting its OWN TreeIDs for the same bids.
//! - On a web edit the server BlockUpserts a block; a device edit
//!   BlockUpserts another; the device exports a snapshot the server imports.
//!
//! Loro tree node identity is the internal `TreeID` (peer + counter), NOT
//! the `block_id` meta. Two engines that author the same bid on DISJOINT
//! histories (no shared Loro base) mint different TreeIDs; on merge Loro
//! UNIONS them, so the same logical block (same `block_id` meta) exists as
//! two live tree nodes. The two anchors below pin the two halves of the fix:
//!
//! - **T-heal** (`disjoint_merge_dedups_to_single_node_deterministically`):
//!   dedup collapses the twins so each bid renders exactly once, and the
//!   survivor is chosen by a deterministic rule (global-max `TreeID`,
//!   tesela-fte). This does NOT recover the latest text of a disjoint merge —
//!   max-`TreeID` picks by peer, not recency — so it asserts no-duplication +
//!   determinism only.
//! - **T-converge** (`shared_base_converges_with_correct_text`): when the
//!   device imports the server's doc as a SHARED BASE before authoring, its
//!   BlockUpserts resolve to the existing server nodes (same TreeIDs), so
//!   concurrent edits to different blocks BOTH survive with correct text.
//!   This is the real convergence fix; dedup alone cannot achieve it.

use std::sync::Arc;

use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, SyncEngine};

const ALPHA_BID: [u8; 16] = [0x01; 16];
const BETA_BID: [u8; 16] = [0x02; 16];

fn engine(device_byte: u8) -> LoroEngine {
    let device = DeviceId::from_bytes([device_byte; 16]);
    let hlc = Arc::new(Hlc::new(device));
    LoroEngine::new(device, hlc)
}

/// Same body both sides parse independently. Explicit bids so each side's
/// FlatBlocks carry identical block ids — but each side mints its own
/// TreeIDs when it seeds.
fn seed_body() -> String {
    "- alpha <!-- bid:01010101-0101-0101-0101-010101010101 -->\n\
     - beta <!-- bid:02020202-0202-0202-0202-020202020202 -->\n"
        .to_string()
}

/// Drive the disjoint-history scenario end to end and return the merged
/// SERVER render: two engines seed the same note independently (disjoint
/// histories), the server edits alpha, the device edits beta, the device
/// exports a full snapshot, and the server imports it. Used twice in the
/// determinism check below so a rebuilt run can be compared byte-for-byte.
async fn run_disjoint_merge(note_id: [u8; 16]) -> String {
    // Peer S (server) and peer D (device): DIFFERENT DeviceIds => different
    // Loro PeerIDs => disjoint histories when each seeds independently.
    let server = engine(0x11);
    let device = engine(0x22);

    // Seed the SAME note on BOTH engines INDEPENDENTLY from the same
    // markdown. They do NOT share a base — no import between them here.
    for eng in [&server, &device] {
        eng.record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("note".into()),
            title: "Note".into(),
            content: seed_body(),
            created_at_millis: 1,
        })
        .await
        .expect("seed NoteUpsert");
    }

    // Web edit on the SERVER: alpha -> "alpha EDITED" (same bid 0101…).
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
        .expect("server BlockUpsert alpha");

    // Device edit: beta -> "beta from device" (same bid 0202…).
    device
        .record_local(OpPayload::BlockUpsert {
            block_id: BETA_BID,
            note_id,
            parent_block_id: None,
            order_key: "a1".into(),
            indent_level: 0,
            text: "beta from device".into(),
            after_block_id: None,
        })
        .await
        .expect("device BlockUpsert beta");

    // Device exports its FULL snapshot (mirrors produceNoteDelta(nil)).
    let snapshot = device
        .export_doc_update(note_id, None)
        .await
        .expect("device snapshot export");

    // Apply the device snapshot into the SERVER (the inbound /ws delta).
    server.apply_relay_updates(&[(note_id, snapshot)]).await;

    server
        .render_note(note_id)
        .await
        .expect("server render after merge")
}

/// Count how many bullet lines carry the given bid (matched on the
/// `<!-- bid:… -->` marker so duplicate render of the same logical block is
/// visible regardless of its text).
fn bid_bullet_count(render: &str, bid_hex_dashed: &str) -> usize {
    render
        .lines()
        .filter(|l| l.contains(&format!("bid:{bid_hex_dashed}")))
        .count()
}

/// T-heal (Part E anchor): a disjoint merge dedups to a single node per bid,
/// deterministically. Asserts invariant 1 (exactly one bullet per bid) and
/// invariant 2 (the survivor / render is stable across a fresh rebuild).
/// Does NOT assert text-correctness of the disjoint merge — max-`TreeID` is
/// deterministic but not recency-aware, so it may keep a stale twin's text;
/// recovering the latest text is what the SHARED-BASE path (T-converge) is
/// for, not dedup.
#[tokio::test]
async fn disjoint_merge_dedups_to_single_node_deterministically() {
    let note_id = [0xAB; 16];

    let merged = run_disjoint_merge(note_id).await;
    eprintln!("=== MERGED SERVER RENDER (run 1) ===\n{merged}\n=== END ===");

    // Invariant 1: each distinct bid renders EXACTLY ONCE — no duplicate
    // bullet for the unioned twins.
    let alpha_bullets = bid_bullet_count(&merged, "01010101-0101-0101-0101-010101010101");
    let beta_bullets = bid_bullet_count(&merged, "02020202-0202-0202-0202-020202020202");
    assert_eq!(
        alpha_bullets, 1,
        "expected alpha bid to render exactly once, got {alpha_bullets}; render:\n{merged}"
    );
    assert_eq!(
        beta_bullets, 1,
        "expected beta bid to render exactly once, got {beta_bullets}; render:\n{merged}"
    );

    // Invariant 2 (determinism): re-render the SAME merged doc — must be
    // byte-identical (the dedup survivor + walk order don't drift between
    // renders).
    let reread = run_disjoint_merge(note_id).await;
    assert_eq!(
        merged, reread,
        "disjoint merge render is non-deterministic across rebuilds:\n--- run 1 ---\n{merged}\n--- run 2 ---\n{reread}"
    );
}

/// T-converge (Part D anchor): when the device imports the server's note doc
/// as a SHARED BASE before authoring, concurrent edits to DIFFERENT blocks
/// on the two sides both survive with correct text, each block exactly once
/// (invariant 3). Documents that the real fix is the shared base: because
/// the device's BlockUpserts resolve to the server's existing TreeIDs (not
/// freshly minted twins), the merge is a clean LWW-per-block, no ghosts.
#[tokio::test]
async fn shared_base_converges_with_correct_text() {
    let note_id = [0xCD; 16];

    let server = engine(0x11);
    let device = engine(0x22);

    // Server seeds the note (NoteUpsert) — authoritative base.
    server
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("note".into()),
            title: "Note".into(),
            content: seed_body(),
            created_at_millis: 1,
        })
        .await
        .expect("server seed NoteUpsert");

    // SHARED BASE: device imports the server's doc BEFORE it authors. Now
    // the device's tree carries the server's TreeIDs for alpha/beta.
    let base = server
        .export_doc_update(note_id, None)
        .await
        .expect("server base export");
    device
        .import_doc_update(note_id, &base)
        .await
        .expect("device import shared base");

    // Device edit (on the shared base): beta -> "beta from device".
    device
        .record_local(OpPayload::BlockUpsert {
            block_id: BETA_BID,
            note_id,
            parent_block_id: None,
            order_key: "a1".into(),
            indent_level: 0,
            text: "beta from device".into(),
            after_block_id: None,
        })
        .await
        .expect("device BlockUpsert beta");

    // Device exports a snapshot; server imports it.
    let device_snapshot = device
        .export_doc_update(note_id, None)
        .await
        .expect("device snapshot export");
    server
        .import_doc_update(note_id, &device_snapshot)
        .await
        .expect("server import device snapshot");

    // Server edit AFTER the device's beta is merged: alpha -> "alpha EDITED".
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
        .expect("server BlockUpsert alpha");

    let merged = server
        .render_note(note_id)
        .await
        .expect("server render after converge");
    eprintln!("=== SHARED-BASE CONVERGED RENDER ===\n{merged}\n=== END ===");

    // Both edits present, correct text, each block exactly once.
    let alpha_bullets = bid_bullet_count(&merged, "01010101-0101-0101-0101-010101010101");
    let beta_bullets = bid_bullet_count(&merged, "02020202-0202-0202-0202-020202020202");
    assert_eq!(
        alpha_bullets, 1,
        "expected alpha bid exactly once, got {alpha_bullets}; render:\n{merged}"
    );
    assert_eq!(
        beta_bullets, 1,
        "expected beta bid exactly once, got {beta_bullets}; render:\n{merged}"
    );
    assert_eq!(
        merged.matches("alpha EDITED").count(),
        1,
        "expected 'alpha EDITED' exactly once; render:\n{merged}"
    );
    assert_eq!(
        merged.matches("beta from device").count(),
        1,
        "expected 'beta from device' exactly once; render:\n{merged}"
    );

    // No stale pre-edit ghost text survived.
    let stale_alpha = merged
        .lines()
        .filter(|l| {
            let t = l.trim_start_matches([' ', '\t', '-']).trim();
            t.starts_with("alpha ") && !t.starts_with("alpha EDITED")
        })
        .count();
    let stale_beta = merged
        .lines()
        .filter(|l| {
            let t = l.trim_start_matches([' ', '\t', '-']).trim();
            t.starts_with("beta ") && !t.starts_with("beta from device")
        })
        .count();
    assert_eq!(
        stale_alpha, 0,
        "expected NO stale pre-edit 'alpha' bullet, got {stale_alpha}; render:\n{merged}"
    );
    assert_eq!(
        stale_beta, 0,
        "expected NO stale pre-edit 'beta' bullet, got {stale_beta}; render:\n{merged}"
    );
}
