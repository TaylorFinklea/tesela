# Bid-native Logseq block references

**Bead:** `tesela-8zd.7` · **Tier:** Lead · **Status:** revised implementation spec
**Dependency:** `tesela-ewj.1` must land first. Its engine-owned importer/hydration
path is the only permitted apply path.

## Review disposition

This revision adopts every blocker and major finding in the 2026-07-10 Sol
adversarial review. There are no contested findings.

- Resolution has exactly three visible states: `invalid`, `resolved`, and
  `unavailable` (valid UUID without a live local target).
- Graphite reuses and extends the merged `content-jump.svelte.ts` store instead
  of inventing a second pending-block mechanism.
- Import diagnostics/replacement bids are serialized in the plan; duplicate
  and malformed handling is replay-stable.
- The target cache records target-note ownership and invalidates on target
  note changes/deletes. The convergence test exercises the real Logseq
  plan → ewj.1 hydration → relay sync path.

## Verified baseline

`tesela-core::note_tree` already preserves a valid pre-stamp comment as a
block's UUID: `BID_PREFIX`/`BID_SUFFIX` are the persisted syntax and
`parse_note` adopts the UUID before it would mint a new one. The Loro engine's
`block_index` maps bid to note id, and `find_doc_for_block` uses the map then
lazy-loads the note document. This is the only block-id map this work may use.

The importer currently discards `id::` along with Logseq presentation metadata
in `import_logseq.rs`; it preserves `((uuid))` prose. The existing import
planner is a serde `ImportPlan`/`PlanItem` returned to the settings preview and
then posted back for apply, so it is the correct place for deterministic
anchor diagnostics.

The Graphite shell already has a one-shot jump pipeline:
`requestContentJump` stores a pending request, `GrPage` passes it to
`BlockOutliner`, and the outliner focuses, scrolls, and clears the request.
Today its target is query/snippet search only. The `/g` branch of `gotoNote`
opens a buffer but drops its optional `targetBlockId`, which is why a new
parallel navigation contract would be wrong.

`ParsedBlock` exposes both a legacy line id (`{note_id}:{line}`) and the
canonical `bid`. Copying a block reference must use the latter.

## Decision

### 1. Preserve Logseq UUIDs as Tesela bids

A valid Logseq `id:: <hyphenated UUID>` becomes the existing persisted
comment on its owning Tesela bullet:

```text
- target text <!-- bid:675f6317-... -->
```

No `logseq-id → bid` map is created. The source UUID is the resulting bid,
survives Markdown materialization and Loro sync, and resolves through the
existing global `block_index`.

Property ownership is determined by the importer's block/continuation parser,
not line adjacency. While scanning converted Logseq source, maintain the open
bullet indentation stack. An `id::` line belongs to the currently open block
whose continuation/property region contains that line; a nested bullet closes
its parent's current continuation region. A detached page property, a property
outside any live block, and an `id::` that is not exactly one hyphenated UUID
are not anchors. This handles real indented property lines rather than assuming
that the physical preceding line is the owner.

### 2. Serializable import-plan diagnostics

Extend the existing serde `ImportPlan` with a serializable anchor section,
returned intact to the web preview and supplied unchanged to apply:

| Record | Required fields | Apply policy |
| --- | --- | --- |
| `adopted_anchor` | source-relative path, source block ordinal, source UUID, resulting bid | add that exact pre-stamp |
| `duplicate_anchor` | duplicate UUID, winner source/block, rewritten source/block, **pre-minted replacement UUID** | add the baked replacement pre-stamp and show warning |
| `invalid_anchor` | source-relative path, source block ordinal/line, raw value, reason | do not stamp; warn visibly |
| `detached_anchor` | source-relative path, line, raw value, reason | do not stamp; warn visibly |

Scan deterministically by source-relative path, then source block order. The
first valid occurrence keeps its Logseq UUID. Every duplicate receives a
replacement UUID while planning, not while applying, so retries of the same
serialized plan produce exactly the same Markdown and Loro identity. The plan
preview surfaces every diagnostic. Apply rejects a plan whose baked anchor
mapping is missing or malformed; it must not regenerate ids or allow a flat
last-writer-wins collision.

The converted Markdown removes the Logseq `id::` property only after it has
become the comment. Invalid/detached input follows the existing non-rendering
property behavior, but is now reported instead of silently discarded.

### 3. Engine resolution states

Add a narrow `tesela-sync` read capability that accepts a string token and
returns precisely one of:

- **invalid** — token is not a hyphenated UUID. It is never a live reference.
- **resolved** — valid bid maps to a live block; return canonical bid, owning
  note id, and presentational target text.
- **unavailable** — valid bid has no currently live resolvable target. This
  includes unknown, deleted, evicted-without-loadable snapshot, and a stale
  index mapping. Do not claim to distinguish delete from unknown until a
  durable bid tombstone exists.

