use super::super::relocation::RelocationFailpoint;
use super::*;
use std::collections::BTreeSet;

fn stamped_line(indent: usize, text: &str, bid: [u8; 16]) -> String {
    format!(
        "{}- {} <!-- bid:{} -->\n",
        "  ".repeat(indent),
        text,
        uuid::Uuid::from_bytes(bid)
    )
}

async fn seed_note(engine: &LoroEngine, note_id: [u8; 16], slug: &str, content: String) {
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some(slug.to_string()),
            title: slug.to_string(),
            content,
            created_at_millis: 1,
        })
        .await
        .unwrap();
}

fn relocation_request(
    source_note_id: [u8; 16],
    root_bid: [u8; 16],
    destination_note_id: [u8; 16],
    target_bid: Option<[u8; 16]>,
    placement: MovePlacement,
) -> BlockRelocationRequest {
    let destination_slug = if source_note_id == destination_note_id {
        "2026-07-12"
    } else {
        "2026-07-11"
    };
    BlockRelocationRequest {
        move_id: [9; 16],
        source_note_id,
        source_slug: "2026-07-12".into(),
        root_bid,
        destination_note_id,
        destination_slug: destination_slug.into(),
        target_bid,
        placement,
        destination_seed: None,
    }
}

fn deterministic_daily_seed(slug: &str) -> crate::engine::RelocationNoteSeed {
    crate::engine::RelocationNoteSeed {
        display_alias: Some(slug.into()),
        title: slug.into(),
        content: format!("---\ntitle: {slug}\ncreated: {slug}T00:00:00Z\n---\n"),
        created_at_millis: 1_720_656_000_000,
    }
}

async fn block_structure(
    engine: &LoroEngine,
    note_id: [u8; 16],
    bid: [u8; 16],
) -> (TreeID, u16, Option<[u8; 16]>) {
    let docs = engine.inner.docs.read().await;
    let doc = docs.get(&note_id).unwrap();
    let tree = doc.get_tree("blocks");
    let node = find_node_by_block_id(&tree, &hex_id(&bid)).unwrap();
    let parent =
        read_meta_str(&tree, node, "parent").and_then(|parent| parse_note_id_from_hex(&parent));
    (node, read_indent_level(&tree, node).unwrap(), parent)
}

async fn block_props_typed(
    engine: &LoroEngine,
    note_id: [u8; 16],
    bid: [u8; 16],
) -> Vec<(String, prop_containers::ResolvedValue)> {
    let docs = engine.inner.docs.read().await;
    let doc = docs.get(&note_id).unwrap();
    let tree = doc.get_tree("blocks");
    let node = find_node_by_block_id(&tree, &hex_id(&bid)).unwrap();
    let meta = tree.get_meta(node).unwrap();
    let (props, prop_keys) = prop_containers::read_node_prop_containers(&meta).unwrap();
    prop_containers::read_props_typed(&props, &prop_keys)
}

async fn relocation_render_pair(
    engine: &LoroEngine,
    source: [u8; 16],
    destination: [u8; 16],
) -> (Option<String>, Option<String>) {
    (
        engine.render_note_full(source).await,
        engine.render_note_full(destination).await,
    )
}

async fn relocation_export_pair(
    engine: &LoroEngine,
    source: [u8; 16],
    destination: [u8; 16],
) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    (
        engine.export_doc_update(source, None).await,
        engine.export_doc_update(destination, None).await,
    )
}

async fn remove_indent_metadata(engine: &LoroEngine, note_id: [u8; 16], bid: [u8; 16]) {
    let doc = engine.doc_for_note_mut(note_id).await;
    let tree = doc.get_tree("blocks");
    let node = find_node_by_block_id(&tree, &hex_id(&bid)).unwrap();
    tree.get_meta(node).unwrap().delete("indent_level").unwrap();
    doc.commit();
}

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

async fn live_bid_count(engine: &LoroEngine, note_id: [u8; 16], bid: [u8; 16]) -> usize {
    let docs = engine.inner.docs.read().await;
    let Some(doc) = docs.get(&note_id) else {
        return 0;
    };
    let tree = doc.get_tree("blocks");
    tree.children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|node| read_meta_str(&tree, *node, "block_id") == Some(hex_id(&bid)))
        .count()
}

async fn delete_live_bid(engine: &LoroEngine, note_id: [u8; 16], bid: [u8; 16]) {
    let doc = engine.doc_for_note_mut(note_id).await;
    let tree = doc.get_tree("blocks");
    let node = find_node_by_block_id(&tree, &hex_id(&bid)).unwrap();
    tree.delete(node).unwrap();
    doc.commit();
}

async fn move_live_bids_before(
    engine: &LoroEngine,
    note_id: [u8; 16],
    bids: &[[u8; 16]],
    before_bid: [u8; 16],
) {
    let doc = engine.doc_for_note_mut(note_id).await;
    let tree = doc.get_tree("blocks");
    let anchor = find_node_by_block_id(&tree, &hex_id(&before_bid)).unwrap();
    for bid in bids {
        let node = find_node_by_block_id(&tree, &hex_id(bid)).unwrap();
        tree.mov_before(node, anchor).unwrap();
    }
    doc.commit();
}

async fn set_block_structure(
    engine: &LoroEngine,
    note_id: [u8; 16],
    bid: [u8; 16],
    indent: u16,
    parent: Option<[u8; 16]>,
) {
    let doc = engine.doc_for_note_mut(note_id).await;
    let tree = doc.get_tree("blocks");
    let node = find_node_by_block_id(&tree, &hex_id(&bid)).unwrap();
    let meta = tree.get_meta(node).unwrap();
    meta.insert("indent_level", indent as i64).unwrap();
    meta.insert(
        "parent",
        parent.map(|value| hex_id(&value)).unwrap_or_default(),
    )
    .unwrap();
    doc.commit();
}

async fn overwrite_relocation_proof(
    engine: &LoroEngine,
    note_id: [u8; 16],
    bid: [u8; 16],
    move_id: [u8; 16],
    request_hash: [u8; 32],
) {
    let doc = engine.doc_for_note_mut(note_id).await;
    let tree = doc.get_tree("blocks");
    let node = find_node_by_block_id(&tree, &hex_id(&bid)).unwrap();
    let meta = tree.get_meta(node).unwrap();
    meta.insert("relocation_move_id", hex_id(&move_id)).unwrap();
    meta.insert("relocation_request_hash", hex::encode(request_hash))
        .unwrap();
    doc.commit();
}

async fn insert_duplicate_bid(engine: &LoroEngine, note_id: [u8; 16], bid: [u8; 16], text: &str) {
    let doc = engine.doc_for_note_mut(note_id).await;
    let tree = doc.get_tree("blocks");
    let node = tree.create(TreeParentId::Root).unwrap();
    let meta = tree.get_meta(node).unwrap();
    meta.insert("block_id", hex_id(&bid)).unwrap();
    meta.insert("indent_level", 0i64).unwrap();
    meta.insert("parent", "").unwrap();
    write_block_text(&meta, text).unwrap();
    doc.commit();
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

#[tokio::test]
async fn same_note_relocation_supports_every_placement_after_source_removal() {
    let note_id = [0x81; 16];
    let lead = [0x82; 16];
    let target = [0x83; 16];
    let target_child = [0x84; 16];
    let between = [0x85; 16];
    let root = [0x86; 16];
    let root_child = [0x87; 16];
    let tail = [0x88; 16];
    let content = [
        stamped_line(0, "lead", lead),
        stamped_line(0, "target", target),
        stamped_line(1, "target-child", target_child),
        stamped_line(0, "between", between),
        stamped_line(0, "moved-root", root),
        stamped_line(1, "moved-child", root_child),
        stamped_line(0, "tail", tail),
    ]
    .concat();

    let cases = [
        (
            MovePlacement::Before,
            Some(target),
            vec![
                "lead",
                "moved-root",
                "moved-child",
                "target",
                "target-child",
                "between",
                "tail",
            ],
            0,
            None,
        ),
        (
            MovePlacement::Inside,
            Some(target),
            vec![
                "lead",
                "target",
                "target-child",
                "moved-root",
                "moved-child",
                "between",
                "tail",
            ],
            1,
            Some(target),
        ),
        (
            MovePlacement::After,
            Some(target),
            vec![
                "lead",
                "target",
                "target-child",
                "moved-root",
                "moved-child",
                "between",
                "tail",
            ],
            0,
            None,
        ),
        (
            MovePlacement::Append,
            None,
            vec![
                "lead",
                "target",
                "target-child",
                "between",
                "tail",
                "moved-root",
                "moved-child",
            ],
            0,
            None,
        ),
    ];

    for (placement, target_bid, expected, root_indent, root_parent) in cases {
        let device = test_device();
        let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
        seed_note(&engine, note_id, "2026-07-12", content.clone()).await;
        let root_identity = block_structure(&engine, note_id, root).await.0;
        let child_identity = block_structure(&engine, note_id, root_child).await.0;

        let outcome = engine
            .relocate_subtree(relocation_request(
                note_id, root, note_id, target_bid, placement,
            ))
            .await
            .unwrap();

        assert_eq!(outcome.status, BlockRelocationStatus::Applied);
        assert_eq!(outcome.notes.len(), 1);
        assert!(outcome.notes[0].changed);
        assert!(!outcome.notes[0].created);
        assert_eq!(block_texts(&engine, note_id).await, expected);
        assert_eq!(
            block_structure(&engine, note_id, root).await,
            (root_identity, root_indent, root_parent)
        );
        assert_eq!(
            block_structure(&engine, note_id, root_child).await,
            (child_identity, root_indent + 1, Some(root))
        );
    }
}

#[tokio::test]
async fn same_note_existing_placement_is_a_no_op() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note_id = [0x91; 16];
    let root = [0x92; 16];
    let child = [0x93; 16];
    let target = [0x94; 16];
    let content = [
        stamped_line(0, "moved-root", root),
        stamped_line(1, "moved-child", child),
        stamped_line(0, "target", target),
    ]
    .concat();
    seed_note(&engine, note_id, "2026-07-12", content).await;
    let version_before = engine.doc_version(note_id).await.unwrap();
    let rendered_before = engine.render_note_full(note_id).await.unwrap();

    let outcome = engine
        .relocate_subtree(relocation_request(
            note_id,
            root,
            note_id,
            Some(target),
            MovePlacement::Before,
        ))
        .await
        .unwrap();

    assert_eq!(outcome.status, BlockRelocationStatus::NoOp);
    assert_eq!(outcome.notes.len(), 1);
    assert!(!outcome.notes[0].changed);
    assert_eq!(outcome.notes[0].pre_version, version_before);
    assert_eq!(engine.doc_version(note_id).await.unwrap(), version_before);
    assert_eq!(
        engine.render_note_full(note_id).await.unwrap(),
        rendered_before
    );
}

