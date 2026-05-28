# Loro Cutover — Migration Spec

*Authored 2026-05-28. Status: DRAFT for review. Supersedes the dual-write "soak forever" framing — the goal is a hard cutover to a Loro-authoritative engine, then deletion of the hand-rolled oplog engine.*

## Why (the one-paragraph version)

The hand-rolled sync engine uses last-write-wins per op with whole-file
materialization. Concurrent edits across devices don't converge — they
ping-pong ("flashing", observed live 2026-05-28 on Roshar). CRDT merge
(Loro) converges by construction. The relay protocol, encryption
(AEAD/HKDF), pairing flow, and Cloudflare/HA worker stay **unchanged** —
only the opaque `SyncEnvelope.ciphertext` payload changes from
`postcard(Vec<EncodedOp>)` to Loro update bytes, and both ends run
`LoroEngine`. See [decisions.md 2026-05-27] for the commitment.

## Locked decisions (2026-05-28)

1. **Doc model: hybrid — per-note Loro docs + a small always-resident
   index doc. NOT a single mega-doc.** Single-doc OOMs iOS at real scale
   (dailies compound to thousands/decade; everything-is-a-block → millions
   of blocks; iOS jetsam ceiling → app killed mid-write = self-inflicted
   data loss). Maps onto the existing per-note files + per-note relay
   routing. Full rationale: memory `project-loro-doc-model`.
   - **Index doc** (always resident, small): `note_id → {title, slug,
     created, modified, tags}` + link/graph edges. Note create / delete /
     rename are collaborative through it. Refs (`[[page]]`, `((block))`)
     resolve through it.
   - **Per-note docs** (lazy-loaded, evictable): full note content (see
     Phase 1 schema). One snapshot file per note on disk (already built).
2. **v1 scope: FULL parity before cutover.** Loro must round-trip every
   note byte-identically to disk — frontmatter, page properties, query/
   type pages, non-bullet content — not just bullets. The ~13 structural
   divergences in today's shadow must go to 0 before flipping.
3. **Hard cutover, no coexistence.** Taylor is on Logseq until this is
   solid (memory `feedback-tesela-not-daily-driver-until-migrated`), so
   we flip all relay participants at once and delete the old engine. No
   dual-protocol envelopes, no gradual per-device rollout, no
   backwards-compat shims.

## Architecture after cutover

- **Relay participants** (each runs a `LoroEngine`): Mac server, iOS app
  (via FFI), Savanne's Mac + iOS. The **web client is unaffected** — it's
  a thin HTTP client of the Mac server, never ran an engine.
- **Wire**: per-doc Loro updates. Envelope `ciphertext` =
  `postcard({ doc: NoteId | Index, update_bytes })` (AEAD-sealed as
  today). Relay still a linear seq log of opaque envelopes; each device
  appends its updates, polls others'. Loro merge is commutative +
  idempotent, so out-of-order / duplicate delivery is safe — cleaner than
  today's HLC cursor dance.
- **Cursors**: per-doc Loro version vectors replace the HLC outbound
  cursor. Inbound still tracked by relay `seq`.
- **Materialization**: a note's Loro doc → `serialize_note` → its
  `<mosaic>/notes/<slug>.md` file. LoroEngine is the sole writer. Files
  stay plain Logseq-compatible markdown (they ARE the user's data).
- **Retired**: `tesela-sync::SqliteEngine` oplog + `Vec<EncodedOp>` wire
  format. NOTE: `tesela-core`'s SQLite (search/FTS/links/derived tables,
  fed by the file watcher) is SEPARATE and STAYS.

## Phases (each independently reviewable; one commit-group each)

### Phase 0 — Spike the unknowns (½–1 day, before committing engine code)
Throwaway, like the original Loro spike. Confirm:
- **N-docs over the relay**: export/import per-doc updates keyed by
  note_id; per-doc version vectors; out-of-order + gapped import (Loro
  pending-updates buffering). The relay is already per-note so this
  should be clean — verify.
- **Index doc**: a small always-resident doc for note_id→meta + graph;
  confirm size + update cost at ~3k notes.
- **Full-content round-trip**: prototype a per-note doc schema that
  serializes byte-identically for a frontmatter + bullets + non-bullet
  (`query::` / `# header`) note.
- **Lazy-load + evict**: load a note doc from snapshot, drop it from
  memory, reload — confirm correctness + that eviction actually frees.
