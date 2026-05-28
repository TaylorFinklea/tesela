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
2. **v1 scope: FULL parity before cutover — STRUCTURAL parity, not
   byte-identical.** Loro must round-trip every note's *structure*
   (blocks + properties + frontmatter), reserialized **deterministically**
   (same CRDT state → same bytes; clean diffs, stable grep) — NOT
   byte-identical to the current hand-edited disk bytes. The ~13
   structural divergences go to 0 measured by **parsed-structure
   equivalence**, not raw byte compare. At cutover, a one-time canonical
   reserialization rewrites the mosaic into Loro's deterministic form.
   (Decided 2026-05-28 — byte-identical round-trip is the Logseq-fidelity
   tar pit and pointless under structured-first; see decisions.md.)
3. **Structured-first (Anytype direction): the CRDT is the source of
   truth; markdown files are a deterministic materialized VIEW.** Inverts
   the old "files are truth" line. `query::`/`type::`/`sort::` etc. are
   PAGE PROPERTIES (first-class structured data), not raw text — the
   parser dropping them is a gap, not a content category. Phase 1 models
   block = `{text, properties: map, children}` + page-level properties.
   - **Scope line:** Phase 1 *preserves + merges* properties (parse →
     CRDT maps → deterministic serialize). It does NOT build the property
     *system* (global registry, type inheritance, `extends`, table views)
     — those are the separate roadmap phases in
     `memory/project_property_system_vision` that sit on top of this.
   - **Property values are scalar strings in Phase 1** (achieves parity
     for the scalar page-props in the 13 notes). **Multi-value props
     (`tags`, aliases) as Loro lists for clean union-merge is DEFERRED**
     to the property-system / collaboration phase — needs `value_type` to
     know which props are multi-valued. Known limit: until then,
     concurrent multi-value edits are LWW-on-the-whole-string (tag merges
     will misbehave). Conscious tradeoff, not a surprise.
4. **Hard cutover, no coexistence.** Taylor is on Logseq until this is
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

### Phase 0 — Spike the unknowns — ✅ DONE (GREEN)
Result in `2026-05-28-loro-cutover-spike-report.md`; tests in
`crates/tesela-sync/tests/loro_cutover_spike.rs` (8, all pass).
Validated: N-docs convergence + per-doc VV cursor, the flashing fix at
the CRDT layer, out-of-order/gapped import, snapshot bootstrap, and a
full-content schema (frontmatter `LoroText` + body tree of
`{kind:bullet|raw, indent, text}`) that round-trips non-bullet notes.
Deferred (on-device, later phases): loro-swift multi-doc (Phase 6),
lazy-load/evict memory (Phase 3), index doc perf (Phase 2).
Carried: Loro `PeerID` ↔ `DeviceId` stable mapping.

### Phase 1 — Per-note doc schema: structured properties, STRUCTURAL parity
Model the note as structured data (the everything-is-a-block / Anytype
direction — see locked decision 3):
- **frontmatter**: `LoroText` (verbatim YAML incl. `---` markers).
- **blocks**: `LoroTree`, each node `{text, indent, properties: map}`.
- **page-level properties**: a map on the note (the `query::`/`type::`/
  `sort::` lines that today render empty). Scalar-string values for now;
  multi-value list semantics deferred (decision 3).
- The spike used a `{kind:bullet|raw}` segment model as a stepping stone;
  Phase 1 supersedes "raw" with structured **properties** — a `query::`
  line is a page property, NOT a raw line. No raw-text escape hatch.
- **`tesela_core::note_tree` extension** is the cross-cutting part:
  `parse_body_blocks` currently drops non-bullet lines before the first
  bullet (`note_tree.rs:263`). Extend parse+serialize to capture page +
  block properties. This touches the authoritative SqliteEngine
  materialize path AND the ts-rs `ParsedBlock` (→ web, iOS) — do it as
  its own carefully-tested piece (note_tree has a round-trip property
  test to extend). Serialization is **deterministic**, not byte-identical
  to current disk (decision 2).
