//! Block-level diff: turn two [`NoteTree`] values into a vector of sync ops.
//!
//! Pure function from `(note_id, old, new) -> Vec<OpPayload>`. Caller is
//! responsible for sourcing the old and new trees and for handing the
//! resulting ops to [`crate::SyncEngine::record_local`] or similar.
//!
//! Op shapes:
//! - **New block** (id present in `new`, missing in `old`): `BlockUpsert`
//!   carrying full state.
//! - **Deleted block** (id present in `old`, missing in `new`):
//!   `BlockDelete`.
//! - **Edited block** (id in both, text differs): `BlockUpsert` with the
//!   new state. BlockUpsert is full-state, so it also covers
//!   position changes for edited blocks; we never emit BlockUpsert + Move
//!   for the same id in the same diff.
//! - **Moved block** (id in both, text identical, parent or order
//!   differs): `BlockMove`.
//!
//! Order keys are derived from sibling position in the new tree using a
//! zero-padded decimal scheme (eight digits, lex-sortable). When sibling
//! order changes, every affected block sees a new order key, so a single
//! reorder can produce O(n) `BlockMove` ops. Real fractional indexing
//! (insert-between with no rewrites) is a follow-up; for the initial cut
//! the chatty form is correct and easier to reason about.

use crate::oplog::op::OpPayload;
use std::collections::HashMap;
use tesela_core::note_tree::{FlatBlock, NoteTree};
use uuid::Uuid;

/// Compare two trees of the same note and produce ops describing the
/// transition. `new` is the post-write state; `old` is whatever the peer
/// last knew about (or an empty tree for a brand-new note).
///
/// Backwards-compatible wrapper that defaults to emitting `BlockDelete`
/// for ids missing from `new`. Use [`diff_note_trees_with_options`] when
/// the diff source can't distinguish "user deleted this block" from
/// "user never saw this block" — server-side PUT diffs against a Mac
/// file that may have peer ops the requestor hasn't fetched yet, so
/// inferring deletes from absence is data-lossy.
pub fn diff_note_trees(note_id: [u8; 16], old: &NoteTree, new: &NoteTree) -> Vec<OpPayload> {
    diff_note_trees_with_options(note_id, old, new, DiffOptions::default())
}

/// Tunables for [`diff_note_trees_with_options`].
#[derive(Debug, Clone, Copy)]
pub struct DiffOptions {
    /// When `true` (default), emit a `BlockDelete` for every id present
    /// in `old` but missing from `new`. Safe when the diff sides are
    /// symmetric — i.e. both reflect the same client's view (iOS engine
    /// diffs its own local file vs its own new content). Unsafe when
    /// `old` is the server's authoritative file and `new` is a client
    /// PUT body that may not include peer ops the server has applied
    /// since the client's last fetch: those would be spuriously deleted.
    pub emit_deletes: bool,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self { emit_deletes: true }
    }
}

