# Per-edit splices for web/desktop (tesela-baa) — multi-doc binding spec

2026-07-07 · Lead design (Fable) · grounded in a 6-reader mapping workflow + code verification.

## Problem (what 9iy actually left open)

Same-block concurrent typing must character-interleave. Delivery is fixed (c7s);
this bead is the sufficiency half. The mapping run found the premise of the bead
("web/desktop have no splice path") **stale**: C2.2/C2.3 (68f8100e/faeffdd4, 06-04)
already ship true per-keystroke splices — CM6 edit → client wasm `NoteDoc`
(`note-doc.ts`) → rAF-coalesced TLR2 binary WS frame → server `apply_inbound_delta`
→ `apply_relay_updates` (per-note apply-lock). Desktop = same web bundle.

**The real gap is the binding scope.** `active-note-doc.svelte.ts` is a process-wide
SINGLETON keyed to the focused buffer's slug, and the journal's default daily buffer
resolves to **today's slug only** (`GraphiteShell.svelte:113-117`). The 9iy storm was
typed into the *Jul-2* daily on Jul-3: never bound → every keystroke fell back to the
500ms whole-block HTTP path (`BlockOpsSaver` → `upsert_blocks` → `write_block_text`
minimal-diff) → the desktop authored the monolithic 105-char rewrite the
investigation found. Any non-focused note (past days in the journal, second panes,
non-focused notes) still has this gap today. Dailies are Taylor's primary editing
surface, so in practice much real typing misses the splice path.

## Decision

**Extend the client-owned-replica model to a bounded multi-doc registry.** Reject the
alternative (server-side splice RPC carrying raw `(bid, offset, delLen, insert)`):
raw offsets computed against a remote, concurrently-mutating doc are unsound — iOS
gets away with offset triples only because its FFI engine IS its local replica.
A remote client doing char-level collab must hold a real CRDT replica; web already
has one, it's just artificially limited to one note.

Server side needs **zero changes**: `apply_inbound_delta` routes frames by note id
and already serializes per note.

## Design

1. **`note-doc-registry.svelte.ts`** (evolves `active-note-doc.svelte.ts`; rename in
   place, keep the file's outbound-cursor + rAF-flush mechanics per doc):
   - `Map<slug, Entry>`; `Entry = { doc: NoteDoc, refs: number, lastSentVV, flushHandle }`.
   - `acquireNoteDoc(slug)` / `releaseNoteDoc(slug)` ref-counted from each mounted
     editor surface per note (mirror how `GraphiteShell` drives the current
     `openActiveNoteDoc` `$effect`; each visible journal day acquires its own).
   - LRU cap (start: 8) evicting only `refs == 0` entries; focused note never evicted.
     The singleton existed to prevent doc/subscription leaks — the registry must
     ref-count rigorously; leak test required.
   - `spliceBlock(slug, bid, from, delLen, insert)` replaces `spliceActiveBlock`;
     callers (few, per map): `BlockEditor` updateListener path,
     `BlockOutliner.handleBlockChange` programmatic path, `+layout` inbound,
     `GraphiteShell` lifecycle.
   - Inbound: route each TLR2 update by its doc id to the matching open entry
     (frames are keyed by 16-byte note id); broad-refresh fallback ONLY for notes
     with no entry. Per-doc `lastSentVV`; per-doc rAF flush.
2. **Binding gate stays, keyed by the block's own note.** `BlockEditor`'s
   bid+container-resolves check now consults the registry with the editor's note
   slug (plumb the slug prop where missing — find the existing prop the outliner
   passes; mirror it, don't invent a parallel channel).
3. **Fallbacks preserved (same taxonomy as iOS):** brand-new pre-bid blocks →
   BlockOps HTTP (safe: a block that exists on no other device has no concurrent
   editors); structural ops (move/indent/split-parent) unchanged; programmatic
   edits on unregistered notes → HTTP.
4. **Undo/echo/presence: unchanged mechanisms**, now per-editor-per-doc:
   `externalSync` + `addToHistory:false`, `by:"local"` skip, own-echo window for the
   HTTP path, CM decoration remap for presence.

## Non-goals / deferred

- Engine write-tail debounce (O(note) snapshot+materialize per import) → **tesela-ofu**.
- Server splice RPC — rejected above.
- iOS changes: none. Presence protocol changes: none.
- Op-growth mitigation: none needed — Loro merges same-peer burst commits
  (merge_interval 1000s, 4 KiB change blocks) and relay traffic is tick-cadence
  driven; verified in the cost map.

## Verification

- Unit (web): registry ref-count/evict/reopen/leak; splice routing by slug; inbound
  routing by doc id; fallback selection.
- Integration (Playwright, two pages): the **storm shape** — concurrent same-block
  typing in a NON-focused note (yesterday's daily inside the journal view), both
  sides' characters survive and interleave; regression: focused-note case still works.
- Bead verify_cmd: `cargo test -p tesela-sync -p tesela-server && pnpm --dir web check && pnpm --dir web test:unit`.

## Phases

- P1 registry + rename + unit tests.
- P2 per-editor acquire/release plumbing (journal days, panes) + gate keying.
- P3 Playwright storm test + self-QA in the real app.
- P4 ADR in decisions.md, close tesela-baa (notes → this spec).
