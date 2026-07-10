# Bid-native Logseq block references

**Bead:** `tesela-8zd.7` · **Tier:** Lead · **Status:** implementation spec only
**Dependency:** `tesela-ewj.1` must land first so import writes through the
sole engine path.

## Scope

Preserve Logseq `id::` anchors as Tesela block ids during import, then make
`((bid))` a live web block reference: render its target text, navigate to the
target, copy a stable reference, and degrade visibly when unavailable. Prove
that a reference authored on one Loro engine resolves after sync on another.

This is deliberately bid-native. It adds no UUID translation table and no
parallel block-reference registry.

## Grounded design

### 1. The existing pre-stamp is the bridge

`crates/tesela-core/src/note_tree.rs` already defines the persisted block-id
syntax:

- `BID_PREFIX` is `<!-- bid:` at `:100`.
- `BID_SUFFIX` is ` -->` at `:101`.
- rendering appends the hyphenated UUID comment at `:314-316`.
- parsing removes a valid comment and adopts its UUID at `:405-438`;
  only an absent valid stamp falls through to `Uuid::now_v7()`.

Therefore a Logseq `id:: 675f...` becomes the exact comment
`<!-- bid:675f... -->` on its owning bullet. The comment is stripped from the
presentational block text but stays in Markdown, so the Logseq UUID literally
becomes the permanent Tesela bid. Do not create a `logseq_id → bid` map.

The importer currently strips `id::` along with presentation properties at
`crates/tesela-core/src/import_logseq.rs:647-654`; that behavior changes only
for a valid id anchor associated with a block. It already preserves `((uuid))`
references as literal prose, which is correct until the renderer resolves them.

### 2. Import-time anchor policy

The import-plan pass, before any writes, scans every converted Logseq block in
deterministic order: source-relative path lexicographically, then source block
order. A valid hyphenated UUID in an `id::` property belongs to the immediately
preceding Logseq block, is removed from rendered prose, and produces that
block's bid pre-stamp.

Duplicate policy is graph-wide and deterministic:

1. The first occurrence in that deterministic scan keeps the source UUID.
2. Each later occurrence receives a newly minted Tesela UUID pre-stamp.
3. Every later occurrence creates a plan warning identifying the duplicate
   UUID, winning source/block, rewritten source/block, and minted replacement.
4. Apply proceeds only with that explicit plan result; it never lets the flat
   Loro index silently last-writer-win the duplicate.

Malformed or detached `id::` values are not anchors: preserve the current
non-rendering property behavior and emit a plan warning rather than writing an
invalid bid comment. Existing blocks without `id::` keep the normal stamp path.

### 3. Resolution rides the Loro block index

`LoroEngine` already maintains a global, hydration-rebuilt
`block_index: bid → note_id` (`crates/tesela-sync/src/engine/loro_engine.rs:248-251`,
`:476-477`) and refreshes it whenever a note changes (`:919-922`). It supports
lazy loading through the same mapping (`:1125-1139`). Expose the minimum
read-only engine/server resolution seam needed to return the owning note and
current target block text by bid; it must use this index and load the owning
note if necessary. It must not scan all Markdown files or construct a second
map.

The read-only block-reference response must distinguish:

- resolved: canonical bid, owning note id, and target block presentational text;
- unresolved: malformed, unknown, deleted, or unavailable bid;
- invalid input: not a UUID-shaped bid.

The response is a lookup only. `((bid))` remains literal source text and does
not become a separate Loro operation. A resolved target later changing its text
must refresh web's rendered reference on normal note/WS invalidation; the
reference's identity never changes.

### 4. Web rendering and navigation

Add a `((hyphenated-uuid))` pass to the existing CodeMirror decoration pipeline
in `web/src/lib/cm-decorations.ts`. It follows the current wiki-link pass at
`:864-871`: skip fenced-code and table ranges, preserve the underlying source,
and decorate only a validated token. The decoration pipeline is synchronous;
network resolution belongs in the block/editor data flow and provides a cache
of resolved bids to the decoration pass. A ViewPlugin must never fetch.

For a resolved reference, render the target's current presentational text as
the link label while retaining the raw `((bid))` in the document. Clicking in
Normal mode follows the existing `BlockEditor.svelte:1767-1823` wiki-link
mousedown pattern and navigates through `gotoNote(noteId, bid)`. Insert mode
and modifier-click preserve their existing editing/new-tab behavior.