/// Like [`diff_note_trees`] but lets the caller suppress
/// inferred-delete emission. The server-side PUT path passes
/// `emit_deletes: false` and routes user-intent deletes through an
/// explicit DELETE endpoint instead.
pub fn diff_note_trees_with_options(
    note_id: [u8; 16],
    old: &NoteTree,
    new: &NoteTree,
    opts: DiffOptions,
) -> Vec<OpPayload> {
    let old_index = index_blocks(&old.blocks);
    let new_index = index_blocks(&new.blocks);

    let mut ops = Vec::new();

    // Deletions: anything in old that is missing from new. Suppressed
    // when the diff source isn't authoritative about absence.
    if opts.emit_deletes {
        for (id, _) in &old_index {
            if !new_index.contains_key(id) {
                ops.push(OpPayload::BlockDelete {
                    block_id: uuid_to_bytes(*id),
                });
            }
        }
    }

    // Upserts and moves: walk the new tree in document order so the
    // resulting ops apply cleanly if streamed.
    for (pos, block) in new.blocks.iter().enumerate() {
        let new_view = BlockView::from(block, new.blocks.as_slice(), pos);
        match old_index.get(&block.id) {
            None => {
                // New block: carry a positional hint so the engine inserts
                // it ADJACENT to the block it follows in document order
                // (its predecessor), instead of appending at document end.
                // The predecessor is the block at `pos - 1`; `None` at the
                // top (`pos == 0`) means append, which is correct for a
                // brand-new note seeded in order. The hint is only honored
                // on CREATE — an update-in-place ignores it.
                let after = pos.checked_sub(1).map(|p| new.blocks[p].id);
                ops.push(make_block_upsert(note_id, block.id, &new_view, after));
            }
            Some(old_view) => {
                if block.text != old_view.text {
                    // Text changed; full upsert covers position too. No
                    // positional hint — an existing block is updated in
                    // place and never moved by an upsert.
                    ops.push(make_block_upsert(note_id, block.id, &new_view, None));
                } else if old_view.parent != new_view.parent
                    || old_view.order_key != new_view.order_key
                {
                    // Position-only change.
                    ops.push(OpPayload::BlockMove {
                        block_id: uuid_to_bytes(block.id),
                        new_parent: new_view.parent.map(uuid_to_bytes),
                        new_order_key: new_view.order_key.clone(),
                    });
                }
                // else: identical block, no op.
            }
        }
    }

    ops
}

fn make_block_upsert(
    note_id: [u8; 16],
    block_id: Uuid,
    view: &BlockView,
    after_block_id: Option<Uuid>,
) -> OpPayload {
    OpPayload::BlockUpsert {
        block_id: uuid_to_bytes(block_id),
        note_id,
        parent_block_id: view.parent.map(uuid_to_bytes),
        order_key: view.order_key.clone(),
        indent_level: view.indent,
        text: view.text.clone(),
        after_block_id: after_block_id.map(uuid_to_bytes),
    }
}

/// A flattened view of a block as it sits in its tree. Holds the
/// derived order_key and parent so equality checks are direct.
#[derive(Clone)]
struct BlockView {
    parent: Option<Uuid>,
    order_key: String,
    indent: u16,
    text: String,
}

impl BlockView {
    fn from(block: &FlatBlock, all: &[FlatBlock], pos: usize) -> Self {
        BlockView {
            parent: block.parent,
            order_key: order_key_for(all, pos),
            indent: block.indent,
            text: block.text.clone(),
        }
    }
}

/// Build an `id -> view` lookup for fast presence + comparison.
fn index_blocks(blocks: &[FlatBlock]) -> HashMap<Uuid, BlockView> {
    blocks
        .iter()
        .enumerate()
        .map(|(pos, b)| (b.id, BlockView::from(b, blocks, pos)))
        .collect()
}

/// Compute the sibling-position-based order key for the block at `pos` in
/// `all`. Walks back from `pos` counting how many earlier blocks share
/// the same parent.
fn order_key_for(all: &[FlatBlock], pos: usize) -> String {
    let parent = all[pos].parent;
    let mut idx = 0u32;
    for prior in &all[..pos] {
        if prior.parent == parent {
            idx += 1;
        }
    }
    format!("{:08}", idx)
}