#[tokio::test]
async fn cross_note_relocation_preserves_nested_identity_and_typed_properties() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let source = [0xa1; 16];
    let destination = [0xa2; 16];
    let root = [0xa3; 16];
    let child = [0xa4; 16];
    let tail = [0xa5; 16];
    let target = [0xa6; 16];
    seed_note(
        &engine,
        source,
        "2026-07-12",
        [
            stamped_line(0, "moved-root", root),
            stamped_line(1, "moved-child", child),
            stamped_line(0, "source-tail", tail),
        ]
        .concat(),
    )
    .await;
    seed_note(
        &engine,
        destination,
        "2026-07-11",
        stamped_line(0, "target", target),
    )
    .await;

    let property_ops = [
        (
            "text-scalar",
            PropOp::SetScalar(PropScalar::Text("open".into())),
        ),
        ("int", PropOp::SetScalar(PropScalar::Int(42))),
        ("float", PropOp::SetScalar(PropScalar::Float(2.5))),
        ("bool", PropOp::SetScalar(PropScalar::Bool(true))),
        ("free", PropOp::SetText("mergeable text".into())),
        (
            "ordered",
            PropOp::AddToList(PropScalar::Text("first".into())),
        ),
        ("ordered", PropOp::AddToList(PropScalar::Int(7))),
        ("ordered", PropOp::AddToList(PropScalar::Bool(false))),
    ];
    for (key, value) in property_ops {
        engine
            .record_local(OpPayload::BlockPropertySet {
                note_id: source,
                block_id: root,
                key: key.into(),
                value,
            })
            .await
            .unwrap();
    }
    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: source,
            block_id: child,
            key: "child-text".into(),
            value: PropOp::SetText("child value".into()),
        })
        .await
        .unwrap();
    let root_props_before = block_props_typed(&engine, source, root).await;
    let child_props_before = block_props_typed(&engine, source, child).await;
    let source_pre_version = engine.doc_version(source).await.unwrap();
    let destination_pre_version = engine.doc_version(destination).await.unwrap();

    let outcome = engine
        .relocate_subtree(relocation_request(
            source,
            root,
            destination,
            Some(target),
            MovePlacement::Inside,
        ))
        .await
        .unwrap();

    assert_eq!(outcome.status, BlockRelocationStatus::Applied);
    assert_eq!(
        outcome
            .notes
            .iter()
            .map(|note| (
                note.note_id,
                note.pre_version.clone(),
                note.changed,
                note.created
            ))
            .collect::<Vec<_>>(),
        vec![
            (source, source_pre_version, true, false),
            (destination, destination_pre_version, true, false),
        ]
    );
    assert_eq!(block_texts(&engine, source).await, vec!["source-tail"]);
    assert_eq!(
        block_texts(&engine, destination).await,
        vec![
            "target",
            "moved-root",
            "text-scalar:: open",
            "int:: 42",
            "float:: 2.5",
            "bool:: true",
            "free:: mergeable text",
            "ordered:: first, 7, false",
            "moved-child",
            "child-text:: child value",
        ]
    );
    assert_eq!(block_structure(&engine, destination, root).await.1, 1);
    assert_eq!(
        block_structure(&engine, destination, root).await.2,
        Some(target)
    );
    assert_eq!(block_structure(&engine, destination, child).await.1, 2);
    assert_eq!(
        block_structure(&engine, destination, child).await.2,
        Some(root)
    );
    assert_eq!(
        block_props_typed(&engine, destination, root).await,
        root_props_before
    );
    assert_eq!(
        block_props_typed(&engine, destination, child).await,
        child_props_before
    );
    assert_eq!(
        engine.inner.block_index.read().await.get(&root),
        Some(&BTreeSet::from([destination]))
    );
    assert_eq!(
        engine.inner.block_index.read().await.get(&child),
        Some(&BTreeSet::from([destination]))
    );
}

#[tokio::test]
async fn cross_note_relocation_preserves_an_existing_empty_typed_list() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let source = [0xa7; 16];
    let destination = [0xa8; 16];
    let root = [0xa9; 16];
    let target = [0xaa; 16];
    seed_note(
        &engine,
        source,
        "2026-07-12",
        stamped_line(0, "moved", root),
    )
    .await;
    seed_note(
        &engine,
        destination,
        "2026-07-11",
        stamped_line(0, "target", target),
    )
    .await;
    for value in [
        PropOp::AddToList(PropScalar::Text("only".into())),
        PropOp::RemoveFromList(PropScalar::Text("only".into())),
    ] {
        engine
            .record_local(OpPayload::BlockPropertySet {
                note_id: source,
                block_id: root,
                key: "empty-list".into(),
                value,
            })
            .await
            .unwrap();
    }
    let expected = vec![(
        "empty-list".to_string(),
        prop_containers::ResolvedValue::List(Vec::new()),
    )];
    assert_eq!(block_props_typed(&engine, source, root).await, expected);

    engine
        .relocate_subtree(relocation_request(
            source,
            root,
            destination,
            Some(target),
            MovePlacement::Inside,
        ))
        .await
        .unwrap();

    assert_eq!(
        block_props_typed(&engine, destination, root).await,
        expected,
        "the empty ordered-list container and property key must survive relocation"
    );
}

#[tokio::test]
async fn rejected_relocation_preconditions_leave_notes_byte_identical() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let source = [0xb1; 16];
    let destination = [0xb2; 16];
    let root = [0xb3; 16];
    let child = [0xb4; 16];
    let target = [0xb5; 16];
    seed_note(
        &engine,
        source,
        "2026-07-12",
        [
            stamped_line(0, "root", root),
            stamped_line(1, "child", child),
        ]
        .concat(),
    )
    .await;
    seed_note(
        &engine,
        destination,
        "2026-07-11",
        stamped_line(0, "target", target),
    )
    .await;

    let invalid = [
        relocation_request(source, root, source, Some(root), MovePlacement::Before),
        relocation_request(source, root, source, Some(child), MovePlacement::After),
        relocation_request(
            source,
            [0xff; 16],
            destination,
            Some(target),
            MovePlacement::Inside,
        ),
        relocation_request(
            source,
            root,
            destination,
            Some([0xfe; 16]),
            MovePlacement::Inside,
        ),
        relocation_request(
            source,
            root,
            destination,
            Some(target),
            MovePlacement::Append,
        ),
        relocation_request(source, root, destination, None, MovePlacement::Before),
    ];
    for request in invalid {
        let before = relocation_render_pair(&engine, source, destination).await;
        let err = engine.relocate_subtree(request).await.unwrap_err();
        assert!(matches!(err, SyncError::RelocationRejected(_)));
        assert_eq!(
            relocation_render_pair(&engine, source, destination).await,
            before
        );
    }

    let third = [0xb6; 16];
    seed_note(&engine, third, "third", stamped_line(0, "duplicate", root)).await;
    let source_before = engine.render_note_full(source).await.unwrap();
    let destination_before = engine.render_note_full(destination).await.unwrap();
    let third_before = engine.render_note_full(third).await.unwrap();
    let err = engine
        .relocate_subtree(relocation_request(
            source,
            root,
            destination,
            Some(target),
            MovePlacement::Inside,
        ))
        .await
        .unwrap_err();
    assert!(matches!(err, SyncError::RelocationRejected(message) if message.contains("ambiguous")));
    assert_eq!(
        engine.render_note_full(source).await.unwrap(),
        source_before
    );
    assert_eq!(
        engine.render_note_full(destination).await.unwrap(),
        destination_before
    );
    assert_eq!(engine.render_note_full(third).await.unwrap(), third_before);
}

#[tokio::test]
async fn malformed_source_descendant_indent_rejects_without_mutation() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let source = [0xb7; 16];
    let destination = [0xb8; 16];
    let root = [0xb9; 16];
    let child = [0xba; 16];
    let target = [0xbb; 16];
    seed_note(
        &engine,
        source,
        "2026-07-12",
        [
            stamped_line(0, "root", root),
            stamped_line(1, "malformed-child", child),
        ]
        .concat(),
    )
    .await;
    seed_note(
        &engine,
        destination,
        "2026-07-11",
        stamped_line(0, "target", target),
    )
    .await;
    remove_indent_metadata(&engine, source, child).await;
    let before = relocation_render_pair(&engine, source, destination).await;

    let err = engine
        .relocate_subtree(relocation_request(
            source,
            root,
            destination,
            Some(target),
            MovePlacement::Inside,
        ))
        .await
        .unwrap_err();

    assert!(matches!(err, SyncError::RelocationRejected(message) if message.contains("indent")));
    assert_eq!(
        relocation_render_pair(&engine, source, destination).await,
        before
    );
}

#[tokio::test]
async fn malformed_target_descendant_indent_rejects_without_mutation() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let source = [0xbc; 16];
    let destination = [0xbd; 16];
    let root = [0xbe; 16];
    let target = [0xbf; 16];
    let target_child = [0xc0; 16];
    seed_note(&engine, source, "2026-07-12", stamped_line(0, "root", root)).await;
    seed_note(
        &engine,
        destination,
        "2026-07-11",
        [
            stamped_line(0, "target", target),
            stamped_line(1, "malformed-target-child", target_child),
        ]
        .concat(),
    )
    .await;
    remove_indent_metadata(&engine, destination, target_child).await;
    let before = relocation_render_pair(&engine, source, destination).await;

    let err = engine
        .relocate_subtree(relocation_request(
            source,
            root,
            destination,
            Some(target),
            MovePlacement::After,
        ))
        .await
        .unwrap_err();

    assert!(matches!(err, SyncError::RelocationRejected(message) if message.contains("indent")));
    assert_eq!(
        relocation_render_pair(&engine, source, destination).await,
        before
    );
}

