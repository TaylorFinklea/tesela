//! Convergence tests for the SQLite-backed engine over loopback transport.
//!
//! These exercise the substrate end-to-end without any networking, crypto,
//! or `tesela-core` integration. The TestRig spins up two engines on two
//! in-memory SQLite databases, wires them with a loopback channel pair,
//! and drives convergence scenarios.

use tesela_sync::{
    DeviceId, LoopbackTransport, OpPayload, PeerCursor, SqliteEngine, SyncEngine,
    Transport, TransportTarget,
};
use tokio::sync::Mutex;
use uuid::Uuid;

mod common {
    use super::*;

    /// A pair of engines connected by loopback transport. Each side can
    /// `record_local`, and `sync_a_to_b` (or `_b_to_a`) ships pending ops
    /// across.
    pub struct TestRig {
        pub a: SqliteEngine,
        pub b: SqliteEngine,
        pub device_a: DeviceId,
        pub device_b: DeviceId,
        session_a_to_b: Mutex<Option<Box<dyn tesela_sync::TransportSession>>>,
        session_b_to_a: Mutex<Option<Box<dyn tesela_sync::TransportSession>>>,
    }

    impl TestRig {
        pub async fn new() -> Self {
            let device_a = DeviceId::new_random();
            let device_b = DeviceId::new_random();

            // Each engine gets its own in-memory DB. Using shared cache
            // names so each unique URL is independent.
            let url_a = format!("sqlite:file:engine_a_{}?mode=memory&cache=shared", Uuid::new_v4());
            let url_b = format!("sqlite:file:engine_b_{}?mode=memory&cache=shared", Uuid::new_v4());
            let a = SqliteEngine::open(&url_a, device_a).await.expect("open a");
            let b = SqliteEngine::open(&url_b, device_b).await.expect("open b");

            let (t_a, t_b) = LoopbackTransport::pair(device_a, device_b);
            let session_ab = t_a
                .open(TransportTarget::Peer(device_b))
                .await
                .expect("a opens to b");
            let session_ba = t_b
                .open(TransportTarget::Peer(device_a))
                .await
                .expect("b opens to a");

            TestRig {
                a,
                b,
                device_a,
                device_b,
                session_a_to_b: Mutex::new(Some(session_ab)),
                session_b_to_a: Mutex::new(Some(session_ba)),
            }
        }

        /// Push all of a's pending ops (since b's peer_cursor for a) over
        /// to b, and apply them.
        pub async fn sync_a_to_b(&self) {
            self.sync_one_way(true).await;
        }

        pub async fn sync_b_to_a(&self) {
            self.sync_one_way(false).await;
        }

        /// Full bidirectional sync: a -> b then b -> a (twice for stable
        /// convergence when ops on b's side are produced during apply).
        pub async fn sync_bidirectional(&self) {
            self.sync_a_to_b().await;
            self.sync_b_to_a().await;
        }

        async fn sync_one_way(&self, a_to_b: bool) {
            let (src, dst, src_dev, dst_dev, send_slot, recv_slot) = if a_to_b {
                (
                    &self.a,
                    &self.b,
                    self.device_a,
                    self.device_b,
                    &self.session_a_to_b,
                    &self.session_b_to_a,
                )
            } else {
                (
                    &self.b,
                    &self.a,
                    self.device_b,
                    self.device_a,
                    &self.session_b_to_a,
                    &self.session_a_to_b,
                )
            };

            // dst asks "what's new from src since my cursor?"
            let since = dst.peer_cursor(src_dev).await.expect("peer cursor");
            let batch = src
                .produce_changes_since(dst_dev, since, 1024 * 1024)
                .await
                .expect("produce");

            if batch.ops.is_empty() {
                return;
            }

            let envelope = tesela_sync::SyncEnvelope {
                from_device: src_dev,
                to_group: tesela_sync::GroupId([0u8; 16]),
                nonce: [0u8; 24],
                ciphertext: tesela_sync::oplog::op::EncodedOp::default_batch_encode(batch.ops.clone())
                    .expect("encode batch"),
            };

            // Send over the wire.
            {
                let mut slot = send_slot.lock().await;
                let s = slot.as_mut().expect("session live");
                s.send(envelope).await.expect("send envelope");
            }
            // Receive on the other side.
            let received = {
                let mut slot = recv_slot.lock().await;
                let s = slot.as_mut().expect("session live");
                s.recv().await.expect("recv envelope").expect("not closed")
            };

            // Apply at destination.
            let _changes = dst
                .apply_changes(src_dev, received)
                .await
                .expect("apply changes");

            // dst ACKs back to src so retention can advance.
            dst.ack_peer(src_dev, batch.new_cursor)
                .await
                .expect("ack peer");
        }

