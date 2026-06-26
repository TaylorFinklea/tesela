# Spec: mergeable child containers (disjoint-lineage ROOT fix)

Status: NOT STARTED — fresh-session work. Depends on the loro 1.13.6 upgrade
(landed `e884edc2`). This is layer 2 of the 2026-06-26 convergence fix; layer 1
(the upgrade) stops the crash + heals already-forked twins via dedup. This
layer stops blocks from FORKING in the first place (zero data loss on
simultaneous same-block edits).

## Problem

Two devices that author the same block (same `block_id`) on disjoint Loro
histories mint independent child containers for that block's TEXT (`text_seq`
LoroText) and PROPS (`props` map, per-prop text/list). These are **regular
op-id children** (`LoroMap::get_or_create_container`), which **FORK** across
peers on concurrent first-write — the `project_multidevice_convergence`
disjoint-twin root cause. Forked text containers are what Loro 1.12 panicked
merging (the desktop crash-loop) and what leaves devices drifted (desktop
"Brook" vs iOS "Bro" for one block).

The existing dedup/heal (`peer_genuine_block_changes`, `tombstone_duplicate_twins`)
collapses forked twins to ONE survivor — convergent but LOSSY (the other
twin's concurrent edit is dropped).

## Fix

loro 1.13 adds **mergeable child containers**: child containers under a map key
that **converge across peers on concurrent first-write instead of forking**.
Create block containers with `ensure_mergeable_*` instead of
`get_or_create_container` → two devices authoring the same block share ONE
logical text/props container → concurrent edits MERGE (LoroText interleave / map
LWW), no twins, no dedup loss, no crash.

## Sites (crates/tesela-sync/src/engine/loro_engine.rs)

| line | container | swap to |
|---|---|---|
| 994 (`splice_block_text`) | `text_seq` LoroText | `meta.ensure_mergeable_text("text_seq")` |
| 2124 (`write_block_text`) | `text_seq` LoroText | `meta.ensure_mergeable_text("text_seq")` |
| 2193 (`node_prop_containers`) | `props` LoroMap | `meta.ensure_mergeable_map("props")` |
| 2196 (`node_prop_containers`) | `prop_keys` LoroList | `meta.ensure_mergeable_list("prop_keys")` |
| 2349 (prop text set) | per-prop LoroText | `props.ensure_mergeable_text(key)` |
| 2396 (prop list set) | per-prop LoroList | `props.ensure_mergeable_list(key)` |
| 2688 | TEST only (tags list) | leave or update with prod |

## The hard part: MIGRATION (design this first)

`ensure_mergeable_*` returns **`LoroError::ArgErr` if the key already holds a
NON-mergeable value, leaving it unchanged** (verified in loro 1.13.6 source,
`crates/loro/src/lib.rs` ~2229). So on EVERY existing fleet doc (all blocks have
regular `text_seq`/`props` already), a naive swap just errors → no-op → no fix
for existing blocks. And delete+recreate-as-mergeable, done INDEPENDENTLY per
device, re-forks (each device's recreated mergeable container is itself a new
op-id → disjoint again).

Migration strategy options (pick + prototype):
1. **New-blocks-only (gradual).** New blocks born mergeable; existing blocks
   keep regular containers (still fork until rewritten). Lowest risk, partial.
   Needs the swap to FALL BACK to the regular container on `ArgErr` (read path
   must handle both kinds). Probably the v1.
2. **Authoritative one-shot rebuild.** Bump a doc-model version; ONE
   authoritative node (the desktop/relay) rebuilds each note doc with mergeable
   containers from its current materialized state, broadcasts the new doc as the
   canonical base, every device adopts it (discarding local regular-container
   lineages). Converges but is a hard cutover (a device with unsynced local
   edits in a regular container loses them unless drained first). Mirrors the
   original Loro cutover bootstrap.
3. **Relay-coordinated migration.** Migration op flows through the relay so all
   devices converge on the same mergeable container identity.

## Acceptance

- Two engines authoring the SAME block on disjoint histories + concurrent text
  splices → merge → BOTH edits survive (interleave), no twins, byte-identical
  across devices. (Extend `disjoint_splice_convergence.rs` to assert BOTH texts
  present, not just convergence-to-one.)
- Existing regular-container docs still read/merge correctly (no data loss for
  the fleet) under the chosen migration path.
- Full tesela-sync suite green; multi-device live test (sim + desktop) shows
  simultaneous same-block edits both survive.

## Verify

`cargo test -p tesela-sync` + a new disjoint-concurrent-splice test asserting
both inserts survive + a migration test (regular-container doc → mergeable).

## Risk

HIGH — touches the core block doc model + the whole fleet's on-disk docs.
Do NOT rush. Prototype the migration on a copy of a real mosaic before shipping.
