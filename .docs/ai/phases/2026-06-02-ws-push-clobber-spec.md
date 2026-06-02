# WS-push clobber fix (C + A) — spec (2026-06-02)

> The FINAL data-loss vector, PROVEN on the wire. User chose C + A.
> Build subagent-driven, two-stage review. Verify ON THE WIRE (DIAG-WS) on the
> sim before any device test. Diagnostics stay gated behind `TESELA_DIAG_WRITES`
> and are REMOVED once green.

## Proven root cause (wire-captured + code-confirmed)
DIAG-WS caught it live:
```
18:00:17  web POST /blocks: upsert c54f1628 "Awesome sweet"   (HTTP, protected, server correct)
18:00:20  iOS WS frame (origin=3, 323 bytes = WHOLE-NOTE SNAPSHOT)
          REMOVED/CHANGED-FROM: "Awesome sweet"
          ADDED/CHANGED-TO:     "Awesome"                     ← iOS stale snapshot LWW-reverts web's edit
18:00:21/23  iOS WS frames 103/106 bytes = genuine single-block deltas of iOS's OWN "Cool fire" edit (apply cleanly)
```
The chain (all 3 links code-confirmed):
1. **Full snapshot, not delta:** `RelayTicker.produceDeltaFrame` (RelayTicker.swift ~399) passes `sinceVv = lastPushedVV[slug]`; `lastPushedVV` (~101) is in-memory-only, empty on cold launch, advanced ONLY by `commitPushedDelta` after a confirmed send, never persisted/seeded. So the FIRST push of a note per session → `sinceVv=nil` → `export_doc_update(None)` → `ExportMode::Snapshot` (loro_engine.rs:390) = the whole tree incl. iOS's stale "Awesome".
2. **iOS's "Awesome" was stale:** web's "sweet" (HTTP block op) was broadcast to iOS via `ws_delta_tx`, but iOS hadn't merged it into its engine before exporting (delivery race / mid-edit). So iOS's snapshot carried the old "Awesome".
3. **Stale won the merge:** server applies the frame via `apply_inbound_delta` → `apply_relay_updates` → `import_doc_update` → raw `doc.import` (loro_engine.rs:489). The block-text meta is a Loro LWW register; iOS's "Awesome" op was **concurrent** with the server's "sweet" op on that register and won the (lamport, peer) tiebreak. **The WS-apply path has NONE of the base-diff / block-granular protection the HTTP path has.**

**Why concurrent, not dominated (VERIFY — load-bearing):** if iOS shared the server's exact authoring base for the "Awesome" block (T7 bootstrap), web's "sweet" would causally DOMINATE the base and iOS's snapshot (carrying only up-to-base for that block) couldn't revert it. The clobber implies iOS holds its OWN op on "Awesome" concurrent with web's — i.e. a residual disjoint-lineage for that block (these daily blocks predate T7/E1; or `recordNoteDiff` re-authored the block from stale markdown). The implementer MUST verify this against the live behavior — it determines whether A alone suffices or C is required (it informs both, but C is the universal guarantee regardless).

## Fix — two parts

### Part C (PRIMARY, server) — protect the WS-apply path like HTTP: apply only blocks the peer actually changed
Bring `apply_inbound_delta` / `apply_relay_updates` to parity with the proven HTTP `upsert_blocks` rule ("never re-assert a block the writer didn't change"). The peer's WS frame must NOT raw-`doc.import` into the authoritative doc.
- **Mechanism to design + VERIFY against Loro semantics (do NOT prescribe blind):** the goal is "apply only the blocks whose value the peer GENUINELY changed relative to the shared history, never a re-assertion of a block the peer didn't touch this session." Candidate approaches the implementer must evaluate and pick the correct one:
  - **(C1) Import into a scratch CLONE of the authoritative doc; per-block decide.** Clone the server's current note doc, `import` the peer frame into the clone, render per-block. For each block, compare clone-text vs authoritative-text. A block that differs is a *candidate*. The hard part: distinguish "peer genuinely changed it (newer op)" from "peer re-asserted a stale concurrent value that LWW happened to pick in the clone." Use the Loro op metadata: the peer frame's NEW ops (those past the server's current VV for that doc) identify which blocks the peer actually authored. Apply ONLY blocks touched by genuinely-new peer ops; for blocks the peer didn't newly author, KEEP the server's value. Re-emit the kept changes as block ops via `record_local(BlockUpsert)` on the authoritative doc (so they get the server's lamport + the existing dedup/materialize/fan-out).
  - **(C2) VV-gated apply:** track the per-(note) VV the server last had; from the incoming frame, extract only ops the server's VV doesn't dominate (genuinely new peer ops) and apply only those. Skip ops the server already causally has or that are concurrent re-assertions of un-changed blocks.
  - Whatever is chosen MUST: preserve T7 engine-render (the authoritative `doc.import` side-effects/materialize must stay correct), preserve S4 inbound-delta application for GENUINE peer edits (iOS editing block B must still apply!), stay idempotent + commutative (re-applying a frame = no-op), and reuse the existing `tombstone_duplicate_twins` dedup. It must NOT just "always keep server" (that would drop genuine peer edits — the inverse bug).
