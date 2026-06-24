# Current State

## Branch
- `main` — **all pushed to origin** (2026-06-23). Tree clean. `.docs/ai/review/` + `AuthKey_*.p8` untracked (latter gitignored — NEVER commit it).

## Active work
- **NONE in progress.** iOS Phase-5 property-registry parity is COMPLETE + audited + hardened + shipped. Awaiting Taylor's real-device test of **TestFlight builds 43 / 46 / 47**: 43 = raw-lines fix; **46 = working authoring** (date/slash/NLP/chips); 47 = hardening. (44 = a stray agent cut; 45 had the silent no-op-write bug the post-ship audit caught — both superseded.)
- Product-test checklists in hdeck: `~/.harness/reports/tesela/20260623-ios-phase5/` (approval block `ios-phase5-results`) + the earlier `20260622-session-features`.

## Plan
- (empty — no active phase loop)

## Blockers
- None.

## Open questions / next pick
- **Taylor's build 46/47 device test** — findings become the next fix batch (esp. the raw-lines case + any noisy NLP).
- **Engine-side raw-lines ROOT cure** (`reconcile_tree_to_blocks` strip-and-lift, the durable fix for ALL clients): GATED on `migrate_in_text`/fleet-readiness — an old FFI that can't read the lifted property container could re-broadcast a fleet-wide property erase. ⚠ A Rust/relay change needing fleet coordination, NOT an iOS build; the shipped iOS display strip handles the symptom meanwhile.
- **Next milestone** = type-system VIEWS (keyboard-first kanban/sets per type) — the deferred hard part of the type system, web + iOS. Or whatever Taylor prioritizes.

## Shipped this session (all on `main` + TestFlight/desktop; details in git log + specs + decisions.md)
- **Per-type property config — web Phases 1-4** (spec `phases/2026-06-22-per-type-property-config-spec.md`): per-type `property_overrides` on Tag pages (REPLACE choices / 3-state `show` / default), Tabler icons, plurals, the config UI, per-choice colors. Browser product-test passed.
- **Desktop rebuilt + relaunched** (#73 closed) — per-type config live in `/Applications/Tesela.app`. `feedback_rebuild_relaunch_tauri` authorized (close/rebuild/relaunch w/o asking).
- **Fixed `tesela reindex`** (`cmd_reindex` did `upsert_note`, never `index_type_info` → type/property caches stayed empty) + **upgraded Taylor's 15 live type pages** in-place via the note API (no reimport, no data loss).
- **iOS Phase 5** (spec `phases/2026-06-23-ios-property-registry-parity-spec.md`): registry built client-side from synced Property/Tag pages; date authoring; registry-driven slash + inline NLP; chip colors. Then a 3-finder audit caught + fixed a BLOCKER (structured writes no-opped on a bare bid) + 6 more, and a build-47 hardening pass.

## Known-flaky / landmines
- `tesela-server put_base_diff.rs` sync tests are pre-existing non-deterministic flaky under the full parallel run (pass isolated) — not a regression.
