# Loro Cutover — Phase 0 Spike Report

*2026-05-28. Spike: `crates/tesela-sync/tests/loro_cutover_spike.rs` (8 tests, all GREEN). Throwaway — delete after the cutover lands.*

## Verdict: GO. Every Rust-validatable assumption holds.

| Assumption | Result | Test |
|---|---|---|
| Two docs converge on disjoint concurrent edits | ✅ | `two_docs_converge_on_disjoint_concurrent_edits` |
| **Same-block concurrent edit converges, no ping-pong** (the flashing fix) | ✅ deterministic + stable across re-sync | `concurrent_same_block_text_edit_converges_no_pingpong` |
| Out-of-order / gapped relay delivery converges | ✅ Loro buffers pending updates | `out_of_order_and_gapped_updates_converge` |
| VersionVector encodes/decodes for the wire (relay cursor) | ✅ | `version_vector_round_trips_for_transport` |
| Snapshot bootstraps a fresh device (Savanne join) | ✅ | `snapshot_bootstraps_a_fresh_device` |
| Page-property / `query::` / `# header` note round-trips byte-identical | ✅ | `full_content::page_property_note_round_trips` |
| Mixed bullets + raw round-trips | ✅ | `full_content::mixed_bullets_and_raw_round_trips` |
| Full-content note converges across devices | ✅ | `full_content::full_content_doc_converges_across_devices` |

## Key findings → decisions for the build phases

1. **Relay sync primitive (Phase 4):** each device tracks `doc.oplog_vv()`
   per note. To sync to a peer: `doc.export(ExportMode::updates(&peer_vv))`
   → ship bytes → peer `doc.import(bytes)`. Commutative + idempotent, so
   the relay's store-and-forward log (out-of-order, duplicates) is safe.
   The peer's VV is the cursor — `VersionVector::encode()/decode()` puts
   it on the wire. This *replaces* the HLC outbound cursor entirely.

2. **The flashing fix is real and at the CRDT layer.** The exact
   2026-05-28 scenario (two devices editing the same block's text)
   converges to one value both sides agree on, and stays put across
   repeated sync rounds. No application-level conflict logic needed.

3. **Full-content schema (Phase 1):** per-note doc =
   - `frontmatter`: `LoroText` (verbatim YAML incl. `---` markers)
   - `body`: `LoroTree` of segments, each meta `{kind: "bullet"|"raw",
     indent: i64, text: String}`.
   - render: frontmatter verbatim, then per segment — bullet →
     `"  "*indent + "- " + text`, raw → verbatim line.
   This round-trips the non-bullet notes (the ~13 structural
   divergences) byte-identically. NOTE: this is a richer model than
   today's `tesela_core::note_tree` (bullets only); Phase 1 must either
   extend `note_tree`'s parser/serializer to emit `raw` segments too, or
   keep the mapping inside LoroEngine. Extending `note_tree` is cleaner
   because SqliteEngine + the disk format then agree — recommend that.

4. **Snapshot = bootstrap.** New device imports a full snapshot, then
   keeps up via incremental `updates(&vv)`. Per-note snapshots already
   persist to `.tesela/loro/<id>.bin` (built 2026-05-28).

## Not validated here (deferred, with reason)
- **loro-swift multi-doc on iOS** — original Loro spike
  (`2026-05-27-loro-spike-report.md`) confirmed loro-swift uses the same
  UniFFI generator and basic compat. Multi-doc + index surface is
  exercised when the FFI swap happens (Phase 6), on sim + Roshar.
- **Lazy-load/evict actually frees memory** — Rust-side `Drop` of a
  `LoroDoc` frees it; the real test is iOS memory under load (Phase 3,
  on-device).
- **Index doc at ~3k notes** — trivial structurally; size/perf measured
  when built (Phase 2).

## Open design item carried into Phase 1
Loro `PeerID` (u64) ↔ Tesela `DeviceId` (16 bytes): need a stable
derivation (e.g. first 8 bytes of the device id, or a registered map).
Loro assigns/accepts a PeerID per doc; must be stable per device so a
device's own ops are recognized across restarts.
