# Multi-device convergence fix — spec (2026-05-31)

> Execution spec for task #146 (within milestone #140 "instant multi-device sync").
> Build subagent-driven, two-stage review per task. Land **all of Part E + Part D
> together** before the live device test (R1: an E-only state has a transient
> edit-drop window — see Risks). Part B receive-cap is a tiny include; full
> snapshot→delta is a flagged follow-up.

## Root cause (confirmed)

Each note is its own Loro doc: a `LoroTree` named `"blocks"`, each block a node
under Root with meta `{ block_id (hex of bid), text, indent_level, parent }`.
Loro tree **node identity is the internal `TreeID` (peer+counter), NOT the
`block_id` meta**. The Mac server seeds note docs from disk; iOS `recordNoteDiff`
re-authors blocks from its *own* markdown into a doc that **never imported the
server's doc as a base**, so `BlockUpsert`'s `find_node_by_block_id` misses and
`tree.create(Root)` **mints a new TreeID** under the iOS peer. Same bid → two
TreeIDs. iOS ships a **full snapshot every keystroke**; the server imports it and
Loro **unions** the twins. `note_tree_from_doc` renders both (no dedup), and the
next web block-diff save updates only one twin (FxHashMap scan order →
nondeterministic), leaving a stale ghost = "my web edit reverted on refresh".

Deterministic repro (currently RED): `crates/tesela-sync/tests/disjoint_history_revert.rs`.
Run: `cargo test -p tesela-sync --test disjoint_history_revert -- --nocapture`.

Self-heal quirk: a title/frontmatter-only edit takes the `NoteUpsert` path
(`clear_block_tree` + `seed_tree_from_flatblocks`) which rebuilds the tree clean
— why it "works for a while, then breaks".

## Fix shape

- **Part E — heal + contain (low risk):**
  - E1: deterministic **dedup-by-block_id** (collapse twins to one node), in
    render (`note_tree_from_doc`) AND as an import-time tombstone
    (`import_doc_update`). Heals already-corrupted on-disk docs.
  - E2: **gate the iOS relay side-channel** (`hubMode` flag) so the cached-pairing-code
    coordinator can't inject foreign state while the WS hub path is active.
- **Part D — converge (the real fix):** give the device the server's doc as a
  **shared base** before it authors, via `GET /loro/notes/{id}/snapshot` +
  iOS import-before-first-author. Then iOS BlockUpserts resolve to the *existing*
  server nodes → true convergence.