For an unresolved reference, retain the literal `((bid))`, style it visibly as
unresolved, and expose an accessible reason/tooltip. A normal-mode click must
not move to a guessed page; it reports an unobtrusive unavailable-reference
notice. This is intentional visible degradation, not a silent dead link.

### 5. Copy command and manifest

Add a focused-block editor command alongside the existing
`web/src/lib/editor/commands/` modules (for example, inspect `link.ts` before
choosing the exact module shape). It copies exactly `((<hyphenated bid>))` to
the system clipboard. It is available only when the focused block has a stable
bid; otherwise it is disabled or reports that the block has not yet been
stamped. It must never copy the legacy line-number id.

The command registers through the real command registry and is emitted by
`web/scripts/generate-command-manifest.mjs`, which loads both built-ins and all
editor command modules. Regenerate the checked-in manifest; do not hand-edit
it. The command must be reachable through the documented registry surfaces
that its metadata enables.

## Sequencing

### 1. Gate on the sole-writer importer path

**Work:** Confirm `tesela-ewj.1` is complete before changing import apply. Read
its resulting importer/engine write path and retain its single-authority rule.

**Acceptance:** The implementation never writes an imported pre-stamped file
outside the engine-managed import path.

**Verify:** `cargo test -p tesela-core -p tesela-sync`.

### 2. Preserve anchors and report duplicate ids in import planning

**Work:** Adapt the importer around the verified `id::` stripping path so a
valid block property yields the existing bid comment on its owner. Add the
whole-graph deterministic duplicate scan and plan warnings. Mirror the current
import fixture setup and apply tests in `import_logseq.rs`; do not add a
side-map.

**Acceptance:** Importing a valid Logseq anchor produces the exact
`<!-- bid:<hyphenated UUID> -->` syntax, the rendered block text contains no
`id::` line, and a duplicate preserves the first UUID while the second gains a
new UUID plus an actionable warning.

**Verify:** `cargo test -p tesela-core import_logseq`.

### 3. Expose bid lookup through the existing engine index

**Work:** Add the narrow read-only resolution capability from Loro's existing
`block_index` through the server route layer. It returns owner plus current
block text and handles an evicted target by the engine's established
load-on-demand path. Follow the existing bid parsing/validation conventions in
`crates/tesela-server/src/routes/notes.rs:683-689`.

**Acceptance:** A known bid resolves without a corpus scan; unknown, deleted,
and malformed bids are distinguished; no new persistent map exists.

**Verify:** `cargo test -p tesela-sync -p tesela-server`; `cargo clippy -p tesela-sync -p tesela-server -- -D warnings`.

### 4. Render and follow block references in the web editor

**Work:** Add the decoration/cache/click path, using the verified wiki-link
pass and `BlockEditor` Normal-mode click interception as patterns. Keep source
text editable and code fences/table cells literal.

**Acceptance:** A resolved `((bid))` displays target text and navigates to its
note and bid; an unresolved or malformed token is visibly non-resolving;
Insert-mode click behavior is unchanged.

**Verify:** `pnpm --dir web check`; `pnpm --dir web test:unit`.

### 5. Register the copy-block-reference command

**Work:** Add the focused editor command, use the existing focused-block state,
and regenerate the manifest with the repository script.

**Acceptance:** The palette/registry manifest contains the command; a stamped
focused block copies exactly `((uuid))`; an unstamped block cannot copy a
line-based surrogate.

**Verify:** `pnpm --dir web run generate:commands`; `pnpm --dir web test:unit`; `pnpm --dir web check`.

### 6. Prove cross-device convergence

**Work:** Add a real two-engine test using the existing Loro delta/import
pattern in `crates/tesela-sync/src/engine/loro_engine/tests/ops.rs`: device A
creates a target with a known bid and a block containing its `((bid))`
reference, exports updates, and device B imports them. Resolve on B through
the production index seam.

**Acceptance:** B returns A's target note and text for the same bid after
sync. This is a regression test for both durable pre-stamping and hydration of
the global index.

**Verify:** `cargo test -p tesela-sync -p tesela-server && pnpm --dir web run check && pnpm --dir web test`.

## Out of scope

- iOS rendering, tapping, or copy UI for block refs; this bead establishes the
  engine/server contract web needs, and a later iOS parity bead can consume it.
- Logseq `{{embed}}` rendering (`tesela-8zd.8`).
- Block reference transclusion, editable previews, backlinks-to-blocks,
  permission semantics, or cross-mosaic references.
- Repairing pre-existing duplicate bids outside an explicitly imported Logseq
  graph.
- Any new persistent id mapping or a change to the Markdown bid comment
  format.
