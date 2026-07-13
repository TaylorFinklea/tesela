use super::*;
use std::collections::BTreeSet;

async fn insert_nested_block(
    engine: &LoroEngine,
    note_id: [u8; 16],
    bid: [u8; 16],
    text: &str,
) -> LoroDoc {
    let doc = engine.doc_for_note_mut(note_id).await;
    let tree = doc.get_tree("blocks");
    let parent = tree.create(TreeParentId::Root).unwrap();
    let nested = tree.create(TreeParentId::Node(parent)).unwrap();
    let meta = tree.get_meta(nested).unwrap();
    meta.insert("block_id", hex_id(&bid).as_str()).unwrap();
    write_block_text(&meta, text).unwrap();
    meta.insert("indent_level", 1i64).unwrap();
    doc.commit();
    doc
}

async fn nested_block_is_live(engine: &LoroEngine, note_id: [u8; 16], bid: [u8; 16]) -> bool {
    let docs = engine.inner.docs.read().await;
    let Some(doc) = docs.get(&note_id) else {
        return false;
    };
    let tree = doc.get_tree("blocks");
    let Some(node) = find_node_by_block_id(&tree, &hex_id(&bid)) else {
        return false;
    };
    !matches!(tree.is_node_deleted(&node), Ok(true))
}

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

#[tokio::test]
async fn nested_live_bid_is_owned_after_refresh_and_snapshot_restart() {
    let dir = tempfile::tempdir().unwrap();
    let device = DeviceId::from_bytes([0xe1; 16]);
    let engine =
        LoroEngine::with_snapshot_dir(device, Arc::new(Hlc::new(device)), dir.path().to_path_buf())
            .await
            .unwrap();
    let note_id = [0x31; 16];
    let bid = [0x41; 16];

    let doc = insert_nested_block(&engine, note_id, bid, "nested").await;
    engine.refresh_note_derived(note_id, &doc).await;
    assert_eq!(
        engine.inner.block_index.read().await.get(&bid),
        Some(&BTreeSet::from([note_id]))
    );
    engine
        .save_snapshot_checked(dir.path(), note_id)
        .await
        .unwrap();
    drop(engine);

    let reopened =
        LoroEngine::with_snapshot_dir(device, Arc::new(Hlc::new(device)), dir.path().to_path_buf())
            .await
            .unwrap();
    assert_eq!(
        reopened.inner.block_index.read().await.get(&bid),
        Some(&BTreeSet::from([note_id]))
    );
}

#[tokio::test]
async fn duplicate_nested_owner_is_ambiguous_after_snapshot_restart() {
    let dir = tempfile::tempdir().unwrap();
    let device = DeviceId::from_bytes([0xe2; 16]);
    let engine =
        LoroEngine::with_snapshot_dir(device, Arc::new(Hlc::new(device)), dir.path().to_path_buf())
            .await
            .unwrap();
    let note_a = [0x51; 16];
    let note_b = [0x52; 16];
    let bid = [0x61; 16];

    for (note_id, text) in [(note_a, "nested a"), (note_b, "nested b")] {
        let doc = insert_nested_block(&engine, note_id, bid, text).await;
        engine.refresh_note_derived(note_id, &doc).await;
        engine
            .save_snapshot_checked(dir.path(), note_id)
            .await
            .unwrap();
    }
    drop(engine);

    let reopened =
        LoroEngine::with_snapshot_dir(device, Arc::new(Hlc::new(device)), dir.path().to_path_buf())
            .await
            .unwrap();
    assert_eq!(
        reopened.inner.block_index.read().await.get(&bid),
        Some(&BTreeSet::from([note_a, note_b]))
    );

    let err = reopened
        .record_local(OpPayload::BlockDelete { block_id: bid })
        .await
        .expect_err("duplicate nested owners must remain ambiguous");
    assert!(matches!(
        err,
        SyncError::Protocol(message) if message.contains("ambiguous")
    ));
    assert!(nested_block_is_live(&reopened, note_a, bid).await);
    assert!(nested_block_is_live(&reopened, note_b, bid).await);
}

#[tokio::test]
async fn competing_registration_cannot_slip_between_validation_and_delete() {
    let device = DeviceId::from_bytes([0xe3; 16]);
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note_a = [0x71; 16];
    let note_b = [0x72; 16];
    let bid = [0x73; 16];
    upsert_block(&engine, note_a, bid, "note a", None).await;
    let peer_device = DeviceId::from_bytes([0xe4; 16]);
    let peer = LoroEngine::new(peer_device, Arc::new(Hlc::new(peer_device)));
    upsert_block(&peer, note_b, bid, "note b", None).await;
    let competing_update = peer.export_doc_update(note_b, None).await.unwrap();

    let (validated, resume) = engine.pause_next_bid_mutation_after_validation().await;
    let deleting = {
        let engine = engine.clone();
        tokio::spawn(async move {
            engine
                .record_local(OpPayload::BlockDelete { block_id: bid })
                .await
        })
    };
    validated.wait().await;

    let competing_started = Arc::new(tokio::sync::Barrier::new(2));
    let mut registering = {
        let engine = engine.clone();
        let competing_started = competing_started.clone();
        tokio::spawn(async move {
            competing_started.wait().await;
            engine.import_doc_update(note_b, &competing_update).await
        })
    };
    competing_started.wait().await;
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(25), &mut registering)
            .await
            .is_err(),
        "competing registration must wait while validated delete owns the transition"
    );
    assert_eq!(
        block_text(&engine, note_b, bid).await,
        None,
        "competing import must not mutate its note before ownership serialization"
    );
    assert_eq!(
        engine.inner.block_index.read().await.get(&bid),
        Some(&BTreeSet::from([note_a]))
    );

    resume.wait().await;
    deleting.await.unwrap().unwrap();
    registering.await.unwrap().unwrap();

    assert_eq!(engine.render_note(note_a).await.unwrap(), "");
    assert!(engine.render_note(note_b).await.unwrap().contains("note b"));
    assert_eq!(
        engine.inner.block_index.read().await.get(&bid),
        Some(&BTreeSet::from([note_b]))
    );
}
