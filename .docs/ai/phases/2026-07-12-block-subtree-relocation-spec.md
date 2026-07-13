# Block subtree relocation spec

**Bead:** `tesela-b54` · **Decision date:** 2026-07-12 · **Owner tier:** Senior

## Goal

Let a user move a block together with its complete descendant subtree before,
inside, or after another block in Graphite Dailies. The same interaction works
within one day and across days, preserves durable block identity and typed
properties, survives interruption, and converges through the existing Loro
relay paths.

Architecture rationale and rejected alternatives live once in
`decisions.md` under “2026-07-12 — Cross-note subtree relocation is a
recoverable engine operation.”

## Product contract

- Scope: web/Tauri Graphite `JournalView` + `BlockOutliner`. Native iOS is
  unchanged.
- Dragging a root always moves its contiguous complete subtree.
- Same-day and cross-day moves expose the same placement semantics.
- A move is server-confirmed before the browser removes the source subtree.
- No relocation path uses whole-note PUT or client-composed copy/delete.
- Stable `bid`s survive. Line-derived web block ids may change with note/line.
- Text, internal order, relative indentation, and typed property values survive.
- The command registry exposes the same operation without requiring a mouse.

## Placement semantics

The destination is one of four placements:

- **Before target:** insert immediately before the target. The moved root takes
  the target's indent and parent; descendants keep their relative depth.
- **Inside target:** append as the target's last child, after its current
  descendant subtree. The moved root takes `target indent + 1` and the target
  becomes its parent.
- **After target:** insert after the target's complete descendant subtree. The
  moved root takes the target's indent and parent.
- **Append to note:** insert after the final live block as a top-level subtree.
  Date-header and empty-day drops use this placement.

For same-note moves, compute the destination after conceptually removing the
source subtree. A target equal to the root or inside its descendants is invalid.
A placement that yields the existing order is an idempotent no-op.

## Command contract

One cross-note route accepts an exact request shape:

```json
{
  "move_id": "uuid",
  "source_note_id": "2026-07-12",
  "root_bid": "uuid",
  "destination_note_id": "2026-07-11",
  "target_bid": "uuid-or-null",
  "placement": "before|inside|after|append"
}
```

Rules:

- `target_bid` is required for `before`, `inside`, and `after`; it is null for
  `append`.
- Source root and destination target are addressed by stable `bid`, never the
  web line-derived id.
- The server derives the complete source subtree and semantic property payload;
  the client never submits child content as authority.
- A missing destination is auto-created only when its id is an ISO daily date.
  Only `append` is valid for an absent daily. The trusted daily seed is created
  inside the destination-durable phase, not by calling the ordinary create
  route during preflight; a rejected drop therefore leaves no empty daily.
  Missing non-daily destinations fail without changing the source.
- A retry with the same `move_id` and byte-equivalent request returns the same
  success. Reusing a `move_id` with different arguments is a conflict.
- Success returns refreshed source and destination notes (one note for a
  same-note move) so caches can settle without guessing.

## Engine relocation

Add relocation as a typed `SyncEngine` capability rather than synthesizing
existing `OpPayload::BlockMove`/`BlockDelete` calls. Follow the engine's existing
per-note apply-lock, checked snapshot, materialization, and atomic temp-rename
patterns.

### Preparation

1. Resolve source/destination slugs to stable note ids and bootstrap both docs.
2. Acquire their apply locks in lexicographic note-id order; acquire once when
   source equals destination.
3. Re-read under the locks. Validate root, target, placement, and ownership.
4. Snapshot the authoritative subtree: ordered bids, text containers, indent,
   parent relationships, and resolved typed property values. Preserve each
   scalar's canonical type plus every list member in resolved order; do not
   flatten non-text values into materialized strings.
5. Compute the final placement from the locked authoritative trees.
6. Atomically persist a relocation intent before changing a doc.

### Durable intent

The intent lives beside the engine's durable state and contains:

- request fields and stable source/destination note ids;
- the complete semantic subtree snapshot needed to reconstruct the destination;
- the computed destination ancestry/order;
- phase: `prepared`, `destination_durable`, or `source_durable`.

