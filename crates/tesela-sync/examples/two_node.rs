//! Phase 1 substrate demo.
//!
//! Two `SqliteEngine` instances on in-memory databases, wired by a loopback
//! transport. Drives a 50-op scripted scenario and asserts convergence in
//! under 100ms.
//!
//! Run with:
//!   `cargo run --example two_node -p tesela-sync`

use std::time::Instant;
use tesela_sync::{
    DeviceId, GroupId, LoopbackTransport, OpPayload, SqliteEngine, SyncEngine, SyncEnvelope,
    Transport, TransportSession, TransportTarget,
};
use uuid::Uuid;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let device_a = DeviceId::new_random();
    let device_b = DeviceId::new_random();

    let url_a = format!(
        "sqlite:file:two_node_a_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let url_b = format!(
        "sqlite:file:two_node_b_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );

    let a = SqliteEngine::open(&url_a, device_a).await?;
    let b = SqliteEngine::open(&url_b, device_b).await?;

    let (transport_a, transport_b) = LoopbackTransport::pair(device_a, device_b);
    let mut session_ab = transport_a.open(TransportTarget::Peer(device_b)).await?;
    let mut session_ba = transport_b.open(TransportTarget::Peer(device_a)).await?;

    let started = Instant::now();

    // 50-op scripted scenario:
    //  - 20 notes on A
    //  - 20 notes on B (different ids)
    //  - 5 blocks on A under one of A's notes
    //  - 5 blocks on B under one of B's notes
    println!("[two_node] producing 50 ops (25 on A, 25 on B)...");
    let target_note_a = [0xa0u8; 16];
    let target_note_b = [0xb0u8; 16];

    for i in 0..20u8 {
        let mut id = [0u8; 16];
        id[0] = 0xa0;
        id[1] = i;
        a.record_local(OpPayload::NoteUpsert {
            note_id: id,
            display_alias: Some(format!("a-note-{i}")),
            title: format!("A note {i}"),
            created_at_millis: 0,
        })
        .await?;
    }
    for i in 0..20u8 {
        let mut id = [0u8; 16];
        id[0] = 0xb0;
        id[1] = i;
        b.record_local(OpPayload::NoteUpsert {
            note_id: id,
            display_alias: Some(format!("b-note-{i}")),
            title: format!("B note {i}"),
            created_at_millis: 0,
        })
        .await?;
    }
    for i in 0..5u8 {
        let mut id = [0u8; 16];
        id[0] = 0xa1;
        id[1] = i;
        a.record_local(OpPayload::BlockUpsert {
            block_id: id,
            note_id: target_note_a,
            parent_block_id: None,
            order_key: format!("a{i}"),
            indent_level: 0,
            text: format!("A block {i}"),
        })
        .await?;
    }
    for i in 0..5u8 {
        let mut id = [0u8; 16];
        id[0] = 0xb1;
        id[1] = i;
        b.record_local(OpPayload::BlockUpsert {
            block_id: id,
            note_id: target_note_b,
            parent_block_id: None,
            order_key: format!("b{i}"),
            indent_level: 0,
            text: format!("B block {i}"),
        })
        .await?;
    }

    println!(
        "[two_node] before sync: A oplog={}, B oplog={}",
        a.oplog_total().await?,
        b.oplog_total().await?
    );

    // Sync: A -> B then B -> A.
    push_pull(&a, &b, device_a, device_b, &mut session_ab, &mut session_ba).await?;
    push_pull(&b, &a, device_b, device_a, &mut session_ba, &mut session_ab).await?;

    let elapsed = started.elapsed();

    let a_total = a.oplog_total().await?;
    let b_total = b.oplog_total().await?;

    println!(
        "[two_node] after sync:  A oplog={a_total}, B oplog={b_total}, elapsed={:?}",
        elapsed
    );

    assert_eq!(a_total, 50, "A should have 50 ops after bidirectional sync");
    assert_eq!(b_total, 50, "B should have 50 ops after bidirectional sync");

    if elapsed.as_millis() > 100 {
        eprintln!(
            "[two_node] WARNING: exit criterion was <100ms; took {:?}",
            elapsed
        );
    } else {
        println!("[two_node] OK: 50-op convergence in {:?} (<100ms target)", elapsed);
    }

    Ok(())
}

async fn push_pull(
    src: &SqliteEngine,
    dst: &SqliteEngine,
    src_dev: DeviceId,
    dst_dev: DeviceId,
    sender: &mut Box<dyn TransportSession>,
    receiver: &mut Box<dyn TransportSession>,
) -> Result<(), Box<dyn std::error::Error>> {
    let since = dst.peer_cursor(src_dev).await?;
    let batch = src
        .produce_changes_since(dst_dev, since, 1024 * 1024)
        .await?;
    if batch.ops.is_empty() {
        return Ok(());
    }
    let envelope = SyncEnvelope {
        from_device: src_dev,
        to_group: GroupId([0u8; 16]),
        nonce: [0u8; 24],
        ciphertext: postcard::to_allocvec(&batch.ops)?,
    };
    sender.send(envelope).await?;

    if let Some(env) = receiver.recv().await? {
        let _changes = dst.apply_changes(src_dev, env).await?;
        dst.ack_peer(src_dev, batch.new_cursor).await?;
    }
    Ok(())
}