fn uuid_to_bytes(id: Uuid) -> [u8; 16] {
    *id.as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tesela_core::note_tree::parse_note;

    fn note(id_byte: u8) -> [u8; 16] {
        [id_byte; 16]
    }

    fn parse(s: &str) -> NoteTree {
        parse_note(s)
    }

    #[test]
    fn empty_old_emits_block_upsert_per_block() {
        let new = parse("- One\n- Two\n");
        let old = NoteTree {
            frontmatter: None,
            page_properties: vec![],
            blocks: vec![],
            stamped_any: false,
        };
        let ops = diff_note_trees(note(1), &old, &new);
        assert_eq!(ops.len(), 2);
        for op in &ops {
            match op {
                OpPayload::BlockUpsert {
                    note_id, text, ..
                } => {
                    assert_eq!(*note_id, [1u8; 16]);
                    assert!(text == "One" || text == "Two");
                }
                _ => panic!("expected BlockUpsert, got {:?}", op),
            }
        }
    }

    #[test]
    fn new_block_in_middle_carries_predecessor_hint() {
        // Old: A, C. New: A, B, C. B is the inserted block; its
        // `after_block_id` hint must point at A (its predecessor in
        // document order) so the engine inserts it adjacent, not at end.
        let initial = parse("- A\n- C\n");
        let a_id = initial.blocks[0].id;
        let b_id = Uuid::now_v7();
        let c_id = initial.blocks[1].id;
        let new = NoteTree {
            frontmatter: None,
            page_properties: vec![],
            blocks: vec![
                FlatBlock { id: a_id, parent: None, indent: 0, text: "A".into() },
                FlatBlock { id: b_id, parent: None, indent: 0, text: "B".into() },
                FlatBlock { id: c_id, parent: None, indent: 0, text: "C".into() },
            ],
            stamped_any: false,
        };
        let ops = diff_note_trees(note(7), &initial, &new);
        let mut saw_b_hint = false;
        for op in &ops {
            if let OpPayload::BlockUpsert {
                block_id,
                after_block_id,
                ..
            } = op
            {
                if *block_id == *b_id.as_bytes() {
                    assert_eq!(
                        *after_block_id,
                        Some(*a_id.as_bytes()),
                        "inserted block B must point its after-hint at predecessor A"
                    );
                    saw_b_hint = true;
                }
            }
        }
        assert!(saw_b_hint, "expected a BlockUpsert for B, got {:?}", ops);
    }

    #[test]
    fn first_new_block_has_no_predecessor_hint() {
        // A brand-new note: every block is new. The FIRST block has no
        // predecessor, so its hint is None (append is correct for an
        // in-order seed); the SECOND block points at the first.
        let new = parse("- One\n- Two\n");
        let old = NoteTree {
            frontmatter: None,
            page_properties: vec![],
            blocks: vec![],
            stamped_any: false,
        };
        let one_id = new.blocks[0].id;
        let ops = diff_note_trees(note(8), &old, &new);
        for op in &ops {
            if let OpPayload::BlockUpsert {
                text,
                after_block_id,
                ..
            } = op
            {
                if text == "One" {
                    assert_eq!(*after_block_id, None, "first block has no predecessor");
                }
                if text == "Two" {
                    assert_eq!(
                        *after_block_id,
                        Some(*one_id.as_bytes()),
                        "second block points at the first"
                    );
                }
            }
        }
    }

    #[test]
    fn no_change_emits_no_ops() {
        let new = parse("- A\n  - B\n- C\n");
        let serialized = tesela_core::note_tree::serialize_note(&new);
        let old = parse(&serialized);
        let ops = diff_note_trees(note(2), &old, &new);
        assert!(ops.is_empty(), "expected no ops, got {:?}", ops);
    }

    #[test]
    fn text_change_emits_one_block_upsert() {
        let initial = parse("- Original text\n- Untouched\n");
        let serialized = tesela_core::note_tree::serialize_note(&initial);
        let edited_src = serialized.replace("Original text", "Updated text");
        let new = parse(&edited_src);
        let ops = diff_note_trees(note(3), &initial, &new);
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            OpPayload::BlockUpsert { text, .. } => assert_eq!(text, "Updated text"),
            _ => panic!("expected BlockUpsert"),
        }
    }

    #[test]
    fn delete_emits_block_delete() {
        let initial = parse("- Keep\n- Drop\n");
        let serialized = tesela_core::note_tree::serialize_note(&initial);
        let new_src = serialized
            .lines()
            .filter(|l| !l.contains("Drop"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let new = parse(&new_src);
        let ops = diff_note_trees(note(4), &initial, &new);
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            OpPayload::BlockDelete { block_id } => {
                let dropped = initial.blocks.iter().find(|b| b.text == "Drop").unwrap();
                assert_eq!(*block_id, *dropped.id.as_bytes());
            }
            _ => panic!("expected BlockDelete, got {:?}", ops[0]),
        }
    }

    #[test]
    fn move_only_emits_block_move() {
        // Start with A, B both at indent 0. Move B under A (indent 1).
        let initial = parse("- A\n- B\n");
        let a_id = initial.blocks[0].id;
        let b_id = initial.blocks[1].id;
        let new = NoteTree {
            frontmatter: None,
            page_properties: vec![],
            blocks: vec![
                FlatBlock {
                    id: a_id,
                    parent: None,
                    indent: 0,
                    text: "A".into(),
                },
                FlatBlock {
                    id: b_id,
                    parent: Some(a_id),
                    indent: 1,
                    text: "B".into(),
                },
            ],
            stamped_any: false,
        };
        let ops = diff_note_trees(note(5), &initial, &new);
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            OpPayload::BlockMove {
                block_id,
                new_parent,
                new_order_key,
            } => {
                assert_eq!(*block_id, *b_id.as_bytes());
                assert_eq!(*new_parent, Some(*a_id.as_bytes()));
                // B is the first child of A, so order key 00000000.
                assert_eq!(new_order_key, "00000000");
            }
            _ => panic!("expected BlockMove, got {:?}", ops[0]),
        }
    }

    #[test]
    fn insert_in_middle_reorders_following_siblings() {
        // Old: A, B, C. New: A, X, B, C. X is the inserted block.
        // X gets order_key 00000001 (second among parent=None siblings).
        // B's order_key changes 00000001 -> 00000002. So does C.
        let initial = parse("- A\n- B\n- C\n");
        let a_id = initial.blocks[0].id;
        let b_id = initial.blocks[1].id;
        let c_id = initial.blocks[2].id;
        let x_id = Uuid::now_v7();
        let new = NoteTree {
            frontmatter: None,
            page_properties: vec![],
            blocks: vec![
                FlatBlock {
                    id: a_id,
                    parent: None,
                    indent: 0,
                    text: "A".into(),
                },
                FlatBlock {
                    id: x_id,
                    parent: None,
                    indent: 0,
                    text: "X".into(),
                },
                FlatBlock {
                    id: b_id,
                    parent: None,
                    indent: 0,
                    text: "B".into(),
                },
                FlatBlock {
                    id: c_id,
                    parent: None,
                    indent: 0,
                    text: "C".into(),
                },
            ],
            stamped_any: false,
        };
        let ops = diff_note_trees(note(6), &initial, &new);
        // Expect: BlockUpsert for X (new), BlockMove for B and C (order shifted).
        assert_eq!(ops.len(), 3, "got {:?}", ops);
        let mut saw_x = false;
        let mut saw_b_move = false;
        let mut saw_c_move = false;
        for op in &ops {
            match op {
                OpPayload::BlockUpsert { block_id, text, .. } => {
                    if *block_id == *x_id.as_bytes() {
                        assert_eq!(text, "X");
                        saw_x = true;
                    }
                }
                OpPayload::BlockMove {
                    block_id,
                    new_order_key,
                    ..
                } => {
                    if *block_id == *b_id.as_bytes() {
                        assert_eq!(new_order_key, "00000002");
                        saw_b_move = true;
                    }
                    if *block_id == *c_id.as_bytes() {
                        assert_eq!(new_order_key, "00000003");
                        saw_c_move = true;
                    }
                }
                _ => {}
            }
        }
        assert!(saw_x);
        assert!(saw_b_move);
        assert!(saw_c_move);
    }
}
