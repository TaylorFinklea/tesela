# Local-only Empty Leaf Blocks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent independent device entries from reusing one synchronized empty block ID by omitting bare leaf reservations from every materialized/indexed note projection.

**Architecture:** Extract the existing bare-leaf pruning policy into a tree-level `tesela-core` helper, then invoke it at the Loro note-tree rendering boundary before serialization. The Loro node remains available to the creating replica's splice path, but peers never render it; meaningful content or descendants make it visible automatically.

**Tech Stack:** Rust 2021, Loro 1.13.6, Tokio tests, `tesela-core::note_tree`, `tesela-sync::LoroEngine`.

## Global Constraints

- Untouched empty leaves are omitted; empty parents with descendants remain.
- Tags, properties, task state, or prose make a block non-bare.
- Do not tombstone or delete hidden reservations.
- Do not alter normal same-block concurrent-edit semantics.
- Do not enable `TESELA_LORO_MIGRATE_IN_TEXT` or absorb `tesela-wt5`.
- Follow TDD: run each named test red before production edits, then green.
- Run Rust verification serially; do not mutate the live mosaic during tests.

---

### Task 1: Make bare-leaf pruning reusable on parsed note trees

**Files:**
- Modify: `crates/tesela-core/src/note_tree.rs:232-275`
- Test: `crates/tesela-core/src/note_tree.rs:1567-1660`

**Interfaces:**
- Consumes: existing `NoteTree`, `FlatBlock`, and private `block_is_bare` semantics.
- Produces: `pub fn prune_bare_leaf_blocks_in_tree(tree: &mut NoteTree) -> bool`; returns whether any block was removed.
- `prune_bare_leaf_blocks(content: &str)` must delegate to this helper and preserve its byte-identical no-op behavior.

- [ ] **Step 1: Add failing tree-level tests**

Add focused unit tests beside the existing `prune_bare_leaf_blocks` cases:

```rust
#[test]
fn prune_tree_drops_bare_leaf_but_keeps_meaningful_sibling() {
    let mut tree = parse_note(&format!(
        "- Real <!-- bid:{} -->\n- <!-- bid:{} -->\n",
        fixture_uuid(0x50),
        fixture_uuid(0x51),
    ));

    assert!(prune_bare_leaf_blocks_in_tree(&mut tree));
    assert_eq!(tree.blocks.len(), 1);
    assert_eq!(tree.blocks[0].text, "Real");
}

#[test]
fn prune_tree_keeps_empty_parent_with_child() {
    let mut tree = parse_note(&format!(
        "- <!-- bid:{} -->\n  - Child <!-- bid:{} -->\n",
        fixture_uuid(0x52),
        fixture_uuid(0x53),
    ));

    assert!(!prune_bare_leaf_blocks_in_tree(&mut tree));
    assert_eq!(tree.blocks.len(), 2);
}
```

- [ ] **Step 2: Run the new tests and verify RED**

Run:

```bash
cargo test -p tesela-core note_tree::tests::prune_tree_ -- --nocapture
```

Expected: compile failure because `prune_bare_leaf_blocks_in_tree` does not exist.

- [ ] **Step 3: Extract the existing reverse-walk policy**

Implement the public tree helper by moving the current keep-vector/reverse-walk logic out of `prune_bare_leaf_blocks`. Preserve these invariants:

```rust
pub fn prune_bare_leaf_blocks_in_tree(tree: &mut NoteTree) -> bool {
    let mut keep = vec![true; tree.blocks.len()];
    let mut kept_indents = Vec::with_capacity(tree.blocks.len());
    for (idx, block) in tree.blocks.iter().enumerate().rev() {
        let has_deeper_successor = kept_indents
            .last()
            .is_some_and(|next_indent| *next_indent > block.indent);
        if block_is_bare(block) && !has_deeper_successor {
            keep[idx] = false;
        } else {
            kept_indents.push(block.indent);
        }
    }
    let changed = keep.iter().any(|kept| !kept);
    if changed {
        tree.blocks = std::mem::take(&mut tree.blocks)
            .into_iter()
            .zip(keep)
            .filter_map(|(block, kept)| kept.then_some(block))
            .collect();
    }
    changed
}
```

Then make `prune_bare_leaf_blocks(content)` call the helper and return the original bytes when it returns `false`.

- [ ] **Step 4: Run core pruning tests and verify GREEN**