#[tokio::test]
async fn trusted_daily_seed_uses_frontmatter_without_placeholder_block() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let source = [0xc1; 16];
    let destination = [0xc2; 16];
    let root = [0xc3; 16];
    let placeholder = [0xc4; 16];
    seed_note(
        &engine,
        source,
        "2026-07-12",
        stamped_line(0, "moved", root),
    )
    .await;
    let mut request = relocation_request(source, root, destination, None, MovePlacement::Append);
    request.destination_seed = Some(crate::engine::RelocationNoteSeed {
        display_alias: Some("2026-07-11".into()),
        title: "2026-07-11".into(),
        content: format!(
            "---\ntitle: 2026-07-11\ncreated: 2026-07-11T00:00:00Z\n---\n{}",
            stamped_line(0, "", placeholder)
        ),
        // `content` frontmatter is the authoritative rendered timestamp;
        // this canonical seed field remains available to Task 4 request
        // hashing/receipts and intentionally does not replace frontmatter.
        created_at_millis: 1_720_656_000_000,
    });

    let outcome = engine.relocate_subtree(request).await.unwrap();

    assert_eq!(outcome.status, BlockRelocationStatus::Applied);
    assert_eq!(outcome.notes.len(), 2);
    assert!(outcome.notes[1].created);
    assert!(outcome.notes[1].pre_version.is_empty());
    assert_eq!(block_texts(&engine, destination).await, vec!["moved"]);
    assert_eq!(
        engine.inner.block_index.read().await.get(&placeholder),
        None
    );
    let full = engine.render_note_full(destination).await.unwrap();
    assert!(full.starts_with("---\ntitle: 2026-07-11\ncreated: 2026-07-11T00:00:00Z\n---\n"));
    assert!(!full.contains(&hex_id(&placeholder)));
    assert!(full.contains(&uuid::Uuid::from_bytes(root).to_string()));
}

#[tokio::test]
async fn deterministic_seed_is_ignored_for_an_existing_cross_note_destination() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let source = [0xc5; 16];
    let destination = [0xc6; 16];
    let root = [0xc7; 16];
    let existing = [0xc8; 16];
    seed_note(
        &engine,
        source,
        "2026-07-12",
        stamped_line(0, "moved", root),
    )
    .await;
    seed_note(
        &engine,
        destination,
        "2026-07-11",
        stamped_line(0, "existing", existing),
    )
    .await;
    let mut request = relocation_request(source, root, destination, None, MovePlacement::Append);
    request.destination_seed = Some(deterministic_daily_seed("2026-07-11"));

    let outcome = engine.relocate_subtree(request).await.unwrap();

    assert_eq!(outcome.status, BlockRelocationStatus::Applied);
    assert_eq!(
        block_texts(&engine, destination).await,
        vec!["existing", "moved"]
    );
}

#[tokio::test]
async fn deterministic_seed_remains_invalid_for_a_same_note_move() {
    let device = test_device();
    let engine = LoroEngine::new(device, Arc::new(Hlc::new(device)));
    let note = [0xc9; 16];
    let root = [0xca; 16];
    seed_note(&engine, note, "2026-07-12", stamped_line(0, "root", root)).await;
    let mut request = relocation_request(note, root, note, None, MovePlacement::Append);
    request.destination_seed = Some(deterministic_daily_seed("2026-07-12"));

    let error = engine.relocate_subtree(request).await.unwrap_err();

    assert!(matches!(
        error,
        SyncError::RelocationRejected(message)
            if message.contains("existing destination cannot include a note seed")
    ));
    assert_eq!(block_texts(&engine, note).await, vec!["root"]);
}

async fn snapshot_has_live_bid(
    snapshot_dir: &std::path::Path,
    note_id: [u8; 16],
    bid: [u8; 16],
) -> bool {
    let path = snapshot_dir.join(format!("{}.bin", hex_id(&note_id)));
    let Ok(bytes) = tokio::fs::read(path).await else {
        return false;
    };
    let doc = LoroDoc::new();
    if doc.import(&bytes).is_err() {
        return false;
    }
    let tree = doc.get_tree("blocks");
    let Some(node) = find_node_by_block_id(&tree, &hex_id(&bid)) else {
        return false;
    };
    !matches!(tree.is_node_deleted(&node), Ok(true))
}

async fn open_persistent_relocation_engine(
    device: DeviceId,
    snapshot_dir: &std::path::Path,
    materialize_dir: &std::path::Path,
) -> LoroEngine {
    LoroEngine::with_dirs(
        device,
        Arc::new(Hlc::new(device)),
        snapshot_dir.to_path_buf(),
        Some(materialize_dir.to_path_buf()),
    )
    .await
    .unwrap()
}

async fn seed_recovery_pair(
    engine: &LoroEngine,
    source: [u8; 16],
    destination: [u8; 16],
    root: [u8; 16],
    child: [u8; 16],
    target: [u8; 16],
) {
    seed_note(
        engine,
        source,
        "2026-07-12",
        [
            stamped_line(0, "moved-root", root),
            stamped_line(1, "moved-child", child),
        ]
        .concat(),
    )
    .await;
    seed_note(
        engine,
        destination,
        "2026-07-11",
        stamped_line(0, "target", target),
    )
    .await;
}

fn assert_recovery_required(error: SyncError, move_id: [u8; 16]) {
    assert!(matches!(
        error,
        SyncError::RelocationRecoveryRequired {
            move_id: actual,
            ..
        } if actual == move_id
    ));
}

fn assert_stale_relocation(error: SyncError) {
    assert!(matches!(
        error,
        SyncError::RelocationRejected(message) if message.contains("receipt was pruned")
    ));
}

#[tokio::test]
async fn boot_recovery_completes_every_persisted_relocation_phase_in_snapshot_order() {
    let cases = [
        (RelocationFailpoint::AfterPrepared, true, false),
        (RelocationFailpoint::AfterDestinationDurable, true, true),
        (RelocationFailpoint::AfterSourceDurable, false, true),
    ];

    for (failpoint, source_is_durable, destination_is_durable) in cases {
        let root_dir = tempfile::tempdir().unwrap();
        let snapshot_dir = root_dir.path().join("snapshots");
        let materialize_dir = root_dir.path().join("notes");
        let device = DeviceId::from_bytes([failpoint as u8 + 0x31; 16]);
        let source = [0xd1; 16];
        let destination = [0xd2; 16];
        let root = [0xd3; 16];
        let child = [0xd4; 16];
        let target = [0xd5; 16];
        let request = relocation_request(
            source,
            root,
            destination,
            Some(target),
            MovePlacement::Inside,
        );
        let engine =
            open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
        seed_recovery_pair(&engine, source, destination, root, child, target).await;
        engine.inject_relocation_failure_once(failpoint).await;

        let error = engine
            .relocate_subtree(request.clone())
            .await
            .expect_err("the injected checkpoint must interrupt the move");
        assert_recovery_required(error, request.move_id);
        assert_eq!(
            snapshot_has_live_bid(&snapshot_dir, source, root).await,
            source_is_durable,
            "source snapshot at {failpoint:?}"
        );
        assert_eq!(
            snapshot_has_live_bid(&snapshot_dir, destination, root).await,
            destination_is_durable,
            "destination snapshot at {failpoint:?}"
        );
        drop(engine);

        let recovered =
            open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
        assert!(!nested_block_is_live(&recovered, source, root).await);
        assert!(!nested_block_is_live(&recovered, source, child).await);
        assert!(nested_block_is_live(&recovered, destination, root).await);
        assert!(nested_block_is_live(&recovered, destination, child).await);
        assert_eq!(
            recovered.inner.block_index.read().await.get(&root),
            Some(&BTreeSet::from([destination]))
        );
        assert!(
            tokio::fs::read_to_string(materialize_dir.join("2026-07-11.md"))
                .await
                .unwrap()
                .contains("moved-root")
        );
        assert!(
            !tokio::fs::read_to_string(materialize_dir.join("2026-07-12.md"))
                .await
                .unwrap()
                .contains("moved-root")
        );
    }
}

#[tokio::test]
async fn repeated_boot_recovery_is_a_no_op_and_receipt_replays() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x41; 16]);
    let source = [0xe1; 16];
    let destination = [0xe2; 16];
    let root = [0xe3; 16];
    let child = [0xe4; 16];
    let target = [0xe5; 16];
    let request = relocation_request(
        source,
        root,
        destination,
        Some(target),
        MovePlacement::After,
    );
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_recovery_pair(&engine, source, destination, root, child, target).await;
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterPrepared)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    drop(engine);

    let recovered =
        open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    let source_version = recovered.doc_version(source).await.unwrap();
    let destination_version = recovered.doc_version(destination).await.unwrap();
    drop(recovered);

    let recovered_again =
        open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    assert_eq!(
        recovered_again.doc_version(source).await.unwrap(),
        source_version
    );
    assert_eq!(
        recovered_again.doc_version(destination).await.unwrap(),
        destination_version
    );
    let replay = recovered_again.relocate_subtree(request).await.unwrap();
    assert_eq!(replay.status, BlockRelocationStatus::Replayed);
    assert_eq!(
        recovered_again.doc_version(source).await.unwrap(),
        source_version
    );
    assert_eq!(
        recovered_again.doc_version(destination).await.unwrap(),
        destination_version
    );
}

#[tokio::test]
async fn receipt_replay_and_conflict_survive_reload() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x42; 16]);
    let source = [0xf1; 16];
    let destination = [0xf2; 16];
    let root = [0xf3; 16];
    let child = [0xf4; 16];
    let target = [0xf5; 16];
    let request = relocation_request(
        source,
        root,
        destination,
        Some(target),
        MovePlacement::Before,
    );
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_recovery_pair(&engine, source, destination, root, child, target).await;
    let applied = engine.relocate_subtree(request.clone()).await.unwrap();
    assert_eq!(applied.status, BlockRelocationStatus::Applied);
    let replay = engine.relocate_subtree(request.clone()).await.unwrap();
    assert_eq!(replay.status, BlockRelocationStatus::Replayed);
    assert_eq!(replay.notes, applied.notes);

    let mut conflicting = request.clone();
    conflicting.placement = MovePlacement::After;
    assert!(matches!(
        engine.relocate_subtree(conflicting.clone()).await,
        Err(SyncError::RelocationConflict(_))
    ));
    drop(engine);

    let reopened = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    assert_eq!(
        reopened.relocate_subtree(request).await.unwrap().status,
        BlockRelocationStatus::Replayed
    );
    assert!(matches!(
        reopened.relocate_subtree(conflicting).await,
        Err(SyncError::RelocationConflict(_))
    ));
}