        /// Total rows in the engine's oplog.
        pub async fn oplog_total(&self, which: WhichEngine) -> i64 {
            let engine = match which {
                WhichEngine::A => &self.a,
                WhichEngine::B => &self.b,
            };
            engine.oplog_total().await.expect("oplog_total")
        }
    }

    #[derive(Clone, Copy)]
    pub enum WhichEngine {
        A,
        B,
    }
}

// Extension method on EncodedOp for batch encoding (avoids leaking
// internal wire fns through the public API).
mod _wire_helper {
    use tesela_sync::oplog::op::EncodedOp;

    pub trait EncodedOpBatchExt {
        fn default_batch_encode(ops: Vec<EncodedOp>) -> Result<Vec<u8>, tesela_sync::SyncError>;
    }

    impl EncodedOpBatchExt for EncodedOp {
        fn default_batch_encode(ops: Vec<EncodedOp>) -> Result<Vec<u8>, tesela_sync::SyncError> {
            Ok(postcard::to_allocvec(&ops).map_err(tesela_sync::SyncError::from)?)
        }
    }
}
use _wire_helper::EncodedOpBatchExt;

fn note_upsert(id_seed: u8, title: &str) -> OpPayload {
    OpPayload::NoteUpsert {
        note_id: [id_seed; 16],
        display_alias: Some(format!("note-{id_seed}")),
        title: title.to_string(),
        content: format!("# {title}\n\n"),
        created_at_millis: 0,
    }
}

fn block_upsert(block_seed: u8, note_seed: u8, text: &str) -> OpPayload {
    OpPayload::BlockUpsert {
        block_id: [block_seed; 16],
        note_id: [note_seed; 16],
        parent_block_id: None,
        order_key: format!("a{block_seed}"),
        indent_level: 0,
        text: text.to_string(),
    }
}

#[tokio::test]
async fn one_way_full_corpus() {
    let rig = common::TestRig::new().await;

    for i in 0..20u8 {
        rig.a
            .record_local(note_upsert(i, &format!("Note {i}")))
            .await
            .expect("record");
    }
    assert_eq!(rig.oplog_total(common::WhichEngine::A).await, 20);
    assert_eq!(rig.oplog_total(common::WhichEngine::B).await, 0);

    rig.sync_a_to_b().await;

    assert_eq!(
        rig.oplog_total(common::WhichEngine::B).await,
        20,
        "B should have 20 ops after sync_a_to_b"
    );
}

#[tokio::test]
async fn bidirectional_disjoint() {
    let rig = common::TestRig::new().await;

    for i in 0..10u8 {
        rig.a
            .record_local(note_upsert(i, &format!("A's note {i}")))
            .await
            .expect("record on a");
    }
    for i in 100..110u8 {
        rig.b
            .record_local(note_upsert(i, &format!("B's note {i}")))
            .await
            .expect("record on b");
    }

    rig.sync_bidirectional().await;

    assert_eq!(rig.oplog_total(common::WhichEngine::A).await, 20);
    assert_eq!(rig.oplog_total(common::WhichEngine::B).await, 20);
}

