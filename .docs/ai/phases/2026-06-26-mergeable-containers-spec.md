# Spec: REAL convergence fix — shared-base / rebase triggering (NOT mergeable containers)

Status: investigated + redirected 2026-06-26. **The original "mergeable child
containers" plan in this file was WRONG** — a 5-agent verification workflow
(`investigate-block-fork`, adversarial verdict `refuted:false`) proved it can't
fix the observed drift. This spec now records the ACTUAL fix.

## Why mergeable containers do NOT fix it

Blocks are **LoroTree NODES** (`doc.get_tree("blocks")`), TreeID = peer+counter,
minted locally. Two devices authoring the same `block_id` independently mint
**different TreeIDs** → twin nodes → each twin has its OWN meta map → its own
`text_seq`/`props`. Mergeable child-container IDs are `hash(parent_map_id, key,
type)`; different parent meta maps ⇒ different child IDs ⇒ they STILL diverge.
Mergeable only converges children when the parent node is ALREADY shared. So the
fork is at the **tree-node** level; mergeable containers can't reach across it.
(Verified: `loro_engine.rs:3122-3216`, loro 1.13.6 `lib.rs:2007-2024`.)

## The real root + fix

**Root:** a device authors a block WITHOUT first sharing the peer's Loro base →
mints a rival TreeID → twin. With a shared base, `BlockUpsert` resolves to the
EXISTING node via `find_node_by_block_id` (`loro_engine.rs:3036-3037`) → same
TreeID → no fork → concurrent edits merge in ONE `text_seq` (LoroText interleave;
sequential edits build on each other — no loss).

**The heal machinery ALREADY EXISTS and is proven:**
`import_authoritative_snapshot` + `rebase_twins_onto_snapshot`
(`loro_engine.rs:707-746, 3317-3362`) tombstone the device's rival nodes and
rebase its genuine edits onto the authoritative lineage. Test
`disjoint_device_authoritative_rebase_then_converges`
(`tests/disjoint_device_catchup.rs`) proves it converges WITHOUT loss. **The bug
is the TRIGGERING, not the mechanism.**

## The triggering gaps (what to fix)

1. **Relay-mode has no shared-base path.** `bootstrapNoteIfNeeded`
   (`RelayTicker.swift:319-373`) gets the base via `fetchLoroSnapshot` = HTTP
   `GET /loro/notes/{id}/snapshot`. The CF relay is a mailbox with NO such
   endpoint → 404 → silent return → no rebase. In `.relay` mode the device never
   gets an authoritative base over HTTP.
2. **The relay-inbound apply does LOSSY dedup, not rebase.** `apply_relay_updates
   → apply_doc_update_status` raw-imports + `tombstone_duplicate_twins`
   (min-TreeID dedup, NOT recency-aware → drops a genuine edit). It does NOT call
   `rebase_twins_onto_snapshot`. So twins arriving via the relay converge lossily
   (or not at all pre-1.13.6, where the merge crashed).
3. **Resident-debounce** (`RelayTicker.swift:323-332`): resident-but-divergent
   notes wait out `catchupMinInterval` before catch-up.
4. **Offline first-edit** (`lib.rs:516-539`, `prev_content.is_empty()`): authors
   without a base when the peer's doc hasn't arrived.

## Fix design (deterministic, no central authority)

Make the **relay-inbound apply rebase divergent twins onto a DETERMINISTIC
winner** (min-TreeID lineage — same rule the dedup already uses, so no
ping-pong), re-applying the loser twin's genuine edits onto the winner via the
existing `rebase_twins_onto_snapshot` + `peer_genuine_block_changes` logic.
This converges the RELAY path the same way the HTTP bootstrap converges the
authoritative path — without HTTP, backend-agnostic, and it heals EXISTING
forked docs as their snapshots flow through the relay. Then forks self-heal on
the next sync round regardless of how they formed.

Secondary (defense in depth): a `.relay`-mode shared-base fetch (pull the peer's
latest snapshot from the relay before the first author of a note), and lifting
the resident-debounce when divergence is detected.

## What 1.13.6 already bought us

The loro upgrade (landed `e884edc2`) makes the twin merge APPLY instead of
crashing, so the EXISTING lossy dedup now converges forked notes (one twin's
value wins). That heals Taylor's current drift once both devices run 1.13.6 —
**verify that first**. This spec's rebase-on-relay-inbound is the NO-DATA-LOSS +
no-future-forks upgrade on top of it.

## Acceptance

- Two engines, disjoint twins with DIFFERENT genuine edits, merge via the RELAY
  path → converge to ONE lineage with the genuine edit preserved (extend
  `disjoint_splice_convergence.rs` / mirror `disjoint_device_catchup.rs`).
- No regression: full tesela-sync + tesela-sync-ffi suites green.
- Live: sim + desktop, concurrent same-block edits → converge, no twin, no loss.

## Verify

`cargo test -p tesela-sync -p tesela-sync-ffi` + a new relay-inbound rebase test
(RED before the fix: lossy/twin; GREEN after: converged, genuine edit kept).

## Risk

HIGH — convergence-critical apply path (the project's recurring clobber-bug
surface). TDD, one change, full re-test. Do NOT rush; a botched change
re-introduces data-loss/clobber. Fresh, careful pass recommended.
