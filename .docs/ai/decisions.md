# Architecture Decision Records

Concise log of non-obvious decisions. Newest first.

---

### 2026-07-07 — Query & Views feature set shipped (epics ya4 + vp9 closed); the calls that outlived the specs

Both epics landed in one orchestrated day (10 beads, 7 worktree agents, Lead-reviewed merges, per-merge platform verifies). Design homes: `phases/2026-07-02-typesystem-views-spec.md` (views) + `phases/2026-07-07-jql-authoring-spec.md` (JQL authoring). Decisions made DURING landing, not in the specs:

- **Chip surface strings are cross-platform-canonical in infix JQL** (`is != heading`, not `NOT is:heading`). Both forms canonicalize to the same predicate (shared NOT/cmp inversion), so this is cosmetic-but-locked: chips must insert identical text on web and iOS since the strings persist in synced saved views.
- **QueryWidgetView's row-list died with ya4.3** — and with it the direct-opened-Inbox-note triage chords/status-cycle (the machinery was reachable only there; GrInbox lists + agenda keep theirs). Deliberate removal, not an accident: one table component was the acceptance bar. Parity follow-up: `tesela-9b1`.
- **iOS never writes table column config** (v1): stored `display_table_config` is honored read-only on iOS; header-tap sort is session-local. The web editor owns the stored config (mirror of decision 4's round-trip-authority, scoped per-platform).
- **The conformance fixture is the drift tripwire and it worked**: extended 182→217 cases with ZERO cross-engine disagreements, but the audit surfaced two REAL shared bugs — decimal literals silently truncate in all three tokenizers (`tesela-jow`, lead-floor: semantics change) and the Untagged chip filter is a no-op because `has:tag` never checks `block.tags` (`tesela-0rc`). Both pre-existing, both pinned in the fixture as quirk cases until fixed.
- **Fleet-ops lesson (recurred 4×): Sonnet agents park themselves on background builds despite FOREGROUND instructions** — every long-running dispatch prompt needs the foreground clause, and the orchestrator should expect one nudge per iOS agent. Also: agent worktrees with Rust targets eat ~5-15GB each — remove them at merge (`worktree-disk-pressure` memory; disk hit 99% mid-day).

### 2026-07-07 — tesela-baa ships as a MULTI-DOC registry: extend the client-replica splice model to every mounted editor; server-side splice RPC rejected

A 6-reader mapping pass found the bead's premise stale: web/desktop ALREADY had true per-keystroke splice collab (C2.2/C2.3, 68f8100e/faeffdd4, 2026-06-04 — client wasm LoroDoc → TLR2 WS → `apply_inbound_delta`), but keyed to a process-wide SINGLETON bound to the shell's focused buffer, and the journal's default daily buffer resolves to TODAY only. That singleton scope IS the 9iy storm mechanism on the authoring side: typing into the Jul-2 daily on Jul-3 never spliced — it fell back to 500ms whole-block writes (`write_block_text`), producing the monolithic 105-char rewrite the investigation found. Decisions:

- **Extend the client-owned-replica model (`NoteDocRegistry`, ref-counted, one doc per note with a mounted editor); acquisition moved from the shell's focus effect to `BlockOutliner` mount** — every editing surface (journal days, drawer tabs, tag pages, GrPage) acquires its note's doc, so binding no longer depends on focus at all. Spec: `phases/2026-07-07-per-edit-splices-spec.md`.
- **Server-side splice RPC (wiring `splice_block_text` into a route/WS message) REJECTED for web/desktop:** raw `(offset, delete_len, insert)` triples computed against a remote, concurrently-mutating doc are unsound; iOS gets away with them only because its FFI engine IS its local replica. A remote client doing char-level collab must own a real CRDT replica. (Server unchanged: `apply_inbound_delta` already routes per-note and takes the apply-lock.)
- **Vim undo/redo route via editor-id-guarded focused-doc tracking** (Vim actions are a global registry; the focused BlockEditor sets its note's doc as the undo target).
- **Adversarial review (3 lenses → refute pass) confirmed two majors in the first cut, both fixed + regression-tested:** (1) the outbound cursor advanced past sends `sendBinary` silently dropped while the WS was down — stranding keystrokes forever (worse post-baa: newly-bound surfaces skip the durable HTTP fallback). Now: `send` reports handoff; cursor advances ONLY on success; docs released while unsendable are PARKED at zero refs and drained by `flushAllOutbound()` on WS reconnect. (2) release/flushAll flushed with a null cursor, exporting a merely-viewed note's ENTIRE oplog (bootstrap included — MBs for big notes). Now: cursor baselines to the post-bootstrap version at open + flushes are dirty-gated (remote-only imports never re-broadcast).
- Deferred, filed: engine write-tail debounce `tesela-ofu` (O(note) snapshot+materialize per import — pre-existing, now hotter); journal scroll-depth doc retention `tesela-bjp` (grow-only mountedSections × per-mount doc); loro `get_or_create_container` deprecation / container-twinning investigation `tesela-65f`.
- Verified: 14 registry unit tests (fake models full-history-export + droppable sends), 599 web unit total, svelte-check clean, cargo sync+server green, and a new Playwright two-context same-block storm e2e (per-character survival + convergence — the exact 9iy shape).

### 2026-07-05 — Conductor triage fields live in bd METADATA, never prose (verify_cmd is metadata-only; Arena/cycle silently Untriage prose-triaged beads)

Arena/cycle dispatch refuses untriaged beads. Source of truth: `harness-conductor/src/fields.rs` (+ `arena.rs:716` → `"{} has no verify_cmd metadata; arena requires self-certifying beads"`). The field-resolution rules:

- **`tier_floor`** (`lead`|`senior`|`junior`) + **`complexity`** (`S`|`M`|`L`|`XL`): bd **metadata** preferred; **notes** is a fallback for these two only (a notes range like `S-M` resolves to its upper bound `M`).
- **`verify_cmd`**: bd **metadata ONLY** — notes prose like `Verify: cargo …` is INERT, never scanned. Description text is inert for ALL three fields; a `Verify:`/`tier_floor:`/`complexity:` line embedded in DESCRIPTION or NOTES does nothing.
- Per field: metadata first; if metadata is present but invalid (e.g. `tier_floor=boss`), the item is Untriaged — fail closed, no notes fallback for that field.

**Observed gap (2026-07-05 Arena run on tesela-3tm):** 0 of 38 ready Tesela beads carry Conductor metadata triage — and neither do the closed wave-dispatched beads (pfix.2, ows.1, y11 all `metadata: None`). Triage prose (`tier_floor: senior | complexity: S | Verify: xcodebuild test`) was embedded in DESCRIPTION/NOTES, which Conductor ignores → every bead is Untriaged → never dispatched by Arena and silently Untriaged for cycle. The first Arena run failed preflight `arena: tesela-3tm is untriaged; missing [TierFloor, Complexity]` until metadata was set; `tesela-ows.4` was skipped for the same reason (its triage lives in DESCRIPTION).

**Convention LOCKED:** when an item enters the Tesela backlog, set all three as bd METADATA — `bd update <id> --set-metadata=tier_floor=… --set-metadata=complexity=… --set-metadata=verify_cmd="…"`. Prose triage in description/notes is fine for human readability but is NOT parsed — metadata is canonical. `verify_cmd` must be a real, single-command, runnable gate (confirm green on `main` before setting it; for xcodebuild-dependent beads remember the `.xcodeproj` is gitignored + needs FFI cross-compile + xcodegen first — prefer a wrapper or a non-xcodebuild gate where possible). Notes may carry tier_floor/complexity as a secondary signal but verify_cmd MUST be in metadata.

**Backfill:** the 38 open + closed beads need metadata populated — filed as bead `tesela-1tc` (Lead triage pass, this decision's implementer). Until then, Arena pick-up-and-go is blocked: each run requires a Lead to set metadata on the chosen bead first (acceptable — the orchestrator is Lead; this is the triage step that should have happened at bead-creation time).

### 2026-07-03 — Same-block convergence: keep-winner EXONERATED; the fix axis is delivery (c7s) + per-edit splice authoring (baa)

The 2026-07-03 same-block data-loss incident (two devices typing in one block; minutes of divergence; one side destroyed) was adversarially investigated from full frozen doc history (probe: `probe_incident_9iy.rs`, branch `w7-conv`) + live relay traffic, second-opinioned by gpt-5.5 (approve-with-nits). Durable conclusions:

- **NO disjoint twins spawned — the fte pure max-TreeID keep-winner rule never fired and is exonerated for this class.** Do NOT re-litigate keep-winner semantics on same-block loss reports; the losing side's ops never entered the shared doc at all.
- **The dominant real-world loss class is OUTBOUND DEPOSIT STRANDING**, not CRDT merging: a stranded outbound cursor (documented 2026-06-25 undecodable / 2026-06-29 stale-ahead classes) ships empty frames forever; the build-57 snapshot fallback deposits but never repairs the cursor, and peers never read snapshots outside bootstrap/catch-up (= app restart). Live signature: steady `GET ops?since=N` from all devices with the head frozen, zero `PUT /ops`, looping `PUT /snapshot`.
- **Fix architecture (bead tesela-c7s, in flight):** sender-side cursor repair after a confirmed snapshot deposit MUST interlock with a receiver-side DURABLE pending-import ledger + auto snapshot catch-up — an inert snapshot PUT is never delivery by itself; resumed incremental ops cause a causal gap on peers which auto-heals via catch-up. Acceptance = a peer converges WITHOUT restart from the strand state.
- **Char-level same-block merging additionally REQUIRES per-edit splice authoring on web/desktop** (bead tesela-baa, Lead design, blocked on c7s): whole-block minimal-diff writes (`write_block_text`) preserve more than LWW but cannot give the "keystrokes interleave" product expectation. Delivery (c7s) is necessary; splice authoring is the sufficiency half.
- Op timestamps turn on for real local authoring only — the builtin-views seed MUST stay `ts=0` deterministic (fresh-device-clobber invariant).

### 2026-07-02 — Engine-side block lifecycle (tesela-ows.1 step 2): roll into the CONTAINER, hook in apply_import, guard on the earlier occurrence

Wired the recurrence bump + same-note dependency unblock into the engine apply path so every writer that IMPORTS a `done` flip (WS live-apply, relay, iOS `.relay`) triggers them — previously only `tesela-server`'s HTTP PUT handler did. Step-1 shipped the pure fns in `tesela-core::lifecycle`; step 2 is the engine wiring. Attempts 1–2 died on a data-loss class + a vacuous test; this is attempt 3 under Lead constraints.

- **Hook point = `apply_import` (post-merge, post-twin-heal, pre-snapshot/materialize), NOT `record_local`.** `apply_import` is the single orchestrator behind every import wrapper, and the acceptance is a flip delivered via relay/import. `record_local` (the LOCAL author path) is deliberately NOT hooked: the HTTP handler already rolls there (zero-behavior-change requirement) and hooking it would double-fire. Gated on a real non-done→done flip that `peer_import_plan` detects on the SAME auth-doc fork it already makes for the twins plan, so the common text-edit delta pays no extra markdown render (Lead constraint b). **Known gap:** a fully-offline single device that flips `done` via FFI won't roll until a peer imports+bounces the roll back — a locally-authored lifecycle is a separate follow-up (bead).
- **Constraint (a) — lifecycle props STAY in the typed props container.** The roll is authored as CONTAINER `BlockPropertySet` sets (the same mechanism `reassert_prop_heals` uses), NEVER as an in-text markdown rewrite, and the container is NEVER cleared. `last_completed`/`recurrence_done`/rolled dates/`status` live in the container where disjoint-twin heal's per-key union (`reconcile_orphaned_prop_containers`) protects them. **Why attempt 2 died:** it evicted these to in-text lines (clearing the container so the roll would render); twin-heal unions CONTAINER props only, so a max-`TreeID` pick landing on the non-rolling twin silently WIPED completion memory. Keeping the roll in the container renders correctly for free — the container value wins render-time dedup (`dedup_intext_props_against_container`) — with no clearing, no render-side change. New pure fn `compute_lifecycle_container_sets(prev, next, note_id)` runs the guarded bump + dependency cycles then diffs `next` vs post-lifecycle at the lifecycle-owned-key level, returning per-block container sets keyed by canonical bid.
- **Idempotence guard anchors on the EARLIER of prev/next occurrence date (`min`), not prev-only.** The completed occurrence is the earlier anchor: a concurrent peer's roll in the same merged frame advances ONE side's `deadline::` past the occurrence being completed — the advance can land on `next` (pre-import peer is behind) OR on `prev` (pre-import peer already rolled, importing a stale disjoint duplicate). eb0de36d's prev-only anchor fixed the first direction but double-bumps the second; `min` unifies both. Makes the crossed-duplicate case converge to a single bump regardless of twin-heal union order (no dependence on which node the max-`TreeID` rule keeps).
- **Double-fire idempotence (HTTP + engine):** HTTP handler rolls BEFORE feeding the engine (authors the already-rolled `todo` state), so the engine never sees a `done` flip from HTTP → hook doesn't fire → HTTP behavior byte-identical. Engine path: flip gate (non-done→done) + guard (skip when completion already recorded) + the roll authoring `status:: todo` (a re-imported bump frame shows done→todo via LWW, never a fresh flip) → exactly one bump.
- **Lock (tesela-4ju):** the hook runs inside `apply_import`'s `apply_locks` guard and authors via `record_local_locked` (non-reentrant guard — public `record_local` would deadlock), same discipline as `reassert_prop_heals`.
- **Tag auto-create NOT ported** (needs `NoteStore` for cross-note page creation — engine has no store seam); stays server-side, as in step 1.
- **Revert-discriminating acceptance** `relay_done_flip_triggers_recurrence_bump_once_and_converges` (tesela-sync) + data-loss regression `crossed_duplicate_completions_converge_single_bump_no_dataloss` + 5 min-anchor guard unit tests + 3 container-sets unit tests (tesela-core). Verify: `cargo test -p tesela-core -p tesela-sync -p tesela-server`.

### 2026-07-02 — Multi-user key hierarchy: three layers, device-roster v1 (ADR approved by Taylor)

Taylor approved the tp0.3 spike's direction (harness-deck 2026-07-02; full design: `phases/2026-07-02-multiuser-key-hierarchy-spec.md`). LOCKED direction — implementation stays GATED behind its own TDD + adversarial-crypto-review bar and does not start until Savanne work is scheduled:

- **Three layers.** (1) Content encryption UNCHANGED — one symmetric ContentKey per key-epoch (today's GroupKey; still BIP39-renderable, still random group_id). (2) Identity = per-member Ed25519 (promotes the dormant schema columns) signing roster changes and authored ops. (3) Distribution/authorization = a SIGNED, client-authored roster + the ContentKey WRAPPED to each member's key-agreement key — onboarding stops meaning "re-type the phrase".
- **v1 = per-DEVICE roster**; a "user" is a display-name label grouping devices. Wire/roster schema RESERVES the per-user cert tier (nullable user_id + device-cert slot) so upgrading later is additive — no re-key to introduce it. Do not build the cert chain in v1.
- **Dedicated X25519 key-agreement key per member** (clean signing/KEX separation); wrapping via a vetted sealed-box primitive (crypto_box-class crate), never hand-rolled.
- **Relay stays a pure zero-knowledge mailbox** — roster is client-verified; the relay enforces nothing new (topology lock intact).
- **Revocation** = roster removal + the tp0.1 rotation primitive, with the new ContentKey re-WRAPPED to remaining members — honest devices do NOT re-onboard by phrase.
- **Meanwhile constraint on ra7/P0 crypto:** nothing may assume phrase-retyping is the only onboarding path forever, and key-material handling must not preclude wrapping (no new places that persist the raw key outside the GroupKeyStore seam).

### 2026-06-27 — iOS-delete-needs-manual-desktop-refresh = a WEB reconcile bug; gate the own-echo skip on the CURRENT render

Taylor: an iOS block delete reached the desktop's DATA but the journal UI kept the block until a manual refresh (edits auto-showed; deletes didn't). Proved the engine + materialize correct first (committed test `relay_inbound_delete_updates_peer_materialized_file`), isolating it to the web.

- **Root cause** (`BlockOutliner.applyExternalReparse`): the own-echo fast-path skipped the reparse when `targetBody === lastSentBody`. `lastSentBody` only advances on a LOCAL save; an inbound ADD moves the rendered blocks off it, so a later inbound DELETE that restores `lastSentBody` byte-for-byte was mistaken for our own echo and dropped. **Fix (`38b6ac3b`): compare the CURRENT render — `buildFullContent(blocks).bodyOnly === targetBody`** (true no-op, preserves undo) + keep the mid-typing guard (`targetBody === lastSentBody && hasUnsavedLocalEdits()`). `lastExternalBody` is left untouched — it is the server-canonical PUT base (`baseForPut`); the clobber-safe author-diff depends on it NOT advancing on local saves.
- **Reproduced/verified RED→GREEN** via a live Chrome-DevTools-MCP repro against a `pnpm dev` + relay-off `tesela-server` on a COPY of the mosaic; instrumented `applyExternalReparse` to confirm the exact `eqSent=true` skip. A standalone Chrome client (or a fresh fixture mosaic) does NOT render the JournalView headlessly (v5 workspace-view state the populated mosaic has) — so the Playwright guard runs against a single PAGE note, which renders headlessly and exercises the SAME shared `BlockOutliner` reconcile.
- **Regression guard:** `pnpm test:e2e` (`web/tests/e2e/run.mjs` + `playwright.e2e.config.ts`) — self-contained (tiny fixture + relay-off server + dev + a seeded page); verified RED (old skip) → GREEN (fix). `51407e0b`.

### 2026-06-26 — desktop crash-loop = Loro richtext panic on a poison frame; contain, don't trust the apply

The desktop SIGABRT'd ~2s after every launch (crash loop). Root cause via crash report + a `tesela-server` reproduction (RUST_BACKTRACE): **loro 1.12 PANICS inside its own richtext apply** — `RichtextState::insert_elem_at_entity_index` index-out-of-bounds (`entity_index=4 len=2` on a "de" text chunk) — during `LoroDoc::import` of a specific inbound relay frame (note `e9624f2c…`), in the inbound apply path (`apply_relay_updates → apply_doc_update_status → peer_genuine_block_changes → fork.import`). Unguarded, the panic aborts the whole process; the desktop re-pulls the same frame every 5s relay tick → permanent loop. **Fleet risk:** any device pulling that frame crashes (shared `tesela-sync`).

- **The apply path must be POISON-SAFE — never trust a peer frame not to panic Loro.** Fix `cdb4a0ec`: `probe_import_poison` imports the frame into a FULLY INDEPENDENT copy under `catch_unwind`; a frame that panics (or errors) is skipped (returned as an apply error → bounded-retry), never applied. Gates both `apply_doc_update_status` + `import_doc_update`.
- **`doc.fork()` does NOT isolate — it shares the internal `LoroMutex`.** The first fix attempt forked + caught the panic, but the shared mutex was POISONED by the panic-while-locked; dropping the fork (or the next live-doc access) hit `expect_not_poisoned` → a NON-UNWINDING panic → abort anyway (exit 134). The probe must be a snapshot round-trip into a fresh `LoroDoc` (its own mutex). And the poisoned throwaway must be `std::mem::forget`-leaked — its Drop would re-lock the poisoned mutex and abort. Verified: `tesela-server` vs the live relay went exit 134 (crash) → exit 124 (survives, logs "SKIPPING … poison").
- **Mitigation while rebuilding:** set `desktop.toml relay_url = ""` (disables the embed relay → no inbound apply → no crash) + relaunch — a usable local-only desktop in seconds. Re-enable after the fixed build installs.
- **Caveats / follow-ups:** the poison note `e9624f2c…` stays FROZEN (its frame is skipped, never applies) — the underlying loro richtext-merge bug (concurrent same-block splices producing an OOB diff) needs a loro upgrade or an avoid-the-pattern fix. The per-frame snapshot probe has overhead — optimize later (gate by a risk heuristic). Possibly triggered/surfaced by the build-51 snapshot-fallback (a snapshot frame applied over a fork), but the containment is correct regardless of how the poison was produced.

### 2026-06-26b — convergence: upgrade loro (crash root-cause) + the disjoint-lineage root cause is non-mergeable containers

The containment stopped the crash but left the affected note(s) DRIFTED: the skipped merge never converges (observed block `019f047a` = desktop "Brook" vs iOS "Bro"). Diagnosed the two-layer root cause + fixed layer 1.

- **The crash is a loro 1.12 LIBRARY bug — fix by UPGRADING, not by hand-patching the merge.** loro 1.12 → **1.13.6** (`e884edc2`). The 1.13 changelog fixes exactly this class: 1.13.3 (`8d258cb`) the out-of-order-import panic; 1.13.2 (`7dfda87`) ATOMIC imports that ROLL BACK on state-apply failure (so a bad frame errors cleanly instead of poisoning the doc — kills the `panic_in_cleanup` abort at the source); 1.13.2 (`64aa97c`) broad import hardening. Verified: full tesela-sync suite green (166 + all convergence anchors), no API breaks. The upgrade also lets the existing dedup/heal CONVERGE already-forked twins (the merge now applies instead of crashing) → heals existing drift once BOTH devices run 1.13.6.
- **Couldn't recapture a deterministic poison fixture:** the relay COMPACTS old deltas into clean snapshots (deposit_snapshots), so re-pulling from cursor 0 reached seq 322 with zero poison frames — the relay self-heals poison deltas. The live OOB now only reproduces on a device's LOCAL disjoint twin. So verification = changelog + full suite + the dedup-converges tests + live device test (Taylor), not a unit RED.
- **The DRIFT root cause = disjoint lineages from NON-MERGEABLE child containers.** loro 1.13 DEPRECATED `LoroMap::get_or_create_container` (it "creates regular op-id children" that FORK across peers on concurrent first-write) in favor of `ensure_mergeable_*` (converge instead of forking). Our block `text_seq`/`props` containers use the deprecated method → two devices authoring the same block fork → the disjoint twins that crash/drift. The real no-data-loss fix is to swap to mergeable containers — but `ensure_mergeable_*` ERRORS on an existing non-mergeable container (verified in loro source), so the whole fleet needs a MIGRATION (delete+recreate re-forks per device). **Decision: do NOT rush it** — specced as layer 2 (`phases/2026-06-26-mergeable-containers-spec.md`); layer-1 lossy-dedup convergence is acceptable for one-device-at-a-time use. Ships: desktop rebuild + iOS build 53 on 1.13.6.

### 2026-06-25 — iOS→desktop push broken (zero relay PUT); diagnose before fixing

After builds 48/49 fixed liveness + APNs, a NEW symptom: iOS edits don't reach the desktop and desktop edits overwrite the iOS-authored block. Boundary-confirmed via `wrangler tail`: typing `PUSHTEST_IOS` on iOS produced **zero relay PUT** (desktop cursor frozen). So iOS edits are recorded locally but never pushed; the "clobber" is the consequence (the local-only edit is overwritten when the desktop's state comes down).

- **A 9-agent Workflow (5 parallel tracers → synth → 3 adversarial verifiers) traced the whole push pipeline.** Confirmed mechanism: the Graphite today editor pushes via a PER-KEYSTROKE splice seam (GrDailyView `onTextSplice` → `spliceTodayBlock` → `onLocalSplice` → `RelayTicker.spliceAndPush` → `engine.spliceBlockText`); `onCommitEdit` (blur) is a deliberate no-op. `spliceAndPush` **discarded** `spliceBlockText`'s op count (`_ = try await`, RelayTicker.swift:510); `spliceBlockText` returns **Ok(0) (not a throw)** when the block isn't a live tree node (loro_engine.rs:936) → a 0-op splice is silently swallowed (no PUT, no error). The sibling `setBlockPropertyAndPush` already guards this exact class (`applied==1` + surfaces error); the splice seam never got the guard.
- **BUT all 3 verifiers REFUTED that as THE cause** (high agreement): for a VISIBLE, desktop-bootstrapped block the splice should resolve (note_id = blake3(slug); bid read off the materialized `<!-- bid -->` = the live node's meta) → Ok(1) → version advances → exported → PUT. So zero-PUT means the loss is elsewhere: (a) stale/empty `serverDailyId` (midnight rollover 06-24→06-25 — there IS re-derive-on-refresh handling, may be incomplete), or (b) recorded-but-not-exported (outbound `broadcast_cursor`/produce), or (c) a real applied==0 splice miss.
- **Decision: OBSERVE before fixing** (iron law + 3 refutations; one candidate fix could create TWIN blocks on a bid mismatch). Build 50 (`824ed89a`) added a `lastSpliceDiag` ("Last splice" in Settings → Sync) capturing the previously-discarded `applied` + outbound `sent/failed` + the slug.
- **CONFIRMED via the on-device diagnostic: `slug=2026-06-25 applied=1 sent=0 failed=0`.** So the verifiers were right: NOT the splice-seam discard, NOT a stale slug — the splice records (version advances) but the outbound producer exports nothing. **ROOT CAUSE:** `produce_relay_updates` calls `export_doc_update(note, since=broadcast_cursor)`, which returned `None` when `since` failed to decode (corrupt / version-format change / stale lineage), and produce then **silently skipped** the dirty note (`if let Some(bytes)=…` with no else, loro_engine.rs:1017) → a note with a bad persisted broadcast cursor could NEVER re-broadcast; its local edits stranded forever, then clobbered by the desktop's version on the next inbound. **FIX `56d67001` (build 51):** `export_doc_update` self-heals — on a decode failure or an empty/failed incremental, fall back to a full `ExportMode::Snapshot` (idempotent on the receiver; the next confirmed PUT rewrites a fresh decodable cursor). A dirty resident note now ALWAYS exports; self-heals existing stuck notes on the next tick. RED test `produce_re_emits_when_broadcast_cursor_is_undecodable`.
- **Method note (worth repeating):** the 9-agent Workflow's value was the ADVERSARIAL VERIFY — it produced a confident-but-wrong synthesis (splice-seam discard) and its OWN 3 verifiers refuted it toward "recorded-but-not-exported," which the on-device diagnostic then confirmed. Don't ship a synthesis without the verify pass + a runtime observation. ~857k tokens, 9 agents.

### 2026-06-24 — iOS "data loss" was sync LIVENESS, not push logic; date chips must survive editing

Build 46/47 device test surfaced "iOS edits don't reach web for hours / look lost." Root-caused live against the desktop's CF-relay state, then fixed (`e6d1d83b`, `5c65e9d2`). Non-obvious calls:

- **It was never push LOGIC — edits converge; it's sync LIVENESS.** Diagnosed by curling the desktop's `/sync/relay/status` + daily notes while Taylor edited: the iPhone was on the SAME relay+group (inbound seq matched the desktop's), reads worked, and edits DID arrive — ~2h late, then instantly on app reopen. Two compounding bugs: (1) `RelayTicker` backoff slept `tickIntervalSeconds * (1 << min(consecutiveErrors, 12))` = `2 * 2^12 ≈ 8192s ≈ 2.3h` (every comment claimed ~60s; `maxBackoffMultiplier=12` was used as a SHIFT exponent, not a seconds cap); (2) `.active → start()` no-ops on an existing loop, so foregrounding couldn't wake a loop parked in that sleep — only a background→foreground cycle (which `stop()`s + rebuilds) recovered, hence "converged when I reopened the app." Fix: pure tested `backoffSleepSeconds` capping the RESULT seconds at 60; `wake()` (reset+stop+start → immediate tick) on `.active` in BOTH shells.
- **The desktop embed is loopback-only, so iOS↔Mac is 100% relay-dependent.** The Tauri desktop binds `127.0.0.1` and reads its relay URL from `~/Library/Application Support/tesela/desktop.toml` (`main.rs:163`); the mosaic `config.toml` has no `[sync.relay]`. So there is NO LAN HTTP path to the desktop app — iOS's "direct HTTP (LAN-fast)" mode is dead, relay is the only transport, and any relay-side stall = total iOS→Mac loss. (iOS still shows a stale `127.0.0.1:7474` "Connected" in relay mode — misleading; queued as a sync-UX-honesty follow-up.)
- **Date chips are STRUCTURED state, not prose — keep them visible while editing.** `BlockRow` gated the whole chip row on `!isEditing`; tags/inline props correctly hide during edit (they're in the editable text, would duplicate), but scheduled/deadline/recurring aren't in the text, so hiding them made a date set on a focused block look like it failed. Split via a pure `chipVisibility(...)` (dates ignore `isEditing`).
- **Diagnostic method worth repeating:** for a multi-device "where's my edit?" report, instrument the boundary — poll the receiving node's relay cursor + materialized note in the background while the user makes ONE labeled edit. It localizes the break (not-pushed vs pushed-not-received vs received-not-applied) far faster than reading the sync code.

**Delete-not-propagating (same day, build 48) → APNs background-wake gap.** A block deleted on web stayed on iOS ~44min until a force-close. Root-caused by ELIMINATION, not guessing:

- **Engine ✓ + refresh-chain ✓ ruled out by test.** Inbound apply DOES re-materialize (`apply_doc_update_status` → `refresh_note_derived` + `materialize_note`, loro_engine.rs:1063/69/74), and `applyRemoteChange → refresh` drops the block (new `MockMosaicServiceTests` guard). So neither was the bug.
- **Sim repro proved the foreground/running path WORKS on build 48.** Seeded a sim (`simctl spawn defaults write app.tesela.ios backend.mode relay` + `relay.cachedPairingCode <code from /sync/peer/pairing-code>`), paired to the live CF group; drove add+delete via the desktop API (`POST`/`DELETE /notes/{id}/blocks/{bid}`). A desktop delete dropped on the sim — engine file AND UI — in ~5s on warm `.active` reopen, ~16s on the running loop. ⚠ A sim can't reproduce true iOS background SUSPENSION (it keeps backgrounded apps alive) NOR APNs (no token) — those need a real device.
- **The real bug: APNs token registration wasn't relay-scoped.** While suspended, iOS does zero sync; the background wake is an APNs `content-available` push from the relay on deposit. `wrangler tail` on the live CF Worker showed ZERO `[apns]` on deposits → `listOtherApnsTokens` empty → the CF Worker had NO token for the group. Cause: `RelayTicker.maybeRegisterApnsToken` keyed registration by token ALONE, once-per-session (in-memory) — so after the HA→CF migration the token stayed registered with the OLD relay and never re-registered with CF in-session. A force-quit (fresh session) re-registered → tail then showed `registered device…` + `deposit → waking 1` + `push → 200 OK`. **Fix `594b0403`:** key registration by (token, `cursorScope`) via `apnsRegistrationKey` — re-register on relay change, mirroring the inbound-cursor scoping that fixed the same HA→CF class. Per-launch re-registration preserved.
- **Verify-on-CF gotcha:** `wrangler secret list` confirms the APNs secrets ARE set on the deployed Worker (they were fine); `npx wrangler tail --format pretty` from `cloudflare-relay/` shows the live `[apns]` logs — the fastest way to see whether a device is registered + getting pushed. `wrangler` is npx-only here (Taylor's CF OAuth).
- **APNs is best-effort, not a guarantee** — iOS throttles silent pushes. The RELIABLE path for "open phone → see current state" is the foreground sync (proven ~5s on build 48); APNs just narrows the asleep-and-not-looking window.

### 2026-06-23 — iOS property-registry parity: client-side registry, display-only raw-lines strip, typed composite writes, audit-as-process

iOS task/property handling was entirely hardcoded; Phase 5 made it the THIRD engine that consumes the type/property registry (spec `phases/2026-06-23-ios-property-registry-parity-spec.md`). Non-obvious calls:

- **Build the iOS registry CLIENT-SIDE from the synced Property/Tag pages, NOT `GET /types`.** The rich metadata — `nl_triggers`, `choice_colors`, `chip_*`, `chord_key`/`value_chord_keys` — lives ONLY in Property-page frontmatter; the API/DB (`get_all_property_defs`) carries only `name/value_type/values/default/show/hide_*`. So iOS mirrors web's `buildRegistry`/`getTagPropertyDefs`/`applyOverride` over the synced notes (the same code path works in `.relay` (no server), `.http` (pages already synced locally), and `.mock`). New `PropertyRegistry.swift` + a nested-YAML `FrontmatterParser` (the old single-line scrapers couldn't read `property_overrides: {...}` / `nl_triggers: [...]`). Perf: gate the nested parse to type pages, and rebuild OFF the main actor (it runs on the relay tick).
- **The raw-`status::`/`tags::`-in-the-edit-buffer bug = a display-only iOS strip, NOT the engine fix.** The live-reconcile reads the RAW engine `text_seq` via the FFI, which carries the property lines folded into the block text after a `NoteUpsert` round-trip (the materialized `.md` IS deduped; the raw read isn't). `stripPropertyLines` drops them before they hit the buffer — display-only, never written back, so ZERO convergence risk. The DURABLE root cure (`reconcile_tree_to_blocks` writes prose-only + lifts props into the container, mirroring the BlockUpsert seed arm) is **gated on `migrate_in_text`/fleet-readiness**: an old FFI that can't read the lifted container could re-broadcast a fleet-wide property erase. Deferred until the whole fleet is container-read-capable — a Rust/relay change, not an iOS build.
- **Every iOS property write goes through the typed per-key CONTAINER seam (`setBlockProperty` → `onLocalPropertySet`/`BlockPropertySet`), never a text splice** — that's what makes slash/NLP/date sets converge instead of re-triggering the raw-lines bug. The seam needs a COMPOSITE `"noteId:bid"` address (`resolveLocalBlockBid` → `splitBlockAddress`); a BARE bid silently no-ops (swallowed by `try?`). Freshly-created blocks have no `noteId` until a refresh, so `resolveLocalBlockBid` falls back to resolving the owning note from the in-memory containers.
- **Audit-as-process (the real lesson).** I shipped build 45 with 124 unit tests + 3 adversarial review lenses per phase all green — and the authoring was SILENTLY BROKEN (the bare-bid no-op). The lenses confirmed the writes were *structured* but never that the bid *resolved end-to-end*: "structured" ≠ "persists". A proactive 3-finder audit (find → adversarially verify each finding, 10 found → 7 confirmed) caught it + 6 more. Treat shipped-but-only-component-tested work as guilty until an integration audit proves it; that layer sits between "tests pass" and "works on device".

### 2026-06-22 — Per-type property config: one override map on Tag pages, THREE resolvers kept in sync

Anytype/Logseq-DB parity: one global Property (e.g. Status) carrying DIFFERENT choices/visibility/default PER TYPE (spec `phases/2026-06-22-per-type-property-config-spec.md`). Non-obvious calls:

- **Config = a `property_overrides` map in the Tag page's frontmatter**, resolved through the `extends` chain. Semantics (must match across all engines): choices **REPLACE** if the override has them, `hide_choices` **SUBTRACT**, `default` override wins, 3-state `show` = override else derived from `hide_by_default`. Legacy `hidden_{Prop}` is an alias that folds into `{Prop}.hide_choices` (the one intentional asymmetry).
- **The merge now lives in THREE engines that MUST stay in sync** — Rust `apply_override`/`get_resolved_tag_def` (views/config via `GET /types`), web TS `applyOverride`/`getTagPropertyDefs` (editor seeding/visibility), and (new this session) the iOS registry. Tag/Property pages are markdown notes; `tag_defs`/`property_defs` are DB caches rebuilt on index (zero sync impact). Migration via additive `ALTER ADD COLUMN`.
- **`tesela reindex` was silently incomplete** — `cmd_reindex` called `index.upsert_note` (search index only), never `index_type_info`, so on a mosaic whose type pages were synced-but-never-written, `tag_defs`/`property_defs` stayed empty and `GET /types` returned nothing. Fix: call `index.reindex` (= both). Surfaced only by landing per-type config on Taylor's real 540-note mosaic — fresh mosaics worked because the server SEEDS (writes → indexes) the built-ins.
- **Upgrade existing type pages in place via the note API, never delete+reimport.** A type is just a markdown note + a rebuildable cache, so upgrading = editing frontmatter through the Loro-aware note API (preserves block IDs → clean sync). Reimport would lose custom Tesela types and risk data.

### 2026-06-18 — iOS inline-autocomplete framework, the lazy-materialization page-list gap, and the word-wrap fix

Non-obvious calls from the iOS editor sprint (builds 21–28):

- **One trigger-detection framework for `[[` / `#` / `/`.** `LinkSuggest.detectTrigger` dispatches by kind: `[[` link (may contain spaces → checked FIRST, bounded by `]]`/newline) vs a single whitespace-delimited token starting with `#` (tag) or `/` (slash) at line-start/after-whitespace (so `C#`, `http://x`, `a/b` never trigger). State is one `EditorAutocomplete` carrying the `kind` + a generic `[Suggestion]`; `BlockRow` renders chips IN PLACE of the format buttons in the SAME keyboard-accessory pill (no fragile `inputAccessoryView` resizing). Slash verbs are text-insert/openers (link/tag insert just `[[`/`#` so the picker chains open); block-level actions stay on the toolbar.
- **The page/tag list MUST come from the Loro index, not materialized `.md` on disk.** `[[photography` found nothing because iOS materializes notes **lazily** — a synced-but-never-opened page has no local file, but its title lives in the always-resident index doc. Fix: new FFI `SyncEngineHandle.index_entries() → [IndexEntryRecord]` (wraps `LoroEngine::index_entries`), bridged via `RelayTicker` → cached in `MockMosaicService` (`indexPageCache`/`indexTagCache`, refreshed each `.relay` refresh) → `searchablePages`/`searchableTags` filter the complete list. The on-disk scan is now only a pre-first-index fallback.
- **Word-wrap = `sizeThatFits`, not compression-resistance.** A non-scrolling `UITextView` in SwiftUI reports its full single-line text as its intrinsic width; lowering horizontal compression-resistance (the first two attempts) only *changed* the breakage. The real fix: implement `func sizeThatFits(_:uiView:context:)` on the `UIViewRepresentable` to size the editor to the proposed width + report the wrapped height. (Verify UIKit-keyboard editor changes on-device; the sim sits behind onboarding and can't be driven headlessly to the editor.)
- **pi-offload division (post-confirmation):** self-contained NEW views → pi (gpt-5.5 built the Graphite Search view 5/5 from a tight spec naming the two reference files); framework-coupled editor work → Opus. gpt-5.5's one slip: it committed despite "don't commit." Logged to `~/.claude/model-scorecard.md`.
- **Arch-review verdict (eval of 3 open-source reports, `.docs/ai/review/`):** ~20% signal. Declined the god-module splits (`loro_engine.rs` is 4.3k prod + 5.1k co-located tests; `serve()` is 395 lines not the reported 1,800), trait-splitting for a one-implementation world, localhost-auth, the codegen query parser — team/SaaS patterns with no second implementer/user/hostile network here. Acted only on real findings (C23 data-loss guard + C19/C20/C21/C24/C6 hygiene).

### 2026-06-15 — Query language = JQL; the colon DSL is legacy sugar in the same grammar

Taylor prefers a **JQL-style** query language (`type = task AND points > 5`, `status IN (todo, doing)`, `priority >= 2 ORDER BY due ASC`) over the colon DSL (`tag:task points:>5`). Non-obvious finding that shaped the work: the **engines already parsed full JQL** — the Rust parser (`crates/tesela-core/src/query.rs`) is a recursive-descent grammar (`parse_or → parse_and → parse_unary → parse_predicate`, `BoolExpr{And/Or/Not/Atom}` + `Predicate{Cmp/In}`, `IN`/`LIKE`/`BETWEEN`/`IS NULL`/`ORDER BY`), with the colon DSL as legacy sugar in the SAME grammar; the web TS engine mirrors it 1:1. So JQL was a **front-end + iOS-parity** job, not a new parser.

- **P1 (web, `541a8322`):** made JQL the default authored/displayed form (`/query` template, placeholders, validation hint); `kind = page` validates (not just `kind:`); **`ORDER BY` now sorts the inline query block, L5-typed** (`applySort` reusing `compareTyped`) — the inline block sorts correctly where the server's string-only `apply_sort` doesn't (server typed-sort parity = a P3 follow-up).
- **P2 (iOS, `a38ed68d`):** iOS's `LocalQueryEngine` was deliberately flat-AND-only and dropped OR/parens/IN/LIKE/BETWEEN/IS NULL toward match-all. Restructured to the Rust `BoolExpr` tree (parser + `evalExpr` + `likeMatches`).
- **Parity is a TESTED contract:** the shared `query-conformance.json` fixture gained **69 JQL cases** (41 core `0d6b3f77` + 28 adversarial `cc9d316e`); **Rust is the source of truth** — every `expect` validated against it, and web + iOS must match. 182 cases now green on all three engines. A LIKE escape-set difference (Rust escapes `-&#~`, the iOS port doesn't) was reviewed + **verified harmless** (literal in NSRegularExpression normal mode anyway) and is guarded by cases.
- **Not done (P3):** palette JQL examples, an optional DSL→JQL one-shot converter for existing saved views, error surfacing on malformed JQL, server-side typed `apply_sort`, and the iOS inline-query-widget sort (filter works; order is source-order). The inbox chip composer stays colon-DSL (its whitespace token-split assumes single-token clauses; JQL clauses have spaces). `INBOX_VIEW_DSL` stays colon-DSL (conformance-pinned across Rust+fixture; parser treats both forms as equivalent).

### 2026-06-12 — Agent-pipeline roles: Opus = planner; Codex/Pi execute; Opus implements only L items

**Decision (Taylor), effective when Opus's rate limits recover (~1 week):** lock in a standing division of labor across the agent fleet.

- **Opus (Lead/T1) = PLANNER.** Default job is to spec/triage/architect work as self-contained backlog items for Codex + Pi Mono to execute — NOT to implement broadly. Opus owns the roadmap, decisions, and item decomposition.

### 2026-06-12 — North star: Tesela = emacs 2.0; keyboard + command registry ALWAYS first; stay Svelte (no Zed fork)

**Locked product north star** (2026-06-12, reconfirmed 2026-07-01):

- **Tesela's soul = emacs 2.0: keyboard + command registry, not mouse gestures.** The spine is live-command-registry + keybinding trees (spacemacs/which-key chords), not tiles/whiteboards/visual mode. UX priority: keymaps first, then quick-hands UI surfaces (tabs/sidebars), then embeds. Design means *not* designing — deref to emacs conventions (C-n/p for up/down, C-a/e for home/end, vim chords in dedicated mode).
- **Stay Svelte + Tauri; never fork Zed.** No GPUI dependency; no "bend a different editor's UI to Tesela's data" — that's the old "Obsidian client" road. Tauri's window frame is Tesela's final UI architecture.

### 2026-06-10 — Backups capture the AUTHORITY (Loro state + sync identity); scheduled in-server; provable via /backup/status

Non-obvious decision from the full audit:

- **Backups ≠ snapshots.** Every peer tracks a unique sync identity (node_id); a snapshot is a Loro document. A backup must capture BOTH — the replicated state (can restore and REJOIN the relay group) and the identity (no new node_id, no orphaned peers watching an identity that never appears again).
- **Backups = scheduled in-server**, not client-initiated or a separate subscription. The server guards backup frequency/versioning/retention automatically; clients never need to think about it. Restore is manual (a one-off HTTP POST with a backup ID from the user's list), not continuous.
- **Verifiable:** `/backup/status` lists available backups + their captured timestamps (NOT continuous: only those Tesela officially recorded). A client that tries to restore from a made-up ID gets 404 (not silently ignoring a bad restore).

### 2026-06-09 — Ultracode audit + product review: two-stream plan, relay topology, Reminders containment, full testing program

Ultracode-reviewed the full spec + codebase (9 agents, 7 days, 550k tokens). Major decisions:

- **Two-stream execution = A (relay-hardening) || B (Graphite cutover).** Audit queued A as the blocking P1 (sync-identity sealing + lazy-load audit gates data-loss confidence). This session ships A (builds 51 onward); the audit approved B (web/iOS UI) as deferred but not blocked by A.
- **Relay is Tesela's ONE CANONICAL, permanent, non-negotiable authority (cf. auth.md).** Conformance suite is the contract; Rust relay is frozen after conformance; CF is the production relay. No LAN relay (peer A is never the relay, even on LAN). The dilemma: P2P-on-LAN can't cross AirGap firewalls; a WiFi device can't see a Bluetooth neighbor — so we stay relay-dependent, accept it, optimize the relay.
- **Reminders + Daily Notes containment:** the system can DISPLAY Reminders but can't EDIT them (Apple EKKit lifecycle rules — edited events need an external save path, not a Tesela sync loop). Workaround: read → cache on device, edit in Apple Reminders natively, refresh cache — not a Tesela feature.
- **Full testing program:** Conformance (relay contract) + integration (engine ↔ UI) + convergence (multi-device) + product (device native) + regression guard (existing). One suite per layer.

### 2026-06-08 — App Store export compliance: standard crypto = EXEMPT, but EXCLUDE FRANCE before any PUBLIC release

Apple's crypto rules: standard AES/Ed25519 = EXEMPT from export (no ECL/BIS filing, no geo-restriction). BUT Apple's internal rule = EXCLUDE FRANCE unless a public release is official & your legal team filed (TestFlight counts as "public" — any tester count → excluded BEFORE submitted). Decision: **France is geo-blocked in Xcode's distributionTeams/signingProvisioningProfile [territories.exclude](https://developer.apple.com/documentation/appstoreconnectapi/territory) pre-release** (explicit deny-list). Once the app is in the public App Store and legal files the exemption request, we remove the block.

### 2026-06-08 — Task properties: priority p1/p2/p3 flags + Todoist "detect-inline, lift-below" display (Model B)

Chose **Model B** (priority inline + lifted below) over a sidebar preset model:

- **Inline detection:** slug-inline `priority::p{1,2,3}` are detected and offered as an inline-edit dropdown in the editor (w3/src/lib/components/BlockRow.svelte, the props seam).
- **Lifted below:** once set, priority renders in an above-block visual (like Todoist's y-axis stacking in the inbox, but ours lives below each block on all views). The inline set + the lifted display are ONE binding — edit one, the other updates (no dual-input).
- **One property per item:** only one priority can be set (checked via a uniqueness invariant in the Property-set seam, `setBlockProperty`); multiple tags are free (multi-value), but priority is scalar.

### 2026-06-07 — Tag/chip redesign: colored per-tag pills (right-edge) + ↵/⌘↵ commit gesture

Non-obvious UX decision after the multi-part review:

- **Tags become RIGHT-EDGE colored pills** (no longer inline bare hashtags). Each tag has a distinct color from the registry; the pill is left-padding + chip-interior-color + right-padding. Hovering shows the tag name (tooltip).
- **Commit gesture:** ↵ (Return) commits the editor (loses focus), writes the note; ⌘↵ (Cmd+Return) commits AND OPENS THE NEXT BLOCK (same note, then next daily/section). The gesture is baked into the platform defaults (iOS keyboard return behavior, web onKeyDown).
- **UX win:** tags at the RHS are less visually noisy than inline #tag; the colored pills are scannable (one pixel check → type/priority). The commit gesture cuts twiddling on mobile where tabbing-to-next is three taps.

### 2026-06-06 — /g splits via a Graphite-native pane renderer (GrLayoutTree), NOT by adopting v5 BufferShell

The web chrome team (v5) built a `BufferShell` (a split-pane frame) + a `BufferView` renderer. We chose NOT to adopt it for `/g` (Tesela's own Graphite editor). Why:

- **GrLayoutTree (our `LayoutTree` for Tesela graph nodes) is native Svelte, native Tesela data (LoroTree), and zero BufferShell coupling.** BufferShell is agnostic data (it takes a `Buffer` interface, any upstream can fill it); our tree is not that generic — LoroTree has Tesela-shaped semantics (block IDs, frontmatter, child ordering). Coupling would buy zero and lose some specificity.
- **Ownership & pace.** We own the grid/splitter/pane UX on the Graphite side; v5 owns theirs. Sharing a pane renderer would create a cross-team dependency (bug → blame → who owns the fix?) that neither team wants right now.

### 2026-06-06 — Tasks query stays tag-strict; existing tasks get a one-time #Task backfill (not a query widen)

Tesela's query syntax supports `kind = task | point > 0` (OR) and `kind = task AND point > 0` (AND). We chose to lock the built-in **tasks query to tag-strict** (`kind = task`), NOT widening to point-based (which would auto-include `point: 3` without `#Task`). Why:

- **Semantic lock:** `#Task` is the user's declaration "I intend to track this in the task system." Auto-widening based on inline `point:` would mean untagged high-priority notes silently appearing in Tasks — surprises the user. Stickiness (explicit tag) > convenience.
- **One-time backfill:** We did a one-shot scan and applied `#Task` to all existing `point:` blocks on first boot (no recurring query, no auto-tag-on-edit). Clean break between "point inflation" (old Logseq habit) and "task intent" (Tesela model).

### 2026-06-05 (b) — Loro container-overwrite hazard: nested property containers must be seeded into shared history

Deep crdt fix from the ultra-code audit:

- **The bug:** `NoteUpsert` (the FFI entry point for iOS authoring) creates property containers (Map) inside the block (Tree). If two devices author the same note concurrently, each creates its own local version of the Map. On sync, the LAST incoming merge wins — the earlier device's property edits are obliterated (not clobbered, erased). `setBlockProperty` on device A, then device B's NoteUpsert arrives → A's property is gone.
- **Fix:** Seed the property container into the shared history FIRST (during `create_block`/the first remote apply), then property writes merge cleanly. iOS calls `NoteUpsert` ONLY on local blocks authored on the device; server calls it on remote blocks to materialize them locally. So the seed order is: **server applies → materialize via NoteUpsert → seed property container (create_block)** → later property writes merge; **iOS creates block → property writes → server receives → server applies → seed container → converges.** A bare Upsert without prior seed is a no-op (container already exists from the earlier apply).

### 2026-06-05 — Properties + types milestone: structured-first typed property containers

Shipped Anytype-like properties/types system:

- **Structured-first:** a note's properties are CONTAINERS (Loro Map), not prose — the rendered markdown is a deterministic materialization, not the truth.
- **Typed:** each property is a type-tagged value (string/number/bool/date/select/link/relation). Choice-property shows a per-tag color; multi-value properties are lists.
- **Parity:** Rust engine reads Property pages as the type config, builds tag_defs cache (index-rebuilt), serves via `GET /types` + per-property-write seam. Web TS + iOS (new) mirror the same type system.

### 2026-06-02 — Block text is a nested LoroText (not a map register); discriminator scoped to disjoint twins

Loro container decision from the cutover:

- **Text is `Tree.text_seq[].content : LoroText`**, not a Map register. Reason: LoroText is made for concurrent text edits + character undo; a Map register is last-write-wins and loses interleaving. The nested LoroText is INDEPENDENT per block (block A's text fork ≠ block B's text fork).
- **Discriminator (which twin's text wins on a double-auth) is scoped to the TREE (block's host note), not global.** If device A branches block 1 + device B creates block 1 independently (disjoint twins), the device with a newer TreeID wins the block's identity (bid) → its text is authoritative. Device C's block 2, independently created → device C's TreeID settles block 2. No global "first device wins all" — per-tree is fairer (A wins some blocks, B wins others) and deterministic.

### 2026-05-30 — Defer the HA-relay sync redesign until after Loro/RTC; bypass it locally for now

The relay can be HA (multi-instance behind a load balancer) IF each instance syncs the compacted delta/snapshot state (currently hand-rolled, pre-Loro). This is an asymmetric upgrade (server-side only, no client change). Decision: DEFER. Reason:

- **Loro cutover (committed, phase 5 now) already rewires the delta format.** Baking HA now means two HA designs (hand-rolled → Loro-aware). After Loro: HA is THEN designed + shipped once.
- **Immediate mitigation:** `tesela-server` is single-instance (Azure Container Instances → single container, one relay tick). The server is NOT a bottleneck for P1 (Taylor is the sole tester; multi-tenancy is phase 7+). Scaling HA is P3.
- **Plan:** Phase 6 (RTC + whiteboard) might wire ephemeral state (presence, edit cursors) — let Loro cutover settle before piling RTC into the relay.

### 2026-05-27 — Migrate sync data layer to Loro; relay protocol stays as-is

**Committed full CRDT cutover.** The hand-rolled SqliteEngine oplog is replaced by Loro (the mature Rust CRDT library, used in Logseq). The relay protocol (ops, snapshots, acks) is unchanged; only the in-engine representation changes.

- **Why Loro (over Yjs/Automerge):** Rust-native (zero FFI, zero npm), strong text semantics (LoroText = vim-undo-style character ID), smaller (no Wasm blowup), used in prod (Logseq daily-drivers it, test-proven on mega-docs). Automerge is academic-quality; Yjs is primarily JS. Both need FFI bridges in Rust; Loro doesn't.
- **Why cutover (not gradual):** No dual-write period. Single source of truth from day 1 of phase 4. Mobile (iOS) ships updated FFI; relay stays as-is; Rust engine rewires internals.
- **Hardest parts:** (a) iOS recompile (WASM → updated aarch64-apple-ios Loro FFI); (b) the Markdown round-trip (materialized `.md` ↔ Loro doc parity — must be byte-identical or data is lost on a re-import); (c) lazy-load/evict (the doc model must NOT load all-resident mosaics into RAM on start).
- **Acceptance:** queries work, undo/redo works, sync converges (multi-device), iOS works, markdown export is clean, lazy-load is implemented.

### 2026-05-21 — Workhorse/spark accent split; the spark is a theme, not a rule

Tesela's two accent colors (design decision):

- **Workhorse** = primary interactive (buttons, links, inputs). Slate-600 (cool, business-like).
- **Spark** = highlights + accents (selected state, live cursors, "you-are-here" pointers). Coral-400 (warm, energetic).
- **Design principle:** Spark is a *theme choice*, not a rule wired into components. A user can pick "Spark Off" (both accents = Workhorse) or "Spark Warm" (Spark = orange) or (future) "Spark Cool" (Spark = purple). Components don't check `if (theme.hasSpark)` — they read `theme.accent.spark` (which could be disabled/swapped).

### 2026-05-21 — The v4/v5 chrome token layer aliases the role tokens

Both the v4 (current) and v5 (redesign) chrome share a token vocabulary (role-based: `bg.surface`, `bg.surface-alt`, `text.primary`, `text.secondary`, `border.divider`). The token layer in design is ONE unified set; the Tailwind/Svelte *value* (slate-50 vs slate-100) varies by theme, but the *name* is the same. This lets v4 and v5 coexist on the same palette + theme system.

### 2026-05-21 — Default theme rebranded to warm-dark "Prism"; light variant ships

Tesela's two shipped themes:

- **Prism (dark, warm)** = default. Based on the logo palette (slate + coral). The dark mode is warm-toned (not cold gray), evoking the warm-side ambient lighting (candlelit, sunset).
- **Prism Light (light, warm)** = high-contrast variant for bright rooms. Same warmth, inverted luminance.
- **Future:** cool dark (slate + teal), cool light (ice + navy), high-contrast (WCAG AAA for a11y).

### 2026-05-20 — `tesela-server` bind is config-driven; default stays loopback

The Tauri desktop app (phase 6, deferred) will need to talk to the local server. Default: loopback-only (127.0.0.1:7474). Config: `~/.tesela/server.toml` can set `bind = "0.0.0.0:7474"` if needed.

- **Rationale:** Loopback is secure (no LAN traffic leaks), faster (no network congestion), and matches the single-user desktop expectation. Multi-machine access (P2P relay + LAN HTTP) is phase 6+ (RTC) and outside the MVP.

### 2026-03-30 — Apple-first, web later (platform strategy)

Product strategy (2026-03 decision, reconfirmed 2026-06):

- **iOS is the primary platform.** Ship iOS first; web is a secondary companion. Reasoning: Tesela's UX (keyboard shortcuts, dark mode, offline-first) is natural to iOS; web is a "mirror" for browsers.
- **iPad can be added later** (same codebase + adaptive layouts, not a separate app).
- **macOS desktop = Tauri-wrap** (phase 6); NOT a standalone app. The loopback relay is the sync source; the desktop is a Tesela client like iOS.

### 2026-03-27 — Keyboard-navigable select popover (SelectListView)

iOS components: a reusable SelectListView (keyboard + touch-driven, up/down arrows move selection, return commits, dismiss on outside tap).

### 2026-03-27 — Preserve caller frontmatter in store.create()

When iOS/web create a note via the API, the engine preserves any frontmatter the caller provided (custom metadata, properties, tags). The engine doesn't erase or reset it — it's additive.

### 2026-03-25 — Properties and Tags as pages, not config files

Properties + Tags are stored as `.md` note pages (just like any other note), not in a separate config file (e.g., `properties.json`). Rationale: they sync, version-control, and edit like regular notes; the app builds a cache (property_defs, tag_defs) on index, but the source of truth is the markdown.

### 2026-03-20 — Database-first architecture shift

**Major architectural pivot** (spring 2026): The system is database-centric, not file-centric. SQLite is the authority; markdown files are a *materialized view* (rebuilt on every sync). Queries hit SQLite, not the filesystem. Undo/redo is journaled in SQLite. This is the opposite of the original "files are truth."

### 2026-03-15 — Custom NSTextView outliner, not embedded Neovim

iOS uses a custom NSTextView-based block editor, not an embedded Neovim (which would be too heavy and non-standard). The editor supports block-level operations (indent/unindent, move block up/down) and outliner nesting (collapse/expand).

### 2026-05-19 — iOS bottom chrome: native TabView with `Tab(role: .search)`, not a custom HStack

iOS chrome uses SwiftUI's native TabView with the built-in search tab role (one tap jump to search, keyboard command triggers search). No custom HStack reimplementation.

### 2026-05-20 — One process-wide `EKEventStore`, not one per operation

iOS uses ONE `EKEventStore` per app session (stored in @EnvironmentObject), not created per operation. Reason: EKEventStore is expensive to initialize; reusing it across all Reminders reads/writes is efficient.

### 2026-05-20 — iOS on-device Parakeet ASR via the FluidAudio package

iOS voice input uses the Parakeet ASR model (on-device, fast) via the FluidAudio package, NOT cloud-based speech (no latency, no privacy leak, works offline).

### 2026-05-21 — iOS `renderBody` drops bare leaf blocks instead of persisting them

When rendering a note's body to markdown, bare leaf blocks (no text, no children) are dropped, not preserved. Reason: they clutter the export and are usually accidental (user pressed enter, then undo). The engine still tracks them (they're in the Loro doc); materialization just doesn't emit them.

### 2026-05-22 — Recurrence is an rrule-shaped struct; `Until` end-dates built at noon-UTC

iOS recurrence (e.g., daily, weekly, every-other-week) uses an rrule-compatible struct (FREQ, INTERVAL, BYDAY, UNTIL). `Until` dates are always built as noon-UTC (not midnight local), ensuring consistent cross-timezone behavior.

### 2026-05-22 — Dates on task blocks are typed properties, not inline links

Task dates (due, scheduled, deadline) are stored as typed properties (Property.date), not as inline wikilinks. This makes them queryable and cross-compatible with the property system.

### 2026-05-22 — Agenda is an ambient buffer; recurrence projection lives on the server

The agenda (future tasks, recurring instances, deadline countdown) is an ambient read-only buffer that the server projects from the current note set + recurrence rules. iOS doesn't independently compute projections — it asks the server for the current view.

### 2026-05-22 — iOS NL date parser is a Swift port, not a remote call

iOS parses natural-language dates (`tomorrow`, `next Friday`, `May 3`) locally in Swift (a port of the Rust natural-language-date crate), not via an HTTP request to the server. Reason: latency, offline-first, and the Swift port is lightweight.

### 2026-05-28 — Loro doc model: hybrid (per-note docs + index doc), full-parity hard cutover

**Decision:** The Loro migration uses a **hybrid doc model** — one small always-resident **index doc** (note_id → metadata + graph) plus **per-note Loro docs** (lazy-loaded, evictable). NOT a single mosaic-wide doc. Cutover is a **hard flag-day** with **full parity** (byte-identical round-trip for all notes incl. frontmatter/properties/query pages) as the gate, then the hand-rolled `SqliteEngine` oplog is deleted.

**Why not single-doc:** Claude Code initially recommended one mosaic-wide CRDT ("fine at hundreds of notes"). Claude Desktop correctly rejected this on scale: dailies alone compound to thousands/decade and everything-is-a-block means millions of blocks. A single resident CRDT OOMs iOS (jetsam ceiling) on long sessions → app killed mid-write = the exact data-loss the migration exists to prevent. Cold-start would load the whole snapshot (grows forever); corruption blast-radius = whole mosaic. Every mature system shards (Logseq/Obsidian per-file, Notion per-block, Automerge many-docs, Yjs subdocuments). The hybrid also maps directly onto the existing per-note `.md` files + per-note relay routing — less of a departure than a mega-doc.

**Why full parity before cutover:** Taylor is on Logseq until Tesela sync is solid; nothing should regress vs Logseq when he switches back.

**Why hard cutover:** No daily-driver dependence during migration → no need for dual-protocol coexistence or gradual rollout. Flip all relay participants (Mac server, iOS, Savanne's devices) at once; web is an HTTP client and unaffected.

### 2026-05-28 — Structured-first; CRDT is truth; structural (not byte) parity; scalar props for v1

**Decision:** Tesela's post-Loro data model is **structured-first**: the CRDT (Loro doc) is the single source of truth; Markdown files are a deterministic *materialized view* (not the authority). Parity is *structural* (e.g., "5 blocks with properties X, Y, Z") not byte-identical (e.g., "the exact spacing/quotes must match"). Scalar properties only for v1; multi-value properties (tags, relations) merge-based (deterministic tie-break); link relations come later.

**Why structured:** Conflating "file format = data format" bred the mess (two kinds of truth, dual-write bugs, sync confusion). One truth (Loro) + one export (deterministic markdown) = clarity.

**Why structural parity:** Byte-identical parity is impossible after a re-export (markdown is lossy: `- text` + `- text` under the same parent re-export to the same tree, not two separate ops). Structural parity (same blocks, properties, and graph) is what matters — and it's testable/durable.

**Why scalar props for v1:** Multi-value properties would need a merge strategy for conflicting values (device A picks "red", device B picks "blue" — which wins?). Relations (link + metadata) need the query engine first. Ship scalar + design multi-value properly for phase 5.

### 2026-05-28 — Loro authoritative-writer architecture (relay-payload + flag work)

**Decision:** Inside the engine, the server is the **authoritative writer** — it receives updates from peers, applies them to the central Loro doc, and broadcasts the result back. Peers never author concurrently on the server's doc; each peer has its own local Loro replica that syncs against the server's. Flag-day requirement: **all three engines (Rust server, web client, iOS client) must simultaneously understand the new relay payload format** (Loro ops, not hand-rolled deltas).

**Why:** Authoritarianism simplifies conflict resolution (one arbiter per note) + determinism (all peers converge to the server's result). The cost is latency (RTT to the server for every write, even on LAN). The win is correctness (no stale-timestamp or op-reordering surprises).

**Why flag-day:** Peers running different versions would author on different doc replicas and miss each other's updates (silent data loss). Flip all at once.

### 2026-05-29 — Cutover adversarial review dispositions

The Loro cutover (phase 4, hard flag-day) was adversarial-reviewed by 3 agents. All found the plan sound; the real gotchas were:

- **Lazy-load is an EPIC** (not a 1-shot), gating a residency audit (every `self.inner.docs` walk must be wrapped in a load-check).
- **Markdown round-trip parity is DURABLE** (not local to cutover). Every format change (future inline spans, rich text, new property types) needs a round-trip test.
- **FFI rebuild is EXPENSIVE** (every platform recompiles `tesela-sync-ffi`; iOS needs a new build). Clients run the old binary and can't talk to the new server until their app updates.

### 2026-05-29 — Blank blocks + headings dropped (Loro render policy)

On export (Loro doc → markdown), the engine drops empty blocks (no text, no children) and heading-only blocks (e.g., `# Heading` with no body). Rationale: they're transient in the editor and only clutter the output. The engine never *loses* them — they stay in the Loro doc until explicitly deleted — so re-importing loses nothing. The markdown is for human reading + external tools; the Loro doc is the authority.

### 2026-05-29 — Web daily-editing bugs (post-authoritative-cutover)

Post-cutover issues found on web (all fixed pre-ship):

- Journal day-change: opening yesterday's daily on cutover day → editing it locally → sync arrives → the server's *today's* daily arrives, not yesterday's → confusion. Fix: detect day-change on sync arrival; prompt user to re-open the correct daily if they drifted.
- Undo stack: the journal initially kept undo state per-day; post-cutover undo was server-driven (one shared undo per note). Fixed: one undo *group* per day (visual grouping, not isolated rollback).
- Owner tags (`owner::me`, etc.): the old model had a user identity (Taylor's ID); post-cutover we dropped user/team concept (MVP is single-user). Owner tags broke (no "me" in scope). Fixed: owner is always the local device (no server concept of identity for v1).

### 2026-05-29 — Loro flag-day: sole engine, op-wire deleted, LAN P2P retired

Post-cutover permanent changes:

- The old `SqliteEngine` (hand-rolled oplog) is deleted. Loro is the SOLE engine.
- The `op-wire` (internal JSON format for oplog exchange) is deleted. All relay communication is now Loro-shaped (ops + snapshots).
- LAN P2P (device-to-device sync without the relay) is RETIRED. All sync goes through the relay (even on LAN). Reason: Loro is single-writer-per-note (peers can't merge concurrently, they must serialize through the relay). LAN HTTP is phase 6+ (RTC presence, eventually maybe P2P cursors, but not data sync).

### 2026-05-31 — Multi-device convergence: shared-base bootstrap + dedup heal (the real RTC fix)

The 2026-05-27 Loro decision locked in a new CONVERGENCE algorithm for the multi-device case:

- **Shared-base bootstrap:** On first sync, peers exchange their full doc state to establish a common ancestor (Loro's "frontierMap"). Every peer then applies the shared base + their own local edits → CRDT merge gives deterministic convergence. (Old: "compare snapshots + diff" was error-prone.)
- **Dedup heal:** If two peers' local edits CREATE THE SAME BLOCK ID independently (disjoint twin), the newer TreeID's version wins. The system then de-duplicates + heals the blocks (merge containers, rewrite their ids in-flight) so a third device sees only one block. (Old: twins persisted forever, requiring manual intervention.)
- **Why "the real RTC fix":** RTC (real-time collab) needs concurrent edits to merge predictably. The old system had no consistent merge rule — this one does. Multi-user will fork off later once identity + ACL are designed.

### 2026-06-03 — Cloudflare Worker relay: conformance-as-shared-contract; structural per-group isolation

**Major infrastructure decision:** The relay migrates from hand-rolled Azure/HA-Rust to **Cloudflare Workers (serverless, global, auto-scale)**. The relay is now a CF Worker module (`cloudflare-relay/`) called `relay_worker`.

- **Conformance is the contract:** the relay's behavior is DEFINED by a shared Conformance Suite (Rust + Go + CF tests). Any relay implementation must pass all tests (currently: Rust relay + CF Worker). The suite covers ops, snapshots, acks, peer discovery, admin, and edge cases. If the CF version diverges, tests catch it.
- **Structural per-group isolation:** Each group (mosaic) is isolated at the CF data layer (separate KV namespace, separate Durable Objects). A group's data is never visible to another group, even by mistake.
- **Live:** The CF relay is deployed and Taylor's devices are syncing against it (ra7 migration, 2026-06-10).

### 2026-06-04 — Desktop app: Tauri-wrap `/g`, not a fresh SwiftUI Mac app

**Phase 6 deferred, architecture locked:** The Tesela desktop/macOS app is a **Tauri wrapper** (a native window, a loopback HTTP relay client, no native Rust UI). The `/g` (Graphite editor) runs in the Tauri WebView; sync is via loopback HTTP to a local `tesela-server`. NOT a SwiftUI Mac app (which would require porting the web UI + reimplementing the editor + separate sync code).

- **Why Tauri:** web UI code (Svelte) is reused, sync is the same relay protocol, no app-store friction (can self-update). The Tauri window is Tesela's final desktop surface.

### 2026-06-10 — Block deletes are explicit-only; NoteUpsert can never remove OR resurrect a block

**Engine invariant (enforced):** A block can ONLY be deleted via an explicit `BlockDelete` op (not via `NoteUpsert` re-writing without it). Consequence: iOS/web calling `NoteUpsert` (to reconcile a note's body) will NEVER accidentally erase a block that was deleted on another device. This prevents the "deleted-on-desktop-but-still-visible-on-iOS" sync bug.

### 2026-06-19/20 — APNs instant-sync: the non-obvious gotchas

Push-notification architecture for iOS:

- **APNs token is device-wide** (not per-group, not per-app). One token → all groups can push. Token changes on app reinstall (don't assume it's stable).
- **Registration is relay-scoped** (token + relay identity). If a device switches relays (e.g., HA → CF migration), the OLD relay's APNs token becomes stale; the NEW relay needs a re-registration. Stale registrations silently fail (push dropped, no error).
- **Deposit + broadcast:** When a relay receives a note update, it deposits the update and **broadcasts a content-available push to all other devices in the group** (via their registered APNs tokens). The push wakes the app (even if suspended) to sync.
- **Best-effort:** Apple throttles silent pushes; APNs is not a guarantee. The RELIABLE path is the foreground sync loop (timer-driven, 2–5s).

### 2026-06-29/30 — multi-device convergence + iOS NLP parity: durable decisions

From the live device test (2026-06-29) + the iOS NLP port (2026-06-30):

- **Convergence**: Disjoint twins are healed by the dedup algorithm (keep-winner based on TreeID). Tested live (desktop block created → iOS edit + CREATE (disjoint twin) → BOTH sync → desktop sees iOS's block + its own merged-away).
- **NLP (natural-language parsing):** iOS ported Rust's date parser + keyword extraction to Swift. Non-obvious: the Swift port's `DATE_REGEX` and tokenization don't 1:1 match the Rust version, so a date string parses differently on each platform. Solution: a fixture file (`natlang-conformance.json`, 47 test cases) that both engines must pass; verified parity across builds.

### 2026-06-30 — iOS onboarding epic (tesela-mp0) re-scope + relay-attach fix

The iOS onboarding flow (builds 50–51) had two classes of issues:

- **Relay-attach broken on the live CF Worker:** after copying a pairing code, the app tried to attach but got stuck in "connecting". Root cause: the relay's `/devices/{device_id}` route expected a `PUT` with a body; iOS was sending a `GET`. Fixed in the Worker + iOS.
- **Onboarding epic (tesela-mp0) re-scoped:** the full-page onboarding wizard (with slides, Parakeet intro, etc.) is deferred (post-MVP); the current MVP is minimal (pick name + fetch pairing code → done). The epic is split: Phase 6a = onboarding content, Phase 6b = wizard UX.

### 2026-06-30 — Pairing/identity direction: Anytype-style recovery phrase + QR (Taylor)

**Locked product decision (Taylor):** Pairing/identity recovery uses a **recovery phrase + QR code** (Anytype model), not a username/password or invite links. The recovery phrase is ~12 words that let a user re-sync their identity if they lose a device. QR code is a fast pairing on new devices.

**RESOLVED — the `task-tag-wip` git stash was stale, dropped** (2026-06-30, noted for continuity)

### 2026-07-01 — Loro disjoint-twin convergence: deterministic keep-winner, unified across apply paths (tesela-y11)

**Spec approved 2026-06-30, shipped 2026-07-01.** The system resolves disjoint twins (blocks authored independently on two devices) deterministically by keeping the block with the **higher TreeID** (which device's edit happened logically later). The winner's text + properties are authoritative; the loser's local edits stay in that device's doc (undo-able) but don't sync out.

- **Why deterministic:** every peer converges to the same block (no drift). Why TreeID: it's immutable, comparable, and available at apply-time (no need to fetch block creation timestamps or heuristic "edit recency").
- **Unified across apply paths:** `apply_doc_update_status` (relay inbound) + `import_doc_update` (peer-to-peer relay polling) both use the same keep-winner rule. Verified: full `tesela-sync` suite + new `convergence-disjoint-twins` tests.
- **Limitation:** one device's edits are dropped (local undo history survives, but remote doesn't). For multi-user (Savanne as co-editor), the design will need per-user TreeID (deferred). Ships: desktop rebuild + iOS build 53.

### 2026-07-01 — Full-stack architecture review (Fable 5): nine decisions + the epic plan

Ultracode-style arch review by Claude Fable 5 (the toughest reviewer available), 9 decisions + a major epic plan:

- **ADR-1:** One apply orchestrator in the engine (single place to change convergence rules).
- **ADR-2:** Relay end-state = CF canonical; Rust relay frozen to conformance scope only.
- **ADR-3:** Block-lifecycle semantics move into core (enforce engine-only writes).
- **ADR-4:** Commands-as-data (emacs 2.0 spine investment).
- **ADR-5:** Multi-user needs key-model fork (rotation first, design spike before Savanne).
- **ADR-6:** Lazy-load/evict is an epic with a residency audit gate.
- **ADR-7:** Doc/backlog hygiene is fleet-safety (stale prose actively misleads cheap models).
- **ADR-8:** Ratify CF-DO WebSocket as THE ephemeral-presence transport.
- **ADR-9:** Parity strategy = fixture-first, hoist-second, wasm-when-JQL.

### ADR-1 — One apply orchestrator in the engine

Every doc update goes through ONE apply path (currently: `apply_doc_update_status` in `loro_engine.rs`). This is where the converge rule lives. No ad-hoc applies scattered throughout the codebase. Benefit: to change the convergence rule (e.g., from keep-winner to 3-way-merge), you change one function, test it, and every code path uses it.

### ADR-2 — Relay end-state: CF canonical; Rust relay frozen with an EXPLICIT scope

Executes the locked 2026-06-09 topology. Freeze scope = the conformance suite's surface: ops/register/ack/snapshot/discovery/admin. PERMANENT exclusions, stated so a self-hoster's expectations are set: presence (CF-DO-WS only — the Rust relay pulls no ws feature at all) and APNs delivery (best-effort; only the /devices contract is covered). Preconditions to declare frozen: (a) tesela-p19 fixed — the CI `worker-conformance` job is RED today on the body-cap tests vs the real 16 MiB wrangler.toml, so "one suite gates both" is currently broken on the axis most likely to matter; (b) ra7.3 admin disc-scrub parity + a conformance case (admin-delete → /discover 404, untested on either side); (c) the insert_op TOCTOU seq race documented as a known self-host limitation (verified self-healing: outbound cursor advances only after a confirmed PUT, delta re-sends next tick; CF is structurally immune via DO serialization + AUTOINCREMENT) — fix = BEGIN IMMEDIATE or the CF autoincrement pattern, required only before multi-user self-host is OFFERED, P3. After freeze: Rust-relay changes only for conformance parity, never features.

### ADR-3 — Block-lifecycle semantics move into the core (enforce the engine-only-writes lock)

Enforcement of TWO existing locks (2026-06-09 "every note mutation goes through the engine"; 2026-05-22 "recurrence projection server-side only"), not a new decision. Violations found: (a) recurrence bump, dependency-cycle flips, and tag auto-create exist ONLY in tesela-server's HTTP handlers (notes.rs:3079-3812) — the WS/relay path and iOS `.relay` mode bypass them, which is WHY iOS re-implements `rollRecurringComplete` in Swift (MockMosaicService.swift:451); (b) tesela-cli new/edit/daily, tesela-tui, and tesela-mcp create_note write via FsNoteStore/SqliteIndex and never call `record_local` — the CLI's own new repair subcommands (49d) open LoroEngine directly, proving the gap is unaddressed, not deliberate. Decision: block-lifecycle side effects become pure functions in tesela-core invoked from every author path; CLI/TUI/MCP writes route through the engine (or those commands are gated/labeled local-only as an explicit interim). Delete the Swift recurrence copy once FFI parity exists.

### ADR-4 — Commands-as-data (the emacs-2.0 spine investment)

The registry's bones are right (per-surface `availableOn`, real localStorage rebind layer for shortcut/chord, `:keymap` introspection) but the catalog is code-baked: registration is an import-order side effect (buildV4Commands() at module top-level; /settings/general needed a workaround side-effect import), matching lives in v4/commands.ts not the registry, duplicate ids only console.warn, iOS has 7 hand-wired actions with zero shared vocabulary, MCP hand-maintains a parallel tool list, and slash verb sets have already diverged (web-only Task/Template/Query/Collection vs iOS-only Quote/Divider). Decision: one command MANIFEST (id, label, category, chord, surfaces, keywords, args-shape — no closures) with an explicit `registerBuiltinCommands()` bootstrap and id-collision-as-dev-error; the server serves the manifest (mirroring the property-registry-as-synced-data pattern); iOS palette (tesela-cib) consumes it with a native executor map keyed by stable id; the MCP tool list is generated from it; the keybinding/leader-tree config file (roadmap:430) hangs off the same stable ids. tesela-plugins (Lua) is PARKED as CLI-only experimental — tesela-server never loads it, so it cannot affect the product; any plugin story routes through the manifest later.

### ADR-5 — Multi-user needs a key-model fork: rotation first, design spike before Savanne

Verified: DeviceId carries no keypair (Ed25519 columns dormant), remove_peer only deletes LAN bookkeeping, and NO GroupKey rotation path exists — so a leaked phrase/lost device = permanent unrevocable full membership, and the single symmetric GroupKey cannot express per-user identity/ACL. Decisions: (a) GroupKey ROTATION becomes a designed first-class operation (the minimum kick-a-device story) — spec'd under tesela-tp0's scope; (b) Savanne/multi-user work is GATED on a Lead design spike (per-user Ed25519 + wrapped group keys vs re-key-the-world; retrofitting after shipping means every group re-keys); (c) until then the group model is explicitly single-tenant; (d) a Keychain GroupKeyStore adapter replaces plaintext `.tesela/group_key.bin` (the trait seam exists, no impl does).

### ADR-6 — Lazy-load/evict is an epic with a residency-audit gate

The 2026-05-28 doc-model lock already says "per-note lazy-loaded/evictable"; all-resident is unfinished implementation. Beyond qql's `already_resident` heal-gate landmine, the review found a second coupling: `produce_relay_updates` walks only the resident docs map (loro_engine.rs:1211) — post-eviction, an evicted note's local edits would silently never broadcast (a stall, not a crash). Sequencing: ADR-1 consolidation → audit EVERY `self.inner.docs` walk (produce, rebuild_index, scan/heal twins, note_ids) → lazy-load .bin on miss → evict. iOS (same engine via FFI, tightest memory) is the forcing platform.

### ADR-7 — Doc/backlog hygiene is fleet-safety, not chores

Stale prose actively misleads the cheap models this repo dispatches: roadmap Non-Goals still lists "no iOS, no CRDT sync"; the desktop item lists shipped work as Remaining; architecture.md's body contradicts its own banner; plan.md ×3 are ~10 months stale; lib.rs comments claim a CancellationToken that exists nowhere (grep: 0 hits) and a watchdog env var the embed never sets; web Settings still shows a "Sync All" button that no-ops against 501-retired routes; decisions.md itself now has two header conventions (### top-inserted vs ## bottom-appended). Decision: one hygiene batch fixes all of it, and the standing rule stays "current-state + beads outrank roadmap prose" — but the prose must stop lying anyway.

### ADR-8 — Ratify CF-Durable-Object WebSocket as THE ephemeral-presence transport

The 2026-06-27 spec only recommended it; no lock existed — yet the CF Worker presence WS is implemented (MAC-gated, per-DO broadcast, hibernation-survival) and deployed, and the Rust relay is deliberately presence-less. Ratified: presence = ephemeral CF-DO-WS (+ the local server's WS PRES frames for same-host web tabs); never conformance-scoped; never store-and-poll; never a CRDT-authority concern. The engine's unused FFI presence surface (set_presence/presence_peers/apply_presence — no production caller on any platform) gets deleted or adopted by iOS, not left ambient.

### ADR-9 — Parity strategy: fixture-first, hoist-second, wasm-when-JQL

The query-conformance.json pattern (one fixture in crates/, three thin runners, 182 cases — all three engines DO run it; the "iOS silently degrades JQL" claim was adversarially REFUTED) becomes REQUIRED for every parity subsystem. Confirmed drift the pattern would have caught: Rust's recurrence grammar gained biweekly/fortnightly/quarterly/every-other on 2026-06-20 and NEITHER client mirror was updated (renders as raw literal both clients); iOS parses "today noon", web doesn't; web guards lifts inside wikilinks/URLs, iOS doesn't. Phasing: NOW — Rust-generated fixtures for recurrence, NLP lift, inline spans, chip visibility, property-override resolution + hoist recurrence recognize/format to Rust via the EXISTING uniffi (2 pure fns; clients keep HTTP recur-bump for the op); NEXT — stand up wasm-bindgen for web WHEN the JQL evaluator hoists (retires ~2,350 duplicated client LOC; the 182-case suite is the acceptance gate; web has NO tesela-Rust path today — loro-crdt npm wasm is upstream's); NEVER hoist — inline decorations, chip policy, fuzzy ranking (UI-coupled). Property-type vocabulary unifies on ONE canonical list (Rust ValueType + email/phone/object; three vocabularies already disagree and Rust silently degrades unknowns to Text).

### 2026-07-01b — Taylor's five ratifications on the arch review (harness-deck answers)

All five asks from the 2026-07-01 review answered same-day; these are now LOCKED product/architecture calls:

- **Pure max-TreeID convergence: APPROVED** — the keep decision will depend only on the immutable TreeID (drop genuine-edit preference + stale-guard); Taylor accepts that the higher-TreeID twin's text wins a same-block conflict. Implement via tesela-fte AFTER tesela-engc.1 (one place to change the rule); device-validate on the live CF relay before trusting.
- **Arc order: SPINE FIRST** — commands-as-data (cmdd) + parity fixtures (pfix) before type-system views (ya4).
- **Views slice order: kanban on web first**, then sets/table on the same data layer, then iOS.
- **tesela-plugins (Lua): PARKED ratified** — CLI-only experimental; any plugin story routes through the command manifest (ADR-4).
- **Multi-mosaic end-state: ONE SERVER HOSTING N MOSAICS — committed** (overrides the review's cheaper process-per-mosaic lean). Sequenced AFTER lazy-load/evict; epic tesela-mmos + Lead design spec tesela-mmos.1 (blocked-by tesela-qql). ejn.2's desktop mosaic-switch fix stays interim (hide/disable, don't build a relaunch flow mmos would replace).

### 2026-07-01c — Desktop auto-update: GitHub Releases hosting + Keychain-held signing key (tesela-ejn.1)

Wired `tauri-plugin-updater` (check on startup + View > Check for Updates…, auto-download-install-restart via tauri core's `AppHandle::restart()`, no `tauri-plugin-process` needed for that). Decisions:

- **Manifest hosting: GitHub Releases, not the CF Worker.** The repo is public (`TaylorFinklea/tesela`) and `desktop-release.sh` is a purely LOCAL script (Taylor's own ASC/codesign creds; no CI builds/signs the desktop app) — `releases/latest/download/latest.json` needs zero new infra, zero new secrets, and zero relay-worker route surface. The CF Worker is the sync relay; mixing static-asset hosting into it was rejected as unrelated scope creep. Endpoint is pinned in `src-tauri/tauri.conf.json` `plugins.updater.endpoints`.
- **Signing keypair: generated now, private half lives ONLY in the macOS Keychain** — `security` items `tesela-desktop-updater-key` (the rsign-encrypted private key, itself password-protected) and `tesela-desktop-updater-key-password`, both `-a "$USER"`. Never touched disk outside a one-shot `ai-scratch/` scratch file that was deleted immediately after the Keychain round-trip verified. `desktop-release.sh` now loads both from Keychain (env wins, for CI-secret-style overrides) before `cargo tauri build`, which auto-emits `$APP_BUNDLE.tar.gz`+`.sig` when `TAURI_SIGNING_PRIVATE_KEY[_PASSWORD]` are set and `bundle.createUpdaterArtifacts=true`. The public key is NOT a secret and is committed as `plugins.updater.pubkey` in tauri.conf.json.
- **Known gap, documented not guessed at: the shipped updater tarball is pre-staple.** `cargo tauri build` (step 1) emits the signed `.tar.gz`+`.sig` before notarization/stapling ever run; re-creating it post-staple would need verifying Tauri's exact bundler tar layout against a real signed build, which wasn't available headless. Net effect: an updated app is still notarized (Apple has the record) but does one online Gatekeeper check on first launch of the update instead of reading an offline-stapled ticket. Left as a documented follow-up in `emit_updater_manifest`'s comment rather than shipping an unverified manual re-tar.
- **Full end-to-end (old build detects + installs a new signed release) could not be run headless** — no Apple Developer signing identity/notarization creds in this environment. What's delivered: the Rust wiring compiles/clippy-clean, `scripts/desktop-release.sh --skip-notarize` dry-run verified to no-op cleanly (unchanged from pre-change behavior when no bundle exists), and the manifest-emission logic. Taylor still needs to run one real `scripts/desktop-release.sh` (full notarize path, with `DESKTOP_SIGN_IDENTITY` set) from an OLD installed build, cut a `vX.Y.Z` GitHub release with the ZIP + `.tar.gz` + `.sig` + `latest.json` (the script prints the exact `gh release create`/`upload` command), and confirm the old app's "Check for Updates…" installs it.

### 2026-07-03 — tesela-fr1: recurring-lift prose-eating bug — root cause + the empty-block export finding

Root cause (all three engines — Rust `nlp_lift.rs`, web `date-parser.ts`, iOS `DateParser.swift` — each hand-mirrors `parseDateAndRecurrenceInput`): the "bare recurrence, no date" fallback short-circuited on the recurrence tail already extracted by `extractRecurrence` (`recurrence.or_else(|| recognize(afterField))` / `recExtracted.recurrence ?? parseRecurrenceInput(afterField)`) instead of re-validating the FULL untouched candidate. `extractRecurrence` legitimately strips a *trailing* recurrence phrase off a longer string that still has unparseable prose in front — e.g. `"Call the doctor every sun"` → tail `"every sun"` (recognized), rest `"call the doctor"` (not a date). The short-circuit treated that trailing-strip success as proof the WHOLE candidate was a bare recurrence phrase, so the caller (`longestDateFrom`/`longest_date_from`, which tries decreasing-length word spans) accepted the full multi-word span — prose included — as one consumed token, and `detectTaskTokens` stripped it all, leaving an empty-text block with only the lifted props. Fix: the fallback re-checks the untouched `afterField`/`after_field` directly against the exact-match recognizer (`recognize`/`parseRecurrenceInput` require the WHOLE trimmed string to equal a canonical recurrence phrase), so a candidate with leftover prose fails there and the caller's shrinking-span loop lands on the correct shorter span (just `"every sun"`). Fixed in `crates/tesela-core/src/nlp_lift.rs`, `web/src/lib/date-parser.ts`, `app/Tesela-iOS/Sources/Data/DateParser.swift`; regression tests added in all three plus 5 new shared cases in `crates/tesela-core/tests/fixtures/nlp-lift-conformance.json` (recurring+leading-prose, recurring+priority, recurring+priority+trailing-date, recurring-only, the exact Taylor repro).

**Empty-block export ripple (investigated, not redesigned):** Taylor's repro produced a real all-props/no-text block that took ~5 min to reach other devices and transiently mangled the desktop tag rail. Checked whether empty-text-with-props blocks are second-class on export — the 2026-05-29 decisions.md entry above ("Blank blocks + headings dropped") is **stale**: the same day, commit `0bcca988` reverted the blank-bullet drop (`note_tree_from_doc` keeps blank bullets as the editing surface; only non-bullet body lines/headings are still dropped). Verified with a new regression test, `snapshot_export_import_preserves_props_only_empty_block` (`loro_engine.rs`): an empty-text block carrying only a prop (`priority:: p2`) round-trips byte-identical through `export_doc_update`/`import_doc_update` across two engine instances. So the empty-text-with-props *class* is not dropped or second-class in the current sync path — that part of the ripple is understood and now has a pinned test. What's **not** explained or chased further here: the ~5-minute propagation delay and the transient tag-rail glitch — reproducing/root-causing those needs a live multi-device session, which is out of scope for this fix (no live-mosaic writes allowed in this pass), and the underlying trigger (an ordinary prose+recurring line collapsing to empty text) is gone now that the parser bug is fixed. Left as a reported observation, not a redesign of export policy.

### 2026-07-08 — Dictation P1 (tesela-v5t.1): transcribe.cpp becomes THE server ASR engine; whisper-rs demoted to mutually-exclusive fallback

Taylor green-lit all 4 dictation-modernization phases (harness-deck `20260708-dictation-transcribecpp-research`, incl. NVIDIA Open Model License + en-only streaming v1). P1 shipped: engine seam `crates/tesela-server/src/asr_engine.rs` (family-dispatched, mirrors iOS's `TranscriptionEngine` protocol), catalog gains WER-verified handy-computer GGUFs (canary-180m-flash Q8_0 218MB, parakeet-unified-en-0.6b Q8_0 731MB) with exact sizes + sha256, downloads land in `.part` → stream-hashed → verified → renamed (kills the truncated-download-reads-as-downloaded class), per-entry `file_name` retires the hardcoded-`.bin` mangling (whisper keeps legacy `<id>.bin` so existing installs work).

**The forced decision: whisper-rs and transcribe-cpp CANNOT coexist in one binary.** Both statically vendor ggml; the linker interleaves the two symbol sets and whisper.cpp's backend registry walks transcribe.cpp's Metal backend → `GGML_ASSERT(index == 0)` abort at model load (reproduced; stack in the P1 session). This is why Handy v0.9.0 deleted whisper-rs. Resolution: default feature `transcribecpp` runs ALL families (Whisper GGML `.bin` auto-detected — verified empirically, byte-identical transcript), `whisper-fallback` is the emergency whisper-only build, `compile_error!` guards both-on. E2E verified on a sandbox mosaic: whisper-tiny 9.5s (first load), canary 1.8s, parakeet-unified 9.5s (731MB load), all transcripts correct with PnC. P4's "retire whisper-rs" is thereby mostly pre-paid; what's left there is deleting `whisper-fallback` once transcribe.cpp has soaked, SwiftWhisper's iOS retirement, and the iOS canary spike.

Bycatch worth knowing: sandbox/dev tesela-server boots can hang FOREVER pre-listen at group-identity load when trustd is wedged (the known env blocker) — the Keychain `find-generic-password` never returns. Workaround for sandboxes: `TESELA_GROUP_KEY_FILE_STORE=1` or a fresh mosaic (create path doesn't hang). Recorded via `bd remember`.

### 2026-07-09 — Dictation P2 (tesela-v5t.2): streaming spine — WS session + web capture + engine lease

Built live dictation on top of P1's transcribe.cpp engine. New WS route `GET /transcription/stream` (routes/transcription.rs): client sends binary 16 kHz mono f32-LE PCM frames + a `{"type":"stop"}` text frame; server returns JSON `ready`/`partial`(committed+tentative, only on change)/`final`/`error`. The WS task bridges a bounded mpsc(64) to a `spawn_blocking` worker (`asr_engine::stream_session_blocking`) that leases the single-slot model cache. transcribe.cpp's `session.stream()` drives native streaming for parakeet-unified; whisper/canary (no native streaming) fall back to accumulate-then-batch on stop with `streaming:false` (client shows a spinner, no partials). Web: getUserMedia → AudioContext → `/voice-worklet.js` chunker → WS; `final` → `getDailyNote` + `upsertBlocks` append at document end. Mic button + `voice-capture` command (chord `a v`), `GrVoicePopover` shows committed solid / tentative dimmed.

Decisions:

- **Engine lease is an RAII guard, not a bare flag.** `STREAM_ACTIVE` is cleared by `StreamLease::Drop`, so a Rust panic mid-session (feed/finalize error, poisoned lock) releases the lease as the stack unwinds. A bare flag (first cut) would have wedged `STREAM_ACTIVE=true` forever on one panic, bricking BOTH live dictation and batch for the process. (A transcribe.cpp `abort()`/GGML_ASSERT kills the whole process, so there's no lease to leak there.) The flag is checked UNDER the cache mutex in `transcribe_blocking` — checking it before locking was a TOCTOU that let a batch request load a second 700 MB model beside a live stream.
- **Web session is a generation-token machine.** Every start and every terminal transition bumps `generation`; each async continuation (getUserMedia, worklet addModule, daily-append) captures its generation and bails — stopping the just-acquired resource — when it's stale. This closes the whole class of across-`await` races the review found: cancel-during-getUserMedia was a hot-mic leak (live track, no UI); stop-while-connecting threw a spurious error; clean-close-during-finalizing hung the spinner forever. A 180 s finalize watchdog + onclose-during-finalizing handling replace the hang.
- **Worklet downsamples to 16 kHz itself** rather than trusting `AudioContext({sampleRate:16000})` — WKWebView (the desktop's engine) ignores the rate request and would otherwise feed 48 kHz PCM read as 16 kHz (3× fast = garbage). Linear-interp resampler verified numerically against 16/48/44.1 kHz (reconstructs a 440 Hz sine to 439.8 Hz).
- **Popover keys are scoped, not global.** Escape cancels (ignored while a keystroke targets an editor/input/`.cm-editor`, and a no-op during finalizing so it can't discard a pending transcript); Enter-to-finish was removed as a window listener — pressing Enter in the editor while a note dictates must not submit the session. The Done button finishes.

Verified (release build, sandbox mosaic): parakeet-unified streams correct committed/tentative partials; whisper-tiny batch-fallback returns correct final with `streaming:false`; engine lease cold-load 9.6 s → warm 0.26 s (returned to cache); daily-append lands at document end. Adversarial 3-lens review (concurrency/web-state/protocol) → 8 confirmed findings, all fixed + regression-tested (server lease panic-safety test; web protocol tests).

**Known limitation (not a blocker):** parakeet-unified's buffered streaming (recompute-per-chunk, per the P1 research) runs ~0.4× real-time on M-series, so on a long utterance committed text lags progressively and `final` arrives seconds after stop. Fine for short voice notes (the dominant use). Tuning levers for a follow-up: cache-aware `nemotron-3.5-asr-streaming-0.6b` (both transcribe.cpp + FluidAudio ship it) or a lower latency tier. Filed as a discovered task.

### 2026-07-09 — Arch-review cycle 2 (Fable 5): cutover-spined plan, adversarially hardened

77-agent verified review (9 readers, 68 findings, 0 refuted) + 3-model adversarial panel on the plan itself (Sonnet 5, GLM 5.2 via ollama-cloud, MiniMax M3). Taylor's locks: **Approach 3 — full Logseq parity BEFORE the trial; then full cutover day 1** (Logseq → read-only fallback); **reconcile-import INTO the live mosaic** (it holds all post-Jun-16 content — Taylor has been living in Tesela dailies since the Logseq graph went idle Jun 16); **whiteboards = own milestone, own solution** (Lead spike first, epic filed, not in the cutover cycle); **PDF view/open now, annotation parity = future milestone**; **perf is first-class parity** ("tesela can be slow"). Epics filed: tesela-ewj (import-engine adoption), tesela-8zd (Logseq parity), tesela-u1t (perf), whiteboards; nnm got its 4 children + full dep wiring (nnm.3 runbook gates the trial on the whole cutover-critical set).

- **The central fault line:** the Logseq importer predates the Loro flag-day — all three surfaces write via fs::write, imported notes get no Loro docs, never sync; 44% of the live mosaic (250/560) is engine-invisible TODAY with no guard; conflict Overwrites silently revert on next materialization. Same one-writer class: MCP get_daily_note, Reminders writeback, boot-scan pages, upsert_blocks lifecycle roll.
- **ADR: engine-as-sole-writer for imports** (panel-revised): apply_plan gets a per-item writer param; hydrated items are written ONLY via NoteUpsert (never fs::write-then-hydrate — that re-creates the double-writer race killed 2026-05-26, notes.rs:387-398); server apply-logseq route = canonical hydration site; /import-logseq's CLI-subprocess shell-out dies (flock self-deadlock); non-active-mosaic imports open a temporary locked engine; reuse the EXISTING hydrate_note (cli mosaic_notes.rs:120) + extract the 5 duplicated blake3-slug helpers into one.
- **ADR: wikilink normalization at index+lookup** (not importer rewrite): priority exact-slug > title > alias, oldest-note tie-break, collisions surfaced never silent; ONE canonical normalize fn (four disagreeing implementations exist today; iOS drops the parent segment of [[Parent/Child]] via path.last); indexed sqlite title lookup, not the O(n) get_by_title walk; 3-engine conformance fixture.
- **ADR: block refs are bid-native via the EXISTING `<!-- bid:uuid -->` pre-stamp** (Sonnet 5 found it): importer stops stripping id:: and embeds it as the bid comment — the Logseq uuid BECOMES the bid; resolution rides the existing global block_index; duplicate-id:: mints fresh + warns. No new mapping subsystem.
- **ADR: attachment cross-device availability needs a Lead spike** (M3's catch — the plan had a web route but nothing moves 91MB of assets to the iPhone; assets don't ride the relay): spike decides encrypted relay blob store vs descope-for-trial; web view/paste beads proceed regardless.
- **ADR: ofu debounce may not ship without a durability decision** — no local WAL exists (oplog/retention.rs is a stub); today is crash-safe only because every op flushes synchronously. Debounce ≤ relay tick, flush on backgrounding/blur/SIGTERM, documented+tested loss bound, or a lightweight op journal.
- **Found by the panel, filed:** relay snapshot UPSERT lacks a covers_seq guard (stale device can overwrite fresher snapshot; both impls; tesela-gqd); deposit tick's hash-skip can't gate the export (hash computed FROM the export — gate on doc_version); iOS on-demand fetch needs a per-note relay endpoint on both impls (or accept full-refetch with rehearsal numbers); restore rehearsal runs against a COPY group and probes interleaved deposits.
- **Fleet:** GPT 5.6 adopted on Taylor's direction (sol=Architect/Fable, terra=Opus, luna=Sonnet tiers; usable from 01:00 CDT 2026-07-10 — rate-limited at adoption). Panel scores: Sonnet 5 review 5/5 (3 design blockers + the bid-prestamp find), GLM 5.2 via ollama-cloud 5/5 (opencode-go lane silent-dropped AGAIN — fallback chain load-bearing), MiniMax M3 4/5 content but burned its budget in <think> (known failure mode; pre-digest or schema-drive it next time).
- **Housekeeping:** epics sclr, cmdd, engc closed (all children done; engc's verify gate run green after fixing the stale Inbox→Views test expectation, 727ad184); mmos deliberately stays open (only its spec child exists; committed rearchitecture, sequenced after lazy-load).

### 2026-07-10 — GPT-5.6 fleet orchestration cycle (Fable-led): 6 parity features + spec pipeline

Fable orchestrated a first full GPT-5.6 fleet (Taylor-directed 5.6-only, all three tiers), reviewing/merging/scoring every lane. Pattern: worktree-per-lane, one bead per dispatch, serial within a cluster, Lead reviews diff → merges → scores.

- **Shipped to main (6 features, all Lead-reviewed + web suite 805 green):** FTS content search in ⌘K with jump-to-block (tesela-8zd.10); attachments HTTP route + relative-image render (8zd.1); paste/drop image upload (8zd.2); PDF view/open (8zd.4); rail de-decoration — favorites writer + navigating rows + live Tasks widget (8zd.13); honest ambient sync dot blending real relay health (ewj.8); block move/rearrange keyboard ops (8zd.15).
- **Security fix (automated commit review caught it):** attachment responses now carry `Content-Security-Policy: default-src 'none'; sandbox` + `nosniff` — closes the SVG-served-inline-on-app-origin XSS class. Verified live on the running embed.
- **tesela-myh interim gate SHIPPED (Sol, TDD):** reseed_from_disk now skips-and-warns on any note whose parse→serialize round trip isn't content-preserving (headings/prose), instead of silently deleting content. `stamp_is_content_preserving` made pub in tesela-core. The durable representation fix (tesela-wt5 class) still required before the real-graph cutover import; tesela-myh stays open + gating nnm.3.
- **Lead-spec pipeline established (draft→adversarial-reject→revise, converged in ONE round):** Terra drafted wikilink-normalization (8zd.5), block-refs (8zd.7), attachment-sync (8zd.3); Sol adversarially REJECTED all three (28 grounded findings); Terra revised addressing every one with 0 contested. Specs now on main + implementation-ready. Load-bearing corrections: wikilink resolution is a tesela-sync/Loro-index+FFI capability NOT server-side (relay-only iOS with Mac asleep can't reach tesela-server); tie-break uses a durable creation-order field in the Loro index NOT filesystem mtime (nondeterministic across devices); SQLite COLLATE NOCASE explicitly rejected as a Unicode normalizer; attachment metadata gets a full Loro CRDT registry + AttachmentManifestV1 + causal-delete + Rust-relay capability-discovery gate + GroupKey-restore-before-open. Findings are recorded in each bead's notes.
- **Fleet scorecard (11 dispatches logged):** Luna 6/6 Senior-band closes when decisions are pre-baked (it choice-seeks on real ambiguity); Sol Architect-tier on both the engine gate and the spec review (Fable-comparable review depth); Terra a strong drafter that needs the adversarial pass to be executable — the Terra+Sol pair IS the pipeline. One 8zd.15 lane mis-routed to gpt-5.5 by a wrapper retry typo (flagged via pi session logs, not trusted from the wrapper's self-report). pi-dispatch wrapper gotcha: its 10-min pgrep boundary reports a spurious timeout while the work completes — give implementation lanes longer windows.
- **Housekeeping filed:** tesela-64g upgraded (sigterm_triggers_validated_backup now fails CONSISTENTLY, not just under load — poisons every `cargo test -p tesela-server` gate); a workspace rustfmt-normalization bead (tesela-core has weeks-old drift blocking `cargo fmt --all --check`); iOS block-move (tesela-doo) + web drag-drop (tesela-b54) discovered from 8zd.15.

### 2026-07-11 — Non-bullet Markdown canonicalizes into ordinary CRDT blocks

**Decision:** close `tesela-myh` with a fence-aware canonical lift, not a raw-content sidecar or mixed-kind Loro schema. Any heading, prose paragraph, unindented fence, or diagram the current parser cannot model becomes an ordinary top-level block. The original source syntax may change (`# H` materializes as a heading inside a bid-bearing list block), but every nonblank semantic payload must survive, display, edit, sync, restart, and re-import. This applies the 2026-05-28 structured-parity lock: Loro is truth; Markdown is a deterministic materialized view.

**Why:** ordinary blocks reuse the existing `blocks` tree, stable bids, `text_seq`, properties, relay snapshots, FFI, web, and iOS immediately; old clients understand them, so there is no fleet gate. A root full-body mirror is rejected because it previously doubled snapshot payload. A separate anchored raw store is rejected because block delete/move/old-client insertion makes ownership and order ambiguous. Mixed `kind=raw` nodes preserve order but old engines interpret every tree child as a bullet, requiring an unavailable fleet-negotiation gate.

**Canonical fence rule:** a fence-first block emits a bid-only bullet, then the complete fence as continuations. Putting the bid on the opener or closer is forbidden because it corrupts fence grammar. Blank lines outside fences may normalize; fence payload and extra indentation may not. The strict startup stamper remains conservative; explicit engine hydration may canonicalize only after a structural projection proves no nonblank content was dropped.

**Identity safety:** parsing exposes per-block provenance for ids minted from bidless source. An exact unstamped NoteUpsert replay may rebind only those minted ids to the resident ordered structure; explicit bids are immutable anchors. A changed unstamped payload against any resident block history fails closed because position cannot distinguish edit, insertion, replacement, reorder, or stale resurrection. The caller must rebase on the canonical bid-bearing materialization. Legacy `root.content` retires only when the incoming full structural projection matches it (including every legacy explicit bid); the matching migration first lifts legacy-only regions into the block tree, then removes the duplicate body. This chooses an explicit retry over silent identity guessing or data resurrection.

**Scope boundary:** preserved fenced `query` source is visible but not automatically executable; Logseq datalog-to-Tesela-query translation remains separate parity work. The sibling reorder durability gap found during review is also separate. Full spec: `phases/2026-07-11-nonbullet-canonical-lift-spec.md`.

### 2026-07-12 — Logseq import writes through the addressed Loro engine

**Decision:** every non-dry Logseq apply hydrates `NoteUpsert`s through the engine; the importer never writes note Markdown directly. An active-mosaic request reuses the server's resident engine and existing flock. Any other mosaic gets a temporary engine while its own server lock is held. The standalone CLI also acquires that lock, so it fails visibly instead of racing a server.

**Batch durability:** unique-note imports run with bounded concurrency while each note keeps its apply lock through CRDT mutation, checked snapshot persistence, and checked Markdown materialization. Only the rebuildable shared index checkpoint is deferred to the end. Successful notes remain committed on a partial failure; detailed failures are returned, and the CLI exits nonzero. On restart, the loaded index's complete title/slug/tag/link projection is compared with durable note snapshots and rebuilt on any drift, including same-ID overwrites and ghost-only indexes. This removes the O(n²) index rewrite without widening the per-note mutation-to-snapshot crash window.

### 2026-07-12 — Desktop minimum macOS 12 matches the transcription runtime

**Decision:** declare `bundle.macOS.minimumSystemVersion` as `12.0`. Tauri otherwise targets macOS 10.13, but the bundled transcribe.cpp/ggml backend requires C++ `std::filesystem` (macOS 10.15) and calls Metal shared-event synchronization APIs introduced in macOS 12 without a compatibility guard. macOS 12 is therefore the first supportable deployment target, not merely a compiler workaround.

An environment-only `MACOSX_DEPLOYMENT_TARGET` override was rejected: it would leave the canonical build script broken on a clean machine and let the app metadata overstate runtime compatibility. Keeping the minimum in `tauri.conf.json` makes Tauri apply the same boundary to compilation and the installed bundle's `LSMinimumSystemVersion`.

### 2026-07-12 — A paired desktop embed activates the mosaic's relay configuration

**Decision:** embedded desktop mode resolves its relay URL from `TESELA_EMBED_RELAY_URL` / `desktop.toml` first, then falls back to the selected mosaic's `[sync.relay]` configuration. Pairing writes that mosaic configuration and reports that a restart is required; the restarted app must therefore consume it rather than forcing `TESELA_DISABLE_RELAY` and remaining LAN-only.

The earlier explicit-opt-in-only rule was meant to prevent a desktop embed and standalone server from joining the relay with the same device identity. That duplicate-writer state cannot occur through the supported embedded path because `serve()` holds the mosaic's exclusive server flock for the app lifetime; a standalone server on the same mosaic makes the app fail to start. Explicit desktop configuration remains the highest-precedence escape hatch, while an unpaired mosaic with no relay remains loopback/LAN-only.
