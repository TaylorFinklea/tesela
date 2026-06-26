//! Regression anchor for the 2026-06-26 desktop crash-loop + the drift it left.
//!
//! Two engines seed the SAME note+block INDEPENDENTLY (disjoint Loro histories
//! → different TreeIDs for the same bid: a Mac + an iOS device that each
//! re-authored from markdown). Each then edits the block via INCREMENTAL
//! LoroText splices (the per-keystroke `splice_block_text` path), not a
//! whole-block upsert. Merging the two disjoint-twin richtext states is what
//! panicked loro 1.12 (`insert_elem_at_entity_index` index-out-of-bounds) and
//! crash-looped the desktop on every relay tick.
//!
//! The poison-frame containment stops the crash by SKIPPING that merge — but a
//! skipped merge never converges (the observed drift: desktop "Brook" vs iOS
//! "Bro" for the same block). So this asserts the real invariant: after a
//! bidirectional merge the two engines render the SAME text for the block.
//! RED while the merge crashes/skips; GREEN once the merge actually applies.

use std::sync::Arc;

use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, SyncEngine};

const BID_HEX: &str = "01010101-0101-0101-0101-010101010101";
const BID: [u8; 16] = [0x01; 16];

fn engine(device_byte: u8) -> LoroEngine {
    let device = DeviceId::from_bytes([device_byte; 16]);
    LoroEngine::new(device, Arc::new(Hlc::new(device)))
}

fn seed_body() -> String {
    format!("- seed <!-- bid:{BID_HEX} -->\n")
}

/// The block's rendered bullet line (text differs across replicas; the bid
/// marker is identical), or a sentinel if the block is missing.
fn block_line(render: &str) -> String {
    render
        .lines()
        .find(|l| l.contains(&format!("bid:{BID_HEX}")))
        .unwrap_or("<block missing>")
        .trim()
        .to_string()
}

#[tokio::test]
async fn disjoint_lineage_splice_merge_converges_without_panic() {
    let note = [0xAB; 16];
    let server = engine(0x11);
    let device = engine(0x22);

    // Seed the SAME block on BOTH engines INDEPENDENTLY → disjoint lineages.
    for eng in [&server, &device] {
        eng.record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("note".into()),
            title: "Note".into(),
            content: seed_body(),
            created_at_millis: 1,
        })
        .await
        .expect("seed NoteUpsert");
    }

    // Concurrent INCREMENTAL splices (append after "seed" = utf16 offset 4).
    // Each side splices its OWN text_seq LoroText (different TreeID).
    server
        .splice_block_text(note, BID, 4, 0, "Brook")
        .await
        .expect("server splice");
    device
        .splice_block_text(note, BID, 4, 0, "Bro")
        .await
        .expect("device splice");

    // Bidirectional merge via the inbound relay path (full snapshots).
    let server_snap = server.export_doc_update(note, None).await.expect("server export");
    let device_snap = device.export_doc_update(note, None).await.expect("device export");
    server.apply_relay_updates(&[(note, device_snap)]).await;
    device.apply_relay_updates(&[(note, server_snap)]).await;

    let server_render = server.render_note(note).await.expect("server render");
    let device_render = device.render_note(note).await.expect("device render");

    // Each side renders the block exactly once (twins deduped).
    assert_eq!(
        server_render.matches(&format!("bid:{BID_HEX}")).count(),
        1,
        "server must render the block exactly once:\n{server_render}"
    );
    assert_eq!(
        device_render.matches(&format!("bid:{BID_HEX}")).count(),
        1,
        "device must render the block exactly once:\n{device_render}"
    );

    // The real invariant: the two engines CONVERGE to the same block text.
    assert_eq!(
        block_line(&server_render),
        block_line(&device_render),
        "disjoint-lineage splice merge must converge to the same text\n\
         server: {}\n device: {}",
        block_line(&server_render),
        block_line(&device_render),
    );
}
