use super::*;

mod convergence;
mod ops;
mod relocation;
mod views_and_races;

fn test_device() -> DeviceId {
    DeviceId::from_bytes([1u8; 16])
}

/// Helper: record a top-level BlockUpsert with an optional positional
/// hint. Returns nothing — the caller renders to assert order.
async fn upsert_block(
    engine: &LoroEngine,
    note_id: [u8; 16],
    block_id: [u8; 16],
    text: &str,
    after_block_id: Option<[u8; 16]>,
) {
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id,
            note_id,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: text.into(),
            after_block_id,
        })
        .await
        .unwrap();
}

/// Render a note's blocks as their texts in document (render) order —
/// strips the bid markers so order is the only thing under test.
async fn block_texts(engine: &LoroEngine, note_id: [u8; 16]) -> Vec<String> {
    let rendered = engine.render_note(note_id).await.unwrap();
    rendered
        .lines()
        .filter_map(|l| {
            let t = l.trim_start().trim_start_matches("- ");
            let t = t.split(" <!-- bid:").next().unwrap_or(t).trim();
            (!t.is_empty()).then(|| t.to_string())
        })
        .collect()
}

fn blake3_note_id(slug: &str) -> [u8; 16] {
    tesela_core::stable_uuid_from_slug(slug)
}

/// Read a single block's current text off a note's tree by block_id
/// bytes (matching the dashless hex the engine stores in meta).
async fn block_text(engine: &LoroEngine, note_id: [u8; 16], block: [u8; 16]) -> Option<String> {
    let docs = engine.inner.docs.read().await;
    let doc = docs.get(&note_id)?;
    let tree = doc.get_tree("blocks");
    let node = find_node_by_block_id(&tree, &hex_id(&block))?;
    read_block_text(&tree, node)
}

/// Construct the EXACT wire incident (the wire-captured DISJOINT-lineage
/// case): the server and the device each author block_id A / B
/// INDEPENDENTLY (no shared Loro import), so each mints its OWN `TreeID`
/// for the same `block_id` — the residual disjoint lineage these daily
/// blocks carry (pre-shared-base, or `recordNoteDiff` re-authoring from
/// stale markdown). The server then edits A→"Awesome sweet" via HTTP. The
/// device, holding its own stale A="Awesome" twin, genuinely edits B→"B
/// device" and exports a FULL SNAPSHOT. On a raw `doc.import` the device's
/// A-twin unions with the server's; under the PURE max-`TreeID` rule
/// (tesela-fte) the survivor per bid is ONLY the higher-`TreeID`
/// (higher-peer) twin — the device (0x7f) outranks the server (0x5e), so
/// A resolves to the device's re-shipped "Awesome" and B to "B device".
/// The stale-guard that formerly preserved "Awesome sweet" is dropped
/// (product-approved 2026-07-01: higher-TreeID text wins).
const A_BID: &str = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
const B_BID: &str = "0b0b0b0b-0b0b-0b0b-0b0b-0b0b0b0b0b0b";
const A_BID_BYTES: [u8; 16] = [0x0a; 16];
const B_BID_BYTES: [u8; 16] = [0x0b; 16];