`render_note` → deterministic `serialize_note(frontmatter + blocks +
props)`.
**Acceptance:** the divergence check (rebuilt to compare **parsed
structure**, not raw bytes) hits **0 structural diverged / 0
primary-missing** across the corpus (currently 13–14 + 3). This is the
parity gate. Still shadow at this point. NOTE: the existing
`/loro/divergence` does normalized byte-compare — Phase 1 must switch it
to structural comparison, else deterministic-reformatting shows as false
diffs.

### Phase 2 — Index doc
Build the always-resident index `LoroDoc`: note_id → {title, slug,
created, modified, tags} + link/graph edges. Wire note create / delete /
rename through it. This becomes the source for backlinks + the note
list. **Acceptance:** index reconstructs the current note list + graph;
create/rename/delete reflected.

### Phase 3 — Lazy-load + evict — RESEQUENCED to ~Phase 6 (decided 2026-05-28)
Lazy-load/evict exists solely for iOS memory; iOS doesn't run this Rust
engine until the FFI swap (Phase 6), and the Mac server holds all 518
docs for free. So full lazy-load is deferred to when iOS needs it (and
can be tested on-device). **Groundwork landed now** (`0430616`): a
resident `block_index` (block_id → note_id) so block-only ops resolve
without scanning all docs — the prerequisite for eviction.
Remaining Phase-3 work (do at Phase 6): lazy boot (don't eager-load all
snapshots), LRU evict with a capacity cap, load-on-access in
render_note/doc_for_note_mut, lazy-aware index rebuild, divergence
iteration sourced from the index. Acceptance: memory bounded under
"open 500 sequentially", evicted notes reload.

### Phase 4 — Loro updates over the relay — NEXT (the keystone proof)
**Start fresh-context; this is the migration's whole point.** Concrete plan:

1. **Loro PeerID ↔ DeviceId (do first — it's load-bearing).** Two
   engines' per-note docs only merge cleanly if each device stamps its
   ops with a stable, distinct Loro PeerID. On doc create/load, call
   `doc.set_peer_id(peer_id_from_device(device))`. Derive a u64 PeerID
   deterministically from the 16-byte DeviceId (e.g. first 8 bytes, or a
   hash) so a device's ops are always recognized as its own across
   restarts. Root containers (`get_tree("blocks")`, `get_list`,
   `get_map`) are name-identified, so two docs for the same note_id
   share container ids and their ops merge — the spike confirmed this
   for raw LoroDocs; the engine must set peer ids to keep ops distinct.
2. **Engine-level convergence proof FIRST (additive, no live-relay
   risk).** Add `export_doc_update(note_id, since_vv) -> Option<Vec<u8>>`
   (per-doc `export(ExportMode::updates(&vv))`), `doc_version(note_id) ->
   encoded VV` (the cursor), and `import_doc_update(note_id, bytes)`
   (import into the addressed doc — create if absent, register blocks in
   block_index, refresh the index entry, persist snapshot). Test: two
   LoroEngines (separate snapshot dirs, distinct device/peer ids) make
   CONCURRENT edits to the same note via record_local, exchange updates
   via these methods, and converge to identical render with no
   oscillation across repeated exchange. THIS is the flashing-fix proof
   at the engine level (the spike proved it at the raw-LoroDoc level).
3. **Then wire into the relay (cutover-adjacent — can wait for Phase
   5/7).** The live relay envelope currently carries
   `postcard(Vec<EncodedOp>)` for SqliteEngine; do NOT change that
   format while dual-write runs (it would break the authoritative
   engine). The Loro-update payload (`postcard({doc: NoteId|Index,
   update_bytes})`) replaces it at cutover. Per-doc VV cursors replace
   the HLC outbound cursor then.

- **Acceptance (step 2):** automated two-engine convergence test, no
  flashing. **Acceptance (step 3, at cutover):** live relay carries Loro
  updates; two real devices converge.

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
