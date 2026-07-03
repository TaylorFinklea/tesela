//! Round 3 (tesela-ows.1 step 2, t4): a CONCURRENT property-representation
//! transition on the SAME key from two peers must CONVERGE without error.
//!
//! Both peers share a base doc where block property `note` is a primitive
//! SCALAR — the shape the engine lifecycle hook / any `SetScalar` write
//! produces, and the shape Taylor's live docs already hold. Each peer then
//! CONCURRENTLY rewrites `note` as FREE TEXT (`SetText`) with a DIFFERENT value:
//! exactly the scalar→text transition the round-3 write layer makes tolerant
//! (clear the incompatible scalar occupant, then mint a regular text child). The
//! transition must (a) never error on either peer, and (b) after a bidirectional
//! merge, converge — both engines render the block identically, with the prop
//! appearing exactly once (no leftover scalar beside the text child, no twin
//! duplication).

use std::sync::Arc;

use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, PropOp, PropScalar, SyncEngine};

const BID_HEX: &str = "05050505-0505-0505-0505-050505050505";
const BID: [u8; 16] = [0x05; 16];

fn engine(device_byte: u8) -> LoroEngine {
    let device = DeviceId::from_bytes([device_byte; 16]);
    LoroEngine::new(device, Arc::new(Hlc::new(device)))
}

#[tokio::test]
async fn concurrent_scalar_to_text_transition_on_same_key_converges() {
    let note = [0xC4; 16];
    let peer_a = engine(0x11);
    let peer_b = engine(0x22);

    // Peer A seeds the note + block and writes `note` as a SCALAR.
    peer_a
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("note".into()),
            title: "Note".into(),
            content: format!("- task <!-- bid:{BID_HEX} -->\n"),
            created_at_millis: 1,
        })
        .await
        .expect("seed NoteUpsert");
    peer_a
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: BID,
            key: "note".into(),
            value: PropOp::SetScalar(PropScalar::Text("base".into())),
        })
        .await
        .expect("seed scalar prop");

    // Peer B adopts A's base as a SHARED lineage (same container ids) so the
    // subsequent writes are a TRUE concurrent transition on the SAME key, not a
    // disjoint-twin merge.
    let base = peer_a
        .export_doc_update(note, None)
        .await
        .expect("export base");
    peer_b.apply_relay_updates(&[(note, base)]).await;

    // Concurrent scalar → text transition, different values. Each `.expect`
    // asserts the transition itself never errors (the class fix).
    peer_a
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: BID,
            key: "note".into(),
            value: PropOp::SetText("alpha".into()),
        })
        .await
        .expect("peer A scalar->text transition must not error");
    peer_b
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: BID,
            key: "note".into(),
            value: PropOp::SetText("beta".into()),
        })
        .await
        .expect("peer B scalar->text transition must not error");

    // Bidirectional merge via the inbound relay path.
    let a_snap = peer_a.export_doc_update(note, None).await.expect("export A");
    let b_snap = peer_b.export_doc_update(note, None).await.expect("export B");
    peer_a.apply_relay_updates(&[(note, b_snap)]).await;
    peer_b.apply_relay_updates(&[(note, a_snap)]).await;

    let render_a = peer_a.render_note(note).await.expect("render A");
    let render_b = peer_b.render_note(note).await.expect("render B");

    // Converged: identical render on both peers.
    assert_eq!(
        render_a, render_b,
        "concurrent scalar->text transition must converge:\nA:\n{render_a}\nB:\n{render_b}"
    );
    // The prop renders exactly once — the incompatible scalar was cleared, not
    // left dangling beside the text child.
    assert_eq!(
        render_a.matches("note::").count(),
        1,
        "the `note` prop must render exactly once:\n{render_a}"
    );
    // And it converged to one of the two concurrently-written text values.
    assert!(
        render_a.contains("note:: alpha") || render_a.contains("note:: beta"),
        "converged value must be one of the concurrent text writes:\n{render_a}"
    );
}
