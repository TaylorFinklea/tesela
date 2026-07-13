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
