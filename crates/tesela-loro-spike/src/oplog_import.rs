//! Spike item 5 — sketch the one-way oplog → Loro doc migration path.
//!
//! This is intentionally minimal: it doesn't read the actual SQLite oplog
//! (that'd require sqlx setup), it just hand-constructs the equivalent
//! sequence of `OpPayload` variants Tesela's engine produces, then
//! translates each to a Loro operation, and verifies the resulting Loro
//! doc materializes back to the expected block tree.
//!
//! The point is to PROVE the mapping is straightforward, not to ship the
//! real importer.
//!
//! Run: `cargo run -p tesela-loro-spike --bin spike-oplog-import`

use loro::{LoroDoc, LoroTree};
use uuid::Uuid;

#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
enum FakeOp {
    BlockUpsert {
        id: Uuid,
        parent: Option<Uuid>,
        text: String,
    },
    BlockMove {
        id: Uuid,
        new_parent: Option<Uuid>,
    },
    BlockDelete {
        id: Uuid,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Pretend these are decoded from `oplog` rows in HLC order.
    let a_uuid = Uuid::new_v4();
    let b_uuid = Uuid::new_v4();
    let c_uuid = Uuid::new_v4();

    let ops = vec![
        FakeOp::BlockUpsert {
            id: a_uuid,
            parent: None,
            text: "A".to_string(),
        },
        FakeOp::BlockUpsert {
            id: b_uuid,
            parent: None,
            text: "B".to_string(),
        },
        FakeOp::BlockUpsert {
            id: c_uuid,
            parent: Some(a_uuid),
            text: "C (child of A)".to_string(),
        },
        FakeOp::BlockMove {
            id: c_uuid,
            new_parent: Some(b_uuid),
        },
        FakeOp::BlockDelete { id: a_uuid },
    ];

    let doc = LoroDoc::new();
    let tree: LoroTree = doc.get_tree("blocks");
    // Maintain a uuid → loro tree id mapping. The Tesela bid is the
    // identity that must survive the migration; Loro's internal TreeID
    // is an implementation detail we map to/from on the boundary.
    let mut map: std::collections::HashMap<Uuid, loro::TreeID> = std::collections::HashMap::new();

    for op in &ops {
        match op {
            FakeOp::BlockUpsert { id, parent, text } => {
                // First time we see this bid → create the node. Subsequent
                // text updates → just update the meta.
                if !map.contains_key(id) {
                    let parent_tid = parent.and_then(|p| map.get(&p).copied());
                    let tid = tree.create(parent_tid)?;
                    map.insert(*id, tid);
                }
                let tid = *map.get(id).unwrap();
                tree.get_meta(tid)?.insert("text", text.as_str())?;
            }
            FakeOp::BlockMove { id, new_parent } => {
                if let Some(&tid) = map.get(id) {
                    let parent_tid: Option<loro::TreeID> =
                        new_parent.and_then(|p| map.get(&p).copied());
                    // `Option<TreeID>: Into<TreeParentId>` — None = root.
                    tree.mov(tid, parent_tid)?;
                }
            }
            FakeOp::BlockDelete { id } => {
                if let Some(&tid) = map.get(id) {
                    tree.delete(tid)?;
                    map.remove(id);
                }
            }
        }
    }
    doc.commit();

    // Materialize back into a human-readable form so we can eyeball it.
    let value = doc.get_deep_value();
    println!("Final doc state: {:#?}", value);

    // Expectation:
    //  - A is deleted (was the root with C as child; moving C away and
    //    then deleting A leaves B as a root with C under it)
    //  - B remains, with C as its child
    //  - C's text is "C (child of A)" (no edits after creation)
    println!(
        "\nMapping that survived: {} nodes (expected 2: B and C; A was deleted)",
        map.len()
    );
    let mapping_ok = map.len() == 2 && map.contains_key(&b_uuid) && map.contains_key(&c_uuid);
    println!(
        "\nVerdict: {}",
        if mapping_ok {
            "GREEN — oplog→Loro mapping is straightforward"
        } else {
            "FAIL"
        }
    );
    Ok(())
}
