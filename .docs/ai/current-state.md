# Current State

*Last updated: 2026-05-27 night. Pre-flight pass landed: shadow now prepopulates from oplog at boot (~2118 ops), renders via the same `serialize_note` SqliteEngine writes, NoteUpsert seeds the tree from parsed content. Soak shows 13 of 23 touched notes match, 7 remaining diverge are legacy data (pre-engine direct-file writes that left no BlockDelete ops); new edits add zero divergence.*

## End-to-end verification (this session)

- **Web → record_local**: typed/indented/deleted blocks via Chrome DevTools MCP on `localhost:5174`. Burst test (4 new blocks + delete) added today's daily to the soak — matched on first tick.
- **iOS → apply_changes**: iOS 26.0.1 sim (5B48EF63-34D8-44BE-8D4F-945509D21C53) built + installed + paired earlier. inbound_cursor advanced to 244 from real iOS edits → DualEngine.apply_changes fanned to LoroEngine → no divergence.
- **Boot-time shadow coverage**: 2118 of 2118 oplog payloads applied (was 1795/323 skipped before tombstone fix). Divergence check now covers 23 notes immediately, not just notes touched since boot.
- **Live soak on Roshar (real iPhone, paired)**: 10+ minutes of editing — Taylor added/deleted blocks on the today's daily note. Divergence stayed at the `7 of 23` baseline the whole time. Live blocks (`Dh`, `Ok`, `Nice`, `Cool`, `Fudge`) tracked in lockstep between primary and shadow. iOS → relay → DualEngine.apply_changes → LoroEngine.apply_payload fan-out works end-to-end on a physical device.
- **iOS UX bug surfaced (not a sync issue)**: yesterday-block delete on iOS occasionally re-shows the block briefly. Optimistic-UI vs stale-snapshot race, no divergence impact. Logged to roadmap backlog under `iOS bugs`.

## Latest commits (2026-05-27)

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
