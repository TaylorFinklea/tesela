# Real-time same-block collaborative editing (approach c) — spec (2026-06-03)

> User chose approach (c) after the re-base fix landed: cross-device delivery +
> different-block convergence now work, but two clients typing the SAME block
> at once still overwrite each other. This is the collaborative-editing build.

## Root cause (wire-proven, 2026-06-03 DIAG trace)
Both iOS and web **re-author the WHOLE block text** from their local editor. While a user is typing, the editor is "locked" to the local view (`isEditingBlock` defers inbound refreshes), so it does NOT live-merge the peer's concurrent characters. When the client ships its whole text, `LoroText::update(whole_text)` Myers-diffs it against the peer's value and emits **DELETE ops for the peer's text** → the peer's concurrent edit is clobbered. Trace:
```
WS (phone)  "reprotest 1 dude" → "reprotest 1 dude who"            (phone typing)
HTTP (web)  "reprotest 1 dude Did you increment the version?..."   (web typing same block)
WS (phone)  FROM "...Did you increment...update."  TO "...who is a good guy and"  ← phone DELETED web's text
```
The LoroText CRDT merges character **splices**; the clients send whole-block **replacements**, which defeats it. (This was the explicitly-deferred caveat of the LoroText work — see [[project-block-text-crdt]].)

## What already works (do NOT regress)
- **Lineage convergence** (authoritative re-base, `461293b`/`633b4df`): a disjoint device re-bases onto the server. Proven in the trace (the phone's no-op frame = it agreed with the server) and by `disjoint_device_authoritative_rebase_then_converges`.
- **Different-block / take-turns multi-device editing** converges. Only SAME-block simultaneous typing clobbers.
- Block text is a nested `LoroText` per tree node (key `text_seq`). The engine + WS/relay path merge splices for free WHEN clients emit clean splices.

## The fix — both editors become collaborative views over the per-block LoroText
Single source of truth = the per-block `LoroText`. Each editor: (a) turns local typing into **character-level splice ops** (insert(offset, s) / delete(offset, len)), NOT a whole-text re-author; (b) **live-applies inbound** LoroText changes into the active editor at the right offset, **remapping the cursor** so the user's caret survives a remote insert/delete.

## Phases (each independently testable; subagent-driven, two-stage review)
- **C0 — engine test harness + design lock.** Deterministic test: two LoroEngine replicas on a SHARED base, each applies a *splice* (insert at an offset) to the SAME block concurrently → cross-import → both contributions survive interleaved (already proven for whole-text via `concurrent_same_block_text_merges_not_clobbers`; add the offset-splice variant). Confirms the engine is ready; the work is all client-side. Decide the iOS local-edit→splice mechanism (diff the editor's own before/after, NOT the engine value) and the cursor-remap rule.
- **C1 — iOS collaborative BlockRow.** iOS is already a Loro peer. (1) Local typing → emit the diff of the EDITOR's previous vs current text as LoroText splice(s) against the engine's LoroText (so it's the user's change, not a re-author against a possibly-newer engine value). (2) On an inbound delta that touches the block being edited, apply the remote splice to the live editor (UITextView/TextField) at its offset and **remap the caret** instead of deferring on `isEditingBlock`. Result: iOS↔iOS same-block concurrent merges; iOS stops deleting a peer's text on re-author. Files: `BlockRow`/the block editor view, `MockMosaicService` edit path, `RelayTicker.applyInboundDelta`→editor seam. **Verify:** two sims typing the same block interleave (Claude-driven).
- **C2 — web as a Loro peer (loro-wasm).** Embed `loro-crdt`/`loro-wasm` in `web/`. Open the note's Loro doc, bootstrapped from `GET /loro/notes/{id}/snapshot`. Bind the block editor (CodeMirror) to the block's `LoroText`: local CM `update.changes` → LoroText splices; LoroText subscribe → CM dispatch (remote changes as transactions, selection remapped). Exchange deltas over the EXISTING `/ws` binary channel (web becomes a delta peer, like iOS). In-place TEXT edits stop using the whole-text `POST /notes/{id}/blocks`; structural ops (block insert/delete/move, indent) can stay on the HTTP block-op path initially. **Verify:** two browsers typing the same block interleave; web↔iOS interleave.
- **C3 — integration + wire verification + cleanup.** Re-arm the gated DIAG; drive iOS + web typing the SAME block simultaneously; confirm the trace shows interleaved splices, neither side's chars deleted. Remove DIAG. Update docs/memory. Server stays unchanged structurally (it already merges splices); the HTTP whole-text path remains a fallback.

## Invariants / acceptance
1. Two clients (any mix of web/iOS) typing into the SAME block concurrently → the result contains BOTH users' inserted characters (interleaved), neither deleted. NOT last-writer-wins.
2. A remote insert/delete arriving while you type does not jump or eat your caret (cursor remap).
3. No regression: different-block edits, take-turns edits, lineage re-base, the LoroText same-block whole-text merge tests, and all existing convergence/dedup tests stay green.
4. Structural edits (new block, delete, indent, move) still work (HTTP block-ops path retained).

## Risks / notes
- **Cursor remapping** is the genuinely hard part on both platforms — get the offset math right (a remote insert at offset ≤ caret shifts the caret right by its length; a delete shrinks it). Lock the rule in C0.
- iOS BlockRow is a SwiftUI text editor; applying remote edits to a live `UITextView` mid-typing without fighting the autocorrect/IME is fiddly — prototype early.
- loro-wasm bundle size + SSR (SvelteKit) — load it client-only.
- Keep the HTTP whole-text block path as a fallback (offline, structural). Don't delete it.
- This is the realization of [[project-structured-first-crdt-truth]] for text + the other-session's "make web a Loro peer" recommendation.
- DIAG diagnostics (`TESELA_DIAG_WRITES`) are reverted in source; re-arm for C3, remove after.