#[tokio::test]
async fn checked_materialization_failure_keeps_preceding_phase() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x43; 16]);
    let source = [0x11; 16];
    let destination = [0x12; 16];
    let root = [0x13; 16];
    let child = [0x14; 16];
    let target = [0x15; 16];
    let request = relocation_request(
        source,
        root,
        destination,
        Some(target),
        MovePlacement::Inside,
    );
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_recovery_pair(&engine, source, destination, root, child, target).await;
    let destination_md = materialize_dir.join("2026-07-11.md");
    tokio::fs::remove_file(&destination_md).await.unwrap();
    tokio::fs::create_dir(&destination_md).await.unwrap();

    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    tokio::fs::remove_dir(&destination_md).await.unwrap();
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterDestinationDurable)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    drop(engine);

    let recovered =
        open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    assert!(!nested_block_is_live(&recovered, source, root).await);
    assert!(nested_block_is_live(&recovered, destination, root).await);
}

#[tokio::test]
async fn checked_source_snapshot_failure_keeps_destination_durable_phase() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x44; 16]);
    let source = [0x21; 16];
    let destination = [0x22; 16];
    let root = [0x23; 16];
    let child = [0x24; 16];
    let target = [0x25; 16];
    let request = relocation_request(
        source,
        root,
        destination,
        Some(target),
        MovePlacement::Inside,
    );
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_recovery_pair(&engine, source, destination, root, child, target).await;
    let source_snapshot = snapshot_dir.join(format!("{}.bin", hex_id(&source)));
    tokio::fs::remove_file(&source_snapshot).await.unwrap();
    tokio::fs::create_dir(&source_snapshot).await.unwrap();

    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    tokio::fs::remove_dir(&source_snapshot).await.unwrap();
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterSourceDurable)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    drop(engine);

    let recovered =
        open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    assert!(!nested_block_is_live(&recovered, source, root).await);
    assert!(nested_block_is_live(&recovered, destination, root).await);
}

#[tokio::test]
async fn destination_durable_retry_restores_a_removed_destination_subtree_before_source_delete() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x48; 16]);
    let source = [0x71; 16];
    let destination = [0x72; 16];
    let root = [0x73; 16];
    let child = [0x74; 16];
    let target = [0x75; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_recovery_pair(&engine, source, destination, root, child, target).await;
    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: source,
            block_id: root,
            key: "phase".into(),
            value: PropOp::SetText("captured".into()),
        })
        .await
        .unwrap();
    let expected_props = block_props_typed(&engine, source, root).await;
    let request = relocation_request(
        source,
        root,
        destination,
        Some(target),
        MovePlacement::After,
    );
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterDestinationDurable)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    delete_live_bid(&engine, destination, root).await;
    assert_eq!(live_bid_count(&engine, destination, root).await, 0);

    assert_eq!(
        engine.relocate_subtree(request).await.unwrap().status,
        BlockRelocationStatus::Replayed
    );
    assert_eq!(live_bid_count(&engine, destination, root).await, 1);
    assert_eq!(live_bid_count(&engine, destination, child).await, 1);
    assert!(!nested_block_is_live(&engine, source, root).await);
    assert!(!nested_block_is_live(&engine, source, child).await);
    assert_eq!(
        block_props_typed(&engine, destination, root).await,
        expected_props
    );
    assert_eq!(
        block_texts(&engine, destination).await,
        vec!["target", "moved-root", "phase:: captured", "moved-child"]
    );
    drop(engine);

    let reopened = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    assert_eq!(live_bid_count(&reopened, destination, root).await, 1);
    assert_eq!(live_bid_count(&reopened, destination, child).await, 1);
    assert!(!nested_block_is_live(&reopened, source, root).await);
}

#[tokio::test]
async fn source_durable_retry_repairs_destination_and_redeletes_restored_source_nodes() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x49; 16]);
    let source = [0x76; 16];
    let destination = [0x77; 16];
    let root = [0x78; 16];
    let child = [0x79; 16];
    let target = [0x7a; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_recovery_pair(&engine, source, destination, root, child, target).await;
    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: source,
            block_id: child,
            key: "typed".into(),
            value: PropOp::SetScalar(PropScalar::Int(7)),
        })
        .await
        .unwrap();
    let expected_props = block_props_typed(&engine, source, child).await;
    let source_snapshot = engine.export_doc_update(source, None).await.unwrap();
    let request = relocation_request(
        source,
        root,
        destination,
        Some(target),
        MovePlacement::After,
    );
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterSourceDurable)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );

    let destination_doc = engine.doc_for_note_mut(destination).await;
    let destination_tree = destination_doc.get_tree("blocks");
    let destination_root = find_node_by_block_id(&destination_tree, &hex_id(&root)).unwrap();
    let destination_meta = destination_tree.get_meta(destination_root).unwrap();
    write_block_text(&destination_meta, "corrupt-root").unwrap();
    destination_doc.commit();
    let restored_source = LoroDoc::new();
    engine.set_doc_peer(&restored_source);
    restored_source.import(&source_snapshot).unwrap();
    engine
        .inner
        .docs
        .write()
        .await
        .insert(source, restored_source);
    assert!(nested_block_is_live(&engine, source, root).await);
    assert_eq!(block_texts(&engine, destination).await[1], "corrupt-root");

    assert_eq!(
        engine.relocate_subtree(request).await.unwrap().status,
        BlockRelocationStatus::Replayed
    );
    assert_eq!(live_bid_count(&engine, destination, root).await, 1);
    assert_eq!(live_bid_count(&engine, destination, child).await, 1);
    assert!(!nested_block_is_live(&engine, source, root).await);
    assert!(!nested_block_is_live(&engine, source, child).await);
    assert_eq!(
        block_props_typed(&engine, destination, child).await,
        expected_props
    );
    assert_eq!(
        block_texts(&engine, destination).await,
        vec!["target", "moved-root", "moved-child", "typed:: 7"]
    );
    drop(engine);

    let reopened = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    assert_eq!(live_bid_count(&reopened, destination, root).await, 1);
    assert_eq!(live_bid_count(&reopened, destination, child).await, 1);
    assert!(!nested_block_is_live(&reopened, source, root).await);
    assert!(!nested_block_is_live(&reopened, source, child).await);
}

#[tokio::test]
async fn prepared_retry_places_the_subtree_relative_to_captured_destination_anchors() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x4b; 16]);
    let source = [0x81; 16];
    let destination = [0x82; 16];
    let root = [0x83; 16];
    let child = [0x84; 16];
    let target = [0x85; 16];
    let tail = [0x86; 16];
    let inserted = [0x87; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_recovery_pair(&engine, source, destination, root, child, target).await;
    upsert_block(&engine, destination, tail, "tail", Some(target)).await;
    let request = relocation_request(
        source,
        root,
        destination,
        Some(target),
        MovePlacement::After,
    );
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterPrepared)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );

    upsert_block(&engine, destination, inserted, "inserted", None).await;
    move_live_bids_before(&engine, destination, &[inserted], target).await;
    assert_eq!(
        block_texts(&engine, destination).await,
        vec!["inserted", "target", "tail"]
    );

    assert_eq!(
        engine.relocate_subtree(request).await.unwrap().status,
        BlockRelocationStatus::Replayed
    );
    assert_eq!(
        block_texts(&engine, destination).await,
        vec!["inserted", "target", "moved-root", "moved-child", "tail"]
    );
    assert!(!nested_block_is_live(&engine, source, root).await);
}

#[tokio::test]
async fn durable_retries_restore_the_captured_destination_sibling_order() {
    for (case, failpoint) in [
        (0x4c, RelocationFailpoint::AfterDestinationDurable),
        (0x4d, RelocationFailpoint::AfterSourceDurable),
    ] {
        let root_dir = tempfile::tempdir().unwrap();
        let snapshot_dir = root_dir.path().join("snapshots");
        let materialize_dir = root_dir.path().join("notes");
        let device = DeviceId::from_bytes([case; 16]);
        let source = [case.wrapping_add(0x40); 16];
        let destination = [case.wrapping_add(0x41); 16];
        let root = [case.wrapping_add(0x42); 16];
        let child = [case.wrapping_add(0x43); 16];
        let target = [case.wrapping_add(0x44); 16];
        let tail = [case.wrapping_add(0x45); 16];
        let engine =
            open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
        seed_recovery_pair(&engine, source, destination, root, child, target).await;
        upsert_block(&engine, destination, tail, "tail", Some(target)).await;
        let request = relocation_request(
            source,
            root,
            destination,
            Some(target),
            MovePlacement::After,
        );
        engine.inject_relocation_failure_once(failpoint).await;
        assert_recovery_required(
            engine.relocate_subtree(request.clone()).await.unwrap_err(),
            request.move_id,
        );
        assert_eq!(
            block_texts(&engine, destination).await,
            vec!["target", "moved-root", "moved-child", "tail"]
        );

        move_live_bids_before(&engine, destination, &[root, child], target).await;
        assert_eq!(
            block_texts(&engine, destination).await,
            vec!["moved-root", "moved-child", "target", "tail"]
        );

        assert_eq!(
            engine.relocate_subtree(request).await.unwrap().status,
            BlockRelocationStatus::Replayed
        );
        assert_eq!(
            block_texts(&engine, destination).await,
            vec!["target", "moved-root", "moved-child", "tail"],
            "destination order after {failpoint:?}"
        );
        assert!(!nested_block_is_live(&engine, source, root).await);
        assert_eq!(live_bid_count(&engine, destination, root).await, 1);
        assert_eq!(live_bid_count(&engine, destination, child).await, 1);
    }
}