Write and phase transitions use the existing unique-temp + rename discipline.
The destination root also records the `move_id` and request hash as
non-materialized relocation metadata so recovery can identify its proof-bearing
subtree independent of absolute row position. Completion replaces the full
subtree-bearing intent with a small receipt keyed by `move_id` and request hash;
retain the newest 4,096 full receipts so ordinary network retries return the
stored outcome. Separately retain a permanent compact tombstone of only
`move_id → request_hash` (about 48 bytes per completed request). A matching
retry whose full receipt was pruned fails closed as stale without mutation; a
mismatched reuse remains a conflict forever. This exact tombstone ledger is the
durability trade-off approved on 2026-07-12: replay safety is more important
than strictly bounded idempotence metadata.

An active intent reserves its source root. A different `move_id` cannot prepare
an overlapping move until the first intent recovers or completes; this prevents
two recovery snapshots from later authoring duplicate destination owners.

### Apply order

For a cross-note move:

1. Create the destination nodes in final flat render order using the same bids.
   Re-author resolved typed property values into fresh destination containers;
   cross-doc CRDT container history itself is not portable.
2. Commit, save the destination snapshot with a checked result, materialize it,
   and advance the intent to `destination_durable`.
3. Delete exactly the captured source nodes by source note/doc identity. Do not
   resolve these deletes through the global `block_index`.
4. Commit, save the source snapshot with a checked result, materialize it, and
   advance the intent to `source_durable`.
5. Make the destination the final global block-index owner, refresh the shared
   derived index, persist the permanent move-id/request-hash tombstone, replace
   the intent with its compact completion receipt, then prune full receipts
   beyond the newest 4,096. Tombstones are never pruned.

The source snapshot must never become durable without a durable destination
snapshot containing the entire subtree. A same-note relocation uses the same
intent boundary, reorders/reparents nodes inside one doc, saves one snapshot,
then completes.

### Recovery

Engine bring-up scans relocation intents before accepting writes:

- `prepared`: ensure destination nodes/values exist and make destination
  durable, then continue.
- `destination_durable`: ensure the captured source nodes are deleted and make
  source durable, then continue.
- `source_durable`: repair materialization/index state if needed, then finish.

Completed receipts need no recovery; they answer idempotent retries.

Every step is state-inspecting and idempotent. A crash or surfaced error may
temporarily leave a duplicate, but never leave the subtree absent from both
durable snapshots.

## Sync and concurrency

- Relocation produces ordinary Loro changes in each affected note doc. The
  relay and live WebSocket export one delta per changed note through the
  existing cursor-free path.
- Peers must converge when destination and source deltas arrive in either
  order. Temporary duplicate visibility is allowed between the two arrivals;
  the final state has one live destination subtree.
- Both local note locks stay held through the durable operation, so local edits
  and inbound apply for either addressed note serialize around the move.
- An edit racing on another device at the old source location is a delete-vs-
  edit conflict and follows existing deleted-wins behavior. It must not
  resurrect a source copy.
- Two devices concurrently relocating the same root to different destination
  notes are outside automatic resolution for this slice. Duplicate live bid
  ownership must be detected, logged/surfaced, and excluded from silent
  order-dependent ownership. The block ownership index therefore represents
  either one owning note or an ambiguous set; registration/rebuild must not
  overwrite one live owner with another. Bid-addressed mutation and relocation
  fail closed while ownership is ambiguous, and heal back to one owner after a
  duplicate is removed. Automatic winner/merge policy requires a separate
  architectural decision.

## Server post-write behavior

After the engine reports durable success, perform the normal write tail for
every affected note: re-read materialized notes, refresh search/link indexes,
record versions, ensure tag pages, emit note-updated events, and export live
cursor-free deltas captured from each note's pre-move version.

If any precondition or destination preparation fails, return an error with the
source unchanged. If failure occurs after intent persistence, keep the intent;
the response explains that recovery/retry is required. Never ask the client to
roll back by writing note bodies.

## Web interaction

### Pointer drag

- Add a dedicated handle beside the block bullet. Do not overload the bullet's
  drill-in/context-menu behavior or CodeMirror's text/image drop surface.
- Use a Tesela-specific drag MIME payload containing only source note id, root
  bid, and move id. Treat browser drag data as a locator, not subtree content.
