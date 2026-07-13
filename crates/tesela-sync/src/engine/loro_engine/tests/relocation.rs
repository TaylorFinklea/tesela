use super::*;
use std::collections::BTreeSet;

async fn seed_duplicate_owner(
    engine: &LoroEngine,
    note_a: [u8; 16],
    note_b: [u8; 16],
    bid: [u8; 16],
) {
    upsert_block(engine, note_a, bid, "note a", None).await;
    upsert_block(engine, note_b, bid, "note b", None).await;
}

#[tokio::test]
async fn duplicate_owner_is_ambiguous_and_legacy_mutation_fails() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note_a = [0xa1; 16];
    let note_b = [0xb2; 16];
    let bid = [0xcc; 16];

    seed_duplicate_owner(&engine, note_a, note_b, bid).await;

    let owners = engine.inner.block_index.read().await;
    assert_eq!(owners.get(&bid).unwrap(), &BTreeSet::from([note_a, note_b]));
    drop(owners);

    let note_a_before = engine.render_note(note_a).await.unwrap();
    let note_b_before = engine.render_note(note_b).await.unwrap();
    let err = engine
        .record_local(OpPayload::BlockDelete { block_id: bid })
        .await
        .expect_err("ambiguous block mutation must fail closed");
    assert!(matches!(
        err,
        SyncError::Protocol(message) if message.contains("ambiguous")
    ));
    assert_eq!(engine.render_note(note_a).await.unwrap(), note_a_before);
    assert_eq!(engine.render_note(note_b).await.unwrap(), note_b_before);
}

#[tokio::test]
async fn duplicate_owner_heals_after_one_copy_is_deleted() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note_a = [0xa3; 16];
    let note_b = [0xb4; 16];
    let bid = [0xdd; 16];

    seed_duplicate_owner(&engine, note_a, note_b, bid).await;
    engine
        .record_local(OpPayload::NoteDelete {
            note_id: note_b,
            display_alias: None,
        })
        .await
        .unwrap();

    let owners = engine.inner.block_index.read().await;
    assert_eq!(owners.get(&bid).unwrap(), &BTreeSet::from([note_a]));
    drop(owners);

    engine
        .record_local(OpPayload::BlockDelete { block_id: bid })
        .await
        .expect("the remaining unique owner must be mutable");
    assert_eq!(engine.render_note(note_a).await.unwrap(), "");
    assert!(engine.inner.block_index.read().await.get(&bid).is_none());
}