#[tokio::test]
async fn same_note_durable_retries_restore_target_relative_sibling_order() {
    for (case, failpoint) in [
        (0x4e, RelocationFailpoint::AfterDestinationDurable),
        (0x4f, RelocationFailpoint::AfterSourceDurable),
    ] {
        let root_dir = tempfile::tempdir().unwrap();
        let snapshot_dir = root_dir.path().join("snapshots");
        let materialize_dir = root_dir.path().join("notes");
        let device = DeviceId::from_bytes([case; 16]);
        let note = [case.wrapping_add(0x40); 16];
        let root = [case.wrapping_add(0x41); 16];
        let child = [case.wrapping_add(0x42); 16];
        let target = [case.wrapping_add(0x43); 16];
        let tail = [case.wrapping_add(0x44); 16];
        let engine =
            open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
        seed_note(
            &engine,
            note,
            "2026-07-12",
            [
                stamped_line(0, "target", target),
                stamped_line(0, "tail", tail),
                stamped_line(0, "moved-root", root),
                stamped_line(1, "moved-child", child),
            ]
            .concat(),
        )
        .await;
        let request = relocation_request(note, root, note, Some(target), MovePlacement::After);
        engine.inject_relocation_failure_once(failpoint).await;
        assert_recovery_required(
            engine.relocate_subtree(request.clone()).await.unwrap_err(),
            request.move_id,
        );
        assert_eq!(
            block_texts(&engine, note).await,
            vec!["target", "moved-root", "moved-child", "tail"]
        );

        move_live_bids_before(&engine, note, &[root, child], target).await;
        assert_eq!(
            block_texts(&engine, note).await,
            vec!["moved-root", "moved-child", "target", "tail"]
        );

        assert_eq!(
            engine.relocate_subtree(request).await.unwrap().status,
            BlockRelocationStatus::Replayed
        );
        assert_eq!(
            block_texts(&engine, note).await,
            vec!["target", "moved-root", "moved-child", "tail"],
            "same-note destination order after {failpoint:?}"
        );
        assert_eq!(live_bid_count(&engine, note, root).await, 1);
        assert_eq!(live_bid_count(&engine, note, child).await, 1);
    }
}

#[tokio::test]
async fn same_note_root_can_be_relocated_again_after_reopen() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x75; 16]);
    let note = [0x76; 16];
    let root = [0x77; 16];
    let child = [0x78; 16];
    let target = [0x79; 16];
    let tail = [0x7a; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_note(
        &engine,
        note,
        "2026-07-12",
        [
            stamped_line(0, "moved-root", root),
            stamped_line(1, "moved-child", child),
            stamped_line(0, "target", target),
            stamped_line(0, "tail", tail),
        ]
        .concat(),
    )
    .await;

    let mut move_down = relocation_request(note, root, note, Some(target), MovePlacement::After);
    move_down.move_id = [0x7b; 16];
    assert_eq!(
        engine
            .relocate_subtree(move_down.clone())
            .await
            .unwrap()
            .status,
        BlockRelocationStatus::Applied
    );
    assert_eq!(
        block_texts(&engine, note).await,
        vec!["target", "moved-root", "moved-child", "tail"]
    );
    drop(engine);

    let reopened = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    let mut move_up = relocation_request(note, root, note, Some(target), MovePlacement::Before);
    move_up.move_id = [0x7c; 16];
    assert_eq!(
        reopened
            .relocate_subtree(move_up.clone())
            .await
            .unwrap()
            .status,
        BlockRelocationStatus::Applied
    );
    assert_eq!(
        block_texts(&reopened, note).await,
        vec!["moved-root", "moved-child", "target", "tail"]
    );
    for request in [&move_down, &move_up] {
        assert_eq!(
            reopened
                .relocate_subtree(request.clone())
                .await
                .unwrap()
                .status,
            BlockRelocationStatus::Replayed
        );
        assert_eq!(
            block_texts(&reopened, note).await,
            vec!["moved-root", "moved-child", "target", "tail"]
        );
    }
    drop(reopened);

    let recovered =
        open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    assert_eq!(
        block_texts(&recovered, note).await,
        vec!["moved-root", "moved-child", "target", "tail"]
    );
    for request in [move_down, move_up] {
        assert_eq!(
            recovered.relocate_subtree(request).await.unwrap().status,
            BlockRelocationStatus::Replayed
        );
    }
}

#[tokio::test]
async fn boot_recovery_completes_a_second_same_note_relocation_after_preparation() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x7d; 16]);
    let note = [0x7e; 16];
    let root = [0x7f; 16];
    let child = [0x80; 16];
    let target = [0x81; 16];
    let tail = [0x82; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_note(
        &engine,
        note,
        "2026-07-12",
        [
            stamped_line(0, "moved-root", root),
            stamped_line(1, "moved-child", child),
            stamped_line(0, "target", target),
            stamped_line(0, "tail", tail),
        ]
        .concat(),
    )
    .await;

    let mut move_down = relocation_request(note, root, note, Some(target), MovePlacement::After);
    move_down.move_id = [0x83; 16];
    assert_eq!(
        engine
            .relocate_subtree(move_down.clone())
            .await
            .unwrap()
            .status,
        BlockRelocationStatus::Applied
    );

    let mut move_up = relocation_request(note, root, note, Some(target), MovePlacement::Before);
    move_up.move_id = [0x84; 16];
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterPrepared)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(move_up.clone()).await.unwrap_err(),
        move_up.move_id,
    );
    assert_eq!(
        block_texts(&engine, note).await,
        vec!["target", "moved-root", "moved-child", "tail"]
    );
    drop(engine);

    let recovered =
        open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    assert_eq!(
        block_texts(&recovered, note).await,
        vec!["moved-root", "moved-child", "target", "tail"]
    );
    for request in [move_down, move_up] {
        assert_eq!(
            recovered.relocate_subtree(request).await.unwrap().status,
            BlockRelocationStatus::Replayed
        );
    }
}

#[tokio::test]
async fn current_move_id_with_a_different_source_proof_hash_remains_fail_closed() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x8d; 16]);
    let note = [0x8e; 16];
    let root = [0x8f; 16];
    let child = [0x90; 16];
    let target = [0x91; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_note(
        &engine,
        note,
        "2026-07-12",
        [
            stamped_line(0, "moved-root", root),
            stamped_line(1, "moved-child", child),
            stamped_line(0, "target", target),
        ]
        .concat(),
    )
    .await;

    let mut request = relocation_request(note, root, note, Some(target), MovePlacement::After);
    request.move_id = [0x92; 16];
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterPrepared)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    let mut different_hash = *blake3::hash(&postcard::to_allocvec(&request).unwrap()).as_bytes();
    different_hash[0] ^= 0xff;
    overwrite_relocation_proof(&engine, note, root, request.move_id, different_hash).await;
    let rendered_before = engine.render_note_full(note).await;
    let version_before = engine.doc_version(note).await.unwrap();

    assert_recovery_required(
        engine.relocate_subtree(request).await.unwrap_err(),
        [0x92; 16],
    );
    assert_eq!(engine.render_note_full(note).await, rendered_before);
    assert_eq!(engine.doc_version(note).await.unwrap(), version_before);
    assert_eq!(
        block_texts(&engine, note).await,
        vec!["moved-root", "moved-child", "target"]
    );
}

#[tokio::test]
async fn converged_peer_without_a_local_tombstone_can_relocate_the_same_root() {
    let origin_device = DeviceId::from_bytes([0x85; 16]);
    let origin = LoroEngine::new(origin_device, Arc::new(Hlc::new(origin_device)));
    let note = [0x86; 16];
    let root = [0x87; 16];
    let child = [0x88; 16];
    let target = [0x89; 16];
    seed_note(
        &origin,
        note,
        "2026-07-12",
        [
            stamped_line(0, "moved-root", root),
            stamped_line(1, "moved-child", child),
            stamped_line(0, "target", target),
        ]
        .concat(),
    )
    .await;
    let mut move_down = relocation_request(note, root, note, Some(target), MovePlacement::After);
    move_down.move_id = [0x8a; 16];
    origin.relocate_subtree(move_down).await.unwrap();

    let peer_device = DeviceId::from_bytes([0x8b; 16]);
    let peer = LoroEngine::new(peer_device, Arc::new(Hlc::new(peer_device)));
    let snapshot = origin.export_doc_update(note, None).await.unwrap();
    peer.import_doc_update(note, &snapshot).await.unwrap();
    assert!(peer.inner.relocation_tombstones.lock().await.is_empty());
    assert_eq!(
        block_texts(&peer, note).await,
        vec!["target", "moved-root", "moved-child"]
    );

    let mut move_up = relocation_request(note, root, note, Some(target), MovePlacement::Before);
    move_up.move_id = [0x8c; 16];
    assert_eq!(
        peer.relocate_subtree(move_up).await.unwrap().status,
        BlockRelocationStatus::Applied
    );
    assert_eq!(
        block_texts(&peer, note).await,
        vec!["moved-root", "moved-child", "target"]
    );
}

#[tokio::test]
async fn unknown_cross_note_destination_proof_remains_fail_closed() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x93; 16]);
    let source = [0x94; 16];
    let destination = [0x95; 16];
    let root = [0x96; 16];
    let child = [0x97; 16];
    let target = [0x98; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_recovery_pair(&engine, source, destination, root, child, target).await;
    let mut request = relocation_request(
        source,
        root,
        destination,
        Some(target),
        MovePlacement::After,
    );
    request.move_id = [0x99; 16];
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterPrepared)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    insert_duplicate_bid(&engine, destination, root, "foreign-root").await;
    let foreign_move_id = [0x9a; 16];
    overwrite_relocation_proof(&engine, destination, root, foreign_move_id, [0x9b; 32]).await;
    assert!(engine
        .inner
        .relocation_tombstones
        .lock()
        .await
        .get(&foreign_move_id)
        .is_none());
    let rendered_before = relocation_render_pair(&engine, source, destination).await;
    let bytes_before = relocation_export_pair(&engine, source, destination).await;
    let source_version = engine.doc_version(source).await.unwrap();
    let destination_version = engine.doc_version(destination).await.unwrap();

    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    assert_eq!(
        relocation_render_pair(&engine, source, destination).await,
        rendered_before
    );
    assert_eq!(
        relocation_export_pair(&engine, source, destination).await,
        bytes_before
    );
    assert_eq!(engine.doc_version(source).await.unwrap(), source_version);
    assert_eq!(
        engine.doc_version(destination).await.unwrap(),
        destination_version
    );
}