Run:

```bash
cargo test -p tesela-core note_tree::tests::prune_ -- --nocapture
```

Expected: all tree-level and string-level pruning tests pass.

- [ ] **Step 5: Commit Task 1**

```bash
git add crates/tesela-core/src/note_tree.rs
git commit -m "refactor(core): expose note-tree bare-leaf pruning (tesela-ju7)"
```

---

### Task 2: Enforce local-only blank leaves at the Loro projection boundary

**Files:**
- Modify: `crates/tesela-sync/src/engine/loro_engine/render.rs:1-45,171-220,296-310`
- Test: `crates/tesela-sync/src/engine/loro_engine/tests/ops.rs:1540-1570`
- Test: `crates/tesela-sync/src/engine/loro_engine/tests/convergence.rs:1160-1320`
- Update: `.docs/ai/current-state.md`
- Update: `.docs/ai/phases/2026-07-14-local-only-empty-blocks-spec.md`

**Interfaces:**
- Consumes: `tesela_core::note_tree::prune_bare_leaf_blocks_in_tree` from Task 1.
- Produces: identical pruning for `render_note`, `render_note_full`, materialized Markdown, and index derivation through `doc_full_markdown`.
- Leaves `read_block_text` and the underlying Loro node unchanged so the creating client can splice into its reservation.

- [ ] **Step 1: Invert the old blank-render regression and add the parent case**

Replace `blank_blocks_are_kept_as_editing_surface` with assertions that the real block remains and the bare leaf bid is absent. Add a second test creating an empty parent plus non-empty child and assert both bids remain in `render_note`.

```rust
#[tokio::test]
async fn bare_leaf_blocks_are_hidden_from_rendered_projection() {
    let engine = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    let note_id = [0x4b; 16];
    let real_id = uuid::Uuid::from_bytes([0x4a; 16]);
    let empty_id = uuid::Uuid::from_bytes([0x4b; 16]);
    let content = format!(
        "- real <!-- bid:{real_id} -->\n- <!-- bid:{empty_id} -->\n"
    );
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("blank-leaf".into()),
            title: "Blank leaf".into(),
            content,
            created_at_millis: 1,
        })
        .await
        .unwrap();

    let rendered = engine.render_note(note_id).await.unwrap();
    assert!(rendered.contains(&real_id.to_string()));
    assert!(!rendered.contains(&empty_id.to_string()));
}

#[tokio::test]
async fn empty_parent_with_child_remains_in_rendered_projection() {
    let engine = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    let note_id = [0x4c; 16];
    let parent_id = uuid::Uuid::from_bytes([0x4c; 16]);
    let child_id = uuid::Uuid::from_bytes([0x4d; 16]);
    let content = format!(
        "- <!-- bid:{parent_id} -->\n  - child <!-- bid:{child_id} -->\n"
    );
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("empty-parent".into()),
            title: "Empty parent".into(),
            content,
            created_at_millis: 1,
        })
        .await
        .unwrap();

    let rendered = engine.render_note(note_id).await.unwrap();
    assert!(rendered.contains(&parent_id.to_string()));
    assert!(rendered.contains(&child_id.to_string()));
}
```

Use the existing `record_local(OpPayload::NoteUpsert { ... })` setup pattern in the adjacent test rather than constructing Loro containers directly.

- [ ] **Step 2: Add the two-replica incident regression**

Add a test near `write_block_text_empty_base_concurrent_char_merges` that uses real engine exports/imports:

