# Current State

## State as of 2026-05-29 (latest) — iOS FFI swapped to LoroEngine + blank/heading drop

**iOS now runs the authoritative LoroEngine over the FFI** (commit `5b00fe7`), and **blank blocks + headings are dropped** per Taylor (`feat(sync): drop blank blocks...`). The cutover is code-complete on both Mac + iOS; what remains is the paired on-device cross-device test + the flag-day flip of Taylor's real mosaic.

### iOS FFI swap (done + sim-verified)
- `SyncEngineHandle.inner` → `Arc<dyn SyncEngine>` + `notes_dir` field; new `open_loro(mosaic_path, device_id_hex)` builds an authoritative LoroEngine (snapshot=`<mosaic>/.tesela/loro`, materialize=`<mosaic>/notes`). `SyncCoordinator` ticks branch on `uses_loro_relay_payload()` internally (mirrors the Mac tick: produce→TLR2→put→commit on confirmed send; poll→TLR2 decode→apply, decode-err skips). No new uniffi exports — Swift only swapped the constructor (`RelayTicker.openEngineIfNeeded` → `openLoro`).
- Build pipeline (uniffi **0.31 = library mode**): `cargo build -p tesela-sync-ffi --target aarch64-apple-ios{,-sim} --release` → `cargo run -p tesela-sync-ffi --features cli --bin uniffi-bindgen -- generate --library target/debug/libtesela_sync_ffi.dylib --language swift --out-dir app/Tesela-iOS/Generated/` → copy `Generated/tesela_sync_ffiFFI.h` to `CFFI/` (the modulemap reads CFFI's copy) → `cd app/Tesela-iOS && xcodegen generate` → `xcodebuild -scheme Tesela -sdk iphonesimulator -configuration Debug -destination 'generic/platform=iOS Simulator' build`.
- **Verified**: both targets cross-compile; bindings regen (+44 lines, just open_loro); app builds + links; launches on the booted sim (Tesela-Test `5B48EF63-...`) without crashing; **LoroEngine opens on-device** (`with_dirs` created `Documents/sync-ios-mosaic/.tesela/loro`). iOS starts EMPTY + bootstraps from the relay (non-canonical device → no reseed), per the multi-device design.

### Blank blocks + headings dropped
`note_tree_from_doc` skips empty/whitespace bullets; headings were already absent (flat-block model). Confirmed-desired (the `2026-05-17` heading drop is intended, not a regression). `decisions.md` 2026-05-29.

### NEXT — the paired on-device cross-device test (needs Taylor + iPhone)
1. **Install the new build on Roshar** (physical iPhone 15 Pro): the device `.a` is built (`target/aarch64-apple-ios/release/libtesela_sync_ffi.a`); run the app to Roshar from Xcode (or `xcodebuild -sdk iphoneos` + devicectl). Device-gated.
2. **Run the Mac authoritative server** on the real mosaic — but ⚠️ this REWRITES the Logseq graph (reseed + canonicalize 512 files, drop the `2026-05-17` heading + blank blocks). Taylor cleared destroying data, but it's his live Logseq files: `TESELA_LORO_AUTHORITATIVE=1 TESELA_LORO_RESEED=1 tesela-server --mosaic ~/Library/Application\ Support/tesela/logseq`. (Test on a copy first if preserving Logseq.) The HA relay is already configured; Mac = canonical Loro broadcaster.
3. **Cross-device**: iPhone (paired, same relay group) imports the Mac's v2 broadcasts → materializes notes; edit concurrently on web + iPhone → verify convergence, no flashing.
4. **Flag-day** afterward: delete SqliteEngine/DualEngine/op-wire + DR drill.

Convergence is already proven at 3 levels incl. the real HTTP relay (`tesela-relay/tests/loro_cutover_e2e.rs`); the on-device test confirms it end-to-end across real Mac↔iPhone.

A copy-based authoritative smoke `/tmp/tesela-auth-smoke.tHfTGN` (port 7475) + the dual-write server (real mosaic, port 7474) may still be running — both disposable.

---

## State as of 2026-05-29 — Loro relay payload + authoritative writer LANDED + live-validated

**The reversible, Mac-testable core of the cutover is done, behind flags, and proven on the real 512-note corpus.** Taylor cleared destroying data ("I'm just using logseq while you do this"). Commits: `1c64d52` (relay payload + flag), `db21ded` (real-relay e2e test). 100 sync lib tests + 17 relay tests green.

### What shipped
- **v2 relay payload** (`wire/mod.rs`): `LoroDocUpdate{doc,update_bytes}` + `TLR2`-magic-prefixed postcard codec. Collision-proof vs legacy bare `postcard(Vec<EncodedOp>)` (a v1 payload decodes to `None` on the v2 path → skipped, never mis-applied). Index doc NOT broadcast (peers rebuild locally — self-healing index).
- **Authoritative LoroEngine** (`loro_engine.rs`): `with_dirs(.., materialize_dir)` → writes `<slug>.md` on every apply (apply_payload + import_doc_update), atomic tmp+rename; NoteDelete removes the file. `reseed_from_disk` (canonical-device bootstrap). Broadcast cursors persisted to `_broadcast.bin`. New trait methods `uses_loro_relay_payload`/`produce_relay_updates`/`apply_relay_updates`.
- **Relay tick** (`sync_relay.rs`): branches on `uses_loro_relay_payload()` — Loro path does decode→apply / produce→encode→put; legacy path untouched. AEAD seal/open unchanged (payload-opaque).
- **main.rs**: `TESELA_LORO_AUTHORITATIVE` → bare LoroEngine as sync_engine (SqliteEngine bypassed; reads via FsNoteStore off disk Loro owns). `TESELA_LORO_RESEED` → reseed at boot.

### Convergence proven at 3 levels (no flashing)
1. engine (`two_engines_converge...`), 2. engine+wire (`..._through_wire_codec`), 3. **engine+wire+AEAD+HTTP-relay** (`tesela-relay/tests/loro_cutover_e2e.rs` — two authoritative engines over the in-process relay converge to identical render AND identical on-disk files, stable across extra rounds).

### Live smoke on a COPY of the real logseq mosaic (512 notes) — ALL GREEN
Booted authoritative+reseed server (port 7475) on `/tmp/tesela-auth-smoke.*` (copy, to not touch Taylor's live Logseq files):
- reseeded **512 notes**, booted, listening.
- **Stale-shadow fix validated**: `2026-05-27` regained its `status::`/`tags::` block props + full "...send out posts were it can" text (reseed-from-disk corrects staleness); `2026-05-22` cleaned 2 orphaned malformed `status:: done` lines. Dry-run `2026-05-27` now **byte-identical**.
- Only data change is the KNOWN `2026-05-17` bare-`# heading` drop (CRDT model limit, documented).
- Read path (GET /notes, GET /notes/:id) via FsNoteStore ✓. Write path (PUT → record_local → materialize → re-read with canonical bid) ✓.

### NEXT (genuinely remaining, mostly device-gated)
- **iOS FFI swap** (the real device-gated mile): expose Loro per-doc + relay methods via uniffi, swap SyncEngineHandle to LoroEngine, build `.a` for both ios targets, install Roshar+sim, pass stable device id, snapshot load on open. Then the web↔iPhone cross-device convergence test (needs the iPhone).
- **Two real Macs** (optional pre-iOS): point two authoritative servers at one relay group (copy group identity between mosaics) — exercises the literal tesela-server `tick()` wrapper, which the e2e test mirrors but doesn't call (tesela-server is bin-only).
- **Flag-day**: flip Taylor's real mosaic to authoritative+reseed once, delete SqliteEngine/DualEngine/Vec<EncodedOp> wire, DR drill.
- **BlockMove under Loro**: verify web indent/outdent round-trips before flag-day.

### Adversarial review DONE (2026-05-29) — 8 confirmed, 3 fixed, 2 deferred
3-reviewer + per-finding-verify workflow on the diff. Fixed (commit after `1c64d52`): **[#1]** cursor advanced before send-confirm → lost delta on failed PUT (now `produce_relay_updates` returns `(id,bytes,captured_vv)` + new `commit_broadcast_cursors` advances only after confirmed PUT; failed send retries); **[#2/#3]** Loro decode-Err didn't advance `max_seq` → poison envelope stalled inbound forever; **[#4/#5/#6]** NoteDelete orphaned the `.md` when `display_alias=None` (now resolves slug before inner-delete). **Deferred [#7/#8]**: slug-rename orphans/dups — only on rename (no rename flow here; `blake3(slug)` ids consistent everywhere; common bootstrap verified on 512 notes). 102 sync + 17 relay tests green. See `decisions.md` 2026-05-29.

Architecture decisions recorded in `decisions.md` (2026-05-28 entry). Old dual-write server still runs on the REAL mosaic (pid was 92195, release binary, port 7474) — harmless; not the cutover path.

---

## State as of 2026-05-28 late night — cutover dry-run landed

**Pushed the cutover's NON-destructive leading edge: a materialization dry-run that shows exactly what LoroEngine would write to disk as sole writer, diffed against every live file BEFORE any writer flip.** Commit `7d4f214`. 92 sync tests green. Server running `TESELA_LORO_DUAL_WRITE=1` (pid 92195), port 7474.

- `LoroEngine::render_note_full` (full `.md`: verbatim frontmatter + page props + blocks) + `SyncEngine` trait method + DualEngine forward. `GET /loro/notes/:slug` now returns `would_materialize`, `disk_raw`, `materialize_byte_identical`, `materialize_structurally_equal`.
- **Live dry-run over all 512 notes:** 495 byte-identical (flip = no-op), 14 structural-only (cosmetic reformat, mostly query pages), **3 real diverge**: `2026-05-27` + `2026-05-22` = shadow STALE vs disk (`.bin` snapshots predate Logseq edits; `seed_shadow_from_disk` skips already-tracked notes); `2026-05-17` = CRDT model limit (bare `# heading` body not a bullet block).
- **Cutover-critical conclusion (recorded in `phases/2026-05-28-loro-cutover-phase5-6-plan.md` §dry-run):** flag-day MUST reseed Loro docs from authoritative DISK (force NoteUpsert → tree-reconcile), not oplog/snapshots — else the 2 stale notes lose data on the flip. Dry-run endpoint is the DR verification tool (re-run before+after reseed).

**STOPPED before the destructive parts** (writer flip, live relay protocol change, deleting SqliteEngine) — confirmed via tracing the plumbing that making LoroEngine authoritative requires suppressing SqliteEngine's writes (two writers conflict), a delicate cross-engine flip whose flag-on test mutates the real mosaic + whose final web↔iPhone test needs the device. The dry-run de-risks materialization correctness without that danger. NEXT remains the destructive cutover per `phase5-6-plan.md` §recommended order (1→6).

---

## State as of 2026-05-28 night

**Phases 0–2 DONE + verified live + adversarially reviewed + 7 review-bugs fixed. Honest divergence on the live 518-note corpus is 3 (all explained, all resolved at cutover). Ready for Phase 3 — the foundation is now reviewed, not assumed.**

### Adversarial review (29 agents, 6 dimensions × verification) → 18 confirmed findings
Full report + per-finding triage: `phases/2026-05-28-loro-review-findings.md`.
- **7 fixed + tested + committed** (`c33a88d`,`c27818f`,`fad0280`,`ba2fffb`): [1/9] BlockDelete child-reparent, [2] NoteUpsert self-heal reconcile, [4] divergence check sees block-property values, [5] divergence check sees unmodeled non-bullet residue (killed a dangerous symmetric-lossy false-negative), [6] index ghost-prune, [7] index comma-collision, [8] snapshot tmp race.
- **[3] deleted-note resurrection — won't-fix**: SqliteEngine-specific (append-only oplog slug resolution); cutover fixes it free (LoroEngine NoteDelete drops the doc). Per "don't baby the doomed engine."
- **Backlogged (low-incidence/severity)**: [10/11] non-bullet body (1 note, `# 2026-05-17`, cutover cleanup), [12] divergence coverage, [13–18] hardening. All recorded in the findings doc.

The review was worth it: [4]/[5] revealed the earlier "divergence → 2" rested on a too-lenient check. The **honest number is 3** (2 frozen #111 oplog-drift + 1 legacy heading) — strictly better to know before an irreversible cutover.

### Resequencing + Phase 3 groundwork (decided 2026-05-28)
Phase 3 (lazy-load/evict) is iOS-only with no consumer until the FFI swap, and is a big cross-cutting refactor. **Resequenced to ~Phase 6**; Phase 4 (the keystone flashing-fix proof) goes next. Landed the shared prerequisite now: a resident **block_index** (block_id → note_id, `0430616`) so block ops resolve O(1) and lazy-load is unblocked later. 88 sync tests green.

### Phase 4 step 1+2 DONE — the flashing fix is PROVEN at the engine level (`80a1cd1`)
- Loro PeerID↔DeviceId stable mapping (`peer_id()`, `set_doc_peer()` on every doc create/load/import).
- Per-doc update sync: `doc_version` (encoded VV cursor), `export_doc_update(note_id, since_vv)`, `import_doc_update(note_id, bytes)` + `refresh_note_derived`. Additive — the live `Vec<EncodedOp>` relay path is untouched (dual-write intact).
- **Keystone test** `two_engines_converge_on_concurrent_edits_no_flashing`: two engines (distinct peers) edit the same note concurrently, exchange Loro updates via VV cursors, converge to identical render, stable across re-exchange. The hand-rolled LWW engine could not do this — this is why the migration exists.

### Phase 5 engine-level DONE — relay broadcast cursor model proven (`d29e631`)
`produce_relay_updates`/`apply_relay_updates` + per-note `broadcast_cursor`. Test `three_engines_converge_via_broadcast_relay`: 3 distinct-peer engines edit one note concurrently, broadcast rounds converge all three, steady-state round broadcasts nothing (bounded re-broadcast). The recon's #1 risk (cursor model) is de-risked at the engine level, non-destructively (live relay still on Vec<EncodedOp>). 90 sync tests green.

### Cutover recon + plan banked (`edfac22`): `phases/2026-05-28-loro-cutover-phase5-6-plan.md`
4-agent map of relay tick / FFI / materialization / iOS. **Critical resolution: the relay MUST carry Loro update bytes, not Vec<EncodedOp>** (one recon agent's op-wire approach would reintroduce flashing — replaying ops into independent docs mints non-merging peer-specific nodes). Verified: iOS runs a Rust FFI engine (SyncEngineHandle→SqliteEngine); LoroEngine producer methods were the gap (now built).

### NEXT (the DESTRUCTIVE cutover — fresh effort + device): wire into the LIVE relay
Everything up to here is non-destructive + tested by me. Remaining = the cutover proper, all in `phase5-6-plan.md` §recommended order: (1) expose per-doc sync + render via FFI; (2) live relay envelope = Loro payload behind a protocol-version byte + persist per-note broadcast cursor; (3) `TESELA_LORO_AUTHORITATIVE` flag → LoroEngine writes the .md files; (4) iOS LoroEngine + snapshot load + device-id bridge, build .a for both ios targets, install Roshar+sim; (5) flag-day + delete DualEngine/SqliteEngine/op-wire + DR drill; (6) verify web indent/outdent (BlockMove) under Loro. The hard "does Loro fix flashing" question is PROVEN; this remaining work is integration + a destructive live-wire flip whose final test (web↔iPhone convergence) needs the device.

### (superseded) earlier NEXT note
The remaining cutover-adjacent work: replace the relay envelope payload (`postcard(Vec<EncodedOp>)`) with Loro update bytes (`postcard({doc: NoteId|Index, update_bytes})`) + per-doc VV cursors, and make LoroEngine authoritative for materialization. Do NOT change the live envelope while dual-write with SqliteEngine runs — this is the cutover (Phase 5→7). Also at Phase 6: full lazy-load/evict (groundwork = block_index, done) + iOS FFI swap, verify on Roshar. Then Phase 7: flag-day cutover + delete oplog engine + DR drill.

### Earlier in the session (still true)

- Phase 0 spike GREEN (flashing fix proven at CRDT layer). Phase 1 page-property parity. Phase 2 index doc (518 notes, 448 with tags, 128 with link edges) — all on the live corpus.
- **Self-healing versioned index** (`902439e`): per-note docs are self-describing (content+slug+title on root meta); on boot a version-gated rebuild refreshes the index from them. No more manual cache clears on index schema changes — verified live ("rebuilt index schema 0 → 2"). This is reusable infra for every later phase.
- Divergence holds at **2 of 518** (both #111 oplog-vs-disk, resolved at cutover) + 3 primary-missing.
- Latest commits: `902439e` self-healing index, `1b07636` tags+link graph, `c8164d7` index doc, `74055e5`/`5959835` page-prop parity + structural divergence, `25fcbcb` note_tree page props, `8373139` Phase 0 spike.

**NEXT (gated on review findings):** Phase 3 — lazy-load/evict (bounded iOS memory), then Phase 4 — Loro updates over the relay (the on-device flashing-fix proof). Do NOT start Phase 3 until the review workflow's confirmed findings are triaged + fixed — building on unreviewed foundation is the mistake the review prevents.

Server running with `TESELA_LORO_DUAL_WRITE=1`, self-healing index live. Endpoints: `/loro/index`, `/loro/divergence`, `/loro/notes/<slug>`.

---

## State as of 2026-05-28 afternoon

**Migration plan locked + Phase 0 spike GREEN.** The cutover plan is `phases/2026-05-28-loro-cutover-spec.md` (Phases 0–7, hybrid per-note-docs + index doc, full-parity hard cutover). Phase 0 spike (`crates/tesela-sync/tests/loro_cutover_spike.rs`, 8 tests) proved every load-bearing assumption — most importantly the **flashing fix at the CRDT layer** (concurrent same-block edits converge deterministically, no ping-pong) and a **full-content schema** that round-trips the non-bullet notes. Report: `phases/2026-05-28-loro-cutover-spike-report.md`.

**Phase 1 — page-property parity ACHIEVED.** Divergence dropped 13–14 → **3** (match 512) on the live corpus. The structured-property model works end to end.

- [x] `note_tree` captures page properties (`25fcbcb`). Also fixed a latent data-loss bug (page props dropped on block ops). 364 tests green.
- [x] LoroEngine stores (ordered `page_props` LoroList, wholesale on NoteUpsert) + renders page properties (`74055e5`).
- [x] Divergence check + `/loro/{divergence,notes}` endpoints compare PARSED STRUCTURE, not bytes (`74055e5`, `5959835`). `structurally_equal()` in dual_engine compares page_properties + block (text, indent); `normalize()` retained only for display.
- Chose ordered LoroList over map+sort (preserves on-disk order deterministically); per-key/multi-value merge deferred to property-system phase.

**Remaining divergence (3 + 3, NOT page-property):**
- 3 structural diverges are the **oplog-vs-disk class (#111)** — block-order / stale-block mismatches from earlier edit history + some of my own leftover test debris in the live mosaic (`8320e597` has "perf test block four", `7d98c130` has stale "so they all rolled over"). To reach literal 0 we either reconcile those notes (re-derive oplog from disk) or accept them as known-stale history. NOT a model bug.
- 3 primary-missing: files not found on disk for note_ids the oplog references (a73d66, affc1a, 0138057). Pre-existing.

**Phase 2 step 1 DONE + verified live** (`c8164d7`): always-resident Loro **index doc** (note_id → {title, slug}), separate from per-note docs, persisted to `<dir>/_index.bin`. `GET /api/loro/index` dumps it. On the live corpus it populated **all 518 notes**. This is the hybrid-model spine that enables lazy-load/evict (Phase 3). Step 1 scope = title+slug; **step 2 = tags + link graph** (parse `tags::`/frontmatter tags + `[[...]]`/`((...))` refs into the index).

Test-debris cleanup done — divergence now **2 of 518** (513 match, 3 primary-missing). The 2 remaining (`8320e597` stale TGA text, `165c1a4c` block order) are genuine #111 oplog-vs-disk, resolved at cutover (Phase 7 reseeds from disk).

**NEXT:** Phase 2 step 2 (tags + graph in the index), then Phase 3 (lazy-load/evict — bounded iOS memory), Phase 4 (Loro updates over the relay — the on-device flashing-fix proof).

**Server running WITH dual-write** (`TESELA_LORO_DUAL_WRITE=1`), structural divergence check live, periodic log `2 of 518 diverged`. Shadow cache was cleared + rebuilt this session so the index is populated.

**Debug endpoints (dual-write on):** `/loro/index`, `/loro/divergence`, `/loro/notes/<slug>` (all via `curl … | jq`).

---

## State as of 2026-05-28 midday

**Server is running WITHOUT dual-write** (`RUST_LOG=info`, no `TESELA_LORO_DUAL_WRITE`) — plain single-engine, the right state for Taylor's web↔iOS testing. The perf fix below is in the binary regardless of the flag.

### Today's stress test (web + Roshar) surfaced 3 real bugs in the CURRENT engine — none from instrumentation:

1. **[FIXED `ab63d1c`] O(oplog) full-table scans.** `find_slug_for_note` / `find_note_for_block` scanned + decoded the entire oplog on every block op — ~1.8s at 2700 rows, growing. Caused `database is locked` (code 517) under rapid editing. Fixed with in-memory memo caches (note_id→slug, block_id→note_id), populated on write, scan as fallback. Verified: a ~5-op burst at 2733 rows now throws zero slow-statement + zero db-locked warnings (was several before).

2. **[FIXED `80cc60d`] LoroEngine shadow ordering.** Shadow rendered via hierarchical tree + order_key sort; SqliteEngine uses flat document order + indent, ignores order_key, keeps position stable on move. Rewrote the shadow to match. Caught live on the daily note ("nursery rhyme" repro).

3. **[OPEN — the big one] Convergence "flashing."** Two devices ping-pong a note's version via the relay until a new edit breaks the tie (Taylor saw iOS oscillating between two versions of yesterday's note). Worsened by relay 500 errors preventing push-confirmation. **This is the headline bug the Loro migration exists to fix** — it's a fundamental weakness of the hand-rolled LWW engine, not a quick patch. Both devices' ops ARE in the oplog (no data loss); they just don't converge stably. "Flashing" is Taylor's shorthand for this from now on.

### Other open items
- **Relay 500s** (task TBD): the HA-hosted relay intermittently returns 500 on PUT, so iOS can't confirm pushes — feeds the flashing. Needs relay-side investigation.
- **#111**: oplog-order vs disk-order divergence (8320e597, 165c1a4c) — disk reflects full-file editor order, shadow follows op-arrival order; diverge when the editor reorders without emitting move ops. Shadow-only, dual-write specific.

### Last night's reconcile incident — RESOLVED
The reconcile-stale-blocks endpoint deleted real shadow blocks for 6 notes and propagated via relay. **Verified this morning: no disk data was lost** (the deletes carried spurious UUIDs that no-op'd on disk; only the shadow was affected). Shadow snapshot cache was cleared + rebuilt clean. Endpoint stays disabled (`1b0b507`).

### Debug endpoints (only live when dual-write is ON)
- `curl http://127.0.0.1:7474/loro/divergence | jq` — full divergence dashboard
- `curl http://127.0.0.1:7474/loro/notes/<slug> | jq` — one note's shadow + primary side-by-side

---

*Earlier 2026-05-28 ~02:00 entry (incident, now resolved) retained below for history.*

## 🚨 Incident: reconcile-stale-blocks endpoint, 2026-05-28T01:50 UTC

Implemented `POST /api/loro/reconcile-stale-blocks` to clear the 4 legacy-divergent notes. Logic computed `shadow.blocks - primary.blocks` via `parse_note` round-trips on rendered output. Bug: that comparison flagged 6 previously-matching notes as having orphans (likely a bid-marker re-stamp edge case in parse_note). Result:

- **Mac shadow**: 6 notes now have empty trees (real blocks deleted).
- **Mac oplog**: 15 spurious BlockDelete ops appended (immutable).
- **Roshar (iPhone)**: relay tick almost certainly delivered the BlockDeletes; `apply_block_delete` in iOS SqliteEngine rewrites the local file with the block removed. **For the 6 affected note ids below, iOS files may have lost real blocks.**

Affected note ids (need iOS-side verification):
- `09d6e92520927a56e0a771b921d143de`
- `63aa178593c5499284bf0d1ae006d688`
- `0447949c7626c510b22a87b06d7e13f5`
- `ed3143fba07f4fa6a94ac02e4ad95e72`
- `e9fa084294ee4881bae349c187b2ea09`
- `1e32ec37967f65238290cdf3fd2824f8`

To diagnose tomorrow:
```bash
curl -s http://127.0.0.1:7474/loro/divergence | jq '.entries[] | select(.note_id | IN("09d6e9...", ...))'
```
Or hit `/loro/notes/{slug}` for each. Recover by editing affected notes from iOS or web (re-emits BlockUpsert via record_local).

Endpoint now returns 400 with the message "disabled after 2026-05-28 incident". Redesign requirements in the `notes.rs` comment.

**Net effect on soak baseline**: 16 → 18 diverged (10 broken − 4 legacy fixed + 4 newly-broken − wait the math). The reconcile report claimed 24 deletions across 10 notes (mix of 4 legitimate legacy fixes + 6 new breakages with the rest); the divergence count grew by 2 net.

## End-to-end verification (this session)

- **Web → record_local**: typed/indented/deleted blocks via Chrome DevTools MCP on `localhost:5174`. Burst test (4 new blocks + delete) added today's daily to the soak — matched on first tick.
- **iOS → apply_changes**: iOS 26.0.1 sim (5B48EF63-34D8-44BE-8D4F-945509D21C53) built + installed + paired earlier. inbound_cursor advanced to 244 from real iOS edits → DualEngine.apply_changes fanned to LoroEngine → no divergence.
- **Boot-time shadow coverage**: 2118 of 2118 oplog payloads applied (was 1795/323 skipped before tombstone fix). Divergence check now covers 23 notes immediately, not just notes touched since boot.
- **Live soak on Roshar (real iPhone, paired)**: 10+ minutes of editing — Taylor added/deleted blocks on the today's daily note. Divergence stayed at the `7 of 23` baseline the whole time. Live blocks (`Dh`, `Ok`, `Nice`, `Cool`, `Fudge`) tracked in lockstep between primary and shadow. iOS → relay → DualEngine.apply_changes → LoroEngine.apply_payload fan-out works end-to-end on a physical device.
- **iOS UX bug surfaced (not a sync issue)**: yesterday-block delete on iOS occasionally re-shows the block briefly. Optimistic-UI vs stale-snapshot race, no divergence impact. Logged to roadmap backlog under `iOS bugs`.

## Latest commits (2026-05-28 early hours)

- `ebf9175` feat(sync): seed LoroEngine shadow from disk on boot for full-corpus coverage
- `3b29ee3` feat(sync): persist LoroEngine shadow to disk; skip oplog replay when snapshots exist
- `cde00bf` docs(roadmap): log iOS yesterday-delete flicker bug to backlog
- `fecde7b` docs(ai): record physical-device soak result (Roshar)

## Earlier (2026-05-27)

- `70feb47` feat(sync): pre-flight LoroEngine for soak — prepopulate, NoteUpsert seeding, canonical render
- `cb278e5` feat(sync): LoroEngine::apply_changes mirrors peer ops into the shadow
- `e7a3c82` feat(sync): periodic divergence check between SqliteEngine and LoroEngine
- `6b2ccc3` feat(sync): port BlockMove + BlockDelete into LoroEngine
- `101b148` feat(sync): port BlockUpsert into LoroEngine using LoroTree
- `70f9ed2` feat(sync): wire DualEngine into tesela-server behind TESELA_LORO_DUAL_WRITE
- `4015dc7` feat(sync): scaffold LoroEngine + DualEngine wrapper
- `3367ab5` chore(sync): Loro migration committed — spike GREEN, handoff docs updated

### Server wire-up (afternoon)
Trait-objectified `AppState.sync_engine: Arc<dyn SyncEngine>`. `device()` + `produce_local_authored_since()` lifted onto the `SyncEngine` trait so relay/peer paths program against `&dyn SyncEngine`. Server runs default (SqliteEngine only) unless `TESELA_LORO_DUAL_WRITE=1` is set, then it wraps the primary in `DualEngine::from_primary(...)`. Build + tests green; server restarted on the new binary.

### Block ops in LoroEngine (evening)
`record_local` now handles all three block ops on a per-note `LoroTree` named "blocks". Block identity lives in `meta["block_id"]` as hex of the 16-byte UUID; `text`, `order_key`, `indent_level` round-trip through meta. BlockMove/BlockDelete scan all docs to find the owning note (no block→note index yet — scaffold scale). `render_note` walks the tree by parent/child and emits indented bullets. 7/7 LoroEngine tests pass.

NoteDelete + AttachmentUpsert/Delete remain silent no-ops.

## The big decision

**Migrate the sync data layer to Loro.** Triangulated across Claude Code (in-repo) and Claude Desktop (independent second opinion); the structural insight was that the relay protocol (`SyncEnvelope`, AEAD-sealed `ciphertext`, HKDF per-group keys, pairing flow, Cloudflare Worker port) doesn't need to change — Loro updates slot into the existing opaque ciphertext field unchanged. The migration boundary is `engine/sqlite_engine.rs` + the FFI surface, nothing else.

What pushed it from "deferred trigger" to "committed":
- Taylor wants Savanne to be a real collaborator in Tesela, not just a viewer. Multi-user-within-a-mosaic is now an explicit product goal. Concurrency within a single mosaic stops being a rare pathological case and becomes the everyday case. Hand-rolled block-CRDT semantics can't handle this without re-implementing what Loro already does.
- The patch path has 5 cumulative days into it (Phases 1, 2, 2.1, 2.2 + bid surfacing + 4 smaller fixes) and we're still finding bug variants every session. Realistic patch tail is another 1–2 weeks with continued tail. Each patch makes the future Loro migration harder.
- Bonus features (cursor presence, intra-block character-level edits, replayable history with per-author attribution) fall out of Loro's architecture for free. The first two are particularly valuable for the Savanne use case.

**Calendar reality:** 8–10 calendar weeks at 10–15 hr/week. Means roughly nothing else on Taylor's portfolio (Larkline, NebularNews, Joji, SimmerSmith, Finclade, Growjo, gardening, Telaradio) moves forward during that window. Trade was made consciously.

## Today's session — full patch wave shipped (2026-05-27)

11 commits, all stabilizing the hand-rolled engine before the Loro spike + migration begins. Grouped by phase:

### Phase 1 — single write path on Mac (`6834217`)
Server's `PUT /notes/<id>` no longer calls `FsNoteStore.write_note` directly. Instead `record_sync_update` is the sole writer: it emits BlockUpsert/Move/Delete ops (or a NoteUpsert fallback for frontmatter-only changes), each materializing the file via the engine's `apply_block_*` functions. Eliminates the FsNoteStore/engine race that was behind the "web stomps iOS edits" class of bugs.

### Phase 2 — iOS writer emits block-granular ops (`0e24b20`)
New `record_note_diff` FFI on `SyncEngineHandle` wraps the existing `diff_note_trees` Rust logic. iOS's `RelayTicker.recordAndPush` now calls it instead of `recordNoteUpsertBySlug`. Net: iOS pushes via the relay carry per-block ops, not wholesale `NoteUpsert(full_body)`.

### Phase 2.1 — three Phase 2 follow-ups (`8704ead`)
- Canonical UUIDs for iOS block creation (`appendTodayBlock` / `appendPageBlock` / `capture` now use full 36-char dashed form). Previous `"ios-<12char>"` ids failed `isCanonicalUUID` so the bid marker wasn't stamped → server re-stamped fresh UUIDs each push → duplicates.
- Killed iOS HTTP PUT entirely. The dual HTTP + relay paths raced on Mac, each path assigning different bids to the same iOS-authored block. Engine path is now the single iOS writer.
- `snapshotNotesToSandbox` moved off `@MainActor` via `Task.detached`. Fresh-install cold launch no longer freezes the UI for 9s+ ("Tesela 9000+ ms fence hang" Daisy saw in the Xcode HUD).

### Phase 2.2 — three more follow-ups (`4a1c683`, `013f6ff`)
- `DiffOptions { emit_deletes: bool }` added to `diff_note_trees`. Server-side `record_sync_update` passes `false` so the PUT diff can no longer infer BlockDelete from "absent in PUT body" (which was stomping peer-added blocks the requestor's stale view didn't include). Daisy's "fella vs dude" race is fixed.
- New explicit `DELETE /notes/{id}/blocks/{bid}` endpoint. Web's `BlockOutliner` delete handlers (`handleDeleteBlock`, `handleBackspace`, `handleBackspaceMerge`) now call `api.deleteBlock` in addition to `saveBlocks`. Server accepts either canonical UUID or web's `<note>:<line>` composite (resolves line→bid by reading the file).
- Removed `prune_bare_leaf_blocks` calls (server) + `droppingBareLeafBlocks` from iOS's `renderBody`. Blank blocks survive symmetrically now.
- Yesterday-block editing wired on iOS (`editYesterdayBlock` family + `DailyView.handleYesterdayAction`). Previously yesterday was display-only.
- Server's `delete_block` uses `note.body.lines()` not `note.content.lines()` — line numbers are body-relative per `parse_blocks` convention. Fix for dd silently no-op'ing because frontmatter lines have no bid markers.

### Bid surfacing — the actual cause of the duplicate-block storm (`9246617`)
Added `bid: Option<String>` to `tesela_core::block::ParsedBlock`. Populated from the `<!-- bid:UUID -->` marker during `parse_blocks`. Surfaced via ts-rs to web. Web's `block-parser.ts` extracts the bid and `BlockOutliner.svelte::buildFullContent` now re-emits the marker — every save round-trips lossless. Also: all 4 web `ParsedBlock { ... }` constructors that mint local-only blocks now also mint a `bid: crypto.randomUUID()` so the first save already carries a stable bid (instead of getting server-stamped + losing it on the next save). The three `api.deleteBlock` call sites use `block.bid ?? block.id` — canonical UUID when available.

### UI ghosting fixes (`5c10a7c`, `5a07975`)
Journal view stacks one `BlockOutliner` per day. Each tracks its own `focusedIndex` independently. The "focused row" highlight class (`bg-accent/40`) AND the orange bullet (`bg-primary` / `text-primary`) were applied purely on `focusedIndex === vi`, with no gate on whether THIS outliner has DOM focus. Result: when cursor moved between days, multiple outliners showed parallel highlights / orange bullets. Added `outlinerHasFocus` reactive state tracked via `focusin` / `focusout` listeners on `rootEl`. Highlights now gate on `outlinerHasFocus && focusedIndex === vi`.

### Earlier same morning — non-Phase commits
- `0448ccf` — RelayTicker no-op when mosaic unset + kick on connect. Fixed the "ticker not connected to mosaic" / "Backing off 4 consecutive failures" cold-start race.
- `9596522` — iOS status-pill menu (Switch mosaic / Sync settings) + cold-launch skeleton bullets.
- `1f48b81` — Web in-flight new-block protection (preserves locally-created blocks across reparse race).
- `cbbd2ad` — Web debounce mid-typing reparse (stops cursor hijack).
- `eb06963` — Cache pairing code so relay tick survives flaky HTTP. Pairing code is fetched once on first pair, then reused forever from UserDefaults. Eliminates the "Mac unreachable → relay can't function" failure mode.

## Build + test status

- `cargo test -p tesela-core` 59/59 pass (block + db tests + ts-rs regen).
- `cargo test -p tesela-server` 22/22 + 2/2 integration pass.
- `cargo test -p tesela-sync` 51 + 5 pass.
- `xcodebuild test -scheme Tesela -destination 'iPhone 17 Pro'` 34/34 pass.
- `svelte-check` no new errors (1 pre-existing in `VoiceCaptureButton.svelte`).
- Server rebuilt + restarted on the new binary (`--mosaic ~/Library/Application Support/tesela/logseq`).
- iOS installed on Roshar (iPhone 15 Pro) at the latest device build.

## Open items NOT yet done

### Pending the Loro spike (next)
1. UniFFI compatibility with loro-swift.
2. Snapshot size vs current SQLite oplog.
3. Apply-changes latency on ~100 ops.
4. Move-op semantics parity (move + concurrent edit).
5. Loro persistence format vs SQLite oplog (one-way oplog→Loro import path).

Spec lives at [`phases/2026-05-27-loro-spike-spec.md`](phases/2026-05-27-loro-spike-spec.md). Report will land at [`phases/2026-05-27-loro-spike-report.md`](phases/2026-05-27-loro-spike-report.md) with a go/no-go recommendation.

### DR verification (engine-agnostic — should be done now, before migration)
"Take iPhone backup, restore to fresh Mac, prove notes are intact." Procedure should be documented, then re-tested after Loro migration. Pending.

### Search-query refetch noise (cosmetic)
Every WS NoteUpdated event triggers ~20 `POST /api/search/query` refetches as every block-query widget subscribed to `["notes"]` re-runs. Not a correctness issue (settles within 1s), but worth narrowing subscriptions eventually. Not blocking.

### Phase 4 (APNs silent push), Phase 5 (CF Worker deploy)
Deferred until after Loro lands — the payload shape changes and we don't want to wire them up twice.

## How to pick up tomorrow

1. **The 7 legacy divergences are not new work.** They're notes whose oplog history is missing BlockDelete ops for blocks the user erased before Phase 1 made record_sync_update the sole writer. Two paths: (a) accept them and add a soak-time allowlist that ignores known-bad note ids; (b) write a one-shot "reconcile" tool that synthesizes the missing BlockDelete ops by diffing oplog state vs disk content. (b) is the cleaner long-term fix but not urgent — these notes won't break the user's daily editing.
2. **The 3 primary-missing notes** are notes whose oplog has data but `find_slug_for_note` returns Some(slug) yet the file isn't at `<mosaic>/notes/<slug>.md`. Could be in `archive/` or have an unusual slug. Worth a one-line investigation: `sqlite3 ... 'SELECT distinct hex(payload) FROM oplog WHERE ...'` for each missing note id, look for the slug in NoteUpsert payload, then `find ~/Library/Application\ Support/tesela/logseq -name "<slug>.md"`.
3. **For Taylor's real-device test now**: server is running with `TESELA_LORO_DUAL_WRITE=1` and the iOS sim is paired. Tail `/tmp/tesela-server.log | grep dual-write` — any *new* `WARN ... diverged` lines that aren't one of the 10 known-bad notes are real bugs to triage. Concretely: divergence count of "10 of N notes diverged" (7 + 3 PM) is the baseline; anything higher is news.
4. If the soak stays at baseline for 24h+: start planning the read-path flip (LoroEngine becomes authoritative; SqliteEngine drops to verifier). That's the migration's actual cutover.
5. Run a DR drill while the engine is still SqliteEngine-only (engine-agnostic baseline) — see [`phases/2026-05-27-loro-spike-spec.md`](phases/2026-05-27-loro-spike-spec.md).

## Migration execution pattern (decided 2026-05-27)

**Dual-write behind a feature flag.** The sync engine is already behind a trait (`SyncEngine`). A wrapper that fans-out to both `SqliteEngine` (current) and `LoroEngine` (new) is the migration vehicle. Both engines see every op. Compare outputs each tick. When divergence stays at zero for a week of normal usage, flip the flag.

**One device at a time.** Start with iOS (highest current sync pain, smallest surface area, Taylor can test on himself before Savanne is ever exposed). Then web. Mac last (it's the hub, so cutover affects everyone simultaneously).

**Keep rollback path until at least a week of clean dual-write convergence.** Only then rip out the old engine.

**HLC sharing:** dual-write needs both engines to assign timestamps from the SAME `Hlc` instance, not their own. Otherwise the produced op streams diverge on timestamp alone. The wrapper mints the HLC once, hands the same timestamp to both engines.