#[tokio::test]
async fn durable_retry_recomputes_ancestry_from_a_reparented_target() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x50; 16]);
    let source = [0x91; 16];
    let destination = [0x92; 16];
    let root = [0x93; 16];
    let child = [0x94; 16];
    let target = [0x95; 16];
    let parent = [0x96; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_recovery_pair(&engine, source, destination, root, child, target).await;
    let request = relocation_request(
        source,
        root,
        destination,
        Some(target),
        MovePlacement::After,
    );
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterDestinationDurable)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );

    upsert_block(&engine, destination, parent, "parent", None).await;
    move_live_bids_before(&engine, destination, &[parent], target).await;
    set_block_structure(&engine, destination, target, 1, Some(parent)).await;

    assert_eq!(
        engine.relocate_subtree(request).await.unwrap().status,
        BlockRelocationStatus::Replayed
    );
    let (_, root_indent, root_parent) = block_structure(&engine, destination, root).await;
    let (_, child_indent, child_parent) = block_structure(&engine, destination, child).await;
    assert_eq!((root_indent, root_parent), (1, Some(parent)));
    assert_eq!((child_indent, child_parent), (2, Some(root)));
    assert_eq!(
        block_texts(&engine, destination).await,
        vec!["parent", "target", "moved-root", "moved-child"]
    );
    assert!(!nested_block_is_live(&engine, source, root).await);
}

#[tokio::test]
async fn destination_durable_retry_fails_closed_when_target_vanishes() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x51; 16]);
    let source = [0x97; 16];
    let destination = [0x98; 16];
    let root = [0x99; 16];
    let child = [0x9a; 16];
    let target = [0x9b; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_recovery_pair(&engine, source, destination, root, child, target).await;
    let request = relocation_request(
        source,
        root,
        destination,
        Some(target),
        MovePlacement::Inside,
    );
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterDestinationDurable)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    delete_live_bid(&engine, destination, target).await;

    assert_recovery_required(engine.relocate_subtree(request).await.unwrap_err(), [9; 16]);
    assert!(nested_block_is_live(&engine, source, root).await);
    assert!(nested_block_is_live(&engine, source, child).await);
    assert_eq!(live_bid_count(&engine, destination, root).await, 1);
    assert_eq!(live_bid_count(&engine, destination, child).await, 1);
}

#[tokio::test]
async fn destination_durable_retry_fails_closed_on_duplicate_target_without_mutation() {
    for (case, placement) in [
        (0x54, MovePlacement::Before),
        (0x59, MovePlacement::Inside),
        (0x5a, MovePlacement::After),
    ] {
        let root_dir = tempfile::tempdir().unwrap();
        let snapshot_dir = root_dir.path().join("snapshots");
        let materialize_dir = root_dir.path().join("notes");
        let device = DeviceId::from_bytes([case; 16]);
        let source = [0xb1; 16];
        let destination = [0xb2; 16];
        let root = [0xb3; 16];
        let child = [0xb4; 16];
        let target = [0xb5; 16];
        let engine =
            open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
        seed_recovery_pair(&engine, source, destination, root, child, target).await;
        let request = relocation_request(source, root, destination, Some(target), placement);
        engine
            .inject_relocation_failure_once(RelocationFailpoint::AfterDestinationDurable)
            .await;
        assert_recovery_required(
            engine.relocate_subtree(request.clone()).await.unwrap_err(),
            request.move_id,
        );
        insert_duplicate_bid(&engine, destination, target, "duplicate-target").await;
        let before = relocation_render_pair(&engine, source, destination).await;
        let bytes_before = relocation_export_pair(&engine, source, destination).await;
        let source_version = engine.doc_version(source).await.unwrap();
        let destination_version = engine.doc_version(destination).await.unwrap();

        assert_recovery_required(
            engine.relocate_subtree(request.clone()).await.unwrap_err(),
            request.move_id,
        );
        assert_eq!(
            relocation_render_pair(&engine, source, destination).await,
            before,
            "rendered notes changed for {placement:?}"
        );
        assert_eq!(
            relocation_export_pair(&engine, source, destination).await,
            bytes_before,
            "exported note bytes changed for {placement:?}"
        );
        assert_eq!(
            engine.doc_version(source).await.unwrap(),
            source_version,
            "source version changed for {placement:?}"
        );
        assert_eq!(
            engine.doc_version(destination).await.unwrap(),
            destination_version,
            "destination version changed for {placement:?}"
        );
        assert!(nested_block_is_live(&engine, source, root).await);
        assert_eq!(live_bid_count(&engine, destination, root).await, 1);
        assert_eq!(live_bid_count(&engine, destination, target).await, 2);
    }
}

#[tokio::test]
async fn source_durable_retry_fails_closed_on_invalid_target_ancestry_without_mutation() {
    for (case, ancestry, placement) in [
        (0x55, "missing-parent", MovePlacement::Before),
        (0x56, "duplicate-ancestor", MovePlacement::Inside),
        (0x57, "ancestor-cycle", MovePlacement::After),
        (0x58, "captured-root-cycle", MovePlacement::Inside),
    ] {
        let root_dir = tempfile::tempdir().unwrap();
        let snapshot_dir = root_dir.path().join("snapshots");
        let materialize_dir = root_dir.path().join("notes");
        let device = DeviceId::from_bytes([case; 16]);
        let source = [case.wrapping_add(0x40); 16];
        let destination = [case.wrapping_add(0x41); 16];
        let root = [case.wrapping_add(0x42); 16];
        let child = [case.wrapping_add(0x43); 16];
        let target = [case.wrapping_add(0x44); 16];
        let ancestor = [case.wrapping_add(0x45); 16];
        let missing = [case.wrapping_add(0x46); 16];
        let engine =
            open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
        seed_recovery_pair(&engine, source, destination, root, child, target).await;
        let request = relocation_request(source, root, destination, Some(target), placement);
        engine
            .inject_relocation_failure_once(RelocationFailpoint::AfterSourceDurable)
            .await;
        assert_recovery_required(
            engine.relocate_subtree(request.clone()).await.unwrap_err(),
            request.move_id,
        );

        match ancestry {
            "missing-parent" => {
                set_block_structure(&engine, destination, target, 0, Some(missing)).await;
            }
            "duplicate-ancestor" => {
                upsert_block(&engine, destination, ancestor, "ancestor", None).await;
                insert_duplicate_bid(&engine, destination, ancestor, "duplicate-ancestor").await;
                set_block_structure(&engine, destination, target, 0, Some(ancestor)).await;
            }
            "ancestor-cycle" => {
                upsert_block(&engine, destination, ancestor, "ancestor", None).await;
                set_block_structure(&engine, destination, target, 0, Some(ancestor)).await;
                set_block_structure(&engine, destination, ancestor, 0, Some(target)).await;
            }
            "captured-root-cycle" => {
                set_block_structure(&engine, destination, target, 0, Some(root)).await;
            }
            _ => unreachable!(),
        }
        let before = relocation_render_pair(&engine, source, destination).await;
        let bytes_before = relocation_export_pair(&engine, source, destination).await;
        let source_version = engine.doc_version(source).await.unwrap();
        let destination_version = engine.doc_version(destination).await.unwrap();

        assert_recovery_required(
            engine.relocate_subtree(request.clone()).await.unwrap_err(),
            request.move_id,
        );
        assert_eq!(
            relocation_render_pair(&engine, source, destination).await,
            before,
            "rendered notes changed for {ancestry}"
        );
        assert_eq!(
            relocation_export_pair(&engine, source, destination).await,
            bytes_before,
            "exported note bytes changed for {ancestry}"
        );
        assert_eq!(
            engine.doc_version(source).await.unwrap(),
            source_version,
            "source version changed for {ancestry}"
        );
        assert_eq!(
            engine.doc_version(destination).await.unwrap(),
            destination_version,
            "destination version changed for {ancestry}"
        );
        assert!(!nested_block_is_live(&engine, source, root).await);
        assert_eq!(live_bid_count(&engine, destination, root).await, 1);
        assert_eq!(live_bid_count(&engine, destination, child).await, 1);
    }
}

#[tokio::test]
async fn source_durable_missing_target_failure_does_not_delete_the_remaining_destination_copy() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x53; 16]);
    let source = [0xa1; 16];
    let destination = [0xa2; 16];
    let root = [0xa3; 16];
    let child = [0xa4; 16];
    let target = [0xa5; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_recovery_pair(&engine, source, destination, root, child, target).await;
    let request = relocation_request(
        source,
        root,
        destination,
        Some(target),
        MovePlacement::Inside,
    );
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterSourceDurable)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    assert!(!nested_block_is_live(&engine, source, root).await);
    delete_live_bid(&engine, destination, child).await;
    delete_live_bid(&engine, destination, target).await;
    let before = relocation_render_pair(&engine, source, destination).await;

    assert_recovery_required(engine.relocate_subtree(request).await.unwrap_err(), [9; 16]);
    assert_eq!(
        relocation_render_pair(&engine, source, destination).await,
        before
    );
    assert_eq!(live_bid_count(&engine, destination, root).await, 1);
    assert_eq!(live_bid_count(&engine, destination, child).await, 0);
}

#[tokio::test]
async fn same_note_retry_reorders_an_anchorless_captured_subtree() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x52; 16]);
    let note = [0x9c; 16];
    let root = [0x9d; 16];
    let child = [0x9e; 16];
    let tail = [0x9f; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_note(
        &engine,
        note,
        "2026-07-12",
        [
            stamped_line(0, "moved-root", root),
            stamped_line(1, "moved-child", child),
            stamped_line(0, "tail", tail),
        ]
        .concat(),
    )
    .await;
    let request = relocation_request(note, root, note, None, MovePlacement::Append);
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterDestinationDurable)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    delete_live_bid(&engine, note, tail).await;
    move_live_bids_before(&engine, note, &[child], root).await;
    assert_eq!(
        block_texts(&engine, note).await,
        vec!["moved-child", "moved-root"]
    );

    assert_eq!(
        engine.relocate_subtree(request).await.unwrap().status,
        BlockRelocationStatus::Replayed
    );
    assert_eq!(
        block_texts(&engine, note).await,
        vec!["moved-root", "moved-child"]
    );
    assert_eq!(live_bid_count(&engine, note, root).await, 1);
    assert_eq!(live_bid_count(&engine, note, child).await, 1);
}