- **The discriminator is causal, not text-equality:** "peer changed block X" = the peer frame contains an op on X's register that is causally AFTER the server's current value of X. A stale re-assertion is concurrent-or-dominated → skip. Implement via Loro VV/op inspection; verify the exact API (`doc.import` returns import status / pending; `LoroDoc::version`, `oplog_vv`, op iteration) in loro 1.12.
- Mirror the HTTP rule's spirit: `crates/tesela-server/src/routes/notes.rs upsert_blocks` (~485-533) + `record_sync_update` base-diff (P1) — "apply only the author's real changes." C is that rule for the WS path.

### Part A (iOS, defense-in-depth) — never ship a whole-note snapshot that re-asserts un-edited blocks
- Seed `lastPushedVV[slug]` from the bootstrap base VV the moment a note becomes resident, so `produceDeltaFrame` ALWAYS passes a real `sinceVv` → `ExportMode::updates(vv)` = ONLY ops iOS authored since the base. A delta of iOS editing "Cool fire" then contains ONLY the "Cool fire" op — never an "Awesome" re-assertion. `bootstrapNoteIfNeeded` (RelayTicker.swift ~240-274) already fetches the server base + imports it; capture `engine.noteVersion(slug)` right after `importNoteSnapshot` and store it as `lastPushedVV[slug]`.
- Handle the gap: a note resident WITHOUT a captured baseline (resident from a prior session's local edits, never bootstrapped this session) — on first push, if `lastPushedVV[slug]` is nil, do a catch-up/bootstrap FIRST (or capture the current VV as the floor) so the first push is still a bounded delta, not a full snapshot of stale state. Verify the resident-but-unbootstrapped path.
- This shrinks frames + blast radius; C is the guarantee if A's baseline is ever missing.

## Invariants
1. A peer's WS frame NEVER reverts a block the peer didn't change this session — a concurrent peer edit to a DIFFERENT block leaves the first peer's block untouched on the server. (The exact repro: web edits A, iOS concurrently edits B + pushes → A keeps web's value.)
2. A GENUINE peer edit still applies over the WS (iOS editing block B must converge to all clients <1s — don't regress S4/the hub).
3. Idempotent + commutative: re-applying the same frame is a no-op; out-of-order frames converge.
4. No regression: HTTP base-diff/block-granular (P1/P2/S0-S4), T7 engine-render, dedup/tombstone, the convergence tests.
5. iOS steady-state ships bounded deltas, not whole-note snapshots (A).

## Repro / tests (the spine)
- **Rust (server/sync): the wire repro as a test.** Seed a note A+B on a server engine. A peer engine bootstraps from the server snapshot (shared base). Server applies an HTTP-style block op A→"A edited". Peer (stale: still has old A) edits B→"B edited" and exports a FULL SNAPSHOT (since_vv=None). Apply that snapshot via the NEW protected WS path. ASSERT: server render = "A edited" + "B edited" (A NOT reverted). Then a SECOND test: peer exports a since_vv DELTA of just B → applies → both survive (genuine edit still works). A THIRD: re-apply the same frame → no-op (idempotent).
- Run `cargo test -p tesela-sync -p tesela-server`. ALL convergence tests stay green (concurrent_whole_body_clobber, disjoint_history_revert, put_base_diff, snapshot_merge_keeps_local, partial_delta_needs_base, block_granular_write, positional_insert*).
- iOS (A): `xcodebuild ... Simulator build` SUCCEEDED; if the FFI changes, regen + note it (likely no FFI change — pure Swift VV seeding).
- **WIRE VERIFICATION (Claude-driven on the sim, the real proof):** with `TESELA_DIAG_WRITES=1`, web edits block A + iPad-sim concurrently edits block B and pushes. Assert the DIAG-WS log shows NO `REMOVED/CHANGED-FROM "A edited"` (no clobber) and the server file keeps both. Reproduce the EXACT 18:00 incident and confirm it no longer reverts.

## Risks
- **C must not invert the bug:** "keep server's value" only for blocks the peer DIDN'T newly author; a genuine newer peer edit MUST still win. The discriminator is causal (peer op newer than server's current), not "always server." Get this exactly right — test the genuine-edit case (invariant 2).
- **Loro semantics are subtle** — verify the VV/op-inspection API in loro 1.12; the "import into scratch clone + diff" must not corrupt the authoritative doc (keep `doc.import` on the authoritative doc side-effect-free until the decision is made, or apply via `record_local` after).
- **Disjoint-lineage residual:** if the live mosaic's blocks have disjoint lineages (pre-T7), C handles it (it doesn't rely on shared base — it inspects new ops). A relies on the base; note this.
- Don't regress the hub's <1s genuine-edit delivery (invariant 2) or the relay path.
- Keep the diagnostics gated + REMOVE both DIAG middlewares (mod.rs + ws.rs) once the fix is verified.

## Acceptance
1. Rust repro tests green (stale snapshot doesn't revert A; genuine B edit applies; idempotent).
2. `cargo test -p tesela-sync -p tesela-server` + web `npm run test:unit` + `xcodebuild` all green; no convergence regression.
3. WIRE-verified on the sim: the 18:00 incident no longer reverts; DIAG-WS shows no stale re-assertion; genuine concurrent different-block edits all survive on the server + all clients <1s.
4. Diagnostics removed; final commit clean.