#[tokio::test]
async fn concurrent_different_blocks_same_note() {
    let rig = common::TestRig::new().await;

    let note_seed = 50u8;
    rig.a
        .record_local(note_upsert(note_seed, "Shared note"))
        .await
        .expect("a creates note");
    rig.b
        .record_local(note_upsert(note_seed, "Shared note"))
        .await
        .expect("b creates note");

    rig.a
        .record_local(block_upsert(1, note_seed, "A's block"))
        .await
        .expect("a block");
    rig.b
        .record_local(block_upsert(2, note_seed, "B's block"))
        .await
        .expect("b block");

    rig.sync_bidirectional().await;

    // Each side: 2 of its own ops + 2 from the peer = 4 total.
    assert_eq!(rig.oplog_total(common::WhichEngine::A).await, 4);
    assert_eq!(rig.oplog_total(common::WhichEngine::B).await, 4);
}

#[tokio::test]
async fn block_level_concurrent_edits_converge_on_disk() {
    use tesela_core::note_tree::{parse_note, serialize_note};
    use tesela_sync::diff::diff_note_trees;

    let tmp_a = tempfile::TempDir::new().expect("tmp a");
    let tmp_b = tempfile::TempDir::new().expect("tmp b");
    tokio::fs::create_dir_all(tmp_a.path().join("notes"))
        .await
        .unwrap();
    tokio::fs::create_dir_all(tmp_b.path().join("notes"))
        .await
        .unwrap();

    let device_a = DeviceId::new_random();
    let device_b = DeviceId::new_random();
    let url_a = format!(
        "sqlite:file:bl_a_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let url_b = format!(
        "sqlite:file:bl_b_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let a = SqliteEngine::open_with_mosaic(&url_a, Some(tmp_a.path().into()), device_a)
        .await
        .expect("a");
    let b = SqliteEngine::open_with_mosaic(&url_b, Some(tmp_b.path().into()), device_b)
        .await
        .expect("b");

    let (t_a, t_b) = LoopbackTransport::pair(device_a, device_b);
    let mut sess_a = t_a
        .open(TransportTarget::Peer(device_b))
        .await
        .expect("a session");
    let mut sess_b = t_b
        .open(TransportTarget::Peer(device_a))
        .await
        .expect("b session");

    // Step 1: A creates the note with stamped block ids. Use parse_note
    // to mint canonical bids then ship the stamped content.
    let initial = parse_note("- Alpha\n- Beta\n- Gamma\n");
    let stamped_content = serialize_note(&initial);
    let note_id = [42u8; 16];
    let alpha_id = initial.blocks[0].id;
    let gamma_id = initial.blocks[2].id;
    a.record_local(OpPayload::NoteUpsert {
        note_id,
        display_alias: Some("triple".to_string()),
        title: "Triple".to_string(),
        content: stamped_content.clone(),
        created_at_millis: 0,
    })
    .await
    .expect("a NoteUpsert");

    // Step 2: A → B sync of the initial NoteUpsert.
    sync_one_way(&a, &b, device_a, device_b, &mut sess_a, &mut sess_b).await;
    let b_initial = tokio::fs::read_to_string(tmp_b.path().join("notes/triple.md"))
        .await
        .expect("b has triple.md");
    assert!(b_initial.contains(&alpha_id.to_string()));
    assert!(b_initial.contains(&gamma_id.to_string()));

    // Step 3: A edits Alpha, B edits Gamma — concurrently, before sync.
    let a_edited = parse_note(&stamped_content.replace("- Alpha ", "- Alpha edited by A "));
    let a_ops = diff_note_trees(note_id, &initial, &a_edited);
    assert_eq!(a_ops.len(), 1);
    for op in a_ops {
        a.record_local(op).await.expect("a BlockUpsert");
    }
    let b_edited = parse_note(&b_initial.replace("- Gamma ", "- Gamma edited by B "));
    let b_initial_tree = parse_note(&b_initial);
    let b_ops = diff_note_trees(note_id, &b_initial_tree, &b_edited);
    assert_eq!(b_ops.len(), 1);
    for op in b_ops {
        b.record_local(op).await.expect("b BlockUpsert");
    }

    // Mirror the new content to disk on each side (the producer in
    // tesela-server does this; here we simulate it directly).
    tokio::fs::write(
        tmp_a.path().join("notes/triple.md"),
        serialize_note(&a_edited),
    )
    .await
    .unwrap();
    tokio::fs::write(
        tmp_b.path().join("notes/triple.md"),
        serialize_note(&b_edited),
    )
    .await
    .unwrap();

    // Step 4: bidirectional sync.
    sync_one_way(&a, &b, device_a, device_b, &mut sess_a, &mut sess_b).await;
    sync_one_way(&b, &a, device_b, device_a, &mut sess_b, &mut sess_a).await;

    // Step 5: both files contain both edits.
    let final_a = tokio::fs::read_to_string(tmp_a.path().join("notes/triple.md"))
        .await
        .unwrap();
    let final_b = tokio::fs::read_to_string(tmp_b.path().join("notes/triple.md"))
        .await
        .unwrap();
    assert!(
        final_a.contains("Alpha edited by A"),
        "A missing its own edit: {final_a}"
    );
    assert!(
        final_a.contains("Gamma edited by B"),
        "A missing B's edit: {final_a}"
    );
    assert!(
        final_b.contains("Alpha edited by A"),
        "B missing A's edit: {final_b}"
    );
    assert!(
        final_b.contains("Gamma edited by B"),
        "B missing its own edit: {final_b}"
    );
}

async fn sync_one_way(
    src: &SqliteEngine,
    dst: &SqliteEngine,
    src_dev: DeviceId,
    dst_dev: DeviceId,
    send_sess: &mut Box<dyn tesela_sync::TransportSession>,
    recv_sess: &mut Box<dyn tesela_sync::TransportSession>,
) {
    let since = dst.peer_cursor(src_dev).await.expect("peer cursor");
    let batch = src
        .produce_changes_since(dst_dev, since, 1024 * 1024)
        .await
        .expect("produce");
    if batch.ops.is_empty() {
        return;
    }
    let envelope = tesela_sync::SyncEnvelope {
        from_device: src_dev,
        to_group: tesela_sync::GroupId([0u8; 16]),
        nonce: [0u8; 24],
        ciphertext: tesela_sync::oplog::op::EncodedOp::default_batch_encode(batch.ops.clone())
            .expect("encode"),
    };
    send_sess.send(envelope).await.expect("send");
    let recv = recv_sess
        .recv()
        .await
        .expect("recv")
        .expect("not closed");
    dst.apply_changes(src_dev, recv).await.expect("apply");
    dst.ack_peer(src_dev, batch.new_cursor).await.expect("ack");
}

#[tokio::test]
async fn duplicate_envelope_is_idempotent() {
    let rig = common::TestRig::new().await;

    rig.a
        .record_local(note_upsert(7, "Once"))
        .await
        .expect("record");

    rig.sync_a_to_b().await;
    assert_eq!(rig.oplog_total(common::WhichEngine::B).await, 1);

    // Re-apply the same op via a fresh envelope; content_hash dedup
    // should kick in.
    let batch = rig
        .a
        .produce_changes_since(rig.device_b, PeerCursor::Earliest, usize::MAX)
        .await
        .expect("produce");
    let envelope = tesela_sync::SyncEnvelope {
        from_device: rig.device_a,
        to_group: tesela_sync::GroupId([0u8; 16]),
        nonce: [0u8; 24],
        ciphertext: tesela_sync::oplog::op::EncodedOp::default_batch_encode(batch.ops)
            .expect("encode"),
    };
    let changes = rig
        .b
        .apply_changes(rig.device_a, envelope)
        .await
        .expect("apply replay");
    assert_eq!(changes.applied, 0, "no new ops applied");
    assert_eq!(changes.deduped, 1, "the one op was deduped");
    assert_eq!(rig.oplog_total(common::WhichEngine::B).await, 1);
}