#[tokio::test]
async fn same_note_retry_rebuilds_when_a_captured_bid_has_a_live_duplicate() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x54; 16]);
    let note = [0xa6; 16];
    let root = [0xa7; 16];
    let child = [0xa8; 16];
    let target = [0xa9; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_note(
        &engine,
        note,
        "2026-07-12",
        [
            stamped_line(0, "target", target),
            stamped_line(0, "moved-root", root),
            stamped_line(1, "moved-child", child),
        ]
        .concat(),
    )
    .await;
    let request = relocation_request(note, root, note, Some(target), MovePlacement::Before);
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterDestinationDurable)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    insert_duplicate_bid(&engine, note, root, "duplicate-root").await;
    assert_eq!(live_bid_count(&engine, note, root).await, 2);

    assert_eq!(
        engine.relocate_subtree(request).await.unwrap().status,
        BlockRelocationStatus::Replayed
    );
    assert_eq!(live_bid_count(&engine, note, root).await, 1);
    assert_eq!(live_bid_count(&engine, note, child).await, 1);
    assert_eq!(
        block_texts(&engine, note).await,
        vec!["moved-root", "moved-child", "target"]
    );
}

#[tokio::test]
async fn prepared_retry_reconciles_partial_destination_authoring_without_duplicate_bids() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x4a; 16]);
    let source = [0x7b; 16];
    let destination = [0x7c; 16];
    let root = [0x7d; 16];
    let child = [0x7e; 16];
    let target = [0x7f; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_recovery_pair(&engine, source, destination, root, child, target).await;
    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: source,
            block_id: root,
            key: "root-text".into(),
            value: PropOp::SetText("captured root".into()),
        })
        .await
        .unwrap();
    for value in [
        PropScalar::Text("first".into()),
        PropScalar::Int(2),
        PropScalar::Bool(true),
    ] {
        engine
            .record_local(OpPayload::BlockPropertySet {
                note_id: source,
                block_id: child,
                key: "child-list".into(),
                value: PropOp::AddToList(value),
            })
            .await
            .unwrap();
    }
    let expected_root_props = block_props_typed(&engine, source, root).await;
    let expected_child_props = block_props_typed(&engine, source, child).await;
    let request = relocation_request(
        source,
        root,
        destination,
        Some(target),
        MovePlacement::After,
    );
    engine
        .inject_relocation_failure_once(RelocationFailpoint::DuringDestinationAuthoring)
        .await;

    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    assert_eq!(live_bid_count(&engine, destination, root).await, 1);
    assert_eq!(live_bid_count(&engine, destination, child).await, 0);

    assert_eq!(
        engine.relocate_subtree(request).await.unwrap().status,
        BlockRelocationStatus::Replayed
    );
    assert_eq!(live_bid_count(&engine, destination, root).await, 1);
    assert_eq!(live_bid_count(&engine, destination, child).await, 1);
    assert_eq!(
        block_props_typed(&engine, destination, root).await,
        expected_root_props
    );
    assert_eq!(
        block_props_typed(&engine, destination, child).await,
        expected_child_props
    );
    assert_eq!(
        block_texts(&engine, destination).await,
        vec![
            "target",
            "moved-root",
            "root-text:: captured root",
            "moved-child",
            "child-list:: first, 2, true",
        ]
    );
    drop(engine);

    let reopened = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    assert_eq!(live_bid_count(&reopened, destination, root).await, 1);
    assert_eq!(live_bid_count(&reopened, destination, child).await, 1);
    assert!(!nested_block_is_live(&reopened, source, root).await);
    assert!(!nested_block_is_live(&reopened, source, child).await);
}

#[tokio::test]
async fn pending_intent_rejects_an_overlapping_move_with_another_id() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x46; 16]);
    let source = [0x26; 16];
    let first_destination = [0x27; 16];
    let second_destination = [0x28; 16];
    let root = [0x29; 16];
    let child = [0x2a; 16];
    let first_target = [0x2b; 16];
    let second_target = [0x2c; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_recovery_pair(
        &engine,
        source,
        first_destination,
        root,
        child,
        first_target,
    )
    .await;
    seed_note(
        &engine,
        second_destination,
        "2026-07-10",
        stamped_line(0, "second-target", second_target),
    )
    .await;
    let first = relocation_request(
        source,
        root,
        first_destination,
        Some(first_target),
        MovePlacement::After,
    );
    let mut second = relocation_request(
        source,
        root,
        second_destination,
        Some(second_target),
        MovePlacement::After,
    );
    second.move_id = [0x7a; 16];
    second.destination_slug = "2026-07-10".into();
    engine
        .inject_relocation_failure_once(RelocationFailpoint::AfterPrepared)
        .await;
    assert_recovery_required(
        engine.relocate_subtree(first.clone()).await.unwrap_err(),
        first.move_id,
    );

    assert_recovery_required(
        engine.relocate_subtree(second).await.unwrap_err(),
        first.move_id,
    );
    drop(engine);

    let recovered =
        open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    assert!(!nested_block_is_live(&recovered, source, root).await);
    assert!(nested_block_is_live(&recovered, first_destination, root).await);
    assert!(!nested_block_is_live(&recovered, second_destination, root).await);
}

#[tokio::test]
async fn recovery_finds_a_proof_bearing_subtree_after_destination_reorder() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x47; 16]);
    let source = [0x2d; 16];
    let destination = [0x2e; 16];
    let root = [0x2f; 16];
    let child = [0x30; 16];
    let target = [0x31; 16];
    let tail = [0x32; 16];
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    seed_note(
        &engine,
        source,
        "2026-07-12",
        [
            stamped_line(0, "moved-root", root),
            stamped_line(1, "moved-child", child),
        ]
        .concat(),
    )
    .await;
    seed_note(
        &engine,
        destination,
        "2026-07-11",
        [
            stamped_line(0, "target", target),
            stamped_line(0, "tail", tail),
        ]
        .concat(),
    )
    .await;
    let request = relocation_request(
        source,
        root,
        destination,
        Some(target),
        MovePlacement::After,
    );
    let destination_md = materialize_dir.join("2026-07-11.md");
    tokio::fs::remove_file(&destination_md).await.unwrap();
    tokio::fs::create_dir(&destination_md).await.unwrap();
    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    tokio::fs::remove_dir(&destination_md).await.unwrap();

    let destination_doc = engine.doc_for_note_mut(destination).await;
    let destination_tree = destination_doc.get_tree("blocks");
    let target_node = find_node_by_block_id(&destination_tree, &hex_id(&target)).unwrap();
    let tail_node = find_node_by_block_id(&destination_tree, &hex_id(&tail)).unwrap();
    destination_tree.mov_before(tail_node, target_node).unwrap();
    destination_doc.commit();
    engine
        .save_snapshot_checked(&snapshot_dir, destination)
        .await
        .unwrap();
    engine.materialize_note_checked(destination).await.unwrap();

    assert_eq!(
        engine.relocate_subtree(request).await.unwrap().status,
        BlockRelocationStatus::Replayed
    );
    assert_eq!(
        block_texts(&engine, destination).await,
        vec!["tail", "target", "moved-root", "moved-child"]
    );
    assert!(!nested_block_is_live(&engine, source, root).await);
}

async fn seed_relocation_peer_from(
    origin: &LoroEngine,
    peer: &LoroEngine,
    source: [u8; 16],
    destination: [u8; 16],
) {
    for note_id in [source, destination] {
        let snapshot = origin.export_doc_update(note_id, None).await.unwrap();
        peer.import_doc_update(note_id, &snapshot).await.unwrap();
    }
}

#[tokio::test]
async fn source_and_destination_deltas_converge_in_both_arrival_orders() {
    let origin_device = DeviceId::from_bytes([0x51; 16]);
    let origin = LoroEngine::new(origin_device, Arc::new(Hlc::new(origin_device)));
    let source = [0x31; 16];
    let destination = [0x32; 16];
    let root = [0x33; 16];
    let child = [0x34; 16];
    let target = [0x35; 16];
    seed_recovery_pair(&origin, source, destination, root, child, target).await;

    let mut peers = Vec::new();
    for byte in [0x52, 0x53] {
        let device = DeviceId::from_bytes([byte; 16]);
        let peer = LoroEngine::new(device, Arc::new(Hlc::new(device)));
        seed_relocation_peer_from(&origin, &peer, source, destination).await;
        peers.push(peer);
    }
    let source_before = origin.doc_version(source).await.unwrap();
    let destination_before = origin.doc_version(destination).await.unwrap();
    origin
        .relocate_subtree(relocation_request(
            source,
            root,
            destination,
            Some(target),
            MovePlacement::After,
        ))
        .await
        .unwrap();
    let source_delta = origin
        .export_doc_update(source, Some(&source_before))
        .await
        .unwrap();
    let destination_delta = origin
        .export_doc_update(destination, Some(&destination_before))
        .await
        .unwrap();

    for (peer, order) in peers.into_iter().zip([
        [(&source, &source_delta), (&destination, &destination_delta)],
        [(&destination, &destination_delta), (&source, &source_delta)],
    ]) {
        for (note_id, delta) in order {
            peer.import_doc_update(*note_id, delta).await.unwrap();
        }
        assert!(!nested_block_is_live(&peer, source, root).await);
        assert!(nested_block_is_live(&peer, destination, root).await);
        assert_eq!(
            peer.inner.block_index.read().await.get(&root),
            Some(&BTreeSet::from([destination]))
        );
    }
}

#[tokio::test]
async fn concurrent_edit_at_old_source_does_not_resurrect_after_relocation() {
    let origin_device = DeviceId::from_bytes([0x61; 16]);
    let peer_device = DeviceId::from_bytes([0x62; 16]);
    let origin = LoroEngine::new(origin_device, Arc::new(Hlc::new(origin_device)));
    let peer = LoroEngine::new(peer_device, Arc::new(Hlc::new(peer_device)));
    let source = [0x41; 16];
    let destination = [0x42; 16];
    let root = [0x43; 16];
    let child = [0x44; 16];
    let target = [0x45; 16];
    seed_recovery_pair(&origin, source, destination, root, child, target).await;
    seed_relocation_peer_from(&origin, &peer, source, destination).await;
    let source_before = origin.doc_version(source).await.unwrap();
    let destination_before = origin.doc_version(destination).await.unwrap();

    peer.splice_block_text(source, root, 10, 0, "-remote")
        .await
        .unwrap();
    let old_source_edit = peer
        .export_doc_update(source, Some(&source_before))
        .await
        .unwrap();
    origin
        .relocate_subtree(relocation_request(
            source,
            root,
            destination,
            Some(target),
            MovePlacement::After,
        ))
        .await
        .unwrap();
    let source_delete = origin
        .export_doc_update(source, Some(&source_before))
        .await
        .unwrap();
    let destination_insert = origin
        .export_doc_update(destination, Some(&destination_before))
        .await
        .unwrap();

    origin
        .import_doc_update(source, &old_source_edit)
        .await
        .unwrap();
    peer.import_doc_update(destination, &destination_insert)
        .await
        .unwrap();
    peer.import_doc_update(source, &source_delete)
        .await
        .unwrap();

    for engine in [&origin, &peer] {
        assert!(!nested_block_is_live(engine, source, root).await);
        assert!(nested_block_is_live(engine, destination, root).await);
        assert_eq!(
            engine.inner.block_index.read().await.get(&root),
            Some(&BTreeSet::from([destination]))
        );
    }
}

