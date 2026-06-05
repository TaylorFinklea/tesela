# Property-container multi-device convergence — design (2026-06-05c)

Resolves the Loro container-overwrite hazard (decisions.md 2026-06-05(b)) before implementing the Phase-1 safety cluster. Code-grounded (5-lens workflow, all file:line verified against `crates/tesela-sync/src/engine/loro_engine.rs` + `crates/tesela-core/src/note_tree.rs`). **This supersedes the original P1.6→P1.9 ordering in the spec addendum.**

## The hazard (recap)
Loro derives a child container's id from the op that CREATES it. Two peers that each FIRST-create a nested container at the same map key concurrently mint RIVAL ids → on merge one OVERWRITES the other (LWW at the map register, not union). Affects the per-block `props` LoroMap + `prop_keys` list, and per-key nested `text` LoroText / multi `LoroList`. Union holds only once the container is in SHARED history before peers diverge. The proven analog is `text_seq` (a per-block nested LoroText) which converges via the same get-or-create discipline + shared-base bootstrap + twin heal.

## Resolved design

### 1. Eager container seeding (the fix for the common path)
Eager-seed `props` + `prop_keys` at the TWO block-node creation sites that already seed `text_seq` (`write_block_text`, which does `meta.get_or_create_container("text_seq", …)`):
- `seed_tree_from_flatblocks` — after `write_block_text` in the `for block` loop (~`loro_engine.rs:2290`).
- the `BlockUpsert` apply arm — after `write_block_text(&meta, text)` (~`:3188`).
Add `node_prop_containers(&meta)?` (get-or-create, discard handles — idempotent). Because the node meta map reaches shared history via the shared-base bootstrap before peers diverge, concurrent get-or-create resolve to the SAME child id.
- **Fixes completely:** all scalars (primitive LoroValue in the shared map = LWW register, no sub-containers) + first-property-of-any-type with DISTINCT keys.
- **Page-root props need NO change** — `page_prop_containers` returns root `doc.get_map("props")`/`get_list("prop_keys")` (content-addressed ids, rival-free). Only their per-key children inherit the hazard.
- **Snapshot-cost gate:** measure the snapshot delta on the 1.3 MB `ai-business` note (the relay-413 note, `project_relay_413_blocks_sync`). Empty Map+List per block ≈ near-zero; if material, fall back to lazy-mint-on-shared-base. Measurable guard, not a blocker.