- Highlight the root plus visible descendants during drag. The drag preview
  shows root text and moved-block count.
- Each row has top/bottom edge zones for before/after and a center zone for
  inside. Render a line for edge placement and a contained highlight for
  inside placement.
- Date headers and empty-day bodies accept append. Hovering an unmounted day
  may mount it for targeting; creating its note waits for confirmed drop.
- Auto-scroll the journal near viewport edges. Clear all drag state on drop,
  drag-end, or Escape.
- Self/descendant targets and malformed/external drag payloads are inert.

### Keyboard parity

- Register a named `Move block subtree` editor command in the canonical command
  registry and expose it through the palette plus an available leader/block
  chord verified against the current manifest.
- Starting it from the focused block stores the source locator and enters a
  visible temporary move mode without mutating data.
- `j`/`k` traverse valid block targets using the journal's existing cross-day
  focus behavior; `b`, `i`, and `a` commit before, inside, and after. A date
  header/empty-day target commits append. Escape cancels with no request.
- Existing same-day Alt-arrow movement remains available and is regression
  tested; the new mode is the full cross-day equivalent.

### Cache and error behavior

- Do not optimistically delete or clone blocks. Show a pending state on the
  dragged subtree and target while the request runs.
- On success, seed returned note data and invalidate both note/list journal
  queries. Restore focus to the moved root at its destination.
- On precondition failure, clear pending state, retain source focus, and toast
  the server error. On recoverable post-intent failure, tell the user retry is
  safe and retain the original request/move id for that retry; the next refresh
  reflects recovery.

## Safety invariants

1. A move addresses one stable root bid and includes every live descendant.
2. Destination durability precedes source deletion durability.
3. Final live blocks retain bids, text, typed property values, internal order,
   and relative depth.
4. The browser never authors subtree content or composes copy/delete requests.
5. Same request id + same arguments is idempotent; changed arguments conflict.
6. Source/destination apply locks are acquired in deterministic order.
7. Final block-index ownership points at the destination and never depends on
   source/destination delta arrival order.
8. No valid failure state loses the subtree from both durable snapshots.
9. Two live notes claiming one bid become an explicit ambiguous-owner state;
   normal block mutation never silently selects one by iteration/arrival order.

## Verification matrix

### Pure web tests

- Extract a root plus nested descendants.
- Before/inside/after placement and indent rebasing.
- Same-note index adjustment after source removal.
- Self/descendant/no-op rejection.
- Cross-note append and source/destination note-id projection.
- Drop-zone classification and custom payload validation.

Start red in `web/tests/unit/block-tree-move.test.mjs`; targeted command:
`node --test web/tests/unit/block-tree-move.test.mjs`.

### Engine tests

- Same-note before/inside/after reorders persist through cold reload.
- Cross-note move preserves bids, text, nested order, and scalar/list typed
  properties.
- Destination snapshot is durable before any source snapshot without blocks.
- Recovery from every intent phase; repeated recovery is a no-op.
- Same move retry succeeds; mismatched reuse conflicts.
- Source/destination deltas applied in either order converge to one final copy.
- Concurrent source edit follows delete-wins without resurrection.
- Duplicate live cross-note bid ownership is detected and surfaced.

### Server and browser tests

- Route validation, daily auto-create, refreshed two-note response, indexing,
  versions, events, and live deltas.
- Drag before/inside/after within one day and across two days.
- Date-header/empty synthetic-day append.
- Invalid targets, server failure, and Escape cancellation.
- Keyboard move mode across a day boundary and focus restoration.
- Rendered Graphite check: meaningful page, no framework overlay, healthy
  console, screenshot evidence, and interaction proof in web/Tauri view.

Final gates:

```bash
cargo fmt --all
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
pnpm --dir web check
pnpm --dir web test:unit
```

## Out of scope

- Native iOS drag/move UI.
- Moving blocks to arbitrary pages outside the Graphite Dailies interaction.
  The engine command remains note-generic for future callers.
- Copy semantics; this operation is move-only.
- Automatic resolution of two concurrent relocations of the same root to two
  different destination notes.
- Preserving cross-doc CRDT container operation history; semantic typed values
  are preserved and begin new target containers.