#[tokio::test]
async fn pruned_applied_receipt_is_stale_after_subtree_moves_away_and_back() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x71; 16]);
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    let source = [0x51; 16];
    let destination = [0x52; 16];
    let root = [0x53; 16];
    let child = [0x54; 16];
    let target = [0x55; 16];
    seed_recovery_pair(&engine, source, destination, root, child, target).await;
    let mut first = relocation_request(
        source,
        root,
        destination,
        Some(target),
        MovePlacement::After,
    );
    first.move_id = [0; 16];
    engine.relocate_subtree(first.clone()).await.unwrap();

    let cap_note = [0x56; 16];
    let cap_root = [0x57; 16];
    let cap_target = [0x58; 16];
    seed_note(
        &engine,
        cap_note,
        "2026-07-12",
        [
            stamped_line(0, "cap-root", cap_root),
            stamped_line(0, "cap-target", cap_target),
        ]
        .concat(),
    )
    .await;
    for sequence in 1u128..=4096 {
        let mut no_op = relocation_request(
            cap_note,
            cap_root,
            cap_note,
            Some(cap_target),
            MovePlacement::Before,
        );
        no_op.move_id = sequence.to_be_bytes();
        let outcome = engine.relocate_subtree(no_op).await.unwrap();
        assert_eq!(outcome.status, BlockRelocationStatus::NoOp);
    }

    let mut records = tokio::fs::read_dir(snapshot_dir.join("_relocations"))
        .await
        .unwrap();
    let mut record_count = 0;
    while records.next_entry().await.unwrap().is_some() {
        record_count += 1;
    }
    assert_eq!(record_count, 4096);
    let holding = [0x59; 16];
    let holding_target = [0x5a; 16];
    seed_note(
        &engine,
        holding,
        "2026-07-10",
        stamped_line(0, "holding-target", holding_target),
    )
    .await;
    let mut move_away = relocation_request(
        destination,
        root,
        holding,
        Some(holding_target),
        MovePlacement::After,
    );
    move_away.move_id = [0x91; 16];
    move_away.source_slug = "2026-07-11".into();
    move_away.destination_slug = "2026-07-10".into();
    engine.relocate_subtree(move_away).await.unwrap();

    let mut move_back = relocation_request(holding, root, source, None, MovePlacement::Append);
    move_back.move_id = [0x92; 16];
    move_back.source_slug = "2026-07-10".into();
    move_back.destination_slug = "2026-07-12".into();
    engine.relocate_subtree(move_back).await.unwrap();
    assert!(nested_block_is_live(&engine, source, root).await);
    assert!(!nested_block_is_live(&engine, destination, root).await);
    drop(engine);

    let reopened = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    let mut conflicting = first.clone();
    conflicting.destination_slug = "different-destination".into();
    assert!(matches!(
        reopened.relocate_subtree(conflicting).await,
        Err(SyncError::RelocationConflict(_))
    ));
    let before = relocation_render_pair(&reopened, source, destination).await;
    let source_version = reopened.doc_version(source).await.unwrap();
    let destination_version = reopened.doc_version(destination).await.unwrap();
    assert_stale_relocation(reopened.relocate_subtree(first.clone()).await.unwrap_err());
    assert_eq!(
        relocation_render_pair(&reopened, source, destination).await,
        before
    );
    assert_eq!(reopened.doc_version(source).await.unwrap(), source_version);
    assert_eq!(
        reopened.doc_version(destination).await.unwrap(),
        destination_version
    );
    assert!(nested_block_is_live(&reopened, source, root).await);
    assert!(!nested_block_is_live(&reopened, destination, root).await);
}

#[tokio::test]
async fn pruned_no_op_receipt_is_stale_without_reapplying_placement() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x72; 16]);
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    let note = [0x61; 16];
    let root = [0x62; 16];
    let target = [0x63; 16];
    seed_note(
        &engine,
        note,
        "2026-07-12",
        [
            stamped_line(0, "root", root),
            stamped_line(0, "target", target),
        ]
        .concat(),
    )
    .await;
    let mut first = relocation_request(note, root, note, Some(target), MovePlacement::Before);
    first.move_id = [0; 16];
    assert_eq!(
        engine.relocate_subtree(first.clone()).await.unwrap().status,
        BlockRelocationStatus::NoOp
    );

    for sequence in 1u128..=4096 {
        let mut no_op = relocation_request(note, root, note, Some(target), MovePlacement::Before);
        no_op.move_id = sequence.to_be_bytes();
        engine.relocate_subtree(no_op).await.unwrap();
    }
    let mut reorder = relocation_request(note, root, note, Some(target), MovePlacement::After);
    reorder.move_id = [0x93; 16];
    engine.relocate_subtree(reorder).await.unwrap();
    assert_eq!(block_texts(&engine, note).await, vec!["target", "root"]);
    drop(engine);

    let reopened = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    let mut conflicting = first.clone();
    conflicting.placement = MovePlacement::After;
    assert!(matches!(
        reopened.relocate_subtree(conflicting).await,
        Err(SyncError::RelocationConflict(_))
    ));
    let before = reopened.render_note_full(note).await;
    let version = reopened.doc_version(note).await.unwrap();
    assert_stale_relocation(reopened.relocate_subtree(first).await.unwrap_err());
    assert_eq!(reopened.render_note_full(note).await, before);
    assert_eq!(reopened.doc_version(note).await.unwrap(), version);
    assert_eq!(block_texts(&reopened, note).await, vec!["target", "root"]);
}

#[tokio::test]
async fn receipt_prune_failure_is_retried_by_replay_without_unindexed_accumulation() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x74; 16]);
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    let note = [0x71; 16];
    let root = [0x72; 16];
    let target = [0x73; 16];
    seed_note(
        &engine,
        note,
        "2026-07-12",
        [
            stamped_line(0, "root", root),
            stamped_line(0, "target", target),
        ]
        .concat(),
    )
    .await;
    let mut oldest = relocation_request(note, root, note, Some(target), MovePlacement::Before);
    oldest.move_id = [0; 16];
    assert_eq!(
        engine
            .relocate_subtree(oldest.clone())
            .await
            .unwrap()
            .status,
        BlockRelocationStatus::NoOp
    );
    let relocation_dir = snapshot_dir.join("_relocations");
    let oldest_path = relocation_dir.join(format!("{}.bin", hex_id(&oldest.move_id)));
    let oldest_receipt = tokio::fs::read(&oldest_path).await.unwrap();
    tokio::fs::remove_file(&oldest_path).await.unwrap();
    tokio::fs::create_dir(&oldest_path).await.unwrap();

    let mut newest = None;
    for sequence in 1u128..=4096 {
        let mut request = relocation_request(note, root, note, Some(target), MovePlacement::Before);
        request.move_id = sequence.to_be_bytes();
        if sequence == 4096 {
            assert_recovery_required(
                engine.relocate_subtree(request.clone()).await.unwrap_err(),
                request.move_id,
            );
            newest = Some(request);
        } else {
            assert_eq!(
                engine.relocate_subtree(request).await.unwrap().status,
                BlockRelocationStatus::NoOp
            );
        }
    }

    tokio::fs::remove_dir(&oldest_path).await.unwrap();
    tokio::fs::write(&oldest_path, oldest_receipt)
        .await
        .unwrap();
    let newest = newest.unwrap();
    assert_eq!(
        engine.relocate_subtree(newest).await.unwrap().status,
        BlockRelocationStatus::Replayed
    );
    let mut records = tokio::fs::read_dir(&relocation_dir).await.unwrap();
    let mut record_count = 0;
    while records.next_entry().await.unwrap().is_some() {
        record_count += 1;
    }
    assert_eq!(record_count, 4096);
    assert_stale_relocation(engine.relocate_subtree(oldest).await.unwrap_err());
}

#[tokio::test]
async fn failed_tombstone_publish_is_retried_before_intent_becomes_a_receipt() {
    let root_dir = tempfile::tempdir().unwrap();
    let snapshot_dir = root_dir.path().join("snapshots");
    let materialize_dir = root_dir.path().join("notes");
    let device = DeviceId::from_bytes([0x73; 16]);
    let engine = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    let note = [0x64; 16];
    let root = [0x65; 16];
    let target = [0x66; 16];
    seed_note(
        &engine,
        note,
        "2026-07-12",
        [
            stamped_line(0, "root", root),
            stamped_line(0, "target", target),
        ]
        .concat(),
    )
    .await;
    let request = relocation_request(note, root, note, Some(target), MovePlacement::Before);
    let tombstones_path = snapshot_dir.join("_relocation_tombstones.bin");
    tokio::fs::create_dir(&tombstones_path).await.unwrap();

    assert_recovery_required(
        engine.relocate_subtree(request.clone()).await.unwrap_err(),
        request.move_id,
    );
    tokio::fs::remove_dir(&tombstones_path).await.unwrap();
    assert_eq!(
        engine
            .relocate_subtree(request.clone())
            .await
            .unwrap()
            .status,
        BlockRelocationStatus::Replayed
    );
    tokio::fs::remove_file(
        snapshot_dir
            .join("_relocations")
            .join(format!("{}.bin", hex_id(&request.move_id))),
    )
    .await
    .unwrap();
    drop(engine);

    let reopened = open_persistent_relocation_engine(device, &snapshot_dir, &materialize_dir).await;
    assert_stale_relocation(reopened.relocate_subtree(request).await.unwrap_err());
}