async fn seed_disjoint(server: &LoroEngine, device: &LoroEngine, note: [u8; 16]) {
    // BOTH author the same note body independently — disjoint Loro
    // lineages (distinct TreeIDs for the same block_ids).
    let content = format!("- Awesome <!-- bid:{A_BID} -->\n- B base <!-- bid:{B_BID} -->\n");
    for e in [server, device] {
        e.record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("daily".into()),
            title: "Daily".into(),
            content: content.clone(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    }
}

/// Seed a shared base for `note` with one block `A_BID_BYTES` holding
/// `text`, then return a second replica that has imported the base — so
/// both share the SAME `text_seq` lineage (the merge precondition, NOT
/// disjoint twins).
async fn splice_shared_base(note: [u8; 16], text: &str) -> (LoroEngine, LoroEngine) {
    let dev_a = DeviceId::from_bytes([0xa7; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    a.record_local(OpPayload::BlockUpsert {
        block_id: A_BID_BYTES,
        note_id: note,
        parent_block_id: None,
        order_key: "00000000".into(),
        indent_level: 0,
        text: text.into(),
        after_block_id: None,
    })
    .await
    .unwrap();

    let dev_b = DeviceId::from_bytes([0xb7; 16]);
    let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
    let base = a.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &base).await.unwrap();
    // `read_block_text` maps empty text → None, so compare against the
    // expected `None` when seeding an empty block.
    let expect = if text.is_empty() { None } else { Some(text) };
    assert_eq!(
        block_text(&b, note, A_BID_BYTES).await.as_deref(),
        expect,
        "shared base seeded on B"
    );
    (a, b)
}

/// A realistic content bid: a UUID-shaped 16-byte id a client actually
/// authors + syncs (`parse_body_blocks` / iOS stamp these), NOT the parked
/// deterministic-seed placeholder. The production garble used such a bid
/// (c35861c0) created on iOS, synced, then re-edited on the desktop's
/// disjoint lineage.
fn content_bid(seed: &str) -> [u8; 16] {
    let h = blake3::hash(seed.as_bytes());
    let mut id = [0u8; 16];
    id.copy_from_slice(&h.as_bytes()[..16]);
    id
}

/// Count LIVE tree nodes carrying `block` on a note's doc — disjoint
/// same-bid twins show as > 1.
async fn block_twin_count(engine: &LoroEngine, note_id: [u8; 16], block: [u8; 16]) -> usize {
    let docs = engine.inner.docs.read().await;
    let Some(doc) = docs.get(&note_id) else {
        return 0;
    };
    let tree = doc.get_tree("blocks");
    let hex = hex_id(&block);
    let mut n = 0;
    for node in tree.children(TreeParentId::Root).unwrap_or_default() {
        if matches!(tree.is_node_deleted(&node), Ok(true)) {
            continue;
        }
        if read_meta_str(&tree, node, "block_id").as_deref() == Some(hex.as_str()) {
            n += 1;
        }
    }
    n
}

/// Read a block's `props` scalar by note + block id, navigating the doc
/// the way the apply arm writes it. Mirrors `block_text`.
async fn block_prop_scalar(
    engine: &LoroEngine,
    note_id: [u8; 16],
    block: [u8; 16],
    key: &str,
) -> Option<PropScalar> {
    let docs = engine.inner.docs.read().await;
    let doc = docs.get(&note_id)?;
    let tree = doc.get_tree("blocks");
    let node = find_node_by_block_id(&tree, &hex_id(&block))?;
    let meta = tree.get_meta(node).ok()?;
    let (props, _keys) = prop_containers::node_prop_containers(&meta).ok()?;
    prop_containers::prop_get_scalar(&props, key)
}

/// Read a block's `props` multi-value list by note + block id.
async fn block_prop_list(
    engine: &LoroEngine,
    note_id: [u8; 16],
    block: [u8; 16],
    key: &str,
) -> Vec<PropScalar> {
    let docs = engine.inner.docs.read().await;
    let Some(doc) = docs.get(&note_id) else {
        return Vec::new();
    };
    let tree = doc.get_tree("blocks");
    let Some(node) = find_node_by_block_id(&tree, &hex_id(&block)) else {
        return Vec::new();
    };
    let Ok(meta) = tree.get_meta(node) else {
        return Vec::new();
    };
    let Ok((props, _keys)) = prop_containers::node_prop_containers(&meta) else {
        return Vec::new();
    };
    prop_containers::prop_get_list(&props, key)
}

fn user_view(id: &str, name: &str, dsl: &str, order: i64) -> crate::engine::ViewRecord {
    crate::engine::ViewRecord {
        id: id.to_string(),
        name: name.to_string(),
        dsl: dsl.to_string(),
        order,
        builtin: false,
        display_mode: "list".to_string(),
        display_group_by: None,
        display_show_done: None,
        display_table_config: None,
    }
}

/// Ship `from`'s produced relay updates to `to` through the real wire
/// codec, then commit `from`'s broadcast cursor (a confirmed send).
/// Same shape as `two_authoritative_engines_converge_through_wire_codec`'s
/// inline helper.
async fn ship_relay(from: &LoroEngine, to: &LoroEngine) -> usize {
    use crate::wire::{decode_loro_relay_payload, encode_loro_relay_payload, LoroDocUpdate};
    let updates = from.produce_relay_updates().await;
    if updates.is_empty() {
        return 0;
    }
    let payload: Vec<LoroDocUpdate> = updates
        .iter()
        .map(|(doc, update_bytes, _vv)| LoroDocUpdate {
            doc: *doc,
            update_bytes: update_bytes.clone(),
        })
        .collect();
    let committed: Vec<([u8; 16], Vec<u8>)> =
        updates.into_iter().map(|(d, _b, vv)| (d, vv)).collect();
    let wire = encode_loro_relay_payload(&payload).unwrap();
    let decoded = decode_loro_relay_payload(&wire)
        .unwrap()
        .expect("v2 payload");
    let pairs: Vec<([u8; 16], Vec<u8>)> = decoded
        .into_iter()
        .map(|u| (u.doc, u.update_bytes))
        .collect();
    let n = to.apply_relay_updates(&pairs).await.applied_count();
    from.commit_broadcast_cursors(&committed).await;
    n
}