### 2. Per-key list/text first-touch — v1 = documented boundary
Eager-seeding the map does NOT fix same-key nested-child first-touch. Loro exposes NO API to mint a nested container at a caller-chosen id (same constraint that forbade caller-chosen TreeID) → deterministic-child-id is INFEASIBLE. The "active rival-reconcile" (recover the LWW-loser via `doc.get_container(loser_id)`) is REJECTED for v1: that reachability-after-overwrite + snapshot-GC-survival is UNVERIFIED — do not build on it.
- **v1:** eager-seed (map shared) + shared-base bootstrap (key's first op propagates before concurrent adds) for the common path; the disjoint-twin heal (§4) unions list props for the disjoint case.
- **Documented limitation:** concurrent first-declare of the SAME multi-value key on two never-synced devices loses the loser's adds until next sync, where the heal restores them (eventual, not first-write-immediate). Same free-TEXT key concurrent first-declare = pick-winner-by-HLC (no text-union primitive for a register collision); acceptable — free-text isn't the union target. Do NOT JSON-blob multi-values to dodge this (defeats the tag-merge fix).

### 3. Migrate-on-apply (P1.6) — deterministic-shape, flag default-OFF
NOT authoritative-single-writer (the relay is a dumb opaque-delta pipe, no version negotiation; the Mac-server isn't always-on → no enforcement point, would strand offline edits). Deterministic-shape is enforceable locally with zero coordination. In the `BlockUpsert` apply arm, behind a default-OFF flag (mirror `TESELA_LORO_RESEED`, resolved once at engine construction):
1. Per-line parse incoming `text` with `PROPERTY_RE`. **Conservative:** strip a line ONLY if it is SOLELY `key:: value` after indent-trim (a false-positive mid-prose strip is irreversible text loss). Lowercase keys; route `tags::` → `AddToList`, not scalar.
2. `write_block_text(&meta, prose_only)` (stripped).
3. Fold via `node_prop_containers` (the eager-seeded shared map) + `apply_prop_op` per stripped line.
4. One `doc.commit()`; idempotent (re-apply finds prose already clean → no-op).
Deterministic shape: same incoming text + same classification → same `prop_keys` order (= text order) on every device → concurrent migrators converge; residue = the per-key child fork (§2/§4).

### 4. NoteUpsert non-authoritative (P1.8) + twin heal carries props (P1.9)
**P1.8 — prop ops are the SOLE writers of `props`:**
- Extend `tree_matches_blocks` (~`:2237`) to also compare materialized props per block AND compare STRIPPED prose on both sides (so a migrated prose-only tree isn't seen as "drifted" vs an old peer's in-text-property body and destructively reseeded). id+text+indent match but props differ → do NOT reseed.
- When reseed is genuinely unavoidable: snapshot each surviving block_id's materialized props before `clear_block_tree`; replay via `apply_prop_op` after `seed_tree_from_flatblocks`. **Reseed stays SERVER-ONLY** (gate on `materialize_dir.is_some()`/authoritative-writer) — a device reseed re-mints rival ids. Regression-test this.
- `set_page_properties` (~`:2071`): gate it to touch ONLY the legacy `page_props` list, never root `props`. Reconcile into root props first, then clear legacy (P1.6/P1.11).

**P1.9 — grow `PeerBlockChange` + reconcile by union.** Current: `struct PeerBlockChange { block_id, text, indent }` (~`:2588`). Grow with `props: Vec<(String, ResolvedValue)>` where `ResolvedValue ∈ { Scalar(PropScalar), Text(String), List(Vec<PropScalar>) }`.
- In `peer_genuine_block_changes` (~`:2683`), read EVERY twin node's props (`read_node_prop_containers`) in the fork BEFORE `tombstone_duplicate_twins` runs (mirror `twin_texts` capture). Merge per key: list → union all twins' members deduped (first-occurrence-stable, matches `prop_get_list_dedup`); scalar → union-all-distinct-keys then `prop_keys_resolved`-dedup (same-key value collision = LWW, no recency analog); text → the genuine-edit twin's value (discriminated like text via `server_block_text_history`).
- Re-assert: after the existing text re-assert, emit ONE `OpPayload::BlockPropertySet { note_id, block_id, key, value }` per resolved key onto the survivor (per-key, NOT a props-carrying BlockUpsert — the per-key route re-asserts lists by UNION via `AddToList`; a BlockUpsert risks whole-map LWW). Idempotency-guard each (current-vs-target diff). The heal must AddToList the loser's MISSING values, never replace the winner's list wholesale.
- **One shared helper** `reconcile_orphaned_prop_containers(doc, owner)` serves both the twin heal and the union re-assert — do not build two divergent recovery paths.

### 5. Pruner (P1.7) + empty-seeded-map caveat
One-line fix (~`note_tree.rs:236`): `let bare = block.text.trim().is_empty() && block.properties.is_empty();` (matches iOS `droppingBareLeafBlocks` which already checks `properties.isEmpty`). Latent-correctness fix — `prune_bare_leaf_blocks` has ZERO live callers today (Phase 2.2 removed auto-prune); land it before any future re-wiring.
- **Empty-seeded-map caveat:** eager-seeding mints an EMPTY `props` map per block; it must NOT make a blank bullet non-bare. It doesn't, because `FlatBlock.properties` is built by `materialize_props` (only emits keys resolved via `prop_keys_resolved`) → empty map → empty Vec → still bare. Safe ONLY as long as `FlatBlock.properties` reflects MATERIALIZED props, never raw container existence. Guard with a test.

## Build order (TDD, dependency-driven — REPLACES the numeric order)

**P1.9b — eager-seed (FOUNDATION, first).** Gates P1.6/P1.8(b)/P1.9.
- `concurrent_first_property_set_on_shared_block_both_survive`: two engines share a base with a propsless block; A first-sets scalar `status`, B first-sets scalar `priority` (distinct keys) concurrently; exchange → BOTH keys on both replicas (fails without seed).
- `concurrent_same_key_scalar_set_is_deterministic_lww`: same-key concurrent scalar → identical winner on both.
- Acceptance gate: snapshot-delta on the 1.3 MB note.

**P1.9 — twin heal carries props (before P1.6).** Depends P1.9b; gates P1.6.
- `disjoint_twins_each_with_distinct_property_both_survive`.
- `disjoint_twins_each_add_to_same_list_key_union` → survivor list `[x,y]` deduped (union, not LWW-replace).

**P1.7 — pruner (one line; anytime after the materializer shape).**
- `prune_keeps_property_only_block`; `prune_drops_block_with_empty_props` (the empty-seeded guard).

**P1.8 — NoteUpsert non-authoritative.** Depends P1.9b.
- `note_upsert_does_not_clobber_concurrent_block_property`; `note_upsert_drift_reseed_preserves_props`.

**P1.6 — migrate-on-apply (LAST; flag default-OFF).** Depends HARD on P1.9b + P1.9 + P1.8.
- `migrate_on_apply_lifts_intext_prop_and_is_idempotent`; `mixed_fleet_old_peer_reinjects_no_double_emit`; `concurrent_migrate_same_block_converges`.

**Determinism gate (across all):** extend `render_is_byte_identical_regardless_of_prop_op_order` with a migrated-vs-unmigrated byte-equality assertion.

## Residual limitations / rollout
- Per-key same-new-key concurrent first-declare on two never-synced devices = heal-repaired (eventual), not first-write-convergent. Documented boundary, matches text_seq.
- `doc.get_container(loser_id)` reachability UNVERIFIED → v1 does NOT depend on it (heal reads twin props in the fork before tombstone).
- Snapshot growth from two empty containers/block → measure on the 1.3 MB note; lazy fallback exists.
- Scalar twin same-key conflict = LWW (no recency analog). **Product decision: silent LWW-by-HLC for v1** (a conflict-surface affordance is a later polish, no rework to add).
- Reseed stays SERVER-ONLY (device reseed re-mints rivals) — gate + regression test.
- PROPERTY_RE false-positive strips are irreversible — strip only solely-property lines; identical classification across devices is mandatory (divergent classification → divergent prop_keys → non-converging materialization).
- **Live-fleet rollout:** flag default-OFF until the ENTIRE fleet (incl. iOS old FFI) is props-read-capable (old build imports migrated containers, can't read them, renders property-less, could re-broadcast → fleet-wide erase). One-way fleet-coordination gate, not per-device. **Dual-read forever** + always emit `key:: value` in the rendered view (note_tree already does, from `FlatBlock.properties`) so old readers still SEE the property as text. Dedup by key, container over text.
