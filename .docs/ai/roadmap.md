# Tesela Roadmap

## Product Roadmap — refined order (2026-06-03, Taylor + agent)

Taylor's proposed order, refined together. Arc = **infra → platform → features → collaboration**. Sync spec: `phases/2026-06-03-encrypted-replica-spine-spec.md`.

1. **Cloud sync from Cloudflare** ← IN PROGRESS (encrypted-replica spine). Phase 1a durable encrypted relay backup + restore ✓ `e936f56`; 1b-i relay snapshot store + snapshot-gated compaction ✓ `e8be948`; 1b-ii client snapshot-export + bootstrap-from-snapshot ✓ `30f66c7` (full loop e2e). **Phase 2 ✓ (local proof) `397fc30`+`348603a`: the Cloudflare Worker relay (DO-per-group, DO-SQLite) passes the SAME conformance suite the Rust relay does — 19/19 on both via `wrangler dev`** (harness honors `TESELA_RELAY_CONFORMANCE_URL`). 1b-iii SERVER ✓ `a845dbf` — snapshot auto-deposit cadence + bootstrap-from-snapshot wired into the live `sync_relay::tick`/bring-up (env-tunable interval; unit-tested: live tick → relay compacts → fresh engine restores byte-identical). Remaining for step 1: production `wrangler deploy` (needs Taylor's CF account + free-tier check); the iOS `RelayTicker` mirror (needs new FFI `put_snapshots`/`fetch_snapshots`); re-point clients at the cloud URL + demote the Mac from hub (the relay tick is dormant in prod today — Mac-hub WS is the live path). **Pull the MINIMUM key/pairing model in here** — cloud sync isn't usable beyond already-paired devices without it (the "user password" of step 4 is part of this same key model). ⚠ Follow-up #195: Rust relay `insert_op` seq resets after full compaction (Worker is correct).

> **▶ PRIORITY (2026-06-08, Taylor):** the immediate always-on multi-device spine = the **Home-Assistant-hosted encrypted relay** (already running on his HA box) — point the desktop embed (drop its `TESELA_DISABLE_RELAY=1`) + iOS at it, so devices sync with the Mac off. **THEN** migrate to the Cloudflare relay. **NOT a Mac-as-hub** (a stopgap Mac-hub + desktop remote-connect mode were built today, then reverted — see current-state 2026-06-08). The desktop is back to its self-contained loopback embed.
> **▶ TOPOLOGY LOCKED (2026-06-09, Taylor):** relay = zero-knowledge MAILBOX, never an authority (CRDT needs no mediator). HA carries sync NOW → **CF Worker becomes the ONE canonical production spine** once deployed/proven; the Rust relay is then a conformance-frozen self-host option. LAN device↔device P2P stays roadmap step 6 (pure availability upside; idempotent imports make multi-path safe). Mac-hub WS demoted/retired as a device transport in Milestone 3. ⚠ The shipped relay path has confirmed criticals — see the 2026-06-09 audit item in Now; HOLD the product test until Stream A re-ships.
2. **Native desktop app.** ✅ **DECISION LOCKED: Tauri-wrap `/g`** (step 3 feature work then SHARED by web+desktop), NOT a fresh SwiftUI Mac app. Reasoning in `decisions.md` 2026-06-04. **MVP WORKING (2026-06-04):** native window + a child `tesela-server` bound to loopback serving the API + the static `/g` UI same-origin; verified on the real 100-note mosaic (UI + live WS + Loro collab). Single-writer `flock` enforced (the data-safety invariant), parent-death watchdog, embed is a loopback Loro-replica node (relay/peer-sync/mDNS off). Spec: `phases/2026-06-03-tauri-desktop-spec.md`. **Remaining:** `tauri build` → codesign/notarize a real `.app`; native menus/tray/auto-update; `tauri-plugin-single-instance` (focus-existing UX, #202); lib-embed `serve()`; then → step 3.
3. **Mature apps for daily use** — DECOMPOSE (4+ milestones, sequence by daily-driver pain): (a) editor/render — markdown render, code blocks, vim polish (daily-blocking, interleave early); (b) properties + types system (`project_property_system_vision`); (c) widgets / AnyType rail; (d) whiteboarding = its own BIG bet, defer past Logseq-DB parity. Goal: full Logseq-DB replacement.
4. **Onboarding + pairing + sync maturity + user-provided encryption password** (passphrase-derived key). Min slice pulled into 1.
5. **Web: standalone OR connected-to-sync.** ⚠ Question standalone-web (local Loro engine + offline = lots of work for a non-endgame surface). "Web connected to the cloud spine" is nearly free (falls out of 1+2). Likely shrink 5 to that; defer/drop standalone-web.
6. **P2P mode (iOS/iPad/macOS).** LAN direct device↔device over the same sealed Loro deltas + the Cloud↔Local-only toggle. (Browsers can't LAN-P2P.) Independent of 7.
7. **Real multi-device — SPLIT:** (7a) presence/remote cursors for YOUR OWN devices — works over any transport, moderate; (7b) TRUE multi-user (accounts, Ed25519 identity, ACL, "see Savanne") — a MASSIVE phase of its own (`project_savanne_collaborator`).
8. sharing/publishing • app-store distribution + auto-update • history/audit + per-author attribution • AI/agent over your notes (MCP) • monetization/sustainability.

> ⚠ **PRE-PUBLIC-RELEASE GATE (export compliance) — blocks step 8 / any public App Store release.** The app ships standard (exempt) encryption (ChaCha20-Poly1305 sync E2E, etc.). Before publishing publicly you MUST: **deselect France** in App Store Connect → Pricing & Availability (we declared "not available in France" for the exempt path — publishing there = breach), file the **US BIS §740.17 self-classification report**, and review other restricted markets. Full ADR + checklist: `decisions.md` 2026-06-08 "App Store export compliance". TestFlight/internal is unaffected.

> 📣 **PRE-LAUNCH DELIVERABLES (part of step 8) — placeholdered for now:**
> - [ ] **Real marketing / landing page** — product site for the public launch (what Tesela is, screenshots, download/App-Store links).
> - [ ] **Privacy policy page, published at a stable URL** — REQUIRED by Apple (the App Store Connect app record needs a privacy-policy URL before public release; also wanted for TestFlight external testing). Must reflect what we actually collect/sync (local-first, E2E-encrypted sync, no accounts yet) + the cloud-relay backup. Link it from the marketing page + the App Store listing.

**Cross-cutting (not one step):** backup/restore + DR UX (you now HAVE the encrypted cloud backup → surface "restore" + local export); reliability + observability (sync-status indicator, conflict surfacing, telemetry); public-endpoint security hardening (rate-limit/abuse) + **Ed25519 device identity** (pairing has no identity binding today — mandatory once Savanne/public CF exists); conflict/merge UX for non-text (block delete-vs-edit, multi-value props LWW); import/export (Obsidian/Logseq/Notion); perf + search at scale. **Meta:** infra-first is right (multi-device is the current pain), but Taylor daily-drives this → interleave cheap daily wins (markdown/code render) during 1–2.

---

## Now / Next / Later

Active items. Trim as completed.

### Now

> **▶▶ CONVERGENCE FIX — layer 1 LANDED, layer 2 SPECCED (2026-06-26).** The remaining sync problem (actively-edited notes drift iOS↔desktop) is the project's disjoint-lineage residue (`project_multidevice_convergence`) **+ a loro 1.12 library bug**: blocks edited on both devices FORK into disjoint Loro lineages; reconciling the twins panicked loro 1.12 richtext (`insert_elem_at_entity_index` OOB) → crash-loop, and the build-52 containment (`cdb4a0ec`) skipped the merge → permanent drift (block `019f047a`: desktop "Brook" vs iOS "Bro"). decisions.md 2026-06-26.
>   - [x] **Layer 1 — loro 1.12 → 1.13.6 upgrade** (`e884edc2`). 1.13 fixes the crash class (1.13.3 out-of-order-import panic; 1.13.2 ATOMIC import w/ rollback so a bad frame errors cleanly, not poisons the doc; broad import hardening). Stops the crash AND lets the existing dedup/heal CONVERGE already-forked twins (instead of crashing/skipping) → heals existing drift once both devices are on 1.13.6. Full tesela-sync suite green; containment kept as defense-in-depth. **Ships in: desktop rebuild (needs Taylor's /Applications install) + iOS build 53.**
>   - [ ] **Layer 2 — rebase-on-relay-inbound (no-data-loss fix). NOTE: mergeable containers was the WRONG plan** — a 5-agent verification workflow proved it can't fix a TREE-NODE fork (each device mints a different TreeID for the same bid → different meta maps → mergeable child IDs `hash(parent_map_id,…)` still diverge; mergeable only helps when the parent node is already shared). Corrected spec: `phases/2026-06-26-mergeable-containers-spec.md`. Real fix: the engine ALREADY has the heal (`import_authoritative_snapshot` + `rebase_twins_onto_snapshot`, proven by `disjoint_device_authoritative_rebase_then_converges`); the gap is TRIGGERING — the relay-inbound apply does lossy min-TreeID dedup, NOT rebase, and `.relay` mode has no HTTP shared-base path (`fetchLoroSnapshot` 404s on the CF mailbox). Fix = make relay-inbound REBASE divergent twins onto a deterministic (min-TreeID) winner, re-applying the loser's genuine edits — backend-agnostic, self-heals existing + future forks. ⚠ convergence-critical apply path (clobber-bug surface) → TDD, one change, full re-test; do NOT rush. Layer 1 (1.13.6) already heals existing forks via lossy dedup — **verify that first**.
>   - [ ] **June 25/26 stuck-fork residue** (Taylor 2026-06-27): the ONLY past days that fail on iOS (read-only, deletes don't stick, desktop edits don't show) = the two convergence-saga-drifted notes; clean days + today work (routing is identical to today — ruled out via test `ad41929f`). This IS the layer-2 case (lossy dedup didn't fully converge those deep forks). Heals when layer-2 lands; interim nudge = edit them on desktop to re-broadcast. task #11.

> **▶▶ NORTH STAR ARC — multi-device live presence + cursors (collab).** Spec: `phases/2026-06-27-multidevice-presence-spec.md` (researched 2026-06-27, 5-agent workflow, adversarial-verified). loro 1.13.6 gives BOTH primitives FREE (verified): `EphemeralStore` (LWW presence) + stable `Cursor` (op-anchored, survives concurrent edits) — neither in `tesela-sync-ffi` yet. Transport crux: WS has in-memory broadcast (desktop real-time ✅); the **CF relay is store-and-poll — no real-time broadcast**, so iOS-over-relay needs a NEW channel (recommend a **CF Durable-Object WebSocket** for ephemeral presence alongside the unchanged store-poll ops). **Gated on layer-2 convergence** (a cursor on a block whose twin gets tombstoned goes stale). Phases: 0=layer-2 foundation · 1=FFI-wrap Cursor+EphemeralStore · 2=desktop presence over WS · 3=iOS (hub-mode WS → CF-DO WS) · 4=collab polish (selections, names, follow, Savanne). ⚠ confirm the editor caret-read/render APIs (one research agent failed) before Phase 2.

> **▶▶ CURRENT (2026-06-24): build 46/47 device-test fix batch.** Taylor device-tested builds 46/47 → 5 findings. **[1] iOS↔web sync drift/clobber (looked like DATA LOSS)** root-caused to a sync-LIVENESS bug, NOT push logic (edits do converge): `RelayTicker` backoff slept `2 * 2^min(errs,12)` ≈ 8192s ≈ 2.3h between ticks (every comment claimed ~60s), and `.active → start()` couldn't wake a loop parked in that sleep. FIXED `e6d1d83b` — cap backoff ≤60s via a pure, unit-tested `backoffSleepSeconds`; add `wake()` (reset+restart, immediate tick) on foreground in BOTH shells; `RelayBackoffTests` regression guard; full TeselaTests green. Diagnosed live against the desktop's CF-relay state (iPhone WAS on the same relay/group — inbound seq matched; edits converged ~2h late, then instantly on app reopen = the bug's signature). decisions.md 2026-06-24. **Build 48 → TestFlight** (sync liveness `e6d1d83b` + date chip `5c65e9d2`; full TeselaTests 272 green). **Then build 49** (`594b0403`): web→iOS delete-not-propagating root-caused via a sim repro (foreground path WORKS on 48; the gap was background APNs wake) → APNs token registration now relay-scoped (HA→CF migration left CF with no token). Full suite 276 green.
> - [x] **iOS: date indicator while a block is focused** — SHIPPED `5c65e9d2` (build 48): `BlockRow` gated the whole chip row on `!isEditing`; dates are structured (not in the edited prose) so they now stay visible while editing. Pure `chipVisibility` + `BlockRowChipVisibilityTests`.
> - [x] **web→iOS delete not propagating (build 48 device test)** — was the BACKGROUND APNs-wake gap, NOT the foreground path (sim repro proved foreground works ~5s on 48). APNs token registration was token-only/once-per-session → after HA→CF migration CF had no token → no background wake. FIXED `594b0403` (build 49): relay-scoped registration (`apnsRegistrationKey`). decisions.md 2026-06-24. ⚠ APNs is best-effort (iOS throttles silent pushes); the reliable path is foreground sync. **[ ] Taylor verify:** delete on web, leave phone asleep a while, reopen → should be current (and over a day, should converge even while asleep).
> - [ ] **iOS: slash deep-filter parity** — `/p1` should jump to set priority p1 (web's `flattenedSlashFilter`); the iOS slash menu lacks the deep type-to-filter. Bounded port of the web flatten logic into `SlashVerbs`.
> - [ ] **iOS: inline NLP not firing** — NOT a data or logic-asymmetry bug (ruled out 2026-06-24): `priority.md` has `nl_triggers: ["p1".."p4"]`, and web (`task-tokens.ts:147`) + iOS (`EditorAutocomplete:367`) BOTH require `nl_triggers` non-empty + a choice match. Detector IS wired (`BlockRow:508 InlineNLP.detect`, gated on the block's resolved tag defs). Needs a SIM repro to pin: (a) the block must carry a tag bringing the prop def — typing "p1" in an untagged block lifts nothing; (b) detector-invocation in the live editor; (c) the deliberate build-47 "NLP intent gate" hardening (`8f34a96a`) may have over-tightened. ⚠ Don't speculative-fix — could regress the over-offer guards.
> - [ ] **Per-type / per-property color + logo customization** (Taylor, 2026-06-24) — let the user set a type's/property's chip color + icon/logo, the way Anytype and Logseq DB do. Follow-up to the per-type choice colors already shipped; web + iOS.
> - [ ] **Sync UX honesty (follow-up)** — Settings → Sync should show the iPhone's OWN relay URL + a pending/last-successful-push age; in relay mode stop showing the dead `127.0.0.1:7474` "Connected" (it actively misled this diagnosis). Lower urgency now that the backoff is capped.

> **▶▶ CURRENT (2026-06-23): TYPE SYSTEM + iOS PARITY SHIPPED.** Per-type property config (web Phases 1-4: per-type override choices/visibility/default, Tabler icons, plurals, config UI, choice colors — spec `phases/2026-06-22-per-type-property-config-spec.md`) + the full **iOS property-registry parity** (P5.1-5.6: registry built client-side from synced pages, date authoring, registry-driven slash + inline NLP, chip colors — spec `phases/2026-06-23-ios-property-registry-parity-spec.md`), all on `main` + pushed. Desktop rebuilt (#73 CLOSED); `tesela reindex` `cmd_reindex` bug fixed; Taylor's 15 live type pages upgraded in place. A post-ship 3-finder audit caught a BLOCKER (iOS structured writes silently no-opped on a bare bid) + 6 more — all fixed; build-47 hardening on top. **TestFlight builds 43 / 46 / 47** (46 = working authoring). decisions.md 2026-06-22/23. **Now awaiting Taylor's device test of 46/47** — findings → next fix batch.
> - [ ] **Type-system VIEWS (keyboard-first)** — the deferred hard part of the type system: kanban / sets per type (group blocks by a select property, table/list/kanban), web + iOS. Next type-system milestone (the spec deferred views as "the hard part there").
> - [ ] **Engine-side raw-lines root cure** (`reconcile_tree_to_blocks` strip-and-lift, durable for ALL clients) — DEFERRED, fleet-gated on `migrate_in_text` (old FFI can't read the lifted container → fleet-wide property-erase risk). A Rust/relay change needing fleet coordination, NOT an iOS build; the iOS display strip handles the symptom.

> **▶▶ CURRENT (2026-06-18): iOS editor sprint SHIPPED (TestFlight builds 21–28, all pushed + Opus-verified).** Marker unification; Enter indent-inherit + empty-outdent + insert-after-cursor; word-wrap (`sizeThatFits`); capture target-swatch menu; **`[[` / `#` / `/` inline autocomplete** on one trigger-detection framework (`EditorAutocomplete`/`LinkSuggest`; complete page+tag source via new FFI `index_entries()` over the Loro index — fixes the lazy-materialization gap); Graphite **Search view** (gpt-5.5 via pi). See git log + `decisions.md` 2026-06-18. **Also shipped:** arch-review eval (3 open-source reports → ~20% signal) + the C23 backup-restore guard + C19/C20/C21/C24/C6 hygiene batch.
> - [ ] **#64 iOS mobile command palette** — toolbar button → searchable command registry (the `:`/leader stand-in). Needs the registry surfaced to iOS.
> - [ ] **#65 iOS capture sheet footer clipped behind keyboard** (intermittent).

> **▶▶ CURRENT (2026-06-13): see [`phases/2026-06-13-backlog.md`](phases/2026-06-13-backlog.md)** — the live tier-routed backlog. Stream A + Stream B below are **SHIPPED** (historical). Command-registry B1–B4 merged (`4766111`). Opus = Lead/XL + review; fleet (gpt-5.5/minimax) = S/M. Opus on sync (HA-first, defer CF).

**▶▶ QUERY SYNTAX = JQL, not the colon DSL (Taylor, 2026-06-15).** Taylor prefers a **JQL-style** query language (`type = task AND points > 5`, `status IN (todo, doing)`, `priority >= 2 ORDER BY due ASC`) over the current colon DSL (`tag:task points:>5`). **Key fact from recon:** the Rust (`query.rs`) AND web (`query-language.ts`) engines ALREADY ship a full recursive-descent JQL parser (shared `BoolExpr`/`Predicate` types: `AND`/`OR`/parens/`IN`/`NOT IN`/`LIKE`/`BETWEEN`/`ORDER BY`/infix ops), with the colon DSL as a legacy sugar in the same grammar. So this is mostly a FRONT-END + parity job, not a new parser. Phased:
> - **P1 — JQL-first authoring (web)** ✅ DONE `541a8322`: `/query` inserts `query:: type = `; query-block + saved-view placeholders + validation hint all JQL; `kind = page` validates; **`ORDER BY` now sorts the inline block (L5-typed via `applySort`)**. Verified end-to-end (5-agent trace confirmed JQL↔DSL parity; 8 applySort tests + 434 suite + svelte-check + browser self-QA). Product test: `~/.harness/reports/tesela/20260615-jql-queries/`.
> - **P2 — iOS parity** ✅ DONE: spec `phases/2026-06-15-jql-ios-parity-spec.md`. Phase A `0d6b3f77` (41 JQL cases) + impl `a38ed68d` (restructured `LocalQueryEngine` flat-AST → Rust `BoolExpr` tree: `parseOr/parseAnd/parseUnary/parsePredicate`, real OR/parens/IN/NOT IN/LIKE/NOT LIKE/BETWEEN/IS NULL, `evalExpr` tree-walk + `likeMatches`) + hardening `cc9d316e` (28 adversarial cases). **Verified: all 3 engines (Rust+web+iOS) green on 182 conformance cases; full iOS suite 173 green.** Opus line-by-line reviewed the parser/matcher + caught the LIKE escape-set difference (verified harmless). **iOS build 16 needed for on-device test** (Taylor's OK).
> - **P3 — polish:** JQL examples in the command palette / docs; optional DSL→JQL one-shot converter for existing saved views; error surfacing on malformed JQL.

**▶▶ COMMAND-MODEL REDESIGN — SHIPPED to main 2026-06-16/17 (`d8b26944`).** Unified `/` `:` `Space` `⌘K` over one registry (wide which-key leader, `Ctrl+,` insert-leader, slash type-to-filter, colon verb-fold). Browser-QA + a 20-agent adversarial pre-merge sweep caught + fixed 4 ship-blockers the gates missed (2× `each_key_duplicate` crash — leader overlay + slash value-picker; accelerator case-collapse; stranded highlight). Taylor verdict "ship + tweak": the tweak = **slash DEEP type-to-filter** (`/p1` jumps straight to the buried `Properties › Priority › p1` leaf via `flattenedSlashFilter`) — SHIPPED `2d161d8c` (6 unit tests + browser-verified `/manual` → `Properties › Manual…`).
> - [x] **Leader→editor wiring DONE** (`9c7983ba`, 2026-06-17). `Space → i/p` run editor verbs on the focused block via a `tesela:run-editor-command` bridge (mirrors `g f`) + `editorFocused` presence (new `focused-editor` store) + whole-block leader context. 8-agent adversarial review + browser-QA caught + fixed 2 defect classes unit tests missed (per-outliner `focused`-prop double-mutation → `view.hasFocus`; mid-caret word-merge → whole-block context). check 0 / 475 unit (+5). Spec `phases/2026-06-17-leader-editor-wiring-spec.md`; decisions.md has the invariants. **Pending: Taylor's desktop `Space→i→h` tap** (the literal keypress couldn't be driven headlessly) + push.

**▶▶ ACTIVE (2026-06-09 audit + product review): TWO PARALLEL STREAMS — spec [`phases/2026-06-09-audit-hardening-spec.md`](phases/2026-06-09-audit-hardening-spec.md).** Ultracode bug bash (91 confirmed findings, adversarially verified) + arch review (42 fact-checked recs); full report `~/.harness/reports/tesela/20260609-bugbash-arch-review/`. Product-review decisions in `decisions.md` 2026-06-09.
- [x] **Stream A — relay hardening CODE DONE + PROVEN** (verified 2026-06-17; roadmap was stale — landed in intervening work with `audit AX` annotations). **A1** relay seq above compaction watermark (`61506af7`, store.rs `MAX(MAX(seq),compaction_seq)+1`); **A3** poison-envelope skip; **A4/A5** cursor-past-failure retry-bound + relay/group-scoped cursors (sync_relay.rs); **A6** iOS `.relay` read AND write (`182210d7`, MockMosaicService gates + Mock-seed clear); **A7** honest sync status; **A8** mojibake fixed; **A9** PUT propagates record_local failure. **PROVEN:** `cargo test -p tesela-relay` conformance 23/0 + convergence harness (A12) 5/0 — incl. the #195 seq regression, poison-envelope, cursor-restart, compaction-boundary convergence. **All in iOS build 16** (e311ae8c — A1+A6; nothing landed after it) + the 06-15 desktop binary (server fixes ≤06-13) + HA add-on **0.2.2** (auto-published to GHCR on the bump). **A10 PARTIAL** (auto-sync default-OFF containment done; recur-bump `store.update` re-route → M3). A11 CI fmt unblocked (`24f8af67`); A13 iOS-unit/FFI-drift status unverified. **REMAINING = operational rollout (Taylor):** install build 16 on Roshar+Sel · update HA add-on → 0.2.2 · relaunch desktop. Product test → `~/.harness/reports/tesela/20260617-relay-rollout/` (replaces the stale build-6 `20260609-relay-sync-rollout`, now done).
- [x] **Stream B — Graphite cutover COMPLETE** (verified 2026-06-17; roadmap was stale — all landed in intervening work). **B1 (7 /g parity bugs) + B4 (5 web-editor invariant fixes)** verified fixed (5-agent triage + 7 spot-checks, 0 false-fixed): BlockOpsSaver kind-aware coalescing, triage→`api.setBlockProperty`, JournalView bounded `dailyWalkDates`, `gotoNote` /g short-circuit, PeekPopover mounted (GraphiteShell:326) + `open-leader-at` listener (269) + shell ColonCommandLine, `applyRemoteTextEvent`→`deltaToChanges`, Enter/Backspace guard. **B2 flip `/`→/g default** = `b46b756e` (+page.ts redirect). **B3 delete v4/v5 chromes** = `a12a8049` (`feat(web)!`; v4/+page.ts is a redirect stub, no v5 route, `lib/v4`+`lib/v5` behavior modules preserved). Builds + runs /g-only clean all session. **Retrospective parity check DONE 2026-06-17** (mapped all 24 deleted chrome files → /g equivalents): palette (Station→GrCommandPalette **+ fuzzy** — v4_phase6 gap closed), voice/backlinks/note-render/splits/rail/status/pinned/recent all ✓; SearchSurface→⌘K; derived+ambient (backlinks/outline/tasks/local-graph/calendar/agenda/inbox/dashboard/graph) reachable via leader v/g chords; tag VIEW via tag pages + query blocks. **Only 2 dedicated BROWSE surfaces dropped, functionally covered but no dedicated UI — confirm intentional:** v5 `NotesTree` (browse-all-notes → now ⌘K find-by-name only) + `TagsSurface` (all-tags index → now tag pages + query blocks + graph, no flat index). Behavior modules `lib/v4`+`lib/v5` preserved.
- **Milestone 3 (decided): finish the sync spine** — CF Worker deploy (canonical spine; HA relay → conformance-frozen self-host option; LAN P2P stays step 6) + minimum key/pairing model + cursor migration + demote Mac-hub WS + Reminders/recur-bump engine re-route + NoteDelete tombstone design.
- [x] **Audit rec "Back up the authority" — DONE 2026-06-10** (`72c7378`+`1a59e55`+`b7f97aa`): backups now capture `.tesela/loro/` + sync identity (manifest v2; v1 restorable); tesela-server takes scheduled backups (6h default + at startup, env-tunable `TESELA_BACKUP_*`) with GFS pruning; `GET /backup/status` proves it (latest manifest + contents summary + next run); restore drill tests prove nuke→restore → identical Loro VV + device/group identity, NO reseed. ⚠ Audit's in-place-restore-into-running-mosaic hazard (POST /backups/{name}/restore) NOT yet fixed — restore is still safest stopped-engine/CLI. **[ ] USER: relaunch the desktop app (or restart the standalone `tesela-server`)** so the live server runs the scheduler build — fresh binaries already at `target/{release,debug}/tesela-server`.

**ACTIVE (NEW 2026-06-05): Properties + types system — step 3(b).** Full Logseq-DB/AnyType property/type system, structured-first. Spec + arch-review addendum: `phases/2026-06-05-properties-types-spec.md`; decisions: `decisions.md` 2026-06-05. Foundation-first, 6 phases; the **Phase-1 build order (13 TDD steps) is the active `## Plan` in `current-state.md`**. **Landed:** P1.1 typed scalar codec (`tesela-core::property`, `ae2fce1`). **Pending (USER — async on harness-deck `tesela/20260605-properties-product-qs`; non-blocking for engine steps P1.2–P1.9):** 3 product calls — mid-prose property reflow (confirm), out-of-choices guard default, in-editor chip timing. ⚠ Migrate-on-write stays flag-gated **default-OFF** until the whole fleet (incl. old iOS FFI) is read-capable — old builds import the new property containers without error but render them away.

**ACTIVE: instant multi-device sync (Mac-hub WS over Tailscale) — #140.** Spec `phases/2026-05-30-instant-multidevice-spec.md`. Phases 0/A/B/C landed (engine trait + server bidirectional binary-delta /ws + iOS FFI + iOS client). 
> **2026-05-31 — multi-device REVERT bug root-caused + FULLY FIXED.** Web edits vanished with 2 iOS devices open. Cause: disjoint Loro histories (iOS re-authored notes from markdown → different `TreeID`s per bid → Loro unioned twins → stale ghost). Fix (spec `phases/2026-05-31-multidevice-converge-spec.md`, built subagent-driven, repro red→green): [x] E1 deterministic dedup-by-bid heal (`5b05306`,`d1d7b49`), [x] E2 `hubMode` relay-gate + B WS cap (`cc48174`,`09cbb63`), [x] D shared-base bootstrap (`b3b5eef`,`979b2ff`,`2f1b729`,`f381e14`). Server rebuilt+restarted on the fix; Roshar reinstalled clean. [ ] #150 follow-up: iOS snapshot→delta.
> **2026-06-02 — FOUR distinct multi-device data-loss vectors now closed.** (1) disjoint-history twins [E1/D above], (2) stale whole-body PUT [#166/#167 base-diff + #158-161 block-granular], (3) WS snapshot push reverting a peer's HTTP edit [#171/#172/#173 Part C+A], and (4) ⭐ **same-block concurrent text** [#174] — block text was a Loro LWW map register, so two peers editing one block lost a side. **FIXED:** block text → nested **LoroText** sequence CRDT (`69fadc8`, engine-only, zero client/wire change; subagent-built, spec✅+quality-APPROVE two-stage review). Proven 3 ways: engine convergence test + FFI delta round-trip + e2e real-socket same-block-merge test (`43edfee`). DIAG diagnostics removed; server LIVE on the clean LoroText binary (web correct immediately). **[ ] USER: live device round-trip** (web↔iPhone↔iPad converge, no revert) — ⚠ **install the new build on Roshar + Sel FIRST** (old FFI writes the legacy `text` register, shadowed once the server migrates a block to `text_seq` → lost; web is safe). Backlog #177/#178 = non-blocking robustness follow-ups.

**Sync architecture migration to Loro (committed 2026-05-27).** Multi-user (Savanne + Taylor) collaboration is now in scope, which means concurrent edits in a single mosaic stop being the rare pathological case and become the everyday case. The hand-rolled CRDT can't handle this; Loro is the system designed for it. See [decisions.md](decisions.md#2026-05-27) for the full reasoning.

**THE PLAN: [`phases/2026-05-28-loro-cutover-spec.md`](phases/2026-05-28-loro-cutover-spec.md)** — hard cutover to a Loro-authoritative engine, then delete the hand-rolled oplog. Doc model = **hybrid per-note docs + index doc** (NOT single mega-doc — would OOM iOS at scale; see [decisions.md](decisions.md) 2026-05-28). v1 = **full parity** before flip. Goal: kill the convergence "flashing" (LWW ping-pong observed live on Roshar 2026-05-28) that the hand-rolled engine can't fix.

> **2026-05-29 — LORO CUTOVER FINISHED.** Flag-day (`471d619`, ~3.6k lines deleted: SqliteEngine/DualEngine/op-wire gone, Loro is the sole engine), ai-business dedup (`8ef366e`, frontmatter-only root meta → snapshots ~half size), iOS FFI rebuilt + bindings regenerated (`c626d25`, `xcodebuild` BUILD SUCCEEDED), DR drill — all done + green (`cargo test --workspace` 0 failures). LAN P2P (peer_sync) data-plane retired (redundant with the relay spine; reimplement over Loro relay-updates later). Report: [`phases/2026-05-29-loro-cutover-report.md`](phases/2026-05-29-loro-cutover-report.md). **DR drill proved:** restore from `notes/*.md` + reseed rebuilds all 514 notes; ai-business snapshot 5.13 MB → 2.58 MB (now under the 5 MB relay limit).
>
> **One operational step remains (USER-COORDINATED, needs the iPhone):** the live data reset — stop server → backup → `rm -rf <mosaic>/.tesela/loro/` → reseed (`TESELA_LORO_RESEED=1`, one device) → wipe + re-bootstrap the iPhone's local docs — so the dedup lands in production (the size win only applies to fresh docs). Until then the server runs fine on existing docs; ai-business stays unsynced as before. Server launch is now flag-free: `tesela-server --mosaic <real mosaic>`.
>
> **→ ACTIVE MILESTONE: the Graphite redesign** — [`phases/2026-05-29-graphite-redesign-spec.md`](phases/2026-05-29-graphite-redesign-spec.md). Brand-new **SvelteKit web + SwiftUI iOS** frontends to the Graphite design system (clean rebuild, reuse vetted lib logic + Loro FFI/MosaicService), **web + iOS in parallel**, **daily-driver parity then cut over** + delete the old. Phasing: foundation (tokens/icons/primitives) → shell → daily-driver views → cutover → iterate. Rail = AnyType-style widget host. Design source: [`design/graphite/`](design/graphite/). Later: window splits, extra themes, graph/tag-table/settings polish.
>
> - [x] **Foundation** (2026-05-29, `7083956` web + `e316a6f` iOS): shared `tokens.json`, web `--gr-*` token CSS + `/g` tree, iOS `.graphite` Theme; primitives both platforms (GrIcon/Button/Chip/TypeDot/TypeTag/Row/Widget). Gates green (svelte-check clean; xcodebuild SUCCEEDED). Plan: [`phases/2026-05-29-graphite-foundation-plan.md`](phases/2026-05-29-graphite-foundation-plan.md). Deferred: visual parity check of `/g` gallery.
> - [x] **Shell** (2026-05-29, `88e4dfe` web + `c897b98` iOS): web topbar/rail/pane/status + ⌘K palette + leader overlay (new Graphite presentation bound to the EXISTING behavior — `buildV4Commands`/`scoreFuzzy`/`getLeaderTree`/`getWorkspace`/`getVimMode`/`getConnected`, mirroring `v4/+layout` keydown); iOS GrAppShell mirroring the native tab bar + Graphite header + capture sheet, bound to the same MosaicService/RelayTicker. Gates green. Pane/tab content = placeholder. Plan: [`phases/2026-05-29-graphite-shell-plan.md`](phases/2026-05-29-graphite-shell-plan.md). ⚠ Cutover must preserve the reused `lib/v4`+`lib/v5` behavior modules.
> - [x] **Daily-driver views** (2026-05-29, `6a8cbc3` web + `84a4dc0` iOS + `562b192` toggle): daily journal + page outliner (REUSE BlockOutliner/JournalView + a Graphite CM theme), inbox, agenda; iOS Daily/Page/Library/Agenda/Inbox over MosaicService+BlockRow; search = ⌘K/native. **Self-QA'd:** web `/g` renders real data with live editors, ⌘K (39 cmds) + leader (6 chords), 0 errors; iOS sim launches the Graphite shell via `-graphite`. Plan: [`phases/2026-05-29-graphite-views-plan.md`](phases/2026-05-29-graphite-views-plan.md). **Testable now:** http://localhost:5173/g (web, live backend on :7474) + iOS `-graphite` launch arg.
> - [ ] **Toward cutover:** iOS real-data bring-up parity (onboarding/pairing) so `-graphite` shows live data; web day-header pixel-match + edit round-trip confirm; then **cutover** — delete old v4/v5 web + old iOS Views, make GrAppShell the sole entry, **preserve the reused `lib/v4`+`lib/v5` behavior modules**.

**Done (dual-write scaffold + shadow, 2026-05-27/28):**
- [x] Loro spike GREEN (`phases/2026-05-27-loro-spike-report.md`).
- [x] LoroEngine + DualEngine scaffold (`4015dc7`); wired behind `TESELA_LORO_DUAL_WRITE` (`70f9ed2`).
- [x] Block ops + NoteDelete ported; flat insertion-order model matching SqliteEngine (`101b148`,`6b2ccc3`,`80cc60d`).
- [x] Divergence check + debug endpoints `/loro/divergence`,`/loro/notes/:slug` (`e7a3c82`,`8598c15`); shadow persistence + full-corpus disk seed (`3b29ee3`,`ebf9175`).
- [x] Perf: memoized note/block lookups, killed O(oplog) scans (`ab63d1c`).

**Next — cutover Phases 0–7 (see spec):**
1. [x] **Phase 0 spike** — GREEN (`phases/2026-05-28-loro-cutover-spike-report.md`, `8373139`).
2. [x] **Phase 1**: per-note structured content (page properties) → page-prop parity (`25fcbcb`,`74055e5`). Non-bullet body (1 legacy note) deferred — see review findings [5]/[10]/[11].
3. [x] **Phase 2**: index doc — note_id→{title,slug,tags,links} + self-healing versioned rebuild (`c8164d7`,`1b07636`,`902439e`). Verified live: 518 notes, 448 tags, 128 link edges.
4. [x] **Adversarial review of phases 0–2** (29 agents) → 18 findings; 7 fixed (`c33a88d`,`c27818f`,`fad0280`,`ba2fffb`), rest triaged in `phases/2026-05-28-loro-review-findings.md`. Honest divergence: 3/518, all resolved at cutover.
5. [~] **Phase 3** (lazy-load/evict) — RESEQUENCED to ~Phase 6 (iOS-only benefit, no consumer until the FFI swap; decided 2026-05-28). Groundwork landed: resident block_index (`0430616`).
6. [~] **Phase 4 — keystone, step 1+2 DONE** (`80a1cd1`): Loro PeerID↔DeviceId mapping + per-doc update sync (`doc_version`/`export_doc_update`/`import_doc_update`). **Engine-level convergence proven**: two LoroEngines converge on concurrent same-note edits, no flashing, stable (the migration's whole point). Step 3 (wire into the live relay, replacing the `Vec<EncodedOp>` payload) is cutover-adjacent → Phase 5/7.
7. [x] **Phase 5** — live relay wired to the Loro v2 (TLR2 + DEFLATE) payload behind a protocol byte; `TESELA_LORO_AUTHORITATIVE` materializer; broadcast cursor model (`d29e631`). web↔iPhone sync confirmed (2026-05-29).
8. [x] **Phase 6**: iOS FFI swapped to LoroEngine (`open_loro`); `.a` rebuilt + bindings regenerated post-flag-day (`c626d25`, BUILD SUCCEEDED). (Full lazy-load/evict still deferred — iterate-phase.)
9. [x] **Phase 7**: flag-day — deleted SqliteEngine/DualEngine/op-wire (`471d619`), ai-business dedup (`8ef366e`), DR drill done. Loro is the sole engine. Report: `phases/2026-05-29-loro-cutover-report.md`. (Live data reset + iPhone re-bootstrap = user-coordinated follow-up.)

**Patch wave through 2026-05-27 is stable.** 11 commits today closing out Phases 1, 2, 2.1, 2.2 of the sync redesign + bid surfacing + multiple UI ghosting fixes. The hand-rolled engine is in the best state it's ever been; Loro is now the right move because *the next big problem (multi-user)* is not one this engine was designed for, not because the current bugs aren't fixable.

The web client is feature-complete through Phase 2 (Navigation & Discovery): outliner, Vim, slash commands, leader menu, sidebar, command palette, graph, timeline, tag tables, settings, themes, favorites, search highlighting, tag-table filtering, right-sidebar properties, graph filters. Phase 3 candidates below are paused until Loro migration ships.

### Next — Phase 3 candidates (pick one)

**3A: Type System Depth (Anytype vision)**
- Kanban view on tag pages (group blocks by a select property like Status).
- Queries / Sets — saved filters by type + property values, displayed as table/list/kanban.
- Collections — manual groupings of pages.
- Node references — property value links to another page (bidirectional).
- Tag inheritance — `extends` chain (Task → Root Tag), child inherits parent properties.
- Global property registry — search existing property pages when adding to a tag.

**3B: Editor Power Features**
- Visual mode in Vim (character + line selection).
- Block merge on Backspace at start of non-empty block.
- Multi-block selection and operations.
- `/template` slash command — insert from template pages.
- `/date` slash command — date picker UI.
- Block drill-in — focus on a single block and its children.

**3C: Polish & Edge Cases**
- Empty/loading/error states for every view (audit).
- Keyboard shortcuts for favorites (e.g., `f` to toggle).
- Graph: click node → navigate, drag to reposition.
- Right sidebar: inline property editing (not just display).
- Breadcrumb improvements — clickable path segments.
- Mobile/responsive layout considerations.

**3E: Code blocks (rendering) — [x] DONE (roadmap was stale; verified 2026-06-20).**
Both platforms already render fenced ``` ```lang … ``` ``` spans in a monospaced, themed surface with tags/wikilinks NOT parsed inside. **web** = `block-parser.ts segmentText()` (`{type:"code"}` segments; inline parse runs only outside fences) → rendered as `<pre><code class="font-mono … bg-muted/40">` (CollectionBlock/QueryBlock), and the main CodeMirror outliner via `cm-decorations.ts` + `code-highlight.ts` (a dependency-free syntax highlighter — MORE than this item asked). **iOS** = `BlockText.swift` lifts fences into a monospaced code surface. Remaining/deferred: executable code blocks (see `Later`).

**3D: Task Management Depth (Apple Reminders / Todoist parity) — promote sooner**
The user is daily-driving Tesela for tasks; three threads need to ship soon so the system can compete with Apple Reminders / Todoist while preserving the database-first foundation. Detailed scope in **Phase 12** below.
- **Apple Reminders bidirectional sync (priority)** — lets the user lean on iOS location-based reminders, Watch, and Siri while editing in Tesela.
- **Recurring tasks & events** — rrule-subset on `deadline::` / `scheduled::`; auto-roll on completion.
- **Notifications** — desktop + push for deadlines, scheduled times, recurring rolls.
- **Task hierarchy** — subtasks, dependencies, project rollups.

### Later
Rust backlog (parallel work) lives in the Backlog section below — Mechanical and Architectural items are safe for parallel work.

**iOS onboarding pass** — fresh install QR pairing has rough edges that block adoption. Concrete issues seen (2026-05-26): (a) the QR camera scan froze the app on Roshar, requiring force-quit + relaunch to recover; (b) post-pair sync wiring is fragile — `RelayTicker.connect(mosaic:)` runs from `AppShell.task`, which races with `scenePhase.active` firing `relayTicker.start()` and can leave the ticker running with a nil mosaic for 30 s+ (defensive no-op shipped, but the underlying race deserves a proper fix); (c) no visible feedback during/after onboarding that sync is "ready" — user has no way to know they can start writing. Scope: harden the QR scan path (handle camera-permission stalls, time out the scanner, surface progress), guarantee RelayTicker is fully wired before the daily view becomes interactive, add an onboarding completion screen that confirms "you're synced with <Mac name>".

**Bundled desktop app (no CLI required)** — Tesela today requires running `tesela init`, `tesela-server`, and `pnpm --dir web dev` from a terminal. That's a non-starter for actual daily-driver use. Need a single-installer macOS app (Tauri or SwiftUI shell) that bundles the Rust server binary + serves the SvelteKit web client + manages mosaic init/list/switch + import + backup + restore — all reachable from the UI. The CLI keeps existing for power users / scripting. Without this, every "trust" workflow (import a vault, take a backup, restore from a backup) requires terminal literacy. Priority: high once import + backup feel solid via CLI.

**Executable code blocks (org-babel parity)** — fenced ``` ```lang ... ``` ``` blocks become "runnable" units the user can execute in place (a la Emacs org-mode babel / Jupyter cells). Output gets pinned under the block. Languages to support first: shell, Python, JavaScript. Hard parts: sandboxing, output streaming back into the block tree, deciding which interpreter the host vs. user trusts. Design needed before implementation.

**iOS on-device Parakeet inference via FluidAudio** — Tesela's TranscriptionCatalog currently lists Parakeet variants pointed at NVIDIA's raw `.nemo` training-format files (not iOS-runnable). VoiceInk and Handy ship the same model (parakeet-tdt-0.6b-v2) via the FluidAudio Swift package, which bundles a CoreML-converted variant (~450MB on disk). Pull FluidAudio in as a dependency, swap the LocalTranscriptionEngine to dispatch to it for Parakeet IDs, flip `inferenceSupported` to true on those catalog entries.

**Mosaic discovery + server-side multi-mosaic (PRIORITY)** — iOS's "Add mosaic" form currently requires the user to paste a server URL, which is unintuitive: their Mac is already running a `tesela-server` and they expect iOS to see whatever mosaics it has. Two coupled changes needed:
  - **Server-side multi-mosaic**: `tesela-server` today is `--mosaic <one-path>`. Extend to host a list of mosaics with per-mosaic routing (path prefix or header) and a `GET /mosaics` endpoint that lists them. CLI's `init` adds to the list rather than overwriting.
  - **Discovery / pair-handoff**: the existing pair-device flow (6-char short code + QR) is the right place to advertise the host's mosaic list. When iOS finishes pairing, it imports the host's available mosaics into its local `MosaicRegistry` automatically. LAN Bonjour discovery is a separate parallel path for "find devices already on my network."

Without this, users have to spin up a second `tesela-server` on a different port to have a second mosaic — a non-starter for mobile-first daily-driving.

### When Picking Up Work
1. Read `.docs/ai/current-state.md` and the section above.
2. `git log --oneline -10` to see recent changes.
3. Start `tesela-server`: `cargo run -p tesela-server`.
4. Start web dev server: `pnpm --dir web dev`.
5. Pick a phase or ask Taylor what to prioritize.

## What Tesela Is

**North star (locked 2026-06-12, Taylor): Tesela is Taylor's personal emacs 2.0 — but one with a real mobile and RTC story, not just a pretty website.** The whole point is *do everything from the keyboard*. That makes two things the permanent #1 priority, ahead of any view, theme, or feature:

1. **Keyboard-first, always.** Every action must be reachable and driveable without the mouse. New surfaces ship keyboard-complete or they don't ship.
2. **The command registry comes first.** Every action is a named, metadata-carrying command; the palette (⌘K), keybindings, leader/which-key chords, and slash menu are all just *dispatchers* into that one registry. Build the registry as the architectural spine, not as per-feature handlers. Rebindability + introspection ("what's bound to this key?") + eventual extensibility (plugins) all hang off it — that's the emacs-ness, and it's a *command-system* property, not a renderer property.

This is why the stack is **SvelteKit web (Tauri-wrapped desktop) + native SwiftUI iOS + Rust/Loro core**, and *not* a forked Zed: emacs 2.0 needs a command system we own outright over a block-outliner data model, plus a real native-mobile + real-time-collab story. A code editor's text/rope core and its own CRDT (≠ our committed Loro) give us neither, and a fork wouldn't unify iOS anyway. See `decisions.md` 2026-06-12 for the full reasoning. Tauri (not raw browser) is load-bearing here — it lets us claim the native keymap the browser would otherwise steal.

Keyboard-first note-taking system (org-mode/emacs successor). Rust backend + SvelteKit web frontend + native SwiftUI iOS. Taylor's daily-driver tool — reliability matters more than features.

**Core principle:** Database-first, files are export format. Everything is a page.

**Design quality bar:** Linear × Logseq × Zed — craft, restraint, keyboard-first, dark-mode default. (Zed is a *quality/feel* reference, never a fork target.)

## Product Vision

Tesela is NOT just an outliner. It's a personal knowledge operating system — Taylor's **emacs 2.0** — real on desktop, mobile, AND in real-time collaboration. Keyboard reach + the command registry (items 2–4) are the spine everything else plugs into:

1. **Block outliner with Vim mode** — Zed-quality keybindings, per-block editing, block drill-in
2. **Command palette (⌘K)** — Alfred/Raycast-style universal launcher over the command registry: search pages, run commands, create notes, navigate
3. **Slash commands (/)** — in-block quick actions (same registry): change block type, insert template, add property, convert to task
4. **Space/Leader commands** — Neovim which-key-style hierarchical command menu from Normal mode (same registry, user-rebindable): `Space f` → file commands, `Space s` → search, `Space g` → graph
5. **Anytype-style type system** — types, relations, and properties are all pages. Tags are classes. Properties are global entities. Blocks inherit property schemas from their tags. Table/kanban/list views per type.
6. **Sidebar + right panel** — Logseq DB layout: left sidebar (pages, recents, favorites, graph, tiles), right sidebar (backlinks, forward links, properties, pinned pages)
7. **Graph view** — force-directed note relationship graph with click-to-navigate
8. **Daily notes timeline** — scrollable tiles view with inline editing
9. **Search** — full-text search with highlighting, match counts, live results

## Architecture

- **Rust workspace** (`crates/`): tesela-core, tesela-cli, tesela-tui, tesela-mcp, tesela-server, tesela-plugins
- **Web app** (`web/`): SvelteKit 2 + Svelte 5 (runes) + TypeScript + CodeMirror 6 + `@replit/codemirror-vim` + Tailwind v4 + TanStack Query (@tanstack/svelte-query) + Tabler Icons
- **Type system**: Tags, Properties, and Values are pages with YAML frontmatter (Logseq DB + AnyType hybrid — see `memory/project_property_system_vision.md` for deep architecture)

## Rust Backend — stable, not blocked

The server + core library are mature. No immediate feature work needed beyond backlog items.

- Block outliner data model, wiki-link + tag + property parsing
- SQLite/FTS5 indexer with incremental reindex
- REST + WebSocket server (`tesela-server`) with ~95% API coverage
- MCP server for AI integration
- CLI, TUI, plugin system (Lua), backup/restore, LogSeq importer
- Type registry with Tag/Property/Value pages and inheritance

---

## Web Client — Phases

### Phase 1: Core Outliner ✓

Daily-driver outliner with Vim. Migrated from Next.js/React to SvelteKit/Svelte 5 on 2026-04-10.

#### M0–M2 — Core Outliner ✓ (2026-04-09 → 2026-04-11)
- [x] SvelteKit 2 + Svelte 5 scaffold (migrated from Next.js)
- [x] Block parser, always-editable CM6, block operations
- [x] Vim mode + block operators (dd, yy, p, o, O, >>, <<)
- [x] ⌘K Raycast-style command palette with sections, search highlighting
- [x] Slash commands (/task, /todo, /doing, /done, /heading, /property, /link, /date)
- [x] Space leader menu (hierarchical, which-key style)
- [x] Inline autocomplete for #tags and [[wiki-links]]

### Phase 2: Navigation & Views ✓ (2026-04-12 → 2026-04-14)

- [x] Sidebar: Today/Timeline/Graph/Pages nav, Favorites, Recents, collapse toggle
- [x] Tag page table views: sortable columns, per-column filters, inline property editing
- [x] Right sidebar: properties panel (tags, type, custom), backlinks, forward links
- [x] Logseq-style journal timeline with inline editable blocks per day
- [x] Canvas force-directed graph with tag filters, depth slider, theme-aware colors
- [x] Full-text search with bold match highlighting in command palette
- [x] Favorites system (localStorage, star toggle, sidebar section, command palette)
- [x] Settings page (themes, font size, Vim toggle, server URL, shortcuts reference)
- [x] 6 themes: Day, Evening, Woven, Tile Grid, Depth Layers, Neon Glow

### Phase 9: v9 Redesign (IN PROGRESS)

Full redesign vision: `.docs/ai/phases/v9-redesign-vision.md`. Tokyo Night replaces all 6 themes; left+right sidebars become rail+bottom-drawer; rail is the surface for the planned Queries/Sets feature.

#### Phase 9.0 — Columns Shell + Tokyo Night ✓
- [x] 4-region grid (rail / middle / focus / bottom drawer + crumb + status) in `+layout.svelte`
- [x] Tokyo Night palette as the only theme; legacy CSS-var aliases route every component automatically
- [x] JetBrains Mono + Inter Tight typography; Newsreader/Source Sans 3 retired
- [x] `Ctrl+w h/j/k/l` traverses rail / middle / focus / bottom; `1` and `b` toggle bottom drawer
- [x] Right sidebar contents (backlinks, properties) ported into bottom drawer tabs; History + Linked Tasks stubbed
- [x] `themes.ts` deleted; theme picker removed from settings; "Toggle Theme" command removed from ⌘K

#### Phase 9.1 — Saved-Query Widgets (Queries/Sets) ✓
- [x] DSL parser in `tesela-core::query` (Rust + ts-rs export → web/src/lib/types) — supports `kind:` `tag:` `status:` `has:` properties, comparison ops, negation
- [x] `SearchIndex::execute_query` trait method + `SqliteIndex` impl (block-kind via broad SQL prefilter + in-memory refine; page-kind via frontmatter parse)
- [x] `POST /search/query` endpoint in tesela-server
- [x] 9 system widgets (Today, Pages, Tasks, Projects, People, Inbox, Calendar, Recent, Pinned) auto-created on app load
- [x] User-authored saved queries appear as widgets in the rail's Saved section (rail consumes `note_type: Query` notes via `widget-registry`)
- [x] Middle column renders grouped query results with parent breadcrumbs and kind badges (`.kind-badge.kind-task` / `kind-project` / etc.)
- [x] ⌘K "New Query" command + `/widget` slash command + rail footer button
- [→] Block kind glyphs (TASK/PROJECT) in focus pane — deferred to 9.4 (would require restyling cm-decorations away from current "tags hidden" model)

#### Phase 9.2 — Calendar + Inbox Widgets ✓
- [x] Mini calendar pinned in rail (`MiniCalendar.svelte`); per-day rose/teal/amber markers from `GET /calendar/marks`; click-to-navigate-to-daily-note (auto-creates if missing); month nav
- [x] "Event" inferred from `scheduled::` (teal) and "task" from `deadline::` (rose) block properties; ISO date extracted from bare or wiki-wrapped (`[[YYYY-MM-DD]]`) values
- [x] Inbox widget: post-DSL filter excludes blocks from daily notes + Tag/Property/Query/Template pages via the new `page_note_type` field on `QueryItem`
- [x] Triage flow: `t/d/x` single-key handlers in middle column when widget is `inbox` — sets `status::` continuation line, PUTs note, row drops out via WS invalidation
- [x] `note_type` SQL column now populated by `upsert_note` (was previously NULL for all rows; backfilled via `cargo run -p tesela-cli reindex`)
- [→] Project attachment (`p` triage key) — deferred to 9.4

#### Phase 9.3 — History + Linked Tasks Tabs ✓
- [x] SQLite migration `003_note_versions` (note_id, version_number, content, prev_content, created_at)
- [x] `SearchIndex::record_version` / `list_versions` / `get_version` trait methods + SqliteIndex impl. Cap at 200 versions per note (prune oldest in same tx).
- [x] PUT /notes/:id writes a version row before reindex (best-effort; failure logs but doesn't fail the PUT)
- [x] GET /notes/:id/versions and /notes/:id/versions/:version_id endpoints
- [x] `has-link:<id>` DSL predicate in both Rust and TS parsers
- [x] HistoryTab.svelte — timeline list with relative time + +N/−M line counts
- [x] HistoryDiff.svelte modal — side-by-side diff using local LCS line-diff helper; Restore button issues PUT with historical content
- [x] LinkedTasksTab.svelte — reuses /search/query with `kind:block tag:Task has-link:<focused-id>`, grouped by status

#### Phase 9.4 — Polish ✓ (mostly)
- [x] Dynamic per-view keyboard hints in crumb bar — context table by route + widget id
- [x] Mini calendar keyboard nav — arrows / hjkl / PgUp / PgDn / `g t` (today) / Enter
- [x] Drag-to-rearrange widget rail — HTML5 drag-drop on rail rows; persists to `tesela:railOrder`
- [x] Block kind glyphs (TASK/PROJECT badge prefix) in focus pane — `KindBadgeWidget` decoration via new `primaryTagFacet`
- [x] Project attachment (`p` triage key in inbox) — opens `ProjectPicker` modal, sets `project::` block property
- [x] Cmd+Z bleed-through fix — when vim is enabled, document-level Cmd+Z is suppressed inside cm-editor (vim's `u` is the canonical undo)
- [x] Drawer tab badge counts — History tab shows real version count, Linked tasks shows real task count
- [x] Column-view navigation (drilling auto-creates a 2-pane split: previous on left, current on right) — shipped as Phase 9.5b

#### Phase 9.5b — Column-View Navigation ✓
- [x] Replaces the explicit `^w v` toggle from 9.5 with auto-split-on-drill. The vision (Finder column view / yazi / Larkline) is "left = where you came from, right = current."
- [x] Drill rule: source pane content → new left, target → new right, non-source pane is dropped. Every navigation drills (block drill-in, wiki-link click, ⌘K palette, rail click).
- [x] URL spec: `path = right (current); ?back=<noteId>&backBlock=<id>` for left. URL is the single source of truth — reload preserves both panes; browser back unwinds drills.
- [x] `gotoNote(target, block?)` in `active-pane-nav.svelte.ts` rewrites the URL per the drill rule and shifts active side to "right" after every drill. New helpers: `goBack()` (full-screen the left, drop right), `collapseSplit()` (drop ?back=, full-screen the right; used for kanban-mutex).
- [x] `^w v` removed. `^w q` and `^w h` both call `goBack()` (full-screen the left, drop the right) when split is shown. Mental model: left pane is "where I came from"; pressing `h`/left = go there. Esc when right pane is active + vim NORMAL also collapses split. `^w l` flips active side back to right when user has clicked into the left pane.
- [x] Default ratio is 30 (left = 30% width, right = 70%) — back-context is a condensed preview, current pane gets the bulk of the screen. Storage key bumped to `tesela:vSplitRatio:v2` so values from 9.5 (where the meaning of the number was inverted) don't carry over.
- [x] Middle column is the persistent rail-widget result list (Pages / Today / Tasks / Inbox / etc.). When the focus pane lands on a Query note, that becomes the anchored widget — drilling from there into individual pages keeps the same list visible in the middle column. New `current-rail.svelte.ts` store persists the anchor across reloads. Old "Backlinks fallback" branch in `MiddleColumn.svelte` removed (backlinks live in the bottom drawer's Backlinks tab; never duplicated in the middle column).
- [x] Centralized drill via `beforeNavigate` in `+layout.svelte`: every internal `<a href="/p/...">` link click is rewritten through `gotoNote` so the column-view split appears regardless of which component rendered the link. Programmatic gotos (gotoNote / goBack / collapseSplit) are tagged with an `isInternalNavInFlight` flag so they pass through unchanged. This means rail clicks, middle-column row clicks, in-editor wiki links, and any future link surface all drill consistently with no per-component plumbing.

#### Phase 9.5c — Drill is opt-in; middle column removed ✓
- [x] Middle column deleted entirely. The 4-region grid (rail / middle / focus / bottom) becomes a 3-region grid (rail / focus / bottom). Layout grid template updated to `232px 1fr`. `MiddleColumn.svelte` and `current-rail.svelte.ts` removed.
- [x] Query widgets render their result list inline inside the focus pane via the new `QueryWidgetView.svelte` component. Drilling from a result row calls `gotoNote()` directly. Same component is used for the back-pane (left) when the back-context note is a Query.
- [x] Drilling is now opt-in: only block drill-in, wiki-link click in NORMAL mode, and query-result row click create the column-view split. Rail clicks and ⌘K palette picks are plain SvelteKit navigations that replace the focus area full-screen. The global `beforeNavigate` interceptor from 9.5b removed; the `internalNavInFlight` flag and `isInternalNavInFlight` export remain (still used to suppress double-firing when programmatic helpers call `goto`).
- [x] `^w h/l` traversal updated: focus ↔ rail directly (no middle stop). All other chord behavior unchanged.
- [x] Reasoning: the middle column duplicated query results that the focus pane could render natively, and rail-click drilling produced an unwanted 4-pane layout (rail + middle + back-pane + current-pane). One pane in the focus region is the default; drilling adds a second.
- [x] `+page.svelte` swaps roles: path-driven content is now the **right** pane; new `?back=` query drives the **left (back-context)** pane. Save plumbing renamed `right* → back*`. Removed `initialMountChecked` + cleanup-effect race; URL is authoritative.
- [x] Wiki-link click in cm6 (`BlockEditor.svelte` mousedown handler) navigates via `gotoNote` when vim is in NORMAL mode; INSERT mode falls through so the click places the cursor.
- [x] Pane-state store slimmed: removed `openVSplit`/`closeVSplit`/`toggleVSplit`/`vSplitOpen` (URL is truth). Kept active-side + ratio. Kanban `openSplit()` calls `collapseSplit()` first when `?back=` is present.

#### Phase 9.6 — Logseq-style continuous "Dailies" journal ✓
- [x] Replaced the read-only "Today" Query widget with a "Dailies" anchor: rail label "Dailies", URL `/p/dailies`. Clicking lands on a continuous, editable multi-day journal (today on top, older days below).
- [x] Both `/p/dailies` and `/p/<YYYY-MM-DD>` (any note tagged `daily`) render the new `JournalView.svelte`. The route just differs in the anchor — page scrolls to today on `/p/dailies`, to the date on `/p/<YYYY-MM-DD>`. Mini-calendar clicks "scroll the journal to that day" exactly per the Logseq model.
- [x] `JournalView` fetches the latest 500 daily-tagged notes (single API call), sorts descending by date, renders the most recent 30 by default, expands as you scroll past the bottom (IntersectionObserver sentinel + manual "Load older entries" button). Each section is its own `BlockOutliner` with per-noteId debounced save (with `cancelAndFlush`), so edits in any day's section save to that day's file.
- [x] Today is auto-created if missing (call `getDailyNote()` on first mount); same for the anchor date if the URL named a real `YYYY-MM-DD` but the file didn't exist.
- [x] Drill-in (`?block=`) on any block opts back into the standard outliner so the user can focus on a single block; the journal scroll is the un-drilled view.
- [x] Detection in `+page.svelte`: `isDailyJournal = !drillBlockId && (noteId === "dailies" || note_type === "Daily" || tags.includes("daily"))`. Same branch added to the back-pane (column-view left).
- [x] Files: new `web/src/lib/components/JournalView.svelte`; `web/src/lib/system-widgets.ts` (today → dailies); `web/src/lib/widget-registry.svelte.ts` (system-widget-id set updated); `web/src/routes/p/[id]/+page.svelte` (isDailyJournal branch); `notes/today.md` deleted.

#### Phase 9.7 — Daily-driver polish ✓
- [x] **Drill source from left pane click**: row clicks inside the left pane were drilling with `source = right pane content` because the row's `onclick` (bubble) fired before the wrapper's `setVSplitActiveSide('left')`. Switched both pane wrappers to Svelte 5's `onclickcapture` so `vSplitActiveSide` updates BEFORE descendants receive the click. Both wrappers now also have `data-pane="left"|"right"` markers for focus targeting (see below).
- [x] **Focus-shift on drill**: `gotoNote()` now dispatches a `tesela:focus-pane` custom event (with `{ side: "right" }`) two RAFs after the goto resolves; a global handler in `+layout.svelte` finds `[data-pane="right"] .cm-editor .cm-content`, blurs any cm-editor outside the target pane, and focuses the new one. Cursor lands in the right pane after every drill.
- [x] **Cmd+Z bleed-through (vim-on)**: hardened `cmdZHandler` in `+layout.svelte` with `stopImmediatePropagation` and routed Cmd+Z to the new `tesela:outliner-undo` event (Cmd+Shift+Z → `tesela:outliner-redo`). `BlockOutliner` listens and only acts when its root contains the focused element. Cmd+Z now matches the vim `u` chord — full insert-session undo, no per-keystroke cm6 walking.
- [x] **Cancel-and-flush vs redo race**: `flushSave` (and `flushBackSave`, plus the JournalView per-note save) now does an optimistic `setQueryData({ ...note, content })` BEFORE awaiting `api.updateNote`. A WS echo from a prior PUT can no longer overwrite the cache with stale pre-undo body; the post-await `setQueryData` still wins for server-side derived fields.
- [x] **Keyboard-driven Properties tab**: `BottomDrawer` adds `selectedPropertyIndex` state and a `flatProperties` derivation (block-context list when a block is focused, page-context list otherwise). j/k cycles, Enter on a text-typed property toggles into inline edit (existing autofocus + onblur flow), Enter on select/multi-select/date/checkbox focuses the native control. Tab inside the inline input commits via `savePageProperty/saveBlockProperty(..., advance=true)` and advances to the next chip. Visual: `.pchip.selected` adds an amber inset shadow + border.
- [x] Files: `web/src/routes/p/[id]/+page.svelte` (data-pane markers, onclickcapture, optimistic flushSave); `web/src/lib/stores/active-pane-nav.svelte.ts` (focus-pane dispatch); `web/src/routes/+layout.svelte` (focusPaneHandler, harder cmdZHandler with outliner-undo dispatch); `web/src/lib/components/BlockOutliner.svelte` (rootEl bind, undo/redo event listeners); `web/src/lib/components/BottomDrawer.svelte` (keyboard nav for Properties tab, Tab commit-and-advance); `web/src/lib/components/JournalView.svelte` (optimistic pre-set in flushSave); `web/src/app.css` (`.pchip.selected` style).

#### Phase 9.8 — Fuzzy autocomplete + recency ranking ✓
- [x] **Fuzzy match** in `AutocompleteMenu.svelte`: substring filter replaced with a tiered fuzzy scorer (prefix > word-start > substring > subsequence, with position penalties). Used for both `[[` (wiki-link) and `#` (tag) pickers since both pass through the same component. Typing `[[ph` now returns `Phase3GQA, Phase3GDQA, Phase3IQA, Phase3FQA` (prefix matches first); `[[dud` returns `dude` (substring) then `Scheduled / ScheduledItem` (subsequence).
- [x] **Recency tie-break**: same-score items sort by recency rank from `getRecents()`. When the filter is empty, the full list is sorted by recency so the user's recent notes are at the top.
- [x] **Highlight matching characters**: `highlightRuns(label, positions)` splits the label into `{ ch, match }` runs; matched chars render in `<strong class="text-primary font-semibold">` so the user sees why a result matched.
- [x] Files: new `web/src/lib/fuzzy.ts` (scorer + highlight helper); `web/src/lib/components/AutocompleteMenu.svelte` (fuzzy filter, recency sort, highlight rendering).

#### Phase 9.9 — Daily-driver keyboard ergonomics ✓
- [x] **Cross-block j/k auto-scrolls** — `BlockOutliner.handleNavigate` now does `scrollIntoView({ block: "nearest" })` on the new block after focus advances. Cursor stays in viewport during multi-block navigation.
- [x] **Ctrl+U / Ctrl+D as outliner page-jump** — vim chord registered in BlockEditor + new `pageJump` callback in `vimCtx`; jumps 10 blocks per press through the same handleNavigate path so scroll-into-view follows.
- [x] **`^w h` flips active side when split open** — first `^w h` swaps right→left, second `^w h` collapses (full-screen left). Prior behavior collapsed unconditionally. Mirrors `^w l`'s flip-to-right.
- [x] **`^w j` opens drawer if closed** — previously dropped to drawer only when it was already open. Now ensures the drawer is open and focused. Kanban-split path now requires drawer to be already open AND a kanban split AND no column-split.
- [x] **`/p/dailies` auto-focuses today** — JournalView's anchor effect, after `scrollIntoView`, also `.focus()`s the cm-content of the anchored daily so the user can type immediately.
- [x] **`gd` follows wiki-link at cursor (NORMAL mode)** — new vim action in BlockEditor scans for `[[...]]` containing the cursor position and calls `gotoNote(target)`. No-op if cursor isn't inside a wiki-link span.
- [x] **⌘K hardened** — capture-phase + `stopImmediatePropagation` so cm-editor focus on /p/dailies (or any future surface that wires Cmd+K into its keymap) can't swallow the toggle.
- [x] **⌘1-9 quick-pick + action-label hint** — palette items 1-9 get a `⌘N` badge prefix; pressing the chord runs the Nth item's action without arrow nav. Footer shows the selected item's `actionHint` ("Open page", "Run search", "Create note", etc.).
- [x] **Inline properties hidden by default with per-block toggle** — `hiddenKeysFor` now adds every `block.properties` key to the hide set unconditionally. The per-block chevron toggle (already wired) reveals via `.show-props` ancestor. New `gp` vim chord toggles the focused block's expansion state.
- [x] **Query-result row actions** — `QueryWidgetView` auto-focuses on mount so j/k just works. Block-kind rows (Task / typed) show a status glyph button; clicking the glyph (or pressing `s`) cycles status (todo → doing → done) without leaving the list. Reuses `setBlockProperty` from `triage.svelte.ts`.
- [x] Files: `web/src/lib/components/BlockOutliner.svelte` (handleNavigate scroll + handlePageJump + ontoggleprops wiring + hiddenKeysFor); `web/src/lib/components/BlockEditor.svelte` (vim Ctrl+U/D, gd, gp; vimCtx pageJump/toggleProps; module-import gotoNote); `web/src/routes/+layout.svelte` (`^w h` flip + `^w j` drawer-opens path); `web/src/lib/components/JournalView.svelte` (today auto-focus); `web/src/lib/components/CommandPalette.svelte` (capture-phase ⌘K, ⌘1-9, action hint badges, footer); `web/src/lib/components/QueryWidgetView.svelte` (auto-focus, status glyph, `s` cycle).
- [x] Deferred to Phase 10.x: query-row `/`-command picker; in-place row edit; tasks kanban + filter views rebuild; spacemacs-style space menu redesign; per-page vs. global History scope refactor.

#### Phase 9.9 follow-up — daily-driver fixes (round 2) ✓
After 9.9 dogfooding, the user surfaced 8 follow-on bugs. All fixed; #8 (gp) and #9 (⌘K + ⌘1-9) already worked.
- [x] **#6 — `gd` then Esc no longer corrupts parent display.** Drilling via `gd` and going back used to leave the right pane showing the drilled note's content under the parent's title. Root cause: BlockOutliner's body-sync `$effect` had a "preserve focus by id" early-return that always fired on noteId change (the focused block id from the previous note can never be in the new note's reparsed list). Fix: detect noteId change separately from body change and force-reset blocks/focusedIndex/history when the note changes.
- [x] **#10 — `/p/tasks` returns block-tagged results.** SQL pre-filter for `kind:block tag:Task` queries was matching only `body LIKE '%#Task%'` (legacy inline syntax). Blocks using `tags:: Task` continuation-line syntax (e.g. `notes/projects.md`) were excluded before `block_matches` even ran. Fix: relax the LIKE pattern to `%Task%` — over-inclusive at the SQL stage, refined by `block_matches` in-memory.
- [x] **#4 — Ctrl+U / Ctrl+D page-jump in NORMAL mode.** Earlier 9.9 used `Vim.mapCommand("<C-d>", "action", ..., {context:"normal"})`, which never fired on macOS — cm6's `standardKeymap` binds `Ctrl-d`→`deleteCharForward` at a precedence that wins over cm-vim's domEventHandlers. Fix: register Ctrl+U/D in `blockKeymap` (cm6 keymap level), check `getCM(view)?.state?.vim?.insertMode` to yield in INSERT mode (so cm-vim's insert defaults still apply).
- [x] **#1 — `/p/dailies` auto-focus reliable.** Already covered by the BlockOutliner noteId-change reset (#6 fix); JournalView's two-RAF focus path now consistently lands.
- [x] **#2 — ⌘K input keyboard-focused; Esc closes.** HTML `autofocus` was unreliable on modal mount. Fix: bind input ref + `$effect` calling `inputRef.focus()` two RAFs after `open` becomes true. Esc handler moved up to the document-level capture-phase keydown listener so it works even when focus has wandered inside the modal.
- [x] **#3 — Cmd+K → Create lands in INSERT mode.** New notes now seed body with a single empty block (`- \n`) and the palette appends `?fresh=1` to the navigation URL. BlockOutliner's auto-focus effect reads the param and skips the `autoFocused` gate so the empty seed block enters Insert. BlockEditor's prop-change `$effect` now also calls `Vim.handleKey(cm, "i")` post-mount when `startInInsert` becomes true after focus settles (the original onMount-time path missed this case).
- [x] **#5 — j/k navigates by visual line, not logical line.** The vim j/k action was advancing by `s.doc.lineAt(...)` (logical/`\n`-separated). For wrapped paragraphs that's a paragraph-jump, mismatched with the arrow keys. Fix: probe with `view.moveVertically()` and dispatch only if the y-coord changed; cross-block fall-through fires when at the last/first visual line of the editor.
- [x] **#7 — j/k skips collapsed property lines.** Combined with #5 fix: `visualLineMove` iterates candidate positions and only commits the dispatch when the candidate's `.cm-line` element doesn't carry `.cm-tesela-hidden-prop-line` (or `.cm-tesela-tags-line`). If no visible target exists in the editor, the function returns false and the caller cross-blocks.
- [x] Files: `web/src/lib/components/BlockOutliner.svelte` (noteId-change reset; `?fresh=1` autoFocused gate); `web/src/lib/components/BlockEditor.svelte` (Ctrl+U/D in blockKeymap; visual-line `j`/`k` + hidden-line skipping; ArrowDown/Up updated to match; post-mount `startInInsert` effect); `web/src/lib/components/CommandPalette.svelte` (input bind + focus-on-open effect; doc-level Esc; `?fresh=1` on createNote); `crates/tesela-core/src/db/sqlite.rs` (block-query SQL pre-filter relaxed).

#### Phase 10.1 — Query-row in-place edit + slash menu ✓
Brought row-level affordances to query widgets (Tasks, Inbox, etc.) so the user doesn't have to drill into the source page to triage.
- [x] **`e` opens in-place edit on highlighted row.** Inline `<input>` swaps in for the row text; Enter saves via the new `setBlockText(content, blockId, newText)` helper + `api.updateNote`; Esc bails. Saves invalidate both the widget query and the underlying `note` query so the outliner reflects the edit immediately.
- [x] **`/` opens slash menu anchored to highlighted row.** Reuses the existing `SlashMenu.svelte`. Block-kind rows get six commands: Edit text, Open in split, Mark todo / doing / done, Delete block. Page-kind rows get just Open in split. Filtering: typing alphanumerics narrows; arrow keys + Enter / Esc forwarded from QWV's onkeydown to the menu.
- [x] **`setBlockText` + `deleteBlock` helpers in `triage.svelte.ts`.** Same line-number addressing as `setBlockProperty` so the three compose. `setBlockText` preserves indent + bullet + continuation lines; `deleteBlock` walks until the next bullet at `<= indent` and splices the slice.
- [x] **JournalView lands in INSERT.** Calling `.focus()` on cm-content alone wasn't enough — cm6's internal `.cm-focused` lagged and vim stayed NORMAL. Now we dispatch a synthetic `i` keydown on cm-content after focus, which both syncs cm6's focus state and enters INSERT so the user can type immediately on /p/dailies. (Caught during dogfooding the 9.9 follow-up bundle.)
- [x] Files: `web/src/lib/triage.svelte.ts` (setBlockText + deleteBlock); `web/src/lib/components/QueryWidgetView.svelte` (in-place edit, slash menu, key forwarding, edit-input CSS); `web/src/lib/components/JournalView.svelte` (synthetic 'i' for vim INSERT).
- [x] Verified end-to-end via Chrome DevTools MCP: `e` rename → disk update; `/` → menu opens with 6 commands; ArrowDown ×4 + Enter on Mark done → status flips to done on disk and row drops out of `-status:done` filter.
- [ ] Deferred: keyboard chord for "drill in split" (e.g. `^d` on row); status-cycle key in slash menu (currently the `s` shortcut still works on the row, but slash menu has fixed Mark todo/doing/done).

#### Phase 10.1 follow-up — daily-driver task creation ✓
After 10.1 dogfooding, the user surfaced two issues with the Cmd+Enter task-creation flow on dailies:
- [x] **Enter no longer drags continuation lines onto the new block.** Cmd+Enter cycles status (appends `status:: <next>` continuation), then pressing Enter at end of the bullet line was using `doc.sliceString(cursor)` which returned `\nstatus:: <value>` — the whole continuation became the new block's `raw_text`, leaving the original block bare. Fix in `BlockEditor.svelte`'s Enter handler: when the cursor is on the FIRST line of a multi-line block (cursor ≤ first `\n`), keep the continuation tail with the current block; only split the first line. Multi-line content edits past the first line keep the old split-at-cursor behavior.
- [x] **Cmd+Enter auto-tags as `Task`.** User's mental model is "Cmd+Enter creates a task". Previously it only cycled status, so new daily blocks never appeared in `/p/tasks` (which filters on `tag:Task`). Now `handleStatusCycle` also calls `toggleBlockTag(raw, "Task", autoFillNamesForTag("Task"))` if the block has no tag yet AND we're cycling into a non-empty status — so the user gets `status:: <next>`, `tags:: Task`, plus the Task tag-properties auto-filled (Priority/Deadline/Scheduled blanks). Cycling back to empty status leaves tags intact.
- [ ] **Edit-revert in QueryWidgetView** — user reports `e`+Enter edit "times out and reverts" intermittently. Couldn't reproduce in MCP test (typed "dudbar"+Enter saved cleanly to disk and UI). Parked until user re-reports with a reproducer; the fix probably involves an optimistic `setQueryData` on the widget cache so any refetch race can't show stale data.
- [x] Files: `web/src/lib/components/BlockEditor.svelte` (Enter handler keeps continuation on first-line split); `web/src/lib/components/BlockOutliner.svelte` (handleStatusCycle auto-tags Task).

#### Phase 10.1 follow-up #2 — dailies trailing-empty focus ✓
- [x] **Dailies lands on a trailing empty block, not the front of the first block.** JournalView's `ensureTrailingEmpty(noteId)` checks today's body — if the last non-blank line isn't already a bare `- ` bullet, it PUTs a new content with `- \n` appended. The anchor-scroll/focus effect then targets the LAST `.cm-editor .cm-content` in today's section (instead of the first), so the cursor lands on the empty bullet ready to type. After the PUT, the effect's "scrolled-for-anchor" flag resets so the focus re-fires once the new block lands in the DOM.
- [x] Files: `web/src/lib/components/JournalView.svelte` (ensureTrailingEmpty helper + focus effect targets last cm-editor + post-PUT flag reset).

#### Phase 10.1 follow-up #3 — slash + edit conflicts ✓
Two friction items the user surfaced after testing the previous bundle:
- [x] **`e` no longer reverts the title mid-edit.** Pressing `e` while the rename input was focused was bubbling up to QWV's `onkeydown`, which re-ran `startEditRow(row)` → `editingValue = row.label` → wiped what the user had typed. Fix: `handleKeydown` returns immediately when `editingRowId !== null` so the input owns its own keys.
- [x] **`/` on /p/tasks opens the slash menu, not the command palette.** The global `panelHandler` in `+layout.svelte` mapped `/` → dispatch `Cmd+K` to open the palette as a "search" shortcut. It checked target.tagName for INPUT/TEXTAREA/cm-editor but treated the QWV root as a plain element, so the palette stole the keystroke. Fix: extend the panelHandler's "is editing" guard to also bail when target is inside `.qwv` — QueryWidgetView owns its own keyboard scope (`j/k`, `/`, `e`, `s`).
- [x] Files: `web/src/lib/components/QueryWidgetView.svelte` (edit-mode key gate); `web/src/routes/+layout.svelte` (panelHandler `.qwv` opt-out).

#### Phase 10.1 follow-up #4 — leader-chord row menu ✓
The flat slash menu (arrow + Enter or filter-typing) felt sluggish for daily-driver use. User asked for spacemacs/which-key style: `/d` directly marks done, no nav.
- [x] **Replace QWV slash menu with leader-chord menu.** Each row chord is a single keystroke that runs immediately. For block-kind rows: `e` Edit text, `o` Open in split, `t` Mark todo, `i` Mark doing, `d` Mark done, `b` Mark backlog, `x` Delete block. Page-kind rows: `o` Open. The popover anchors under the highlighted row so the user always sees which row will be acted on. Esc or click-outside closes. Unknown letters are swallowed (so they don't bubble back into qwv nav).
- [x] The existing `SlashMenu.svelte` component is untouched — still used inside `BlockEditor.svelte` for inline `/` commands (template / date / tag / etc.).
- [x] Files: `web/src/lib/components/QueryWidgetView.svelte` (chordOpen state, buildChords tree, inline popover render + CSS).

### Phase 10.2 — Unified spacemacs-style leader chord menu ✓
After 10.1's row chord menu, the user asked to apply the chord-leader treatment app-wide and combine it with the existing Space leader (which had limited commands and only worked from NORMAL mode).

- [x] **Generic `ChordMenu.svelte` component** — accepts a tree of `ChordNode { key, label, action?, children? }`. Single-key chords descend or run; Esc/Backspace ascends; click-outside closes. Capture-phase keydown listener so cm-vim doesn't consume the keys. Replaces the deleted `LeaderMenu.svelte`.
- [x] **Unified leader tree.** Top-level groups: `f` File (new/daily/favorite/delete), `b` Block (drill/fold/props/cycle-status/delete/yank), `p` Page (favorite/doc-mode/delete), `s` Search (palette), `g` Go to (home/daily/tasks/inbox/calendar/pages), `w` Window (h/l/j/k/q for pane-nav from any mode), plus `T` Toggle drawer and `y` Yank to clipboard at root.
- [x] **`Ctrl+,` alt-trigger from INSERT mode.** New capture-phase keydown handler in `+layout.svelte` opens the same chord tree from anywhere — including inside cm-editor INSERT mode where `Space` would just type a space. User explicitly asked for this so they don't have to Esc out before reaching for the menu.
- [x] **Block-action dispatch.** "Block" submenu commands dispatch `tesela:block-action` events with `{ kind }`. `BlockOutliner` listens; only the outliner whose `rootEl.contains(document.activeElement)` runs (mirrors the existing `tesela:outliner-undo` filter). Handles drillIn / foldToggle / propsToggle / statusCycle / delete / yank.
- [x] **Page-action dispatch.** `tesela:page-action` mirrors header icon-button actions (favorite / doc-mode / delete) so they're reachable from any keyboard mode. Handler lives in the note `+page.svelte`.
- [x] Files: `web/src/lib/components/ChordMenu.svelte` (new); `web/src/routes/+layout.svelte` (leaderTree, altLeaderHandler, ChordMenu render); `web/src/lib/components/BlockOutliner.svelte` (tesela:block-action listener); `web/src/routes/p/[id]/+page.svelte` (tesela:page-action listener); deleted `web/src/lib/components/LeaderMenu.svelte`.

#### Phase 10.2 follow-up — alt-path hint chips in chord menu ✓
- [x] **`hint?: string` on `ChordNode`** (renamed from initial `vimChord` to keep the field general). Renders as a faint right-aligned kbd chip. Used to advertise an alternative path to the same action.
- [x] **Vim-chord hints** on rows that have a NORMAL-mode equivalent: `b d` ⏎, `b f` za, `b p` gp, `b s` ⌘⏎, `b D` dd, `b y` yy, `f n` ⌘K, `s s` ⌘K, `w h/l/j/k/q` ⌃w h/l/j/k/q, `T` b, `y` "leader Y".
- [x] **URL-path hints** on Go to entries (user requested parity with vim chips on the rest of the menu): `g h` `/`, `g d` `/p/<today>`, `g t` `/p/tasks`, `g i` `/p/inbox`, `g c` `/p/calendar`, `g p` `/p/pages`. Same chip on `f d` (File → Daily) for consistency.
- [x] CSS: `.chord-hint` truncates with ellipsis at `max-width: 14ch` so longer paths don't push the layout.
- [x] Files: `web/src/lib/components/ChordMenu.svelte` (hint field + chip render + .chord-hint CSS); `web/src/routes/+layout.svelte` (annotations).

#### Phase 10.2 follow-up #2 — `g` as Go-to prefix (vim+spacemacs pattern) ✓
- [x] **`g` in vim NORMAL opens the leader chord menu pre-descended into "Go to".** cm6 keymap binding (registered before cm-vim's keymap by extension order) captures `g`, dispatches `tesela:open-leader-at` with `path: ["Go to"]`. The menu lands on the submenu with breadcrumb visible. INSERT/VISUAL modes yield so `g`-prefixed visual operators and inserting the literal letter `g` still work.
- [x] **`gd` → Daily, `gt` → Tasks, `gi` → Inbox, `gc` → Calendar, `gh` → Home, `gp` → Pages** as a natural consequence: each is `g` + the matching first-letter chord in the popup. URL hint chips visible on each row.
- [x] **Wiki-follow rebound to `g f`.** Previous `gd` (Phase 9.9 wiki-follow chord) folded into the popup as `f` Follow wiki link with hint `[[ at ▌`. The action body lives in BlockEditor under `tesela:block-action` listener with kind=`followWiki`; only the editor whose view is currently focused responds.
- [x] **Toggle props (was `gp`) moves to `Space b p` / `Ctrl+, b p`.** No longer a `g`-prefixed chord (since `g p` is now Pages). Still reachable via the unified leader menu's Block submenu.
- [x] `ChordMenu` component gains an `initialPath?: string[]` prop so any caller can open the menu pre-descended into a sub-tree (used by `g` here, but also available for future `t` / `b` etc. prefixes).
- [x] Files: `web/src/lib/components/ChordMenu.svelte` (initialPath prop); `web/src/routes/+layout.svelte` (leaderInitialPath state, openLeaderAtHandler, `g f` Go-to entry); `web/src/lib/components/BlockEditor.svelte` (cm6 `g` keymap binding, removed Vim.mapCommand for gd/gp, new tesela:block-action `followWiki` listener).

#### Phase 10.2 deferred (saved to memory `project_leader_menu_vision.md`):
- [ ] **User-configurable leader tree.** A `~/.tesela/leader.config.{json,toml,ts}` (TBD format) that the user edits to add / remove / rename / re-key entries. Hardcoded tree becomes the merged-in default. Implies an action registry mapping stable IDs (`block.cycleStatus`, `page.toggleFavorite`, etc.) → handlers, so configs reference IDs instead of inline functions.

### Phase 10.3 — In-block `/` slash menu → chord-leader style ✓
The last bounded action surface still using filter+arrow-nav joins the chord pattern.

- [x] **`ChordMenu` extended** with optional `position?: { x, y }` (cursor-anchored mode — overrides centered modal) and `headLabel?: string` (defaults `SPC`; slash menu sets it to `/`). Capture-phase keydown handler now swallows ALL keys when the menu is open (modal behavior) so arrows / vim chords / Cmd+letter combos can't leak through to the cm-editor behind the popover.
- [x] **In-block `/` opens chord popover anchored to caret.** Single-letter chords run actions immediately — `/t` Task, `/T` Tag picker, `/h` Heading, `/p` Property, `/l` Link, `/d` Date, `/q` Query, `/w` Widget, `/c` Collection, `/m` Template. `/s` descends into a Status submenu.
- [x] **Status submenu auto-resolves key collisions.** `assignStatusKeys()` walks each choice's letters and picks the first unclaimed one (with `doing` → `i` and `in-review` → `r` aliases pre-mapped). Falls back to digits 1-9 if all letters are taken. Handles arbitrary user-configured choices like `notes/status.md`'s `["backlog", "todo", "doing", "in-review", "done", "canceled", "on-hold", "dude"]` without dup-key crashes.
- [x] **Hint chips** on Task (`tags:: Task`), Tag picker (`#`), Property (`key:: value`), Link (`[[ ]]`), and each status row (`status:: <choice>`).
- [x] **`SlashMenu.svelte` deleted** — both callers (QWV row chord at 10.1, BlockEditor `/` at 10.3) now use ChordMenu. `AutocompleteMenu.svelte` stays for unbounded surfaces (`#` tag autocomplete, `[[` wiki-link autocomplete).
- [x] Files: `web/src/lib/components/ChordMenu.svelte` (position + headLabel props, modal key swallowing); `web/src/lib/components/BlockEditor.svelte` (getSlashTree, assignStatusKeys, ChordMenu render at slashPosition, removed slashFilter/slashMenuRef/SlashCommand and the keymap branches that forwarded keys to slashMenuRef); deleted `web/src/lib/components/SlashMenu.svelte`.

### Phase 3: Power Features (paused — folded into Phase 9)

#### Anytype-Style Types & Relations
- [x] Kanban view on tag pages (group by select property like Status)
- [→] Queries / Sets — moved into Phase 9.1 (rail widgets ARE the Queries/Sets surface)
- [ ] Collections — manual page groupings
- [ ] Node references — property value links to another page (bidirectional)
- [ ] Tag inheritance — `extends` chain, child inherits parent properties
- [ ] Global property registry — search existing property pages when adding to a tag

#### Editor Power Features
- [x] Visual mode (block-level — V to enter, j/k to extend, d/y/T/J/K)
- [x] Block merge on Backspace at start of non-empty block
- [x] Multi-block selection and operations (visual delete / yank / indent / status / tag)
- [x] `/template` — insert from template pages
- [x] `/date` — date picker UI (with Todoist-style natural-language input)
- [x] Block drill-in (focus single block + children)
- [x] Block fold / collapse (Phase 3K)
- [x] Subtree-aware indent (>>, << bring children with parent)
- [x] Leader Y → OS clipboard (Phase 3K)

#### Polish
- [x] Auto-focus first block on page mount (Phase 3L)
- [x] Esc-in-Normal preserves focused block + cm-editor (Phase 3L)
- [x] 3-region splits with `Ctrl+w h/j/k/l` (left sidebar / outliner / right panel) (Phase 3L)
- [x] Modal focus restore: ⌘K / leader-menu / slash-menu close returns focus to last block (Phase 3L)
- [→] Right sidebar items obsolete — right sidebar replaced by bottom drawer in Phase 9.0
- [ ] Bottom drawer: inline keyboard property editing for Properties tab (j/k navigates, currently only mouse-clickable)
- [ ] Empty/loading/error state audit across all views
- [ ] Graph: drag nodes to reposition

### Phase 12 — Task Management Depth (promoted: Apple Reminders sync, recurring, notifications)

Added 2026-05-07. Tesela is the user's daily-driver task manager; this phase rounds out the surface so it stands on its own against Apple Reminders / Todoist. Apple Reminders sync is the unlock that brings iOS automations (geofencing, Siri, Watch) without giving up file-first editing.

#### 12.1 — Apple Reminders bidirectional sync (HIGH PRIORITY)

Bridge Tesela Task blocks ↔ Apple Reminders items so the user can:
- Add a Task in Tesela → it appears in Reminders → triggers iOS location-based reminders, Watch surface, Siri.
- Tick a Reminder on iPhone → status flips to `done` in Tesela on next sync.
- Use Reminders' geofencing automations ("remind me at home") on Tesela tasks, while editing/searching/linking remains in Tesela.

Architecture (shipped):
- **macOS bridge**: `tesela-server/src/reminders/darwin.rs` uses `objc2-event-kit` directly. Web client never talks to EventKit; all interaction goes through `/sync/reminders/{push,pull}` and the combined `/sync/reminders` route.
- **Identity**: `apple_reminder_id::` on the block stores `EKCalendarItem.calendarItemIdentifier`. Missing identifier → recreate on next push.
- **Conflict gate**: `apple_reminder_synced_at::` (RFC 3339 UTC) is written on every successful push and pull. On pull, we only overwrite Tesela when `EKReminder.lastModifiedDate > synced_at` — i.e. the user actually touched it in Reminders.app since our last sync. Otherwise Tesela keeps its value.
- **Combined sync ordering**: pull-then-push. If the user only edited Tesela, pull no-ops and push writes Tesela → EK. If the user only edited in Reminders.app, pull writes EK → Tesela first, then push reaffirms. Concurrent edits: EK wins per field (documented limitation).
- **Calendar setup**: "Tesela" list auto-created on the Source that owns `defaultCalendarForNewReminders` — the only reliable way to find a writable Source for reminder-type calendars (CalDAV servers may host events but not reminders).

##### 12.1 slice 1 — push (✅ shipped, commit `7bf1560`)

Tesela → Reminders only.
- Eligible: any Task block with a parseable `deadline::` (date or `[[YYYY-MM-DD]]`, optionally with trailing time).
- Property mapping: block text → `title`; `status:: done` → `completed`; `deadline::` → `dueDateComponents` (date-only); `priority:: critical|high|medium|low` → `1|1|5|9`; no priority → `0`.
- Idempotent: first push returns `created`, subsequent pushes return `updated` for the same blocks via the stored identifier.
- POST `/sync/reminders/push` returns `{ created, updated, synced, errors }`.

##### 12.1 slice 2 — pull + combined sync (✅ shipped, commit `0ed74b5`)

Reminders → Tesela, plus a "Sync now" UI surface.
- POST `/sync/reminders/pull` walks the Tesela calendar, diffs each reminder against its matching block, gates on the `lastModifiedDate > synced_at` rule, writes back.
- Pulled fields: title (preserves inline `#tags` by re-appending them after the new text), status (`completed` ↔ `done`/`todo`), deadline (date only), priority (1-4 → high, 5 → medium, 6-9 → low).
- POST `/sync/reminders` does pull-then-push and returns `{ pull, push }`.
- UI: `Cmd+K → "Sync Apple Reminders"` and **Settings → Apple Reminders → Sync now** both call the combined endpoint and show a toast outcome.
- Orphans (reminders with no matching Tesela block) are reported but not imported — slice 3 territory.

##### 12.1 slice 3 — round-trip fidelity & ergonomics

Closing the gaps that slice 2 left open. Status as of 2026-05-09:

1. ✅ **Time-of-day round-trip** (commit `8928131`). `deadline::` now carries an optional `HH:MM` (24h or 12h with AM/PM); push writes `dueDateComponents.{hour,minute}` and pull rebuilds the full timestamp. New `Deadline` struct + 7 unit tests.
2. ✅ **Geofencing (`reminder_location::`)** — push side, title-only. Block property `reminder_location:: <name>` (e.g. `Trader Joes`) attaches via `EKCalendarItem.location`; Reminders.app shows the string and offers a long-press to upgrade it to a real geofence. CLLocation + arrive/leave proximity (full RFC 5545 / EKAlarm.structuredLocation surface) deferred — title-only covers the common "remind me at the grocery store" case.
3. ✅ **Per-list mapping** — push side. `apple_reminder_list:: <name>` on a block routes its push into the named Reminders list (creating it on the user's default source if missing). Untagged blocks still go to the auto-managed `Tesela` calendar. Pull still walks Tesela only — cross-list pull (rebuilding Tesela blocks from arbitrary Reminders lists) is a v2 feature.
4. ✅ **Auto-sync** (commit `797a8a0`). New `reminders::auto` module owning a single `Mutex` so triggers serialize cleanly. Three triggers fire `sync_all`: (a) **startup** — 10s after server boot; (b) **interval** — every 5 minutes; (c) **edit-driven** — debounced 30s after the indexer's `NoteEvent` stream goes quiet. Each call records into a shared `LastSync` exposed at `GET /sync/reminders/status`; the manual `Sync now` button also routes through `AutoSync` so the Settings UI shows a single unified "last synced N minutes ago via <trigger>" line. EKEventStore change-notification observer (true push from EK side) deferred — needs a CFRunLoop and the 5-minute interval covers the user-visible gap.
5. ✅ **Orphan handling** — push side. When `apple_reminder_id::` no longer resolves in EventKit (the Reminder was deleted in Reminders.app), sync stamps `apple_reminder_orphan:: true` on the block and skips it on subsequent pushes until the user clears the flag. Avoids the duplicate-create that would otherwise happen each sync. Pull still ignores reminders that have no matching Tesela block — "import as Task" is a v2 affordance.
6. ✅ **Recurring round-trip** (commit `01a5a63`). `recurring::` ↔ `EKRecurrenceRule` both directions. `Daily / EveryNDays / Weekly / Monthly / Yearly / Weekdays` all round-trip. Diff compares parsed values (so `every 1 week` doesn't flap with `weekly`). BYDAY sets beyond `Weekdays` and end-conditions (`until` / `count`) still deferred — same constraint as Phase 12.2's recurrence engine.
7. ✅ **UI affordances** (commit `797a8a0` together with auto-sync). Settings → Apple Reminders shows last-sync time, trigger, and outcome counts (e.g. "Synced 3m ago via interval · 2 pushed"). Surfacing `apple_reminder_id::` / `apple_reminder_synced_at::` in a "system properties" drawer group is still TODO — they currently render alongside user properties.

Out of scope still: shared lists, attachments, sub-reminders (12.4 handles Tesela-side hierarchy first), multi-account, Reminders categories outside Tasks.

#### 12.2 — Recurring tasks & events ✅ shipped

`recurring::` block property storing an rrule-subset string. On `status:: done`, the engine bumps `deadline::` to the next occurrence in place (Apple-style; one block ↔ one item for life), flips `status::` back to `todo`, and stamps `last_completed::` with the prior date. Shipped forms:
- `recurring:: daily` / `every day`
- `recurring:: weekly` / `every week`, `recurring:: every 2 weeks`
- `recurring:: monthly` / `every month`, `recurring:: every 3 months`
- `recurring:: yearly` / `annually` / `every year`, `recurring:: every 2 years`
- `recurring:: weekdays` (Mon–Fri; from Fri/Sat/Sun → next Mon)
- `recurring:: every N days`

Backend: pure `tesela_core::recurrence` (`parse` + `next_after`) with day-of-month clamping (Jan 31 + monthly → Feb 28/29) and Feb 29 leap handling. `apply_post_save_bumps` in `update_note` auto-detects status flips to `done` and rewrites the block transparently — no client-side trigger needed. Explicit `POST /api/blocks/recur-bump` exists for debugging. Frontend: `parseRecurrenceInput` mirrors the Rust parser; DatePicker has a "repeat" sub-row (none / daily / weekly / monthly / yearly / weekdays / custom); BottomDrawer commit writes `recurring::` alongside `deadline::`; `recurring.md` Property page renders the chip via the existing display-chips system. Mirrors Apple Reminders' recurrence shape so 12.1 sync round-trips correctly. Live-tested 2026-05-09 via PUT to `/notes/{id}` — flipping `status:: done` on `recurring:: monthly` produced the expected bumped deadline + `last_completed::` stamp.

12.2.x — Recurrence Completeness ✅ shipped (2026-05-22), engine + clients. BYDAY sets (`every mon, wed, fri`), `until` / `count` end conditions, skip-occurrence (`recur-bump` `mode: skip`), recurring on `scheduled::` (multi-field anchor), and the `weekends` keyword. **Engine:** `tesela_core::recurrence` + the server bump path + the EventKit round-trip. **Clients:** `parseRecurrenceInput`, the `DatePicker` day-set + end-condition controls, the human-formatted `recurring::` chip + skip menu, the `skip` verb, and the iOS display chip. Spec/plans: `.docs/ai/phases/2026-05-21-recurrence-completeness-{design,engine-plan}.md` + `2026-05-22-recurrence-completeness-clients-plan.md`.

#### 12.3 — Notifications ✅ shipped

Desktop notifications + always-on toast fallback for three event kinds:
- **Deadline approaching** — fires once per (block, deadline) when within the configured lead time (default 1h before deadline). Open Task blocks only.
- **Scheduled time fires** — fires when `scheduled::` time-of-day is reached.
- **Recurring task rolled to next** — fires when `apply_post_save_bumps` advances a recurring task on a `status:: done` flip.

Backend: new `notifications` module on `tesela-server` runs a 60-second tokio interval that walks all notes, parses task blocks, and emits `WsEvent::DeadlineApproaching` / `ScheduledFires` / `RecurringRolled`. Dedupe by `(block_id, kind, deadline_iso)` so the same window doesn't re-trigger every minute (an in-memory `HashSet`; restarts reset, which we accept). Bare-date deadlines anchor to 9 AM local; explicit `HH:MM` honored.

Frontend: `web/src/lib/notifications.ts` owns the dispatch — toast always fires, browser `Notification` only when permission granted. Permission is requested lazily on the first Settings toggle so we don't pop a prompt at boot. Per-kind mute checkboxes in Settings (deadline / scheduled / recurring), each persisted to localStorage. Notification clicks navigate to the source note.

Live-verified 2026-05-09 by setting a deadline 30 minutes ahead and watching the WS event arrive on the next scanner tick.

Out of v1: configurable lead time (currently fixed 1h / 0m), per-property overrides via tag config, web push via service worker (works only when tab open today), in-app linked-block change events.

#### 12.4 — Task hierarchy ✅ shipped (subtasks + same-note deps)

- ✅ **Subtask rollup chip**: `BlockOutliner` walks each block's direct children (one indent deeper) and renders a small `X/Y` pill ahead of the property chips when any child has `status::`. Green when complete, neutral otherwise. Purely visual — no markdown footprint. `Cmd+Enter` quick-add for a child task is still TODO.
- ✅ **Dependencies — `blocked_by::`**: same-note refs only. When `update_note` detects a flipped-to-done block, `apply_dependency_cycles` walks the same note for any `status:: backlog` block whose `blocked_by::` references include the just-done block, then auto-flips it to `todo` if all blockers are now done. Cross-note dependency walking deferred — requires a reverse-index pass per save. Lock pill renders on chips of blocked tasks (amber, with the `lock` icon).
- **Project rollup row** — deferred. Per-tag summary chip ("5/12 done · earliest May 18") would live on tag pages. Today the kanban already gives per-status counts, so this is a nice-to-have rather than a daily-driver gap.

Live-verified 2026-05-09: backlog task auto-flipped to `todo` the moment its blocker's PUT carried `status:: done`.

#### Sequencing

1. **12.2 Recurring** ships first — it's purely local, exercises the property + chip system, and is a prerequisite for 12.1 round-tripping recurring Reminders.
2. **12.3 Notifications** ships second — wires the WS event stream + Notification API; small surface, big perceived value.
3. **12.1 Apple Reminders sync** ships third — biggest scope, needs Swift FFI work; depends on stable `recurring::` semantics from 12.2.
4. **12.4 Hierarchy** rolls in alongside 12.1 as polish.

### Phase 4: Distribution

#### (Optional) Tauri Wrap
- [ ] Tauri shell serving `web/out/`
- [ ] Menu bar, global hotkeys, system tray

**Deferred:** Whiteboards, long-form prose, App Store, plugin marketplace, collaborative editing.

---

## iOS App — Phases

Native SwiftUI iPhone app at `app/Tesela-iOS/` — touch-first, no vim (see memory `mobile-strategy-ios-native`). Phases 0–31 below were not previously tracked in this roadmap; reconstructed from git history on 2026-05-20.

The app talks to a `tesela-server` over HTTP — the `MockMosaicService` class is the *real* client despite its name. It is **not** yet a sync peer (`SyncState` is a debug toggle). Real iOS sync stays gated behind the sync work order (memory `sync-work-order`): iOS sync is last, after block-level sync → LAN transport → desktop sync UX.

### Phases 0–16 — UI shell ✓

- [x] Cross-compile Rust core + Xcode scaffold (`40f7250`)
- [x] Design system: 17 themes, density tiers, type scales (Phases 0–1)
- [x] Component primitives, mock data, Daily front door (Phases 2–4)
- [x] Library tab, PageView with tag chips + collapsible Peek (Phases 5–6)
- [x] Search, context menu, properties sheet, settings, pair flow (Phases 7–10)
- [x] Onboarding, ambient grid, page stack, FFI stub, skeletons (Phases 11–16)

All built against `MockMosaicService`'s in-memory seed — no real data yet.

### Phase 17 — HTTP backend ✓

- [x] `MockMosaicService` gains an `.http` mode against a local `tesela-server`; the app now reads and writes a real mosaic
- [x] 17.1 ATS config; 17.2 property-based task format aligned with the web client

### Phase 18 — Inbox + bottom chrome ✓

- [x] Inbox/triage tab, Mail-style search bar
- [x] 18.1–18.8 iterated the Liquid Glass bottom chrome, landing on iOS 26 native `TabView` with `Tab(role: .search)`

### Phase 19 — Real page content ✓

- [x] Page load, render, and writeback against the server

### Phases 20–31 — editing & voice ✓

- [x] 20–23 block editing, navigation, search, triage
- [x] 24–25 transcription model management
- [x] 26–31 end-to-end + on-device voice (whisper.cpp, streaming, Parakeet gating)

### Recent — pairing, multi-mosaic ✓

- [x] Camera QR + 6-character short-code device pairing
- [x] Auto-reconnect with backoff, disconnect, real pair card
- [x] Native TabView chrome, persistent capture bar, keyboard toolbar
- [x] Device-local multi-mosaic registry (`MosaicProfile` profiles)
- [x] Real per-page backlinks, client-derived outline, local pinned store (`d548cc3`)
- [x] Server-side multi-mosaic — discover / add / switch a server's mosaics (`b8124c3`)

### iOS — Now / Next

- The Peek surface is now real-data on every lens — backlinks, outline, props, tasks, and graph (graph = an outgoing-links list, tap-to-navigate). A real in-app **graph render** is a wanted later item; the list is the interim.
- **Parakeet transcription** — wired on-device via the FluidAudio package (v2/v3/110M, `LocalTranscriptionEngine` family dispatch). Builds clean; the model-download + inference runtime path is not yet device-verified.
- The Settings → Sync page has some mocked elements; sync itself works (per Taylor, 2026-05-20).
- **Architecture note:** the app is an HTTP client, not the UniFFI-embedded core described in memory `mobile-strategy-ios-native`. `tesela-sync` UniFFI bindings are generated, but the embedded-core path is deferred.

---

## Backlog

> Self-contained items any agent can pick up. First agent to start it executes it. Honor each item's `tier_floor` gate before starting; `complexity` is sizing only.

### Product-test feedback (2026-06-15)

- [x] **#PT1 — Enter on a task's property line orphaned the block.** ✅ FIXED `6e4304c4`. The real repro (Taylor): start block → make Task → newline + `testpoints:: 10` → Enter — moved the `tags:: Task` line + scaffold to a NEW block, orphaning the original. Root cause (4-agent trace): the Enter ELSE branch in `BlockEditor.svelte` split the multi-line block at the raw cursor offset with no property-awareness, shipping trailing `tags::`/property lines to the new block. Fix: extracted `planEnterSplit` (`web/src/lib/editor/enter-split.ts`) + a property-line guard (cursor on a `key::`/`tags::` line → keep block intact, drop empty sibling). Verified: 8 unit tests + 426 suite green + svelte-check clean + real-editor repro (block stayed intact, empty sibling created). (The original "inline mangle" framing was a misread — it was always the Enter-split.)
  - **Diagnosis so far** - `PROPERTY_LINE_RE` (`web/src/lib/block-parser.ts:10`) is own-line `^key:: value$`, so the inline text is (correctly) not a property. The garble is a CodeMirror decoration/typing interaction. **Ruled out:** atomic tag-chip widgets (removed from the editor 2026-06-07 — Model A; tags are plain marked text in CM now). Remaining atomic ranges (`web/src/lib/cm-decorations.ts`): block-id `<!-- bid:… -->` hide (`:852`), tables/images/hr/callout. Root cause UNCONFIRMED — the symptom doesn't cleanly map to these.
  - **BLOCKED ON** - a faithful keystroke repro. Get Taylor's exact steps (the block's prior content + the literal keystrokes/screenshot). MCP browser fill does NOT simulate keystroke timing, so it can't reproduce a typing-interaction bug. Do this in a focused editor session, not blind — `cm-decorations.ts` is the app's most delicate code.
  - **Files** - `web/src/lib/cm-decorations.ts` (atomic ranges / `transactionFilter`), `web/src/lib/block-parser.ts` (own-line gate, confirm unchanged).
  - **Acceptance** - With a real keystroke repro, typing `<text> #tag key:: value` leaves the source byte-exact (no dropped/merged chars); the `key:: value` stays plain text (not a property — own-line model preserved).
  - **Verify** - Reproduce the original garble in a real browser, then confirm fixed (typed source matches what was typed). Add a CM regression test if a deterministic trigger is found.
  - **tier_floor** - `senior`
  - **complexity** - `M`

### Opencode-ready reliability polish (2026-06-12)

- [x] **Add a dry-run repair for date-slug dailies missing `daily` tags.**
  - **Scope** - Add a CLI repair command that finds canonical `YYYY-MM-DD.md` notes that do not have the `daily` tag and optionally adds it. Default must be dry-run; `--apply` must be explicit.
  - **Files** - `crates/tesela-cli/src/main.rs`; likely create `crates/tesela-cli/src/repair_daily_tags.rs`; reuse storage/frontmatter helpers from `crates/tesela-core/src/storage/markdown.rs` and `crates/tesela-core/src/storage/filesystem.rs`.
  - **Acceptance** - Dry-run lists only valid date-slug notes missing `daily`; `--apply` adds `daily` without changing the note body or dropping existing frontmatter fields; a second `--apply` reports no changes.
  - **Verify** - `cargo test -p tesela-cli repair_daily_tags`; `cargo test -p tesela-core --lib`; `cargo fmt --all --check`.
  - **tier_floor** - `senior`
  - **complexity** - `M`

- [x] **Improve web journal placeholder previews.**
  - **Scope** - When a journal day is virtualized/unmounted, show a meaningful preview of real block text instead of raw metadata, bid comments, property-only lines, or blank placeholder bullets.
  - **Files** - `web/src/lib/components/JournalView.svelte`; create a pure helper such as `web/src/lib/journal-preview.ts`; add `web/tests/unit/journal-preview.test.mjs`.
  - **Acceptance** - Preview strips `<!-- bid:... -->`; skips property-only lines such as `tags::` and `status::`; skips malformed metadata-looking continuation lines; preserves normal top-level block text; mounted `BlockOutliner` editing behavior is unchanged.
  - **Verify** - `node --test web/tests/unit/journal-preview.test.mjs`; `pnpm --dir web check`.
  - **tier_floor** - `junior`
  - **complexity** - `S`

- [x] **Add an app-level regression for date-slug dailies without `daily` tags.** — pi mono `opencode-go/minimax-m3` (Junior, T3), landed 2026-06-12 in commit `71c94af` (see `.docs/ai/phases/2026-06-12-codex-pi-batch-report.md` Item 7).
  - **Scope** - Add server or web coverage proving a real `YYYY-MM-DD.md` daily without `tags: [daily]` is included in the journal data path, not replaced by an empty synthetic gap day.
  - **Files** - Prefer `crates/tesela-server/tests/` if an integration-test harness already exists; otherwise add focused web coverage under `web/tests/` using a temp mosaic. Read `crates/tesela-core/src/storage/filesystem.rs` for the core behavior first.
  - **Acceptance** - A fixture containing `2026-06-10.md` with body blocks but no `daily` tag returns/renders those body blocks through the journal-facing path.
  - **Verify** - `cargo test -p tesela-server daily_filter_includes_date_slug_daily_without_tag`; `cargo test -p tesela-core storage::filesystem::tests::test_daily_filter_includes_date_slug_notes_without_daily_tag`; `cargo build -p tesela-server`.
  - **tier_floor** - `junior`
  - **complexity** - `S`

- [x] **Harden malformed property-line parsing in web block rendering.**
  - **Scope** - Prevent corrupted property-looking text such as `Deadline::cheduled::` from being treated as a valid property chip or hidden metadata.
  - **Files** - `web/src/lib/block-parser.ts`; `web/tests/unit/block-parser.test.mjs`; inspect `web/src/lib/components/BlockOutliner.svelte` only to confirm display behavior.
  - **Acceptance** - Valid property lines still parse/render normally; malformed property-looking text remains visible/editable as text and is not converted into a chip or silently dropped.
  - **Verify** - `node --test web/tests/unit/block-parser.test.mjs`; `pnpm --dir web check`.
  - **tier_floor** - `senior`
  - **complexity** - `S`

- [x] **Fix web verification script drift.** — pi mono `opencode-go/minimax-m3` (Junior, T3), landed 2026-06-12 in commit `7613293` (see `.docs/ai/phases/2026-06-12-codex-pi-batch-report.md` Item 8).
  - **Scope** - Make the documented `pnpm --dir web lint` command real, or update repo docs to the actual supported command if `check` is intentionally the only static verifier.
  - **Files** - `web/package.json`; `.docs/ai/roadmap.md`; check root/project instructions before editing docs.
  - **Acceptance** - A fresh agent can run the documented web verification commands without hitting "missing script: lint"; no behavior code changes.
  - **Verify** - `pnpm --dir web lint`; `pnpm --dir web check`.
  - **tier_floor** - `junior`
  - **complexity** - `S`

- [x] **Expand the web normal-mode `j/k` regression suite.**
  - **Scope** - Add coverage around the recent stale insert-intent fix so navigation does not re-enter Insert after command palette focus, quick capture focus, textarea/editor blur, or Esc transitions.
  - **Files** - `web/tests/jk-normal-mode.e2e.mjs`; read `web/src/lib/components/BlockEditor.svelte` and `web/src/lib/components/BlockOutliner.svelte` before changing behavior.
  - **Acceptance** - `j/k` navigates only in normal mode; stale auto-insert intent is not reused after focus moves through non-editor UI; existing split/new-block Insert behavior still works when intentionally creating a block.
  - **Verify** - `node web/tests/jk-normal-mode.e2e.mjs`; `pnpm --dir web check`.
  - **tier_floor** - `junior`
  - **complexity** - `S`

- [x] **Render fenced code blocks cleanly in web read mode.**
  - **Scope** - Web-only first pass: render fenced markdown code blocks as preformatted code in read/display mode while preserving raw markdown in edit mode.
  - **Files** - Start with `web/src/lib/block-parser.ts`, `web/src/lib/components/QueryBlock.svelte`, `web/src/lib/components/CollectionBlock.svelte`, and the block text/render component currently responsible for read-mode segments; add focused unit tests under `web/tests/unit/`.
  - **Acceptance** - Fenced code displays monospaced and preserves line breaks; tags and wikilinks inside fences are not parsed; editing the block still shows and saves the raw markdown fence.
  - **Verify** - `node --test web/tests/unit/block-parser.test.mjs`; `pnpm --dir web check`.
  - **tier_floor** - `senior`
  - **complexity** - `M`

- [x] **Add task-toggle stale-state regression coverage.**
  - **Scope** - Add targeted tests around checkbox/task-status updates from web and mobile-shaped payloads so a stale desktop/web view cannot overwrite an already-completed task with old unchecked state.
  - **Files** - `crates/tesela-server/src/routes/notes.rs`; server tests around block/property mutation routes; web side only if the failing path requires `web/src/lib/property-update.ts` or `web/src/lib/components/BlockOutliner.svelte`.
  - **Acceptance** - Toggling a task updates the intended block/status property only; unrelated concurrent block text or task status changes survive; tests exercise the protected block-granular path rather than a whole-note rewrite.
  - **Verify** - `cargo test -p tesela-server task_toggle_does_not_reassert_stale_state`; `cargo test -p tesela-server`; `node --test web/tests/unit/block-ops.test.mjs web/tests/unit/block-ops-saver.test.mjs` if web code is touched.
  - **tier_floor** - `senior`
  - **complexity** - `M`

### Codex/pi mono coordinator batch (2026-06-12)

- [x] **Make the Graphite command palette screen-reader addressable.**
  - **Scope** - Add semantic dialog/listbox structure to the Graphite command palette without changing command scoring, ordering, shortcuts, or execution behavior.
  - **Files** - `web/src/lib/graphite/shell/GrCommandPalette.svelte`; add `web/tests/command-palette-a11y.e2e.mjs` mirroring the existing standalone Playwright style in `web/tests/jk-normal-mode.e2e.mjs`.
  - **Acceptance** - When the palette is open, the scrim exposes a modal dialog label, the input points at the active option via `aria-activedescendant`, command/note rows expose stable ids with `role="option"` and `aria-selected`, the empty state is announced, Escape and click-out still restore pane focus, and command execution behavior remains byte-for-byte equivalent.
  - **Verify** - `node web/tests/command-palette-a11y.e2e.mjs`; `pnpm --dir web check`.
  - **tier_floor** - `junior`
  - **complexity** - `S`

- [x] **Add Vim-style overlay navigation aliases in the TUI.**

- [x] **Lock iOS keyboard-toolbar encoding with pure tests.**
  - **Scope** - Add Swift tests for the pure keyboard-toolbar preference codec so toolbar customization stays stable while view polish continues.
  - **Files** - `app/Tesela-iOS/Sources/Data/KeyboardToolbarItem.swift`; create `app/Tesela-iOS/Tests/KeyboardToolbarItemTests.swift`.
  - **Acceptance** - Tests cover default raw value order, round-trip encode/decode, duplicate removal preserving first occurrence, unknown value dropping, and empty raw string behavior as currently implemented. Do not change toolbar UI or edit sync.
  - **Verify** - `xcodebuild -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 16' test`.
  - **tier_floor** - `junior`
  - **complexity** - `S`

- [x] **Improve Graphite saved-view chip accessibility on iOS.**
  - **Scope** - Add view-layer accessibility labels/hints/traits to the iOS Graphite Inbox saved-view chip bar and New View chip. Keep registry reads/writes and query execution untouched.
  - **Files** - `app/Tesela-iOS/Sources/Graphite/Views/GrInboxView.swift`; `app/Tesela-iOS/Sources/Graphite/GrChip.swift`.
  - **Acceptance** - VoiceOver distinguishes the selected saved view, announces the New View action, and exposes edit/reorder/delete context-menu affordances without changing visual layout, saved-view ordering, or triage behavior.
  - **Verify** - `xcodebuild -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 16' build`; launch with `-graphite` in the iPhone 16 simulator and capture a screenshot path with `xcrun simctl io <sim> screenshot <path>`.
  - **tier_floor** - `senior`
  - **complexity** - `S`

- [ ] **ESCALATE (Opus/Fable): iOS onboarding and RelayTicker race hardening.**
  - **Scope** - The existing iOS onboarding pass includes `RelayTicker.connect(mosaic:)`, scene-phase startup, pairing, and sync-ready state. Those are sync-adjacent and explicitly off-limits for the Codex/pi batch.
  - **Files** - `app/Tesela-iOS/Sources/Views/OnboardingView.swift`; `app/Tesela-iOS/Sources/Data/RelayTicker.swift`; pairing and relay-related call sites discovered while scoping.
  - **Acceptance** - Lead owner re-scopes into safe slices before implementation; no Senior/Junior agent starts this from the current batch.
  - **Verify** - Lead-defined.
  - **tier_floor** - `lead` — ESCALATE (Opus/Fable)
  - **complexity** - `XL`

- [ ] **ESCALATE (Opus/Fable): property FFI page-property remainder.**
  - **Scope** - P1.11 still notes page-property set/clear FFI functions. All FFI/UniFFI bindings and generated iOS binding surfaces are off-limits for this coordinator batch.
  - **Files** - `crates/tesela-sync-ffi/`; `app/Tesela-iOS/Generated/`; `app/Tesela-iOS/CFFI/`.
  - **Acceptance** - Lead owner decides whether/when to expose page-property FFI and updates fleet-migration notes before any implementation.
  - **Verify** - Lead-defined.
  - **tier_floor** - `lead` — ESCALATE (Opus/Fable)
  - **complexity** - `XL`

### Next Senior/Junior batch (2026-06-13)

- [ ] **Restore editor focus after `:` command-line cancel.**
  - **Scope** - Fix the Graphite/v4 ex-command line so `:` then Escape returns focus to the active CodeMirror editor, matching the command palette focus-restore behavior. Keep command execution, autocomplete ordering, and verb registry behavior unchanged.
  - **Files** - `web/src/lib/components/shell/ColonCommandLine.svelte`; read `web/src/lib/graphite/shell/GrCommandPalette.svelte` for the local `restoreFocus()` pattern; update `web/tests/jk-normal-mode.e2e.mjs`.
  - **Acceptance** - `:` opens the command line; Escape closes it and leaves `.cm-editor.cm-focused` true without an extra click; subsequent `j/k` navigation stays in NORMAL mode; Enter/Tab/autocomplete behavior is unchanged.
  - **Verify** - `pnpm --dir web build`; run `target/debug/tesela-server` against a temp mosaic with `TESELA_STATIC_DIR=$PWD/web/build TESELA_DISABLE_MDNS=1 TESELA_DISABLE_RELAY=1 TESELA_DISABLE_PEER_SYNC=1 TESELA_SERVER_BIND=127.0.0.1:7793`, then `REPRO_URL=http://127.0.0.1:7793/g node web/tests/jk-normal-mode.e2e.mjs`; `pnpm --dir web check`.
  - **tier_floor** - `senior`
  - **complexity** - `S`

- [ ] **Trim two concrete Svelte a11y warnings.**
  - **Scope** - Fix the known warnings for the context menu focus contract and the backup auto-on-quit toggle label/state without changing visual styling or backup behavior.
  - **Files** - `web/src/lib/components/ContextMenu.svelte`; `web/src/lib/components/BackupSettings.svelte`.
  - **Acceptance** - The `role="menu"` container is keyboard-focusable and still closes on Escape/outside click; the auto-backup toggle exposes a clear accessible name plus pressed/checked state; no unrelated warning cleanup or design changes.
  - **Verify** - `pnpm --dir web check`.
  - **tier_floor** - `junior`
  - **complexity** - `S`

- [ ] **Harden `repair-daily-tags` against symlink escape.**
  - **Scope** - Stop the repair walker from following symlinks out of the mosaic notes tree, and add regression coverage for a symlinked external date-slug file.
  - **Files** - `crates/tesela-cli/src/repair_daily_tags.rs`.
  - **Acceptance** - Real nested note directories are still scanned; symlinked files/directories are ignored; `--apply` cannot mutate a date-slug file outside `<mosaic>/notes`; existing dry-run/apply/idempotency tests keep passing.
  - **Verify** - `cargo test -p tesela-cli repair_daily_tags`; `rustfmt --edition 2021 --check crates/tesela-cli/src/repair_daily_tags.rs`.
  - **tier_floor** - `senior`
  - **complexity** - `S`

- [ ] **Make MCP JSON formatting unwraps explicit.**
  - **Scope** - Replace the three `serde_json::to_string_pretty(&results).unwrap()` call sites with explicit `.expect(...)` messages that document why serialization is considered infallible for JSON `Value` results. No MCP response shape changes.
  - **Files** - `crates/tesela-mcp/src/tools.rs:150`; `crates/tesela-mcp/src/tools.rs:236`; `crates/tesela-mcp/src/tools.rs:260`.
  - **Acceptance** - The three bare unwraps are gone; output JSON is byte-equivalent for normal tool responses; no broader MCP refactor.
  - **Verify** - `cargo test -p tesela-mcp`; `rustfmt --edition 2021 --check crates/tesela-mcp/src/tools.rs`.
  - **tier_floor** - `junior`
  - **complexity** - `S`

- [ ] **Add a notarized desktop ZIP release recipe.**
  - **Scope** - Codify the desktop release path proven on 2026-06-13: build web static, build Tauri with `--bundles app` to avoid the hanging DMG layout step, sign with the configured Developer ID identity, optionally notarize/staple, and write a ZIP artifact. Do not change runtime code, bundle identifier, version, or any secrets.
  - **Files** - create `scripts/desktop-release.sh`; read `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml` for product/version values; no changes to `src-tauri/src/main.rs`.
  - **Acceptance** - Script has a safe no-notarize path for local verification; full path uses environment-overridable signing/notary inputs; artifact naming includes product/version/arch; DMG packaging remains out of scope.
  - **Verify** - `bash -n scripts/desktop-release.sh`; `scripts/desktop-release.sh --skip-notarize`; `codesign --verify --deep --strict --verbose=2 target/release/bundle/macos/Tesela.app`.
  - **tier_floor** - `senior`
  - **complexity** - `M`

- [ ] **Label Graphite iOS icon-only header buttons.**
  - **Scope** - Add view-layer accessibility labels/identifiers for icon-only Graphite buttons in the Daily and Agenda headers while keeping the primitive's visual metrics unchanged.
  - **Files** - `app/Tesela-iOS/Sources/Graphite/GrButton.swift`; `app/Tesela-iOS/Sources/Graphite/Views/GrDailyView.swift`; `app/Tesela-iOS/Sources/Graphite/Views/GrAgendaView.swift`; read `app/Tesela-iOS/Sources/Graphite/Shell/GrHeader.swift`.
  - **Acceptance** - VoiceOver announces the Daily date picker, Daily settings, and Agenda jump-to-today actions with human labels; icon-only buttons retain 30x30 layout; no data/sync/service code touched.
  - **Verify** - `xcodebuild -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/tesela-ios-grbutton-a11y build`; `xcrun simctl boot "iPhone 16" || true`; `xcrun simctl install booted /tmp/tesela-ios-grbutton-a11y/Build/Products/Debug-iphonesimulator/Tesela.app`; `xcrun simctl launch booted app.tesela.ios -graphite`; `xcrun simctl io booted screenshot /tmp/tesela-ios-grbutton-a11y.png`.
  - **tier_floor** - `senior`
  - **complexity** - `S`

- [ ] **Tighten Graphite Settings VoiceOver semantics.**
  - **Scope** - Improve view-layer accessibility in Graphite Settings only: server-mode segmented buttons, URL/device text fields, active mosaic rows, and edit affordances. Do not touch `RelayTicker`, `MosaicService`, pairing, registry persistence, or save/disconnect logic.
  - **Files** - `app/Tesela-iOS/Sources/Graphite/Views/GrSettingsView.swift`; read `app/Tesela-iOS/Sources/Graphite/GrRow.swift` before deciding whether the row primitive needs an additive accessibility hook.
  - **Acceptance** - VoiceOver can tell which server mode is selected, identify/edit the server URL and device name fields, distinguish the active mosaic, and find the mosaic edit action; visual layout and all backend/sync behavior are unchanged.
  - **Verify** - `xcodebuild -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/tesela-ios-settings-a11y build`; `xcrun simctl boot "iPhone 16" || true`; `xcrun simctl install booted /tmp/tesela-ios-settings-a11y/Build/Products/Debug-iphonesimulator/Tesela.app`; `xcrun simctl launch booted app.tesela.ios -graphite`; `xcrun simctl io booted screenshot /tmp/tesela-ios-settings-a11y.png`.
  - **tier_floor** - `senior`
  - **complexity** - `M`

### Web editor — discovered during B4 (2026-06-10)

- [x] **Split + immediate merge-back orphans the absorbed block's server row.** DONE 2026-06-12: `handleBackspaceMerge` now emits the absorbed-block delete whenever the absorbed block has a bid, even if its temporary editor id is still `:new-…`; unknown deletes are harmless before the creating upsert lands, and required after it lands. Regression `web/tests/split-merge-back.e2e.mjs` proves split mid-text → wait >600ms → Backspace col-0 merge leaves ONE on-disk block with merged text. Verified: red e2e reproduced duplicate; green e2e 3/3; `node --test tests/unit/block-ops.test.mjs tests/unit/block-ops-saver.test.mjs`; `pnpm --dir web check`; `pnpm --dir web build`; `git diff --check`.
- [x] **j/k navigation can randomly land in Insert mode.** DONE 2026-06-12: `BlockEditor` now gates reactive `startInInsert` on `!autoFocused` and notifies the parent only after Vim actually enters Insert; `BlockOutliner` consumes the matching mount/recently-created hint so it is one-shot. Coverage `web/tests/jk-normal-mode.e2e.mjs` checks Esc+k+j back onto a fresh empty block and an Enter-split block stays in NORMAL. Verified: `node tests/jk-normal-mode.e2e.mjs`; `node tests/split-merge-back.e2e.mjs`; `node --test tests/unit/block-ops.test.mjs tests/unit/block-ops-saver.test.mjs`; `pnpm --dir web check`; `pnpm --dir web build`; `git diff --check`. Lint caveat: no `lint` script exists in `web/package.json`.
- [x] **Blank future daily placeholder renders above Today.** DONE 2026-06-12: live `2026-06-13.md` was real but empty (`- <!-- bid:... -->` only), so JournalView's future-daily preservation policy rendered Saturday above Friday. `filterDisplayableDailies` now hides blank future placeholders in the default feed while preserving contentful future dailies and explicit future anchors. Verified: `node --test tests/unit/journal-dates.test.mjs tests/unit/ensure-trailing-empty.test.mjs`; `pnpm --dir web check`; `pnpm --dir web build`; `git diff --check`.

### Sync robustness — LoroText follow-ups (Sonnet candidates, non-blocking)

- [ ] **#177 — LoroText `update_by_line` timeout fallback for huge blocks.** `write_block_text` (`crates/tesela-sync/src/engine/loro_engine.rs:~1305`) calls `LoroText::update(text, UpdateOptions::default())`. `default()` is `timeout_ms: None`, so it CANNOT time out — but runs an unbounded O(n·d) Myers diff on a pathologically long block (>50k chars), blocking the apply thread. **Scope:** set `timeout_ms: Some(...)` and on `Err(UpdateTimeoutError)` retry via `LoroText::update_by_line` (the documented line-granular fallback). **Verify:** `cargo test -p tesela-sync`; add a unit test that a very long block still round-trips. Error is already propagated (never `.ok()`), so this is hardening, not a correctness fix.
- [ ] **#178 — Bound `server_block_text_history` op-replay on large notes.** `server_block_text_history` (`loro_engine.rs:~1813`) replays every server op (`0..end_counter` per peer, import-into-scratch + full-tree walk per step → O(ops×blocks)) to reconstruct per-block text history for the disjoint-twin stale-vs-genuine decision. **Gated correctly** (only runs after the `twin_bids.is_empty()` early-return in `peer_genuine_block_changes`, so the common no-twin WS frame never pays it), but a heavily-edited note WITH a twin makes it pathological while it runs. **Scope:** cap the replay above an op-count threshold (fall back to current-value-only stale detection) or memoize. **Verify:** `cargo test -p tesela-sync` (the `ws_apply_*` disjoint suite must stay green).

### Mechanical (Haiku candidates)

- [ ] Replace one-off `regex::Regex::new(r"#[...]")` in `crates/tesela-server/src/routes/notes.rs:179` with cached `INLINE_TAG_RE`
- [ ] Replace `std::env::current_dir().unwrap()` in `crates/tesela-cli/src/main.rs:196` with `?` + `.context()`
- [ ] Replace 2 `plist_file.to_str().unwrap()` calls in `crates/tesela-cli/src/main.rs:666,690` with `.context()`
- [ ] Replace 3 `serde_json::to_string_pretty(&results).unwrap()` calls in `crates/tesela-mcp/src/tools.rs:150,236,260` with `.expect("reason")`
- [ ] Annotate 2 regex-capture unwraps in `crates/tesela-cli/src/import_logseq.rs:202,244` with `.expect("reason")`
- [ ] Annotate `cap.get(0).unwrap()` in `crates/tesela-core/src/link.rs:38` with `.expect("reason")`
- [ ] Extract hardcoded server bind address `"127.0.0.1:7474"` into a named constant
- [ ] Extract hardcoded backup-retention magic numbers into named constants

### 2026-06-13 Ralph batch — command registry foundation (keyboard-first spine)

Orchestrator: Pi. Spec: `phases/command-registry-spec.md`. Run A1 and B1 in parallel (different models/tiers), then B2, then B3.

- [x] **A1 — Fix clippy errors.**
  - **Scope** — Fix two clippy warnings breaking `cargo clippy --workspace -- -D warnings`: `redundant_closure` in `crates/tesela-core/src/db/sqlite.rs:1129`; `while_let_loop` in `crates/tesela-core/src/query.rs:589`. Actual run exposed additional warnings across the workspace; all were fixed mechanically (no behavior change).
  - **Files** — `crates/tesela-core/src/db/sqlite.rs`; `crates/tesela-core/src/query.rs` (plus fmt fallout across 25 files).
  - **Acceptance** — `cargo clippy --workspace -- -D warnings` passes; no behavioral change.
  - **Verify** — `cargo clippy --workspace -- -D warnings`
  - **tier_floor** — `junior`
  - **complexity** — `S`
  - **ralph_model** — `opencode-go/minimax-m3`

- [x] **B1 — Unified command registry shape + port palette/leader.**
  - **Scope** — Create `web/src/lib/command-registry.svelte.ts` with a `Command` type and singleton registry. Port `buildV4Commands()` entries to register themselves. Update `GrCommandPalette` and `getLeaderTree()` to read from the registry. Keep colon verb dispatch working via `findCommandByVerb`.
  - **Files** — new `web/src/lib/command-registry.svelte.ts`; modify `web/src/lib/v4/commands.ts`, `web/src/lib/v5/leader-tree.svelte.ts`, `web/src/lib/graphite/shell/GrCommandPalette.svelte`, `web/src/lib/graphite/shell/GrLeaderOverlay.svelte`, `web/src/lib/components/shell/ColonCommandLine.svelte`.
  - **Acceptance** — ⌘K palette, Space leader, and `:` verbs behave identically to before; `pnpm --dir web check` is clean.
  - **Verify** — `pnpm --dir web check` + manual palette/leader/colon QA.
  - **tier_floor** — `senior`
  - **complexity** — `M`
  - **ralph_model** — `opencode-go/kimi-k2.7-code`

- [x] **B2 — Keymap introspection + conflict detection.**
  - **Scope** — Build a keymap index from the registry (shortcuts, chords, browser-reserved keys). Detect collisions. Add `:keymap` colon command that lists bindings + conflicts.
  - **Files** — `web/src/lib/command-registry.svelte.ts`; new `web/tests/unit/command-registry.test.mjs`; update `web/src/lib/components/shell/ColonCommandLine.svelte`.
  - **Acceptance** — `:keymap` prints every registered command with its shortcut/chord; conflicts and browser-reserved bindings are flagged.
  - **Verify** — `node --test web/tests/unit/command-registry.test.mjs`; `pnpm --dir web check`; manual `:keymap` QA.
  - **tier_floor** — `senior`
  - **complexity** — `M`
  - **ralph_model** — `opencode-go/kimi-k2.7-code`

- [x] **B3 — Context-aware command dispatch.**

### 2026-06-13 follow-up — junior mechanical items for minimax-m3

- [x] **A2 — Replace `serde_json::to_string_pretty` unwraps in MCP tools with explicit `.expect()`.**
  - **Scope** — Replace 3 bare `.unwrap()` calls in `crates/tesela-mcp/src/tools.rs:150,236,260` with `.expect("...")` documenting why serialization is infallible for JSON `Value` results.
  - **Files** — `crates/tesela-mcp/src/tools.rs`.
  - **Acceptance** — No bare unwraps remain at those call sites; MCP tests still pass.
  - **Verify** — `cargo test -p tesela-mcp`; `cargo clippy --workspace -- -D warnings`.
  - **tier_floor** — `junior`
  - **complexity** — `S`
  - **ralph_model** — `opencode-go/minimax-m3`

- [x] **A3 — Replace regex-capture unwraps in Logseq importer with `.expect()`.**
  - **Scope** — Annotate 2 regex-capture `.unwrap()` calls in `crates/tesela-cli/src/import_logseq.rs:202,244` with `.expect("...")` describing the invariant.
  - **Files** — `crates/tesela-cli/src/import_logseq.rs`.
  - **Acceptance** — No bare unwraps remain; Logseq importer tests still pass.
  - **Verify** — `cargo test -p tesela-cli --lib import_logseq`; `cargo clippy --workspace -- -D warnings`.
  - **tier_floor** — `junior`
  - **complexity** — `S`
  - **ralph_model** — `opencode-go/minimax-m3`

- [x] **A4 — Extract hardcoded backup-retention magic numbers into named constants.**
  - **Scope** — Find magic retention counts/days in backup code and extract them into named constants at the top of the file or module.
  - **Files** — `crates/tesela-backup/src/lib.rs` (and any related files).
  - **Acceptance** — No magic numbers for retention remain inline; backup tests still pass.
  - **Verify** — `cargo test -p tesela-backup`; `cargo clippy --workspace -- -D warnings`.
  - **tier_floor** — `junior`
  - **complexity** — `S`
  - **ralph_model** — `opencode-go/minimax-m3`
  - **Scope** — Add `CommandContext` capture (route, focused buffer kind, vim mode, focused block, split state) and `when` predicates to commands. Filter palette/leader availability from the registry; colon dispatches verbs from the registry.
  - **Files** — `web/src/lib/command-registry.svelte.ts`, `web/src/lib/graphite/shell/GraphiteShell.svelte`, `web/src/lib/graphite/shell/GrCommandPalette.svelte`, `web/src/lib/graphite/shell/GrLeaderOverlay.svelte`, `web/src/lib/components/shell/ColonCommandLine.svelte`.
  - **Acceptance** — Commands that don't apply in the current context are hidden from palette/leader; context changes update availability reactively; existing behavior is preserved.
  - **Verify** — `pnpm --dir web check` + full keyboard QA matrix (palette/leader/slash/colon across page/daily/inbox/agenda contexts).
  - **tier_floor** — `senior`
  - **complexity** — `L`
  - **ralph_model** — `opencode-go/kimi-k2.7-code`

### iOS bugs

- [ ] **Yesterday block delete flicker** — Tapping rapid deletes on yesterday's blocks (`editYesterdayBlock` family) on iOS occasionally re-shows the deleted block for a tick before it disappears again. Almost certainly an optimistic-UI vs stale-snapshot race: the `BlockDelete` materializes, then a queued snapshot/reparse from before the delete lands and re-renders the pre-delete state, then the next render hides it. Web's `BlockOutliner` has "in-flight new-block protection" for the create side — iOS likely needs the symmetric "in-flight delete protection" on the yesterday-edit path. Repro: open yesterday on Roshar, mash delete on a few blocks fast. Reported 2026-05-27 by Taylor; not a sync correctness issue (dual-write divergence log stayed clean throughout).

### Architectural (Sonnet candidates)

- [ ] Split `crates/tesela-core/src/db/sqlite.rs` (1126 lines) into db/migrations.rs, db/search.rs, db/links.rs, db/types.rs
- [ ] Split `crates/tesela-cli/src/main.rs` (826 lines) into `src/commands/` submodule
- [ ] Extract duplicated backup logic into shared `tesela_core::backup` module
- [ ] **Panel-flexibility Playwright smoke (Phase 4 of `.docs/ai/phases/2026-05-11-panel-flexibility-plan.md`)** — three smoke tests in `web/tests/perf/panel-flex.spec.ts` covering (a) BottomTab legacy-string → JSON migration, (b) `railOpen` persisted across reload, (c) `drawerSide=right` persisted across reload. The plan's Task 4.1 has the test bodies. Plumbing decision: either reuse `playwright.perf.config.ts` + spin up the heavy perf-test runner (cost: ~60s setup per CI run), OR run against the existing dev server with a lightweight standalone config. Recommend the latter — these tests need no fixture data; the existing dev mosaic suffices. Estimate: ~80 lines of test code + ~30 lines of standalone config.
- [x] **Importers add `#Task` tag + one-time backfill** — DONE 2026-06-10: both importers now emit a `tags:: Task` continuation line (the engine's materialized form, NOT inline ` #Task` — matches `backfill-task`'s structured AddToList output) for every converted task marker, union-idempotent with existing `tags::` lines / inline `#Task`. Logseq marker set extended (WAIT/IN-PROGRESS/CANCELLED); org `cancelled`→`canceled` fixed to match the status.md seed; Logseq SCHEDULED/DEADLINE now also capture `<date Day HH:MM .+1w>` forms (time kept, repeater dropped — no `recurring::` mapping in the Logseq importer yet, org has it). Backfill subcommand already existed (`tesela backfill-task`, 214fa72). REMAINING (coordinated user step, desktop closed): `tesela --mosaic "$HOME/Library/Application Support/tesela/logseq" backfill-task` (dry run) then same + ` --apply`; then revert the live `tasks.md` query workaround back to `kind:block tag:Task -status:done` if still applied.

- [ ] **Remove the `/v4` route — serve the app from `/`** (do in 2 phases) — the app shell still lives under `/v4` purely as a v4→v5 deep-link-history hedge the user has explicitly waived. No backwards-compat needed.
  - **Phase 1 — move the app to `/` via a SvelteKit route group.** Create `web/src/routes/(app)/` and move the three real app routes into it: `routes/v4/+layout.svelte` → `routes/(app)/+layout.svelte`, `routes/v4/+page.svelte` → `routes/(app)/+page.svelte`, `routes/v4/p/[id]/+page.svelte` → `routes/(app)/p/[id]/+page.svelte` (the `$lib` imports inside those files are unaffected — only the file location moves). Delete `routes/+page.ts` (the `/`→`/v4` redirect; `(app)/+page.svelte` now serves `/`) and `routes/p/[id]/+page.ts` (redirect stub — it would otherwise *collide* with the moved `(app)/p/[id]` route). Retarget the four remaining redirect stubs `routes/{daily,graph,properties,timeline}/+page.ts` from `/v4` → `/`. Code refs: `(app)/+page.svelte` calls `history.replaceState(null, "", "/v4")` → `"/"`; `lib/stores/active-pane-nav.svelte.ts:70` detects the chrome via `pathname === "/v4" || startsWith("/v4/")` — rework to recognize the new app paths (`/` and `/p/...`, NOT `/settings`, `/design`); fix stale `/v4` comments in `routes/+layout.svelte` and `components/v4/Station.svelte`. Verify `/`, `/p/<slug>`, `/daily`, `/settings` all load and tab/pane state survives.
  - **Phase 2 — collapse the duplicate page-deeplink path.** Today `/p/[id]` redirects to `/v4#tile=<slug>` (a hash the index page reads on mount) *and* a real `/v4/p/[id]` route exists in parallel. After Phase 1 there is a real `(app)/p/[id]` route, so the hash mechanism is dead weight: remove the `#tile=` hash-reading logic from `(app)/+page.svelte` and make `(app)/p/[id]/+page.svelte` the single canonical URL deep-link into a page.
  - Out of scope (separate cosmetic cleanup, not blocking): the internal `v4-*` CSS class names, `$lib/components/v4/`, `$lib/v4/tokens.css` — naming only, unrelated to the route.

### Unlinked references (Logseq-style)

- [ ] **Surface "unlinked references" in the v5 backlinks-of-page derived buffer** — placeholder already in the UI under the "Unlinked references" section. Backend changes: add `/api/notes/:id/unlinked` returning notes whose body contains the focused page's `title` OR any of its `aliases` as a plain-text substring without `[[...]]` wrapping. Skip the focused page itself, skip blocks already containing a `[[wiki-link]]` to the focused page on the same line. Frontend changes: TanStack-query the endpoint inside `web/src/lib/renderers/derived/backlinks-of-page.svelte`, render rows that mirror the Backlinks section but with a "Link" inline-action button per row that wraps the mention in `[[...]]` (one-click promote to a real link). Open questions: case-sensitivity (probably case-insensitive); minimum match length (avoid matching short common words — maybe require ≥4 chars); whether to also scan code fences (probably skip).

### Cross-cutting (needs Opus to scope)

- [x] **Frontend perf smoke tests (Phase 14.2)** — extends the Phase 14 regression harness (`.docs/perf/README.md`) to cover the web app, where the last two real-world scaling regressions actually landed (Dailies `limit: 500` + 70-CodeMirror-mount). Pick **Playwright** (battle-tested, used by SvelteKit's own examples) over chrome-devtools-mcp (developer-only). New layout: `web/tests/perf/`, with `pnpm test:perf` running the suite. Setup: spawn `tesela-server` against a `MosaicBuilder::medium()` fixture (already exists at `crates/tesela-fixtures/`); export the synthetic mosaic path via a small `cargo run -p tesela-fixtures-cli` helper that takes `--out <path> --preset medium`. Test scenarios: (1) `/p/dailies` first-paint < 1.5s (the Phase 14 budget), assert via `page.waitForSelector(".day:nth-child(5)", { timeout: 1500 })`; (2) navigating rail → Tasks page → first kanban card visible < 800ms; (3) command palette ⌘K opens + first result rendered < 300ms; (4) Settings → Mosaic → Plan import on a small synthetic Logseq vault < 5s. Each test reports its timing as a Playwright attachment so we can diff across runs. Baseline diff is best-effort (not gated). No PR-blocking — informational. Estimate: ~400 lines of test code + ~50 lines for the fixtures-cli helper.

- [x] **CI perf workflow (Phase 14.3)** — new `.github/workflows/perf.yml`. Triggers: PR + nightly main. Job 1 runs `cargo bench --workspace --benches -- --save-baseline pr-current`. Job 2 fetches the most-recent `main` baseline artifact (uploaded by the nightly run) and diffs via `critcmp pr-current main` (https://github.com/BurntSushi/critcmp — a single-binary criterion diff tool). Posts a PR comment with the table only when any bench regresses >10%. The same workflow nightly-on-main runs the suite and uploads its results as `bench-baseline-main` artifact (with 30-day retention) so PR jobs have a baseline to diff. Frontend Playwright suite (from previous item) runs in the same workflow on the PR side, with timings posted as comment attachments. Doesn't block merge — informational. Estimate: ~150 lines of workflow YAML + brief README pointer.

- [ ] API endpoint integration tests (server routes)
- [ ] New server endpoints needed for web client: `GET /notes/:id/blocks`, `POST /notes/:id/blocks` (block-level CRUD)
- [ ] Block merge with property conflict resolution: when both the merged-from and merged-into blocks have properties, show an overlay dialog letting the user choose which properties to keep (rather than naively concatenating duplicate keys)
- [x] **Outliner-level undo / redo stack** (`u` / `Ctrl+R` for structural ops) — Phase 3M. Snapshot stack in `web/src/lib/stores/outliner-history.svelte.ts`, sprinkled into every structural mutation in BlockOutliner; falls through to cm-editor history when stack empty. Cmd+Z outside vim is a follow-up.
- [x] **Vim-faithful unified `u`** — Phase 3M.1. Insert sessions are atomic: cache a snapshot on Insert-mode entry, promote on the first keystroke. `o<text><Esc>u` reverts the typing first, then on next `u` reverts the block creation — matches vim. Adds prop→cm6 sync `$effect` (with `externalSync` annotation) so undo restores propagate into editor doc.
- [ ] Cmd+Z outside vim (document-level keydown that calls the same outliner undo when not inside an editor)
- [x] Cancel in-flight saves on undo (close the residual race window where a debounced PUT from before the undo overwrites the restored state) — Phase 3M.2. AbortController plumbed through `api.updateNote`; `applySnapshot` calls `saveBlocksImmediate` which fires `onCancelAndFlush` to abort the in-flight PUT and immediately PUT the restored body.
- [x] Cm6 history coherence after outliner undo: when `applySnapshot` writes a block's body via the externalSync transaction, that transaction lands in cm6's history — so subsequent `Cmd+Z` may walk through the just-undone state. — Phase 3M.2. Added `Transaction.addToHistory.of(false)` to the prop→cm6 sync dispatch so externalSync transactions are excluded from cm6 history.
- [x] Block remount after Ctrl+R into Insert mode: when redo restores an empty newly-created block, the BlockEditor's `startininsert` heuristic (focused empty block) fires on remount, leaving vim in Insert. — Phase 3M.2. Added `restoredFocus` flag in BlockOutliner set by `applySnapshot` and cleared on user-initiated focus changes (click, navigate, new-block, empty-state click); the `startininsert` heuristic now checks `!restoredFocus`.
- [ ] **`dw` / `d$` / etc. integrate with `p` paste** (text-register fidelity). Phase 3K's `delete` operator override no-ops the register-controller side of non-linewise deletes, so deleted text isn't recoverable via `p`. Two viable approaches: (a) populate vim's default register via `vimGlobalState.registerController.pushText` (requires importing a non-public symbol from `@replit/codemirror-vim`, may break across versions), or (b) maintain our own text register alongside `blockClipboard`, and have the `pasteBlock` action prefer block clipboard, falling back to text register inserted at cursor. Pick the approach during design; option (b) is friendlier to upgrades.

---

## Constraints

- Design quality bar: Linear × Logseq × Zed — craft, restraint, keyboard-first, dark-mode default
- No business logic in the web client — only in `tesela-core` traits
- Database-first; files are export format
- Everything is a page — types, properties, tags are all note files
- Icons: Tabler Icons in web client
- Command palette is the primary discovery surface for commands

## Non-Goals (for now)

- iOS/iPadOS app
- Multi-device sync (CRDTs)
- App Store distribution
- Plugin marketplace
- Collaborative editing
- Whiteboards / infinite canvas