- **loro-swift multi-doc**: confirm the FFI surface supports N docs +
  index on iOS (the original spike confirmed basic UniFFI compat).
Output: go/no-go + any schema adjustments. Report at
`2026-05-28-loro-cutover-spike-report.md`.

### Phase 1 — Per-note doc schema for FULL note content (parity)
Rewrite the per-note Loro doc to represent the ENTIRE file, not just
bullets:
- frontmatter (verbatim, e.g. `LoroText` or a `LoroMap`),
- block tree (`LoroTree`, text + indent — current flat model),
- non-bullet / raw content (a raw-segment representation so `query::`,
  `# header`, page-property notes round-trip).
`render_note` → byte-identical `serialize_note(frontmatter + body)`.
**Acceptance:** the dual-write divergence report hits **0 diverged /
0 primary-missing** across the whole corpus (currently 13–14 + 3).
This is the parity gate for cutover. Still shadow at this point.

### Phase 2 — Index doc
Build the always-resident index `LoroDoc`: note_id → {title, slug,
created, modified, tags} + link/graph edges. Wire note create / delete /
rename through it. This becomes the source for backlinks + the note
list. **Acceptance:** index reconstructs the current note list + graph;
create/rename/delete reflected.

### Phase 3 — Lazy-load + evict
LoroEngine loads a note's doc on access, evicts on close/idle (LRU,
bounded memory). Snapshots already persist per-note. **Acceptance:**
memory stays bounded under a scripted "open 500 notes sequentially"
test; evicted notes reload correctly.

### Phase 4 — Loro updates over the relay
- `produce_changes_since` → per-doc `export(Updates since peer VV)`.
- `apply_changes` → import update bytes into the addressed doc (+ index).
- Envelope payload format (above); per-doc VV cursors.
- **Acceptance:** two LoroEngine instances (two mosaic dirs, one relay)
  converge on concurrent edits to the same note — no flashing — in an
  automated test. THIS is the bug-fix proof.

### Phase 5 — LoroEngine authoritative for materialization
Loro doc → serialize → disk is the sole writer. `record_local` mutates
Loro, exports an update, materializes the file. SqliteEngine oplog no
longer writes. Still behind the flag, but now "flag on" = Loro is truth.
**Acceptance:** web HTTP edits (Mac server) flow web → LoroEngine →
disk + relay; round-trip to a second engine converges.

### Phase 6 — iOS FFI swap
`tesela-sync-ffi` exposes LoroEngine (loro-swift on the Swift side, or
the Rust FFI wraps LoroEngine). iOS builds, links, seeds from index +
snapshots. **Acceptance:** Roshar runs the Loro engine, syncs with the
Mac, no flashing under the same edit sequence that broke it 2026-05-28.
Test on sim AND Roshar (memory `feedback-ios-test-on-device`).

### Phase 7 — Flag-day cutover + retire the oplog engine
- One-time seed: every device's Loro docs + index built from current
  disk (the Phase 1 seed path, run once per device).
- Switch the relay payload to Loro updates on ALL participants at once.
- Delete `tesela-sync::SqliteEngine` + `Vec<EncodedOp>` wire + dual-write
  scaffolding + debug endpoints. Keep tesela-core search DB.
- **DR drill**: backup → restore to a fresh device → verify intact
  (engine-agnostic; do once here as the cutover gate).

## Risks / watch-items
- **Non-atomic cross-note moves** (per-doc model) — accepted; refs ≫
  physical moves. Index can record a move marker if ever needed.
- **Full-content schema** (Phase 1) is the hardest design — getting
  byte-identical round-trip for arbitrary markdown (not just bullets).
  De-risk in Phase 0 spike.
- **iOS memory under lazy-load/evict** — the whole reason for the hybrid;
  validate eviction actually frees (Phase 3 + Phase 0).
- **Loro PeerID ↔ 16-byte DeviceId** mapping — need a stable derivation.
- **Bootstrapping Savanne (new device)** — index doc snapshot + per-note
  snapshots on demand via pairing flow / relay. Detail in Phase 4/7.

## Out of scope (explicitly)
- Real-time presence / cursors (a Loro freebie later, not v1).
- Character-level collaborative text within a block (Loro enables it;
  v1 keeps block-granular text to match current behavior).
- Web client changes (it's an HTTP client; unaffected).
