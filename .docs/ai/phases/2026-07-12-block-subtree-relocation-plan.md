# Block Subtree Relocation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Apply superpowers:test-driven-development for every production change and superpowers:verification-before-completion before claiming the item complete.

**Goal:** Let Graphite Dailies users move one authoritative block subtree before, inside, or after a block in the same or another day, including appending to an absent ISO daily, by pointer drag or keyboard move mode without risking subtree loss.

**Architecture:** Add one typed, idempotent `SyncEngine::relocate_subtree` operation. `LoroEngine` validates and snapshots the subtree under deterministic per-note locks, persists a recoverable intent, makes the destination durable before deleting the source, then replaces the intent with a compact receipt. The server owns slug/daily validation and runs the normal post-write tail for each affected note. The web client sends only stable locators and waits for refreshed notes before settling its caches.

**Tech stack:** Rust, Tokio, Loro 1.13, Axum, serde/postcard, Svelte 5 runes, TanStack Query, TypeScript, CodeMirror 6, node:test, Playwright.

**Bead/spec:** `tesela-b54`; `.docs/ai/phases/2026-07-12-block-subtree-relocation-spec.md`; ADR in `.docs/ai/decisions.md` dated 2026-07-12.

## Global constraints

- Work only in `/Users/tfinklea/git/tesela/.worktrees/block-drag-dailies` on `feat/block-drag-dailies`.
- Preserve stable `bid`s. Never address a server move with the web block's line-derived `id`.
- Keep business and durability logic in `tesela-sync`; the route only validates transport/daily rules and runs the established server write tail.
- Never use whole-note PUT, `FsNoteStore::daily_note`, `get_daily_note`, or `create_note` to perform or prepare a relocation.
- A newly created daily must contain the moved subtree, not the default seed's placeholder blank bullet. Treat the trusted daily seed as frontmatter/root metadata; omit or replace its blank block while authoring the destination-durable tree.
- Use `save_snapshot_checked` and `materialize_note_checked` at every relocation durability boundary.
- Acquire source/destination apply locks in lexicographic note-id order and hold them through intent transitions, checked snapshots, materialization, and derived ownership refresh.
- Do not optimistically remove or clone browser blocks. The server response is authoritative.
- Do not change native iOS behavior or add arbitrary-page move UI.
- One focused commit per task. Do not push.

## Canonical interfaces