- **Part B — include the WS receive-cap one-liner;** flag full snapshot→delta
  as a follow-up (task #147).
- **Skip Part C** (deterministic TreeID-from-bid): loro 1.12 forbids
  caller-chosen TreeIDs (`create`/`create_at` mint `txn.next_id()`; target-id
  methods are `pub(crate)`). Forking loro = not worth it.

## Invariants (spec-derived — assert these)

1. **No duplication (heal):** for any note doc, render emits **exactly one bullet
   per distinct `block_id`**, regardless of how many TreeIDs carry that bid.
2. **Deterministic dedup:** the surviving twin is chosen by a **stable rule**,
   identical across process restarts / hash reorderings. Default rule:
   **prefer the twin whose `text` meta was most-recently updated IF loro 1.12
   exposes a reliable per-update lamport; else the lexicographically-min `TreeID`
   (peer then counter).** Document which rule was used. (min-TreeID is
   deterministic but NOT recency-aware — see R1.)
3. **True convergence (shared base):** when a device imports the server's note
   snapshot *before* authoring, concurrent edits to *different* blocks on the two
   sides both survive with **correct text**, each block once. This is the real
   green anchor and is **only** achievable via the shared base — NOT via dedup.
4. **Idempotent import:** re-importing the same snapshot is a no-op (no new twins,
   no re-tombstone churn).
5. **Reversible relay gate:** `hubMode=false` rebuilds the relay coordinator from
   the cached pairing code with no Mac HTTP fetch. The relay feature is gated,
   not deleted; the cached code is NOT cleared.

## Test anchors (reframed — important)

The ORIGINAL `disjoint_history_revert.rs` assertion ("latest text survives a
disjoint merge") is **unachievable by dedup alone** (min-TreeID picks by peer,
not recency: in the repro it keeps server-alpha=EDITED ✓ but server-beta=stale,
dropping the device's beta edit ✗). So split into two tests in that file:

- **T-heal `disjoint_merge_dedups_to_single_node_deterministically`** (Part E anchor):
  seed two disjoint engines from the same markdown, edit + merge as today, assert
  invariant 1 (exactly one bullet per bid) and invariant 2 (deterministic winner;
  run the merge twice / rebuild and assert identical render). Does **not** assert
  text-correctness of the disjoint merge.
- **T-converge `shared_base_converges_with_correct_text`** (Part D anchor):
  server seeds note → `export_doc_update(note,None)` → device `import_doc_update`
  (shared base) → device BlockUpserts beta="beta from device" → exports snapshot
  → server imports → server BlockUpserts alpha="alpha EDITED" → render asserts
  BOTH edits present, correct text, each block exactly once (invariant 3).

Do NOT leave a RED test on `main` after the work: T-heal goes green at E1; the
old impossible assertions are replaced, not left failing.

## Tasks (Part E first, then Part D; one commit each)

### E1 — dedup-by-block_id (Rust; `crates/tesela-sync/src/engine/loro_engine.rs`)
- **E1-a** Add `dedup_twins_by_block_id(tree, nodes: Vec<TreeID>) -> Vec<TreeID>`
  (pure): group live nodes by `block_id` meta; for groups >1, keep one per
  invariant 2; preserve original walk order. **Verify first** (R6): how loro 1.12
  exposes `TreeID` ordering (`peer`/`counter` fields or a getter) and whether a
  per-`text`-update lamport is reachable (e.g. `get_last_move_id`+`get_change`, or
  map-entry version) — pick the recency rule if reliable, else min-TreeID; record
  the choice in a code comment. Commit: `fix(sync): deterministic dedup-by-block_id helper`.
- **E1-b** Wire it into `note_tree_from_doc` (filter the `children(Root)` list
  through dedup before building FlatBlocks). Mirror the existing
  `is_node_deleted` filtering. Commit: `fix(sync): dedup duplicate-bid twins at render`.
- **E1-c** Add `tombstone_duplicate_twins(doc, note_id)`; call it in
  `import_doc_update` after `doc.import(bytes)` succeeds and BEFORE
  `refresh_note_derived`. Tombstone non-canonical twins (`tree.delete`) + `doc.commit()`
  if any. Idempotent (invariant 4). Keep tombstoning ONLY on the import path
  (the `NoteUpsert` reseed path already clears twins — R2). Commit:
  `fix(sync): tombstone duplicate-bid twins on import`.
- **E1-d** Rewrite the two test anchors (T-heal + T-converge above) in
  `crates/tesela-sync/tests/disjoint_history_revert.rs`. Run
  `cargo test -p tesela-sync` (all unit + integration green). Commit:
  `test(sync): dedup heal + shared-base convergence anchors`.

### E2 — gate the iOS relay side-channel (Swift)
- **E2-a** `app/Tesela-iOS/Sources/Data/RelayTicker.swift`: add `hubMode` (backed by
  an atomic or plain `@Published`/stored bool — match the file's style). When
  `hubMode==true`: `tickOnce()` returns early; `recordAndPush` skips the
  coordinator build+tick block; on the `hubMode` SETTER transition to true, call
  `dropCoordinator()` so any in-flight coordinator is torn down (R7). Do NOT clear
  the cached pairing code (invariant 5).
  Then set `relayTicker.hubMode = true` in BOTH `GrAppShell.swift` and
  `Views/AppShell.swift` right after `relayTicker.connect(mosaic:)` within the
  `.http` backend branch (mirror how `liveSync.connect(serverURL:)` is gated).
  Commit: `fix(ios): gate relay coordinator while WS hub path is active`.

### B — WS receive cap (Swift)
- **B-a** `app/Tesela-iOS/Sources/Sync/SyncState.swift` `openSocket()`: after
  `webSocketTask(with:)`, before `resume()`, set
  `task.maximumMessageSize = 64 * 1024 * 1024`. Commit:
  `fix(ios): raise WS max message size to 64 MiB`.

### D — shared-base bootstrap (Rust + FFI regen + Swift)
- **D-a** `crates/tesela-server/src/routes/notes.rs`: add `get_loro_snapshot`
  handler — mirror `get_loro_index`; derive note_id via `stable_uuid_from_slug`,
  return `export_doc_update(note_id, None)` bytes as `application/octet-stream`,
  404 when `None`. Register `.route("/loro/notes/{id}/snapshot", get(...))` in
  `routes/mod.rs`. Commit: `feat(server): GET /loro/notes/{id}/snapshot`.
- **D-b** `crates/tesela-sync-ffi/src/lib.rs`: add
  `import_note_snapshot(&self, slug: String, bytes: Vec<u8>) -> Result<(), FfiSyncError>`
  on `SyncEngineHandle` (compute note_id, `inner.import_doc_update`). Mirror the
  existing `apply_delta_frame`/`produce_note_delta` method shape. **Regen Swift
  bindings** (mirror the Phase-B regen invocation: `cargo run -p tesela-sync-ffi
  --bin uniffi-bindgen --features cli -- generate --library
  target/debug/libtesela_sync_ffi.dylib --language swift` into
  `app/Tesela-iOS/Generated/` + `CFFI/`) and confirm `xcodebuild` sees the new
  Swift method. Commit: `feat(ffi): import_note_snapshot for base bootstrap`.
- **D-c** `RelayTicker.swift`: add `bootstrapNoteIfNeeded(slug:) async` — if
  `engine.noteVersion(slug:) != nil` return; else `GET <serverURL>/loro/notes/<slug>/snapshot`
  (mirror the existing HTTP helper / `mosaic` serverURL access); on 200 call
  `engine.importNoteSnapshot(slug:bytes:)`; best-effort (swallow network/non-200).
  Call it at the top of `recordAndPush` after `openEngineIfNeeded()` and BEFORE
  `recordNoteDiff`. Depends on D-b regen. Commit:
  `feat(ios): bootstrap note from server snapshot before first edit`.
- **D-d** `crates/tesela-server/tests/`: end-to-end test — server seeds note,
  bytes from `get_loro_snapshot`-equivalent path, device imports, device edits,
  device snapshot → server import → render has no twins + correct text. Run
  `cargo test -p tesela-server`. Commit: `test(server): snapshot bootstrap prevents twins e2e`.

## FFI / build notes
- Only **D-b** changes the FFI → regen + Xcode rebuild required there.
- Any iOS wiring change (E2-a) must be applied to **both** shells (`GrAppShell.swift`
  AND `Views/AppShell.swift`).
- `xcodebuild` is authoritative over SourceKit (phantom cross-file errors).

## Risks
- **R1 (sequence):** E-only leaves a transient "edit silently dropped" window
  (min-TreeID may keep a stale twin). Land E **and** D before the live device test;
  don't ship E alone.
- **R6:** verify `TreeID` field/getter accessibility + per-text-update recency
  availability in loro 1.12 BEFORE writing E1-a; fall back to min-TreeID.
- **R7:** set-`hubMode` must `dropCoordinator()` or an in-flight tick still fires.
- **R3/R4:** bootstrap race / mid-edit snapshot are CRDT-safe (idempotent import,
  commutative merge) — no data loss; note in code.

## Acceptance (in order)
1. `cargo test -p tesela-sync --test disjoint_history_revert` — T-heal + T-converge green.
2. `cargo test -p tesela-sync` and `cargo test -p tesela-server` — all green.
3. FFI regen + `xcodebuild` build succeeds on the Roshar destination.
4. Live device round-trip (USER step): edit on Mac web `/g` → appears on Roshar
   <1s; edit on Roshar → appears on web <1s; concurrent same-note edits on
   web+iPad+iPhone converge, **no duplicated bullets, no revert on refresh**.

## Follow-up (task #147, deferred)
iOS `produceDeltaFrame` → real `sinceVv` (delta not snapshot) now that the base is
shared; relay-path re-export uses snapshot not exact bytes; latency measurement.