The resolver reads the existing `block_index`, follows its established
lazy-load path, then obtains text from the block's Loro document. It creates
no persistent map and does not scan the Markdown corpus. The server exposes
only a web adapter over this engine capability; future FFI/iOS consumers can
use the engine capability directly.

### 4. Decoration cache and invalidation

The CodeMirror decoration pass remains synchronous and never fetches. A
block-reference resolver/cache lives in the editor data layer and supplies the
pass with records keyed by bid:

```text
bid -> { state, targetNoteId?, targetText? }
```

A resolved `((bid))` displays target text but retains literal source text.
The cache maintains the inverse `targetNoteId -> set<bid>` relation. Existing
websocket `note_updated` invalidation removes every bid targeting that note;
`note_deleted` removes those entries and makes subsequent rendering
`unavailable`. A target edit must therefore refresh its ref labels rather than
leaving a stale cached string. Tests cover edit, deletion, and two different
bids owned by one target note.

Only a valid resolved ref is clickable in Normal mode. A valid unavailable ref
is visibly styled as unavailable and reports a concise notice when clicked.
An invalid token remains visibly invalid/non-link text. This spec deliberately
makes no modifier-click or new-tab claim; CodeMirror decorations are not
anchors and no such behavior is in scope.

### 5. Reuse the Graphite content-jump contract

Extend `web/src/lib/stores/content-jump.svelte.ts`'s existing one-shot payload
into a discriminated target:

- `content` keeps the current `{ query, snippet }` search behavior.
- `bid` carries a validated canonical bid.

Keep its existing monotonically increasing id, note scoping, and clear-once
behavior. Add a request helper for bid jumps rather than another global store.
`GrPage` continues to obtain the pending request and passes it to
`BlockOutliner`. For a bid target, the outliner finds `visibleBlocks` by
`block.bid`, then uses the existing focus/scroll/clear sequence. It must not
match the legacy line id.

The block-ref click adapter first resolves the bid. On `resolved`, it queues a
bid content jump, then opens the owning page through the buffer navigator. This
preserves target focus in `/g`, where current `gotoNote(noteId, bid)` discards
the bid. Non-Graphite routing may retain its existing `?block=` deep-link
behavior, but `/g` is required to use the shared store.

### 6. Copy command

Register a focused-editor command in the real command registry and regenerate
the command manifest. Its authoritative input is
`ctx.editor.block.bid` (a canonical UUID), never `focusedBlock.id` (the
`{note_id}:{line}` display address). It copies exactly `((<hyphenated-bid>))`.
The command is disabled with a clear status when no canonical bid exists.

## Required tests and acceptance

1. **Importer/plan tests**
   - Valid nested continuation ownership emits the exact bid comment and strips
     only the source `id::` property.
   - Duplicate anchors produce a deterministic winner, serialized pre-minted
     replacement, and replay-identical apply output.
   - malformed/detached ids yield serialized diagnostics and no stamp.
2. **Engine tests**
   - Resolved targets use `block_index` plus lazy load; valid missing/deleted
     targets are `unavailable`; non-UUID tokens are `invalid`.
   - The integration test starts with a Logseq fixture, calls plan, applies by
     the post-`tesela-ewj.1` engine/hydration path, exports/imports real Loro
     updates into device B, and resolves the imported bid on B.
3. **Web tests**
   - A resolved ref renders target text, Normal-mode click opens `/g` and
     focuses/centers the target block through the existing content-jump store.
   - An update/delete of the target note invalidates cached labels/states.
   - unresolved/invalid references visibly degrade; Insert-mode editing is
     unchanged; code fences and table ranges remain literal.
   - The registry copy command reads `ctx.editor.block.bid` and copies the
     exact source syntax.
4. **Manual Graphite check**
   - Open a note containing a ref, click it in Normal mode, verify the target
     page opens with the referenced block focused; press Escape/cancel and
     verify the editor focus behavior remains normal.

**Verify:**

```bash
cargo test -p tesela-core -p tesela-sync -p tesela-server
pnpm --dir web check
pnpm --dir web test:unit
pnpm --dir web test:e2e
```

## Sequencing

1. Wait for `tesela-ewj.1`; inspect its final importer-to-engine apply path.
2. Add plan-owned anchor parsing/diagnostics and pre-stamp output.
3. Add engine resolution, then the real importer→hydrate→sync two-engine test.
4. Add the cache/decoration path and extend the existing content-jump store.
5. Register copy-ref, regenerate the manifest, and run `/g` integration
   coverage.

## Out of scope

- iOS block-ref rendering/copy UI, embeds (`tesela-8zd.8`), block backlinks,
  transclusion, permission rules, cross-mosaic references, and repair of
  duplicate bids that predate a Logseq import.