Use these names consistently across tasks. The canonical DTOs and trait method are `pub`; relocation record/snapshot helpers and failpoints are `pub(super)` or private to `loro_engine` as shown.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovePlacement {
    Before,
    Inside,
    After,
    Append,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RelocationNoteSeed {
    pub display_alias: Option<String>,
    pub title: String,
    pub content: String,
    pub created_at_millis: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BlockRelocationRequest {
    pub move_id: [u8; 16],
    pub source_note_id: [u8; 16],
    pub source_slug: String,
    pub root_bid: [u8; 16],
    pub destination_note_id: [u8; 16],
    pub destination_slug: String,
    pub target_bid: Option<[u8; 16]>,
    pub placement: MovePlacement,
    pub destination_seed: Option<RelocationNoteSeed>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockRelocationStatus {
    Applied,
    Replayed,
    NoOp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelocatedNoteVersion {
    pub note_id: [u8; 16],
    pub slug: String,
    pub pre_version: Vec<u8>,
    pub changed: bool,
    pub created: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockRelocationOutcome {
    pub move_id: [u8; 16],
    pub status: BlockRelocationStatus,
    pub notes: Vec<RelocatedNoteVersion>,
}
```

`SyncEngine` gains an object-safe owned method with a default unsupported implementation:

```rust
async fn relocate_subtree(
    &self,
    _request: BlockRelocationRequest,
) -> SyncResult<BlockRelocationOutcome> {
    Err(SyncError::Other("block subtree relocation is unsupported".into()))
}
```

Add typed errors rather than string-matching `Protocol`:

```rust
RelocationRejected(String),
RelocationConflict(String),
RelocationRecoveryRequired { move_id: [u8; 16], message: String },
```

The exact web contract is:

```ts
export type MovePlacement = "before" | "inside" | "after" | "append";

export type BlockMoveRequest = {
  move_id: string;
  source_note_id: string;
  root_bid: string;
  destination_note_id: string;
  target_bid: string | null;
  placement: MovePlacement;
};

export type BlockMoveDragPayload = {
  move_id: string;
  source_note_id: string;
  root_bid: string;
};

export const BLOCK_MOVE_MIME = "application/x-tesela-block-move";
```

The HTTP response is `{ move_id: string, notes: Note[] }`, with source first and destination second for cross-note moves, and exactly one note for same-note moves.

---

## Task 1: Lock the pure relocation contract and placement math

**Files:**

- Modify: `web/tests/unit/block-tree-move.test.mjs`
- Modify: `web/src/lib/block-tree-move.ts`

**Interfaces:**

- Consumes: existing `ParsedBlock` and `blk` test fixture.
- Produces: the web types in “Canonical interfaces” plus the five pure functions below.

- [ ] **Step 1: Write the failing pure-contract tests**

Add table-driven cases for extraction; before/inside/after/append; relative indent; same-note source removal; no-op; self/descendant rejection; drop-zone thirds; and strict MIME parsing. Include:

```js
test("extractSubtree uses stable bids and includes collapsed descendants", () => {
  const blocks = [blk("root", 0), blk("child", 1), blk("grandchild", 2), blk("tail", 0)];
  assert.deepEqual(
    extractSubtree(blocks, "root-bid").map((b) => b.bid),
    ["root-bid", "child-bid", "grandchild-bid"],
  );
});

test("decodeBlockMoveDragPayload rejects external and malformed drag data", () => {
  const payload = {
    move_id: "11111111-1111-4111-8111-111111111111",
    source_note_id: "2026-07-12",
    root_bid: "22222222-2222-4222-8222-222222222222",
  };
  const raw = JSON.stringify(payload);
  assert.equal(decodeBlockMoveDragPayload(["text/plain"], raw), null);
  assert.deepEqual(decodeBlockMoveDragPayload([BLOCK_MOVE_MIME], raw), payload);
  assert.equal(decodeBlockMoveDragPayload([BLOCK_MOVE_MIME], "{"), null);
});
```

- [ ] **Step 2: Run the focused test and observe red**

Run: `node --test web/tests/unit/block-tree-move.test.mjs`  
Expected: FAIL with missing named exports such as `extractSubtree` or `BLOCK_MOVE_MIME`.

- [ ] **Step 3: Implement the minimal pure contract**

Use these pure entry points:

```ts
export function extractSubtree(blocks: ParsedBlock[], rootBid: string): ParsedBlock[];
export function planBlockMove(args: {
  sourceBlocks: ParsedBlock[];
  rootBid: string;
  destinationBlocks: ParsedBlock[];
  targetBid: string | null;
  placement: MovePlacement;
  sameNote: boolean;
}): BlockMovePlan;
export function classifyDropPlacement(clientY: number, rect: Pick<DOMRect, "top" | "height">): "before" | "inside" | "after";
export function encodeBlockMoveDragPayload(payload: BlockMoveDragPayload): string;
export function decodeBlockMoveDragPayload(types: readonly string[], raw: string): BlockMoveDragPayload | null;
```

`BlockMovePlan` contains ordered subtree bids, insertion index after conceptual removal, destination indent/parent, and `noOp`. It is preview/validation logic only; the browser never submits it.

Extend the existing module rather than creating parallel movement logic. Validate UUID strings, exact payload fields, custom MIME, required/null target rules, and descendant targets. Preserve the existing helper API or migrate its five tests in this step.

- [ ] **Step 4: Run focused and type checks and observe green**

Run: `node --test web/tests/unit/block-tree-move.test.mjs`  
Expected: PASS for the original five tests and every new relocation case.

Run: `pnpm --dir web check`  
Expected: exit 0 with no new errors; existing warnings may remain.

- [ ] **Step 5: Commit Task 1**

```bash
git add web/src/lib/block-tree-move.ts web/tests/unit/block-tree-move.test.mjs
git commit -m "test(web): define block subtree relocation contract"
```

---

## Task 2: Make block ownership multi-owner and fail closed

**Files:**

- Modify: `crates/tesela-sync/src/engine/loro_engine/index.rs`
- Modify: `crates/tesela-sync/src/engine/loro_engine.rs`
- Modify: `crates/tesela-sync/src/engine/loro_engine/apply.rs`
- Modify: `crates/tesela-sync/src/engine/loro_engine/twins.rs`
- Modify: `crates/tesela-sync/src/engine/loro_engine/tests.rs`
- Create: `crates/tesela-sync/src/engine/loro_engine/tests/relocation.rs`

**Interfaces:**

- Consumes: `Inner.block_index`, `note_id_for_payload`, `find_doc_for_block`, `refresh_note_derived`.
- Produces: `BlockIndex` and error-bearing unique-owner resolution for Task 3.

- [ ] **Step 1: Write failing duplicate-owner tests**

Add `duplicate_owner_is_ambiguous_and_legacy_mutation_fails` and `duplicate_owner_heals_after_one_copy_is_deleted`. Their central assertions are:

```rust
type BlockOwners = BTreeSet<[u8; 16]>;
type BlockIndex = HashMap<[u8; 16], BlockOwners>;

let owners = engine.inner.block_index.read().await;
assert_eq!(owners.get(&bid).unwrap(), &BTreeSet::from([note_a, note_b]));
drop(owners);

let err = engine
    .record_local(OpPayload::BlockDelete { block_id: bid })
    .await
    .expect_err("ambiguous block mutation must fail closed");
assert!(matches!(err, SyncError::Protocol(message) if message.contains("ambiguous")));
```

- [ ] **Step 2: Run the focused tests and observe red**

Run: `cargo test -p tesela-sync engine::loro_engine::tests::relocation::duplicate_owner`  
Expected: FAIL because the current map retains one owner and delete does not surface ambiguity.

- [ ] **Step 3: Implement multi-owner resolution**

Change the resolver and caller to:

```rust
async fn note_id_for_payload(
    &self,
    payload: &OpPayload,
) -> SyncResult<Option<[u8; 16]>>;

match self.note_id_for_payload(&payload).await? {
    Some(note_id) => {
        let apply_lock = self.apply_lock_for_note(note_id).await;
        let _apply_guard = apply_lock.lock().await;
        self.record_local_locked(payload).await
    }
    None => self.record_local_locked(payload).await,
}
```

For bid-only payloads: no owners → `Ok(None)`; one → `Ok(Some(note_id))`; multiple → log all owners and return `SyncError::Protocol("ambiguous block ownership for <bid>")`. Make `find_doc_for_block` error-bearing and propagate it through current `SyncResult` apply paths. Change `Inner.block_index`, `build_block_index`, deletion, import/upsert registration, and derived refresh to owner sets. Replacement refresh removes the note from every set, drops empty sets, then registers current bids. Do not scan docs as fallback. Task 3 maps ambiguity to `RelocationRejected`.

- [ ] **Step 4: Run focused and regression tests and observe green**

Run: `cargo test -p tesela-sync engine::loro_engine::tests::relocation::duplicate_owner`  
Expected: PASS for ambiguity and healing.

Run: `cargo test -p tesela-sync engine::loro_engine::tests::ops`  
Expected: PASS; unique-owner and unknown-block behavior remains green.

- [ ] **Step 5: Commit Task 2**

```bash
git add crates/tesela-sync/src/engine/loro_engine.rs crates/tesela-sync/src/engine/loro_engine/index.rs crates/tesela-sync/src/engine/loro_engine/apply.rs crates/tesela-sync/src/engine/loro_engine/twins.rs crates/tesela-sync/src/engine/loro_engine/tests.rs crates/tesela-sync/src/engine/loro_engine/tests/relocation.rs
git commit -m "fix(sync): fail closed on ambiguous block ownership"
```

---

## Task 3: Implement in-memory same-note and cross-note relocation

**Files:**

- Modify: `crates/tesela-sync/src/engine/mod.rs`
- Modify: `crates/tesela-sync/src/lib.rs`
- Modify: `crates/tesela-sync/src/error.rs`
- Modify: `crates/tesela-sync/src/engine/loro_engine.rs`
- Modify: `crates/tesela-sync/src/engine/loro_engine/prop_containers.rs`
- Modify: `crates/tesela-sync/src/engine/loro_engine/twins.rs`
- Create: `crates/tesela-sync/src/engine/loro_engine/relocation.rs`
- Modify: `crates/tesela-sync/src/engine/loro_engine/tests/relocation.rs`

**Interfaces:**

- Consumes: Task 2 unique-owner resolution and the canonical Rust DTOs.
- Produces: public `SyncEngine::relocate_subtree` plus private snapshot/placement/apply functions later wrapped by Task 4.

- [ ] **Step 1: Write failing in-memory relocation tests**

Add tests for:

- same-note before/inside/after/append, including source-removal index adjustment and no-op;
- complete nested movement with stable bids, relative indentation, parent metadata, and flat render order;
- cross-note copy/delete preserving `PropScalar::{Text,Int,Float,Bool}`, text properties, and ordered lists;
- root/descendant targets, missing root/target, target/placement mismatch, and ambiguous ownership leave both rendered notes byte-identical;
- a trusted daily seed yields frontmatter plus the moved subtree without a blank placeholder sibling.

The representative call/assertion is:

```rust
let outcome = engine
    .relocate_subtree(BlockRelocationRequest {
        move_id: [9; 16],
        source_note_id: source,
        source_slug: "2026-07-12".into(),
        root_bid: root,
        destination_note_id: destination,
        destination_slug: "2026-07-11".into(),
        target_bid: Some(target),
        placement: MovePlacement::Inside,
        destination_seed: None,
    })
    .await
    .unwrap();
assert_eq!(outcome.status, BlockRelocationStatus::Applied);
assert_eq!(block_texts(&engine, source).await, vec!["source-tail"]);
assert_eq!(
    block_texts(&engine, destination).await,
    vec!["target", "moved-root", "moved-child"],
);
```

- [ ] **Step 2: Run the focused tests and observe red**

Run: `cargo test -p tesela-sync engine::loro_engine::tests::relocation`  
Expected: compile FAIL for missing `BlockRelocationRequest`/`relocate_subtree`.

- [ ] **Step 3: Implement the minimal in-memory engine operation**

Add the canonical DTOs/errors/re-exports and default trait method. Move `ResolvedValue` from `twins.rs` into `prop_containers.rs` as `pub(super)`, update twins imports, snapshot with `read_node_prop_containers` + `read_props_typed`, and re-author with `prop_set_scalar`/`prop_set_text`/ordered `prop_add_to_list`. Never use `materialize_props`.

Implement locked preparation/apply in `relocation.rs`:

- Same note: reorder root children with Loro `mov_to`/`mov_before`/`mov_after` and rewrite moved root/descendant indent/parent metadata, preserving node identity.
- Cross note: create destination root children in final flat order with captured bids/text/metadata/typed values, then delete exact captured source `TreeID`s directly.
- Compute same-note placement after conceptual source removal and revalidate under both locks.

Task 3 implements the trait method directly with the in-memory body. Nothing calls it outside engine tests until Task 5; Task 4 replaces that body with the durable wrapper before the route is added.

- [ ] **Step 4: Run focused and engine regressions and observe green**

Run: `cargo test -p tesela-sync engine::loro_engine::tests::relocation`  
Expected: PASS for placement, identity, properties, blank-seed removal, and rejected preconditions.

Run: `cargo test -p tesela-sync engine::loro_engine::tests::ops`  
Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add crates/tesela-sync/src/engine/mod.rs crates/tesela-sync/src/lib.rs crates/tesela-sync/src/error.rs crates/tesela-sync/src/engine/loro_engine.rs crates/tesela-sync/src/engine/loro_engine/prop_containers.rs crates/tesela-sync/src/engine/loro_engine/twins.rs crates/tesela-sync/src/engine/loro_engine/relocation.rs crates/tesela-sync/src/engine/loro_engine/tests/relocation.rs
git commit -m "feat(sync): relocate block subtrees across Loro notes"
```

---

## Task 4: Add durable intents, idempotent receipts, and boot recovery

**Files:**

- Modify: `crates/tesela-sync/src/engine/loro_engine/relocation.rs`
- Modify: `crates/tesela-sync/src/engine/loro_engine.rs`
- Modify: `crates/tesela-sync/src/engine/loro_engine/tests/relocation.rs`

**Interfaces:**

- Consumes: Task 3 prepare/apply functions and checked persistence/materialization helpers.
- Produces: recoverable `relocate_subtree`, boot recovery, receipts, and engine-captured pre-version outcomes used by Task 5.

- [ ] **Step 1: Add the test-only failpoint and failing recovery tests**

Define the exact test seam in `relocation.rs` and add an optional test-only field to `Inner`:

```rust
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RelocationFailpoint {
    AfterPrepared,
    AfterDestinationDurable,
    AfterSourceDurable,
}

#[cfg(test)]
// Field on Inner; initialize to Mutex::new(None) in every constructor.
relocation_failpoint: tokio::sync::Mutex<Option<RelocationFailpoint>>,

#[cfg(test)]
impl LoroEngine {
    pub(super) async fn inject_relocation_failure_once(
        &self,
        failpoint: RelocationFailpoint,
    ) {
        *self.inner.relocation_failpoint.lock().await = Some(failpoint);
    }
}
```

Each checkpoint consumes the one-shot failpoint immediately after atomically persisting that named phase and returns `RelocationRecoveryRequired` before the next phase. Tests open a tempfile engine, inject each failpoint, assert the error, drop it, reopen with `with_dirs`, and assert one destination copy/no source copy. Also test snapshot order, repeated recovery, replay/conflict, active-intent overlap rejection, position-independent destination proof, reload, the 4,096 full-receipt cap plus permanent compact tombstones, both delta arrival orders, and deleted-wins old-source edits.

- [ ] **Step 2: Run the focused tests and observe red**

Run: `cargo test -p tesela-sync engine::loro_engine::tests::relocation`  
Expected: compile FAIL for `RelocationFailpoint` and missing persistent records/recovery.

- [ ] **Step 3: Implement durable records and boot recovery**

Atomically replace postcard records under `snapshot_dir/_relocations/<move-id>.bin`:

```rust
enum RelocationPhase { Prepared, DestinationDurable, SourceDurable }
enum RelocationRecord { Intent(RelocationIntent), Receipt(RelocationReceipt) }
```

Intent fields: canonical request hash, ids/slugs, semantic subtree, exact captured source nodes/bids, computed destination ancestry/order, optional seed, pre-move encoded version vectors, and phase. Receipt fields: move id, request hash, status, affected notes/pre-versions, destination-root metadata proof, and pruning `HlcTimestamp`. Persist a separate exact compact tombstone containing only move id and request hash for every completed request; full receipts cap at 4,096 while tombstones are retained permanently.

Implement `LoroEngine::relocate_subtree` boundaries:

1. Resolve lock objects, sort note ids, acquire once per distinct note.
2. Validate and persist `Prepared` before mutation.
3. Apply destination, commit, checked-save/materialize, persist `DestinationDurable`.
4. Cross-note only: delete captured source nodes, commit, checked-save/materialize, persist `SourceDurable`.
5. Refresh ownership/derived state, persist the permanent tombstone, replace intent with receipt, and prune old full receipts only.

Reject preparation when another active intent reserves the same source root. Write non-materialized move-id + request-hash metadata on the destination root and locate that proof-bearing subtree independent of absolute position during recovery. After full-receipt pruning, a matching tombstone returns a stale/replayed-safe result without mutation and a mismatched hash conflicts; an old move id never executes twice.

Run recovery in `with_dirs` after loaded docs receive the local peer id and before return. Recovery inspects state and idempotently completes active records. Checked persistence/materialization failure returns `RelocationRecoveryRequired` and keeps the record.

- [ ] **Step 4: Run focused and crate tests and observe green**

Run: `cargo test -p tesela-sync engine::loro_engine::tests::relocation`  
Expected: PASS for all injected phases, retry semantics, pruning, convergence, and race behavior.

Run: `cargo test -p tesela-sync`  
Expected: PASS.

- [ ] **Step 5: Commit Task 4**

```bash
git add crates/tesela-sync/src/engine/loro_engine/relocation.rs crates/tesela-sync/src/engine/loro_engine.rs crates/tesela-sync/src/engine/loro_engine/tests/relocation.rs
git commit -m "feat(sync): recover interrupted subtree relocations"
```

---

## Task 5: Expose the recoverable server command and two-note tail

**Files:**

- Modify: `crates/tesela-server/src/error.rs`
- Modify: `crates/tesela-server/src/routes/mod.rs`
- Modify: `crates/tesela-server/src/routes/notes.rs`
- Create: `crates/tesela-server/tests/block_subtree_move.rs`

**Interfaces:**

- Consumes: Task 4 `BlockRelocationRequest`/`BlockRelocationOutcome` and typed errors.
- Produces: `POST /blocks/move-subtree` with the approved request and `{ move_id, notes }` response.

- [ ] **Step 1: Write failing route and error-mapping tests**

Create a spawned-server suite mirroring `block_granular_write.rs` and real WebSocket tests. Set `TESELA_DISABLE_MDNS=1`, `TESELA_DISABLE_PEER_SYNC=1`, and `TESELA_GROUP_KEY_FILE_STORE=1`. Cover malformed UUID/target rules; self/descendant targets; missing non-daily; rejected absent daily with no file; successful absent-daily append with no blank sibling; cross-note bids/properties/search/links/versions/events/two TLR2 deltas; same-note one result/event/delta; identical retry; and changed move-id reuse 409. Include:

```rust
let response = client
    .post(format!("{base}/blocks/move-subtree"))
    .json(&serde_json::json!({
        "move_id": "11111111-1111-4111-8111-111111111111",
        "source_note_id": "2026-07-12",
        "root_bid": ROOT_BID,
        "destination_note_id": "2026-07-11",
        "target_bid": TARGET_BID,
        "placement": "inside"
    }))
    .send()
    .await
    .unwrap();
assert_eq!(response.status(), reqwest::StatusCode::OK);
let body: serde_json::Value = response.json().await.unwrap();
assert_eq!(body["notes"].as_array().unwrap().len(), 2);
```

Add `#[cfg(test)]` cases in `error.rs` constructing `AppError::Conflict` and `AppError::RetrySafe` and asserting 409 plus 503 `{ "error", "move_id", "retry_safe": true }`. Engine failpoint tests, not a production HTTP hook, prove the post-intent failure source.

- [ ] **Step 2: Run route tests and observe red**

Run: `cargo test -p tesela-server --test block_subtree_move`  
Expected: FAIL with HTTP 404 for `/blocks/move-subtree`.

Run: `cargo test -p tesela-server error::tests`  
Expected: compile FAIL for missing `Conflict`/`RetrySafe`.

- [ ] **Step 3: Implement the route and two-note tail**

Register `POST /blocks/move-subtree`. Use the exact transport DTO; note ids remain slugs and only locators are UUIDs:

```rust
#[derive(Deserialize)]
struct MoveBlockSubtreeReq {
    move_id: uuid::Uuid,
    source_note_id: String,
    root_bid: uuid::Uuid,
    destination_note_id: String,
    target_bid: Option<uuid::Uuid>,
    placement: MovePlacement,
}

#[derive(Serialize)]
struct MoveBlockSubtreeResp {
    move_id: uuid::Uuid,
    notes: Vec<Note>,
}
```

Convert UUID fields to bytes and slugs with `stable_uuid_from_slug` only at the sync boundary. Handler sequence:

1. Bootstrap existing source/destination relay state with `bootstrap_note_if_needed` and re-read.
2. Missing destination: accept only ISO `%Y-%m-%d` + append + null target. Build inert `RelocationNoteSeed` from `daily_note_content`; do not call a writing create/daily route.
3. Call engine and map typed rejection, conflict (409), and recovery-required (`AppError::RetrySafe`/503).
4. Per distinct outcome note, source then destination: re-read, reindex, links, version only for first-apply changed existing notes, tags, `NoteUpdated`, one cursor-free TLR2 export from engine-captured `pre_version`.
5. Return `{ move_id, notes }`, deduplicated for same-note.

On replay, repeat repairable read/index/link/tag/event/delta tail but do not duplicate history. Re-read/reindex failure after durable engine success is also retry-safe 503.

- [ ] **Step 4: Run route and server regressions and observe green**

Run: `cargo test -p tesela-server --test block_subtree_move`  
Expected: PASS for validation, daily creation, indexes/history, events/deltas, same-note, and retries.

Run: `cargo test -p tesela-server`  
Expected: PASS, including `error::tests`.

- [ ] **Step 5: Commit Task 5**

```bash
git add crates/tesela-server/src/error.rs crates/tesela-server/src/routes/mod.rs crates/tesela-server/src/routes/notes.rs crates/tesela-server/tests/block_subtree_move.rs
git commit -m "feat(server): expose recoverable subtree relocation"
```

---

## Task 6: Add the web API and canonical move command

**Files:**

- Modify: `web/src/lib/block-tree-move.ts`
- Modify: `web/src/lib/api-client.ts`
- Modify: `web/src/lib/commands/index.ts`
- Modify: `web/src/lib/command-manifest.json` through `pnpm --dir web generate:commands`
- Create: `web/tests/unit/block-relocation-api.test.mjs`
- Modify: `web/tests/unit/command-manifest-file.test.mjs`

**Interfaces:**

- Consumes: Task 1 `BlockMoveRequest` and Task 5 HTTP contract.
- Produces: `api.relocateBlockSubtree` and canonical `move-block-subtree` command/event.

- [ ] **Step 1: Write failing API and manifest tests**

In `block-relocation-api.test.mjs`, read `api-client.ts` following `block-editor-attachments.test.mjs` and assert the typed method posts `/blocks/move-subtree` and calls `recordLocalSave` for both request slugs and returned note ids. In `command-manifest-file.test.mjs` add:

```js
test("manifest exposes Move block subtree on the free a m chord", () => {
  const entry = manifest.find((item) => item.id === "move-block-subtree");
  assert.ok(entry);
  assert.equal(entry.label, "Move block subtree");
  assert.deepEqual(entry.chord, ["a", "m"]);
  assert.ok(entry.surfaces.includes("editor"));
});
```

- [ ] **Step 2: Run unit tests and observe red**

Run: `pnpm --dir web test:unit`  
Expected: FAIL because the API method and manifest entry are absent.

- [ ] **Step 3: Implement the API method and registry command**

```ts
{
  id: "move-block-subtree",
  verb: "move-block-subtree",
  label: "Move block subtree",
  glyph: "↕",
  category: "tile",
  chord: ["a", "m"],
  surface: "editor",
  keywords: ["move", "block", "subtree", "drag", "daily"],
  when: (ctx) => !!ctx.focusedBlock,
  run: () => window.dispatchEvent(new CustomEvent("tesela:start-block-move")),
}
```

Add `api.relocateBlockSubtree(req: BlockMoveRequest, signal?: AbortSignal)` returning `Promise<{ move_id: string; notes: Note[] }>` and posting to `/blocks/move-subtree`. Call `recordLocalSave` for source/destination before POST and each returned id after. Add the command to the canonical registry, then generate the manifest; never hand-edit JSON.

- [ ] **Step 4: Generate and verify green**

Run: `pnpm --dir web generate:commands`  
Expected: reports `src/lib/command-manifest.json` written with the new command.

Run: `pnpm --dir web check`  
Expected: exit 0 with no new errors.

Run: `pnpm --dir web test:unit`  
Expected: PASS.

- [ ] **Step 5: Commit Task 6**

```bash
git add web/src/lib/block-tree-move.ts web/src/lib/api-client.ts web/src/lib/commands/index.ts web/src/lib/command-manifest.json web/tests/unit/block-relocation-api.test.mjs web/tests/unit/command-manifest-file.test.mjs
git commit -m "feat(web): add block relocation command contract"
```

---

## Task 7: Implement pointer drag, keyboard mode, and same-note parity

**Files:**

- Modify: `web/src/lib/components/BlockOutliner.svelte`
- Modify: `web/src/lib/components/JournalView.svelte`
- Modify: `web/src/lib/block-tree-move.ts`
- Modify: `web/tests/unit/block-tree-move.test.mjs`

**Interfaces:**

- Consumes: Tasks 1/6 pure contract, API method, and `tesela:start-block-move`.
- Produces: opt-in outliner relocation bindings and one Journal-owned `BlockMoveSession`.

- [ ] **Step 1: Write failing session and same-note request tests**

Add the exact pure state types to test imports:

```ts
export type BlockMoveSession = {
  phase: "idle" | "selecting" | "pending" | "retryable";
  request: BlockMoveRequest | null;
  targetBid: string | null;
  targetNoteId: string | null;
  placement: MovePlacement | null;
};

export type BlockMoveSessionAction =
  | { type: "start"; request: BlockMoveRequest }
  | { type: "target"; noteId: string; bid: string | null; placement: MovePlacement }
  | { type: "submit" }
  | { type: "success" | "cancel" | "ordinary-error" }
  | { type: "recoverable-error" };
```

Test every transition and `sameNoteMoveRequestForAction(blocks, focusedBid, noteId, "up" | "down" | "indent", moveId)`. Include:

```js
test("recoverable error retains the exact move id for retry", () => {
  const selecting = reduceBlockMoveSession(IDLE_BLOCK_MOVE_SESSION, {
    type: "start",
    request,
  });
  const pending = reduceBlockMoveSession(selecting, { type: "submit" });
  const retryable = reduceBlockMoveSession(pending, { type: "recoverable-error" });
  assert.equal(retryable.phase, "retryable");
  assert.equal(retryable.request.move_id, request.move_id);
});
```

- [ ] **Step 2: Run pure tests and observe red**

Run: `node --test web/tests/unit/block-tree-move.test.mjs`  
Expected: FAIL for missing session reducer and same-note request helper.

- [ ] **Step 3: Implement pure state and BlockOutliner bindings**

Add `reduceBlockMoveSession`, `IDLE_BLOCK_MOVE_SESSION`, and `sameNoteMoveRequestForAction` to `block-tree-move.ts`. Add this optional prop contract:

```ts
type RelocationBindings = {
  sourceBid: string | null;
  targetBid: string | null;
  placement: MovePlacement | null;
  pending: boolean;
  onDragStart: (event: DragEvent, sourceBid: string) => void;
  onDragOver: (event: DragEvent, targetBid: string, placement: Exclude<MovePlacement, "append">) => void;
  onDrop: (event: DragEvent, targetBid: string, placement: Exclude<MovePlacement, "append">) => void;
  onCancel: () => void;
};
```

Each row exposes `data-note-id`/`data-block-bid`, handle `data-move-handle`, and feedback `data-drop-placement`. Drag only `BlockMoveDragPayload`. Derive count/invalid descendants from full `blocks` while highlighting mounted descendants. Preserve bullet/CodeMirror behavior and reject external MIME.

Route Alt-Up, Alt-Down, and Alt-Right through the same same-note API instead of `saveBlocks`/whole-note PUT. Leave the existing granular Alt-Shift-Left outdent path unchanged.

- [ ] **Step 4: Implement Journal pointer and keyboard orchestration**

Own one session across outliners. Date header/empty body means append. Force-mount synthetic targets without `getDailyNote` and auto-scroll `closest(".gr-outline")`. Keep source rendered pending. Success seeds returned `["note", note.id]` caches, invalidates `["notes"]`, and restores the moved bid. 400/404/409 preserves source/focus; retry-safe 503 retains request/move id.

Use document capture while active: `j`/`k` traverse rows/date headers; `b`/`i`/`a` commit; date header commits append; Escape cancels. Render `data-move-mode` and suppress ordinary cross-day creation/navigation.

Clear state on drop, drag-end, success, ordinary error, or Escape; preserve only a recoverable request for explicit retry.

- [ ] **Step 5: Run web checks and observe green**

Run: `node --test web/tests/unit/block-tree-move.test.mjs`  
Expected: PASS.

Run: `pnpm --dir web check`  
Expected: exit 0 with no new errors.

Run: `pnpm --dir web test:unit`  
Expected: PASS.

- [ ] **Step 6: Commit Task 7**

```bash
git add web/src/lib/components/BlockOutliner.svelte web/src/lib/components/JournalView.svelte web/src/lib/block-tree-move.ts web/tests/unit/block-tree-move.test.mjs
git commit -m "feat(web): move block subtrees across dailies"
```

---

## Task 8: Prove the flow end-to-end and close the handoff

**Files:**

- Modify: `web/tests/e2e/run.mjs`
- Create: `web/tests/e2e/block-subtree-relocation.spec.ts`
- Create: `.docs/ai/phases/2026-07-12-block-subtree-relocation-report.md`
- Modify: `.docs/ai/current-state.md`
- Close bead `tesela-b54`

**Interfaces:**

- Consumes: completed engine/server/web flow.
- Produces: Playwright proof, rendered evidence, phase report, closed bead, and clean full verification.

- [ ] **Step 1: Write the failing E2E fixture and spec**

Seed two ISO dailies with nested stable bids/properties plus one absent adjacent date. Export `TESELA_E2E_SOURCE_DAILY`, `TESELA_E2E_DEST_DAILY`, and `TESELA_E2E_ABSENT_DAILY`. Test same-day placements; nested cross-day drag/focus; absent-day append/no blank; invalid/external drag; Escape; `a m` plus cross-day keys; Alt-arrow reload; source retention and retry messaging. Include:

```ts
test("block subtree relocation: parent and children move to another day", async ({ page }) => {
  await page.goto("/g");
  const source = page.locator(".day[data-daily='" + SOURCE + "']");
  const destination = page.locator(".day[data-daily='" + DESTINATION + "']");
  await source.locator("[data-block-bid='" + ROOT_BID + "'] [data-move-handle]")
    .dragTo(destination.locator("[data-block-bid='" + TARGET_BID + "']"));
  await expect(destination.locator("[data-block-bid='" + ROOT_BID + "']")).toBeVisible();
  await expect(destination.locator("[data-block-bid='" + CHILD_BID + "']")).toBeVisible();
  await expect(source.locator("[data-block-bid='" + ROOT_BID + "']")).toHaveCount(0);
});
```

Use `[data-daily]`, `[data-block-bid]`, `[data-move-handle]`, `[data-drop-placement]`, and `[data-move-mode]`. Select by text only for final content assertions.

- [ ] **Step 2: Run E2E and observe red**

Run: `pnpm --dir web test:e2e`  
Expected: FAIL in the new relocation spec on missing hooks/fixture flow.

- [ ] **Step 3: Resolve E2E harness issues or reject the owning task**

Fix only fixture seeding, selector, and deterministic wait problems in `run.mjs`/the new spec. If a failure reveals missing production behavior, treat it as a Task 7 review rejection, implement the named requirement there with a focused unit test, rerun Task 7 gates, and amend the Task 7 commit before continuing.

- [ ] **Step 4: Run targeted verification and observe green**

Run each separately; expected exit 0/PASS:

```bash
cargo test -p tesela-sync engine::loro_engine::tests::relocation
cargo test -p tesela-server --test block_subtree_move
node --test web/tests/unit/block-tree-move.test.mjs
pnpm --dir web test:e2e
```

- [ ] **Step 5: Perform rendered Graphite QA**

Load `browser:control-in-app-browser`. Start the harness; inspect meaningful rendering/no overlay/healthy console; perform pointer and keyboard moves; save screenshots only under ignored `ai-scratch/` after `git check-ignore -v ai-scratch/probe` succeeds. Do not commit them.

- [ ] **Step 6: Run full gates**

Run each separately; expected exit 0/PASS:

```bash
cargo fmt --all -- --check
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
pnpm --dir web check
pnpm --dir web test:unit
```

- [ ] **Step 7: Write report, close bead, and commit Task 8**

Write the report with interfaces, recovery guarantees, evidence, deferred concurrent-same-root policy, and manual QA. Mark the handoff plan complete and close `tesela-b54`.

```bash
bd close tesela-b54 --reason "Implemented recoverable block subtree relocation; engine, server, web unit, E2E, and full workspace gates pass"
git add web/tests/e2e/run.mjs web/tests/e2e/block-subtree-relocation.spec.ts .docs/ai/phases/2026-07-12-block-subtree-relocation-report.md .docs/ai/current-state.md
git commit -m "test(e2e): verify block subtree relocation"
git status --short --branch
```

Expected: bead closed, commit succeeds, worktree clean, branch remains `feat/block-drag-dailies`.

## Manual QA handoff required in the final response

Include exact click/key paths and outcomes for same-day placements; nested cross-day drag; existing/absent day-header append; `a m`/`j`/`k`/`b`/`i`/`a`/Escape; invalid self/descendant; recovery retry; and regressions around bullet drill-in/context menu, CodeMirror drops, Alt-arrow moves, saving, reload, and cross-day navigation.