```rust
#[tokio::test]
async fn peer_hidden_blank_reservations_become_distinct_authored_blocks() {
    let note = blake3_note_id("local-only-blank-leaves");
    let a_blank = [0xd1; 16];
    let b_blank = [0xd2; 16];
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_b = DeviceId::from_bytes([0xb1; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));

    upsert_block(&a, note, a_blank, "", None).await;
    let a_reservation = a.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &a_reservation).await.unwrap();
    let a_bid = uuid::Uuid::from_bytes(a_blank).to_string();
    assert!(!b.render_note(note).await.unwrap().contains(&a_bid));

    let a_vv = a.doc_version(note).await;
    let b_vv = b.doc_version(note).await;
    upsert_block(&b, note, b_blank, "", None).await;
    upsert_block(&a, note, a_blank, "Is our conductor arena duplicating terminal bench?", None).await;
    upsert_block(&b, note, b_blank, "OpenCode desktop app\nstatus:: todo\ntags:: Task", None).await;

    let a_delta = a.export_doc_update(note, a_vv.as_deref()).await.unwrap();
    let b_delta = b.export_doc_update(note, b_vv.as_deref()).await.unwrap();
    a.import_doc_update(note, &b_delta).await.unwrap();
    b.import_doc_update(note, &a_delta).await.unwrap();

    for engine in [&a, &b] {
        let rendered = engine.render_note(note).await.unwrap();
        assert!(rendered.contains("Is our conductor arena"));
        assert!(rendered.contains("OpenCode desktop app"));
        assert!(!rendered.contains("tags:: TaskIs our conductor arena"));
        assert_eq!(block_text(engine, note, a_blank).await.as_deref(), Some("Is our conductor arena duplicating terminal bench?"));
        assert_eq!(block_text(engine, note, b_blank).await.as_deref(), Some("OpenCode desktop app\nstatus:: todo\ntags:: Task"));
    }
}
```

- [ ] **Step 3: Run the incident tests and verify RED**

Run:

```bash
cargo test -p tesela-sync bare_leaf_blocks_are_hidden_from_rendered_projection -- --nocapture
cargo test -p tesela-sync peer_hidden_blank_reservations_become_distinct_authored_blocks -- --nocapture
```

Expected: both fail because current `note_tree_from_doc` explicitly keeps blank bullets.

- [ ] **Step 4: Prune once in the shared renderer**

In `note_tree_from_doc`, build the complete `NoteTree`, call `prune_bare_leaf_blocks_in_tree`, and return the pruned tree. Remove the stale comment claiming blank bullets must be persisted. Do not delete Loro nodes.

For legacy docs whose root `content` bypasses `note_tree_from_doc`, route the content through the existing byte-preserving `prune_bare_leaf_blocks` function before returning it from `doc_full_markdown`; its no-change path preserves non-outliner bodies byte-for-byte.

- [ ] **Step 5: Run focused sync tests and verify GREEN**

Run:

```bash
cargo test -p tesela-sync bare_leaf_blocks_are_hidden_from_rendered_projection -- --nocapture
cargo test -p tesela-sync empty_parent_with_child_remains_in_rendered_projection -- --nocapture
cargo test -p tesela-sync peer_hidden_blank_reservations_become_distinct_authored_blocks -- --nocapture
cargo test -p tesela-sync write_block_text_empty_base_concurrent_char_merges -- --nocapture
```

Expected: all pass; the last confirms ordinary concurrent edits to an explicitly shared blank block still retain current CRDT semantics.

- [ ] **Step 6: Run the bead verification gate**

Run serially:

```bash
cargo test -p tesela-core -p tesela-sync
cargo clippy -p tesela-core -p tesela-sync --no-deps -- -D warnings
cargo fmt --all -- --check
pnpm --dir web test:unit
pnpm --dir web check
xcodebuild test -project app/Tesela-iOS/Tesela.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17 Pro'
```

Expected: all commands pass. If the named simulator is unavailable, list installed simulators and use the available iPhone 17 Pro runtime without changing source.

- [ ] **Step 7: Update handoff and commit Task 2**

- Mark `tesela-ju7` complete in `.docs/ai/current-state.md` and clear its active plan line.
- Change the spec status to implemented and record the verification commands and counts.
- Do not repair the live mosaic in this commit.

```bash
git add crates/tesela-sync/src/engine/loro_engine/render.rs \
  crates/tesela-sync/src/engine/loro_engine/tests/ops.rs \
  crates/tesela-sync/src/engine/loro_engine/tests/convergence.rs \
  .docs/ai/current-state.md \
  .docs/ai/phases/2026-07-14-local-only-empty-blocks-spec.md
git commit -m "fix(sync): keep empty leaf blocks local until meaningful (tesela-ju7)"
bd close tesela-ju7 --reason "Bare leaf reservations hidden centrally; two-replica incident regression and full gates green"
```

---

## Post-implementation human gate

After code verification and commit, ask for explicit approval before repairing the live 2026-07-14 note. The repair must:

1. take a fresh backup;
2. write through `LoroEngine`, not directly to Markdown;
3. remove the malformed property continuation from the original block;
4. restore the absorbed prose as a new child block with a fresh bid;
5. verify desktop and iOS converge after relaunch.
